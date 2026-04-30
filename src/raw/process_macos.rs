// Mach-O symbol resolution for the macOS port.
//
// - `dlsym(RTLD_DEFAULT, ...)` for global/exported symbols
// - Mach-O LC_SYMTAB/nlist64 parsing via `goblin` for all symbols (including local)
// - Mach-O lazy/non-lazy symbol pointer table lookup for indirect symbols
//   (equivalent of ELF GOT entries — used for hooking imported functions)
//
// The macOS game binary is unstripped x86_64 Mach-O with ~3,500 function symbols.
// All C symbols have the `_` prefix in the Mach-O nlist (e.g., `_stdFileOpen`),
// but dlsym does NOT require the prefix.

use std::collections::HashMap;
use std::ffi::CString;
use std::sync::LazyLock;
use std::sync::Mutex;

use crate::debug;

/// Cache of Mach-O symbols: name (without leading _) -> address.
static MACHO_SYMBOLS: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(build_macho_symbol_map().unwrap_or_default()));

/// Cache of lazy/non-lazy symbol pointer entries: name (without _) -> pointer slot address.
static STUB_POINTERS: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(build_stub_pointer_map().unwrap_or_default()));

/// Find the path to the game executable on macOS.
///
/// Uses `std::env::current_exe()` to get the main binary path.
fn find_game_exe_path() -> String {
    match std::env::current_exe() {
        Ok(path) => {
            let path = path.to_string_lossy().to_string();
            debug::info(format!("Game executable path: {}", path));
            path
        }
        Err(e) => {
            debug::error(format!("Failed to get executable path: {}", e));
            String::new()
        }
    }
}

static GAME_EXE_PATH: LazyLock<String> = LazyLock::new(find_game_exe_path);

/// Get the slide (ASLR offset) for the main executable image.
///
/// On macOS, dyld applies a random slide to the executable. We need to add this
/// slide to symbol addresses from the Mach-O file to get runtime addresses.
fn get_image_slide() -> isize {
    unsafe {
        let count = mach2::dyld::_dyld_image_count();
        for i in 0..count {
            let name_ptr = mach2::dyld::_dyld_get_image_name(i);
            if name_ptr.is_null() {
                continue;
            }
            let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy();
            // The main executable is usually image 0, but check by path
            if name.contains("GrimFandango") || i == 0 {
                let slide = mach2::dyld::_dyld_get_image_vmaddr_slide(i);
                debug::info(format!("Image {} slide: 0x{:x} ({})", i, slide, name));
                return slide;
            }
        }
    }
    0
}

static IMAGE_SLIDE: LazyLock<isize> = LazyLock::new(get_image_slide);

/// Look up a global symbol via dlsym(RTLD_DEFAULT, ...).
///
/// Note: dlsym on macOS does NOT require the `_` prefix for C symbols.
pub fn dlsym_lookup(name: &str) -> Option<usize> {
    let c_name = CString::new(name).ok()?;
    let addr = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c_name.as_ptr()) };
    if addr.is_null() {
        // Try with _ prefix (some symbols need it)
        let prefixed = format!("_{}", name);
        let c_prefixed = CString::new(prefixed).ok()?;
        let addr2 = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c_prefixed.as_ptr()) };
        if addr2.is_null() {
            None
        } else {
            if debug::verbose() {
                debug::info(format!(
                    "dlsym found '_{}' at 0x{:016x}",
                    name, addr2 as usize
                ));
            }
            Some(addr2 as usize)
        }
    } else {
        if debug::verbose() {
            debug::info(format!(
                "dlsym found '{}' at 0x{:016x}",
                name, addr as usize
            ));
        }
        Some(addr as usize)
    }
}

/// Look up a symbol in the Mach-O symtab (including local symbols).
pub fn macho_symbol_lookup(name: &str) -> Option<usize> {
    let map = MACHO_SYMBOLS.lock().unwrap();
    let addr = map.get(name).copied();
    if let Some(a) = addr {
        if debug::verbose() {
            debug::info(format!("Mach-O symtab found '{}' at 0x{:016x}", name, a));
        }
    }
    addr
}

/// Look up a lazy/non-lazy symbol pointer entry for an imported symbol.
/// Returns the address of the pointer slot itself (not the function it points to).
/// This is the Mach-O equivalent of an ELF GOT entry.
pub fn stub_pointer_lookup(name: &str) -> Option<usize> {
    let map = STUB_POINTERS.lock().unwrap();
    let addr = map.get(name).copied();
    if let Some(a) = addr {
        if debug::verbose() {
            debug::info(format!(
                "Mach-O stub pointer for '{}' at 0x{:016x}",
                name, a
            ));
        }
    }
    addr
}

