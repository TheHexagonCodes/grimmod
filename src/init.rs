use std::collections::HashMap;
use std::ffi::CStr;
use std::ptr;
use windows::Win32::System::Diagnostics::Debug::{
    IMAGE_DIRECTORY_ENTRY_IMPORT, IMAGE_NT_HEADERS32,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::System::SystemServices::{
    IMAGE_DOS_HEADER, IMAGE_DOS_SIGNATURE, IMAGE_IMPORT_BY_NAME, IMAGE_IMPORT_DESCRIPTOR,
    IMAGE_NT_SIGNATURE,
};
use windows::Win32::System::WindowsProgramming::IMAGE_THUNK_DATA32;

use crate::raw::memory::BASE_ADDRESS;
use crate::raw::{gl, grim};
use crate::{debug, feature, misc};

pub fn main() {
    debug::info("GrimMod attached to GrimFandango.exe");

    debug::info(format!("Base memory address found: 0x{:x}", *BASE_ADDRESS));

    unsafe {
        if let Some(entry_addr) = get_application_entry_addr() {
            grim::entry.bind(entry_addr);
            grim::entry.hook(application_entry).ok();
        }
    }

    feature::mods();
    feature::hq_assets();
    feature::quick_toggle();
    feature::vsync();
    feature::hdpi_fix();

    misc::validate_mods();
}

/// Wraps the application entry to locate and bind now-loaded functions
extern "stdcall" fn application_entry() {
    unsafe {
        if let Some(imports) = imports() {
            match gl::bind_static_fns(&imports) {
                Ok(_) => debug::info("Static OpenGL functions found"),
                Err(unbound) => {
                    debug::error(format!("Could not find OpenGL function '{}'", unbound))
                }
            };

            match gl::bind_dynamic_fns() {
                Ok(_) => debug::info("Dyanmic OpenGL functions loaded"),
                Err(err) => debug::error(format!("{}", err)),
            };
        }

        grim::entry();
    };
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
unsafe fn imports() -> Option<HashMap<String, usize>> {
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
