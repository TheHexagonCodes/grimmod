use std::collections::HashMap;
use std::ffi::CStr;
use std::ptr;
use windows::Win32::System::Diagnostics::Debug::{
    IMAGE_DIRECTORY_ENTRY_IMPORT, IMAGE_NT_HEADERS32,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::System::Memory::{
    VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE_READ,
};
use windows::Win32::System::SystemServices::{
    IMAGE_DOS_HEADER, IMAGE_DOS_SIGNATURE, IMAGE_IMPORT_BY_NAME, IMAGE_IMPORT_DESCRIPTOR,
    IMAGE_NT_SIGNATURE,
};
use windows::Win32::System::WindowsProgramming::IMAGE_THUNK_DATA32;

use crate::raw::memory::{HookError, BASE_ADDRESS};
use crate::raw::{gl, grim, sdl};
use crate::renderer::video_cutouts;
use crate::{debug, feature, misc};

pub fn main() {
    debug::info("GrimMod attached to GrimFandango.exe");

    if debug::verbose() {
        debug::info(format!("Base memory address found: 0x{:x}", *BASE_ADDRESS));
    }

    if let Err(err) = initiate_startup() {
        debug::error(format!("GrimMod startup failed: {}", err));
    }
}

fn initiate_startup() -> Result<(), String> {
    let (code_addr, code_size) = get_executable_memory_region()
        .ok_or_else(|| "Could not locate executable memory region".to_string())?;
    grim::find_fns(code_addr, code_size).string_err()?;
    // hook the application entry point for the next step of the startup
    let entry_addr = (unsafe { get_application_entry_addr() })
        .ok_or_else(|| grim::entry.not_found())
        .string_err()?;
    grim::entry.bind(entry_addr).string_err()?;
    grim::entry.hook(application_entry).string_err()
}

fn features_startup() -> Result<(), String> {
    let imports = unsafe { get_imports() }
        .ok_or_else(|| "Could not build the map of static imports".to_string())?;
    sdl::bind_static_fns(&imports).string_err()?;
    gl::bind_static_fns(&imports).string_err()?;
    gl::bind_dynamic_fns().string_err()?;
    // hook the game gfx init to complete the startup
    grim::init_gfx.hook(init_gfx).string_err()?;
    init_features().string_err()
}

fn complete_startup() -> Result<(), String> {
    gl::bind_glew_fns().string_err()?;
    video_cutouts::create_stencil_buffer();
    misc::validate_mods();
    Ok(())
}

/// Wraps the application entry to locate and bind now-loaded functions
extern "stdcall" fn application_entry() {
    match features_startup() {
        Ok(_) => debug::info("Successfully attached GrimMod feature hooks"),
        Err(err) => debug::error(format!("GrimMod feature hooks failed to attach: {}", err)),
    };

    grim::entry();
}

/// Wraps the graphics initialization function to also bind the glew functions
/// and create a stencil buffer for cutouts
pub extern "C" fn init_gfx() -> u8 {
    let result = grim::init_gfx();
    if result != 1 {
        return result;
    }

    match complete_startup() {
        Ok(_) => debug::info("All native game functions have been found"),
        Err(err) => debug::error(format!("GrimMod startup completion failed: {}", err)),
    };

    1
}

pub fn init_features() -> Result<(), HookError> {
    feature::mods()?;
    feature::hq_assets()?;
    feature::quick_toggle()?;
    feature::vsync()?;
    feature::hdpi_fix()?;

    Ok(())
}

/// Gets the address of the main application entry function
unsafe fn get_application_entry_addr() -> Option<usize> {
    let main_module = GetModuleHandleA(None).ok()?;
    let dos_header: IMAGE_DOS_HEADER = ptr::read(main_module.0 as *const _);
    if dos_header.e_magic != IMAGE_DOS_SIGNATURE {
        return None;
    }
    let nt_headers_addr =
        (main_module.0 as usize + dos_header.e_lfanew as usize) as *const IMAGE_NT_HEADERS32;
    let nt_headers: IMAGE_NT_HEADERS32 = ptr::read(nt_headers_addr);

    let entry_point_rva = nt_headers.OptionalHeader.AddressOfEntryPoint;
    let entry_point_addr = (main_module.0 as usize + entry_point_rva as usize) as *const ();

    Some(entry_point_addr as usize)
}

/// Gets a map of statically imported functions and their address
unsafe fn get_imports() -> Option<HashMap<String, usize>> {
    let module_handle = GetModuleHandleA(None).ok()?;
    let dos_header: IMAGE_DOS_HEADER = std::ptr::read(module_handle.0 as *const _);

    if dos_header.e_magic != IMAGE_DOS_SIGNATURE {
        return None;
    }

    let nt_headers_addr =
        (module_handle.0 as usize + dos_header.e_lfanew as usize) as *const IMAGE_NT_HEADERS32;
    let nt_headers: IMAGE_NT_HEADERS32 = std::ptr::read(nt_headers_addr);

    if nt_headers.Signature != IMAGE_NT_SIGNATURE {
        return None;
    }

    let import_directory_entry = nt_headers
        .OptionalHeader
        .DataDirectory
        .get(IMAGE_DIRECTORY_ENTRY_IMPORT.0 as usize)?;
    let mut import_table = (module_handle.0 as usize
        + import_directory_entry.VirtualAddress as usize)
        as *mut IMAGE_IMPORT_DESCRIPTOR;

    let mut imports_map = HashMap::new();
    let mut descriptor = std::ptr::read(import_table);

    while descriptor.Name != 0 {
        let mut thunk =
            (module_handle.0 as usize + descriptor.FirstThunk as usize) as *mut IMAGE_THUNK_DATA32;
        let mut original_thunk = (module_handle.0 as usize
            + descriptor.Anonymous.OriginalFirstThunk as usize)
            as *mut IMAGE_THUNK_DATA32;

        while (*original_thunk).u1.AddressOfData != 0 {
            let import_by_name = (module_handle.0 as usize
                + (*original_thunk).u1.AddressOfData as usize)
                as *const IMAGE_IMPORT_BY_NAME;
            let func_name_ptr = &(*import_by_name).Name as *const u8;
            let func_name = CStr::from_ptr(func_name_ptr as *const i8)
                .to_str()
                .unwrap_or_default()
                .to_owned();

            imports_map.insert(func_name, (*thunk).u1.Function as usize);

            thunk = thunk.add(1);
            original_thunk = original_thunk.add(1);
        }

        import_table = import_table.add(1);
        descriptor = std::ptr::read(import_table);
    }

    Some(imports_map)
}

fn get_executable_memory_region() -> Option<(usize, usize)> {
    let mut address: usize = *BASE_ADDRESS;
    let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let mbi_size = std::mem::size_of::<MEMORY_BASIC_INFORMATION>();

    while unsafe { VirtualQuery(Some(address as _), &mut mbi, mbi_size) } != 0 {
        if mbi.State == MEM_COMMIT && mbi.Protect.contains(PAGE_EXECUTE_READ) {
            return Some((address, mbi.RegionSize));
        }
        address += mbi.RegionSize;
    }

    None
}

pub trait StringError<A> {
    fn string_err(self) -> Result<A, String>;
}

impl<A, E: ToString> StringError<A> for Result<A, E> {
    fn string_err(self) -> Result<A, String> {
        self.map_err(|err| err.to_string())
    }
}