/// Build a symbol map from the game executable's Mach-O symbol table.
///
/// This parses LC_SYMTAB and the associated nlist64 entries to find all symbols.
/// The Mach-O `_` prefix is stripped from symbol names for consistency with
/// how they're referenced in the game engine source.
fn build_macho_symbol_map() -> Option<HashMap<String, usize>> {
    let exe_path = &*GAME_EXE_PATH;
    if exe_path.is_empty() {
        return None;
    }
    let data = std::fs::read(exe_path).ok()?;
    let slide = *IMAGE_SLIDE;

    let macho = match goblin::Object::parse(&data).ok()? {
        goblin::Object::Mach(goblin::mach::Mach::Binary(macho)) => macho,
        _ => return None,
    };

    let mut map = HashMap::new();

    for sym in macho.symbols() {
        let (name, nlist) = match sym {
            Ok(s) => s,
            Err(_) => continue,
        };

        if nlist.n_value == 0 {
            continue;
        }

        if name.is_empty() {
            continue;
        }

        // Apply ASLR slide to get runtime address
        let addr = (nlist.n_value as isize + slide) as usize;

        // Store with the leading _ stripped (Mach-O convention)
        let clean_name = name.strip_prefix('_').unwrap_or(name);
        map.insert(clean_name.to_string(), addr);

        // Also store the original mangled name for C++ symbols
        if name != clean_name {
            map.entry(name.to_string()).or_insert(addr);
        }
    }

    // Insert demangled C++ symbol aliases
    let mut demangled_additions: Vec<(String, usize)> = Vec::new();
    for (name, &addr) in &map {
        if let Some(demangled) = try_demangle_itanium(name) {
            if !map.contains_key(&demangled) {
                demangled_additions.push((demangled, addr));
            }
        }
    }
    let demangled_count = demangled_additions.len();
    for (name, addr) in demangled_additions {
        map.entry(name).or_insert(addr);
    }

    debug::info(format!(
        "Built Mach-O symbol map with {} entries ({} demangled aliases) from '{}'",
        map.len(),
        demangled_count,
        exe_path
    ));

    Some(map)
}

/// Build a map of Mach-O lazy/non-lazy symbol pointer entries.
///
/// These are found in __DATA,__la_symbol_ptr (lazy) and __DATA,__nl_symbol_ptr (non-lazy)
/// sections. Each entry is an 8-byte pointer that the dynamic linker fills in.
/// By knowing the slot address, we can overwrite it to redirect imported function calls
/// (equivalent of GOT overwriting on ELF).
fn build_stub_pointer_map() -> Option<HashMap<String, usize>> {
    let exe_path = &*GAME_EXE_PATH;
    if exe_path.is_empty() {
        return None;
    }
    let data = std::fs::read(exe_path).ok()?;
    let slide = *IMAGE_SLIDE;

    let macho = match goblin::Object::parse(&data).ok()? {
        goblin::Object::Mach(goblin::mach::Mach::Binary(macho)) => macho,
        _ => return None,
    };

    let mut map = HashMap::new();

    // Look through imports which goblin resolves from the binding opcodes
    for import in macho.imports().ok()? {
        let name = import.name;
        let addr = (import.address as isize + slide) as usize;
        if addr == 0 || name.is_empty() {
            continue;
        }
        let clean_name = name.strip_prefix('_').unwrap_or(name);
        map.insert(clean_name.to_string(), addr);
        if name != clean_name {
            map.entry(name.to_string()).or_insert(addr);
        }
    }

    debug::info(format!(
        "Built Mach-O stub pointer map with {} entries from '{}'",
        map.len(),
        exe_path
    ));

    Some(map)
}

/// Try to demangle an Itanium ABI C++ symbol name.
fn try_demangle_itanium(mangled: &str) -> Option<String> {
    let rest = mangled
        .strip_prefix("_ZL")
        .or_else(|| mangled.strip_prefix("_Z"))?;

    if !rest.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    let digit_end = rest.find(|c: char| !c.is_ascii_digit())?;
    let len: usize = rest[..digit_end].parse().ok()?;

    let name_start = digit_end;
    let name = rest.get(name_start..name_start + len)?;

    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }

    Some(name.to_string())
}
