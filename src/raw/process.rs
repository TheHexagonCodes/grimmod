#![allow(non_upper_case_globals)]

use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    sync::Mutex,
};

use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{FARPROC, HMODULE},
        System::{
            Diagnostics::Debug::{IMAGE_DIRECTORY_ENTRY_IMPORT, IMAGE_NT_HEADERS32},
            LibraryLoader::{GetModuleHandleA, GetProcAddress},
            Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE_READ},
            SystemServices::{
                IMAGE_DOS_HEADER, IMAGE_DOS_SIGNATURE, IMAGE_IMPORT_BY_NAME,
                IMAGE_IMPORT_DESCRIPTOR, IMAGE_NT_SIGNATURE,
            },
            WindowsProgramming::IMAGE_THUNK_DATA32,
        },
    },
};

use crate::{
    indirect_fns,
    raw::memory::{BindError, BASE_ADDRESS},
};

static IMPORT_MAP: Mutex<Option<HashMap<String, usize>>> = Mutex::new(None);

indirect_fns! {
    extern "stdcall" fn get_proc_address(h_module: HMODULE, lp_proc_name: PCSTR) -> FARPROC;
}

pub fn bind_get_proc_address() -> Result<(), BindError> {
    get_proc_address.bind_symbol("GetProcAddress")
}

/// Finds the dynamic address of an exposed symbol
pub fn get_symbol_addr(name: &str) -> Option<usize> {
    get_export_addr(name).or_else(|| get_import_addr(name))
}

/// Finds the address of an exported symbol (i.e. dynamic GLEW symbols)
pub fn get_export_addr(name: &str) -> Option<usize> {
    let module = unsafe { GetModuleHandleA(None) }.ok()?;
    let name_cstr = CString::new(name).ok()?;
    let symbol = unsafe { GetProcAddress(module, PCSTR(name_cstr.as_ptr() as *const _)) };
    symbol.map(|symbol| symbol as usize)
}

/// Finds the dynamic address of a statically imported symbol
pub fn get_import_addr(name: &str) -> Option<usize> {
    let mut import_map_guard = IMPORT_MAP.lock().unwrap();
    if import_map_guard.is_none() {
        *import_map_guard = unsafe { build_import_map() };
    }
    import_map_guard
        .as_ref()
        .and_then(|import_map| import_map.get(name).cloned())
}

/// Finds the virtual address of a function at runtime
pub fn get_import_virtual_addr(proc_name: &str, module_name: &str) -> Option<usize> {
    let module_name_cstr = CString::new(module_name).ok()?;
    let module = unsafe { GetModuleHandleA(PCSTR(module_name_cstr.as_ptr() as *const _)) }.ok()?;
    let name_cstr = CString::new(proc_name).ok()?;
    // use a dynamic GetProcAddress here for compatibility with other hooking tools
    // e.g. RenderDoc
    let symbol = get_proc_address(module, PCSTR(name_cstr.as_ptr() as *const _));
    symbol.map(|symbol| symbol as usize)
}

/// Gets a map of statically imported functions and their address in the IAT
unsafe fn build_import_map() -> Option<HashMap<String, usize>> {
    let mut imports = HashMap::new();
    let module = unsafe { GetModuleHandleA(None) }.ok()?;
    let module_addr = module.0 as usize;

    let dos_header = (module_addr as *const IMAGE_DOS_HEADER).read();
    (dos_header.e_magic == IMAGE_DOS_SIGNATURE).then_some(())?;

    let nt_headers_ptr: *const IMAGE_NT_HEADERS32 = (module_addr as i32 + dos_header.e_lfanew) as _;
    let nt_headers = nt_headers_ptr.read();
    (nt_headers.Signature == IMAGE_NT_SIGNATURE).then_some(())?;

    let import_directory = nt_headers
        .OptionalHeader
        .DataDirectory
        .get(IMAGE_DIRECTORY_ENTRY_IMPORT.0 as usize)?;
    let import_directory_rva = import_directory.VirtualAddress;
    (import_directory_rva != 0).then_some(())?;

    let mut descriptor_ptr: *const IMAGE_IMPORT_DESCRIPTOR =
        (module_addr as u32 + import_directory_rva) as _;
    let mut descriptor = descriptor_ptr.read();

    while descriptor.Name != 0 {
        let mut thunk_ptr: *const IMAGE_THUNK_DATA32 =
            (module_addr as u32 + descriptor.FirstThunk) as _;
        let mut original_thunk_ptr: *const IMAGE_THUNK_DATA32 =
            (module_addr as u32 + descriptor.Anonymous.OriginalFirstThunk) as _;
        let mut original_thunk = original_thunk_ptr.read();

        while original_thunk.u1.AddressOfData != 0 {
            let import_by_name_ptr: *const IMAGE_IMPORT_BY_NAME =
                (module_addr as u32 + original_thunk.u1.AddressOfData) as _;
            let func_name_ptr = &(*import_by_name_ptr).Name as *const u8;
            let func_name = CStr::from_ptr(func_name_ptr as *const i8)
                .to_string_lossy()
                .to_string();

            imports.insert(func_name, thunk_ptr as usize);

            thunk_ptr = thunk_ptr.add(1);
            original_thunk_ptr = original_thunk_ptr.add(1);
            original_thunk = original_thunk_ptr.read();
        }

        descriptor_ptr = descriptor_ptr.add(1);
        descriptor = descriptor_ptr.read();
    }

    Some(imports)
}

/// Gets some basic info about a memory location
pub fn query_memory_region(addr: usize) -> Option<MEMORY_BASIC_INFORMATION> {
    let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let mbi_size = std::mem::size_of::<MEMORY_BASIC_INFORMATION>();
    let result = unsafe { VirtualQuery(Some(addr as _), &mut mbi, mbi_size) };
    (result != 0).then_some(mbi)
}

/// Finds the first memory region after the base address that allows execution
pub fn get_first_executable_memory_region() -> Option<(usize, usize)> {
    let mut address: usize = *BASE_ADDRESS;

    while let Some(mbi) = query_memory_region(address) {
        if mbi.State == MEM_COMMIT && mbi.Protect.contains(PAGE_EXECUTE_READ) {
            return Some((address, mbi.RegionSize));
        }
        address += mbi.RegionSize;
    }

    None
}

/// Gets the address of the main application entry function
pub fn get_application_entry_addr() -> Option<usize> {
    unsafe {
        let module = GetModuleHandleA(None).ok()?;
        let module_addr = module.0 as usize;

        let dos_header: IMAGE_DOS_HEADER = (module_addr as *const IMAGE_DOS_HEADER).read();
        (dos_header.e_magic == IMAGE_DOS_SIGNATURE).then_some(())?;

        let nt_headers_ptr: *const IMAGE_NT_HEADERS32 =
            (module_addr + dos_header.e_lfanew as usize) as _;
        let nt_headers = nt_headers_ptr.read();

        let entry_point_rva = nt_headers.OptionalHeader.AddressOfEntryPoint;
        let entry_point_addr = module_addr + entry_point_rva as usize;
        Some(entry_point_addr)
    }
}
