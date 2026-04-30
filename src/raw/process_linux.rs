// ELF symbol resolution for the Linux port.
//
// - `dlsym(RTLD_DEFAULT, ...)` for global symbols (T/B in nm output)
// - ELF symtab parsing via `goblin` for local symbols (t/b in nm output)
// - GOT entry lookup for dynamically imported symbols (U in nm output)

use std::collections::HashMap;
use std::ffi::CString;
use std::sync::LazyLock;
use std::sync::Mutex;

use crate::debug;

/// Cache of ELF local symbols: name -> address.
static ELF_SYMBOLS: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(build_elf_symbol_map().unwrap_or_default()));

/// Cache of GOT entries: symbol name -> GOT slot address.
static GOT_ENTRIES: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(build_got_map().unwrap_or_default()));

/// Find the path to the game executable.
///
/// Under box64, `/proc/self/exe` points to the box64 binary (PPC64LE ELF64).
/// The actual game binary path is in argv[0] from `/proc/self/cmdline`.
fn find_game_exe_path() -> String {
    if let Ok(cmdline) = std::fs::read("/proc/self/cmdline") {
        if let Some(nul_pos) = cmdline.iter().position(|&b| b == 0) {
            if let Ok(argv0) = std::str::from_utf8(&cmdline[..nul_pos]) {
                let path = std::path::Path::new(argv0);
                if path.exists() && path.is_file() {
                    if let Ok(data) = std::fs::read(path) {
                        if data.len() > 18 {
                            let is_elf = data[0] == 0x7f
                                && data[1] == b'E'
                                && data[2] == b'L'
                                && data[3] == b'F';
                            let is_32bit = data[4] == 1;
                            if is_elf && is_32bit {
                                debug::info(format!("Found game binary from cmdline: {}", argv0));
                                return argv0.to_string();
                            }
                        }
                    }
                }
            }
        }
    }

    debug::info("Using /proc/self/exe as game binary path (native mode)");
    "/proc/self/exe".to_string()
}

static GAME_EXE_PATH: LazyLock<String> = LazyLock::new(find_game_exe_path);

/// Look up a global symbol via dlsym(RTLD_DEFAULT, ...).
pub fn dlsym_lookup(name: &str) -> Option<usize> {
    let c_name = CString::new(name).ok()?;
    let addr = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c_name.as_ptr()) };
    if addr.is_null() {
        None
    } else {
        if debug::verbose() {
            debug::info(format!("dlsym found '{}' at 0x{:08x}", name, addr as usize));
        }
        Some(addr as usize)
    }
}

/// Look up a symbol in the ELF symtab (including local symbols).
pub fn elf_symbol_lookup(name: &str) -> Option<usize> {
    let map = ELF_SYMBOLS.lock().unwrap();
    let addr = map.get(name).copied();
    if let Some(a) = addr {
        if debug::verbose() {
            debug::info(format!("ELF symtab found '{}' at 0x{:08x}", name, a));
        }
    }
    addr
}

/// Look up a GOT entry for a dynamically imported symbol.
/// Returns the address of the GOT slot itself (not the function it points to).
pub fn got_entry_lookup(name: &str) -> Option<usize> {
    let map = GOT_ENTRIES.lock().unwrap();
    let addr = map.get(name).copied();
    if let Some(a) = addr {
        if debug::verbose() {
            debug::info(format!("GOT entry for '{}' at 0x{:08x}", name, a));
        }
    }
    addr
}

/// Build a symbol map from the game executable's ELF .symtab section.
fn build_elf_symbol_map() -> Option<HashMap<String, usize>> {
    let exe_path = &*GAME_EXE_PATH;
    let data = std::fs::read(exe_path).ok()?;

    let elf = match goblin::Object::parse(&data).ok()? {
        goblin::Object::Elf(elf) => elf,
        _ => return None,
    };

    let mut map = HashMap::new();

    for sym in &elf.syms {
        if sym.st_value == 0 {
            continue;
        }
        if let Some(name) = elf.strtab.get_at(sym.st_name) {
            if !name.is_empty() {
                map.insert(name.to_string(), sym.st_value as usize);
            }
        }
    }

    for sym in &elf.dynsyms {
        if sym.st_value == 0 {
            continue;
        }
        if let Some(name) = elf.dynstrtab.get_at(sym.st_name) {
            if !name.is_empty() {
                map.entry(name.to_string()).or_insert(sym.st_value as usize);
            }
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
        "Built ELF symbol map with {} entries ({} demangled aliases) from '{}'",
        map.len(),
        demangled_count,
        exe_path
    ));

    Some(map)
}

/// Build a GOT entry map from the game executable's ELF relocations.
fn build_got_map() -> Option<HashMap<String, usize>> {
    let exe_path = &*GAME_EXE_PATH;
    let data = std::fs::read(exe_path).ok()?;

    let elf = match goblin::Object::parse(&data).ok()? {
        goblin::Object::Elf(elf) => elf,
        _ => return None,
    };

    let mut map = HashMap::new();

    for reloc in &elf.pltrelocs {
        let sym_idx = reloc.r_sym;
        if let Some(sym) = elf.dynsyms.get(sym_idx) {
            if let Some(name) = elf.dynstrtab.get_at(sym.st_name) {
                if !name.is_empty() {
                    map.insert(name.to_string(), reloc.r_offset as usize);
                }
            }
        }
    }

    debug::info(format!(
        "Built GOT entry map with {} entries from '{}'",
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
