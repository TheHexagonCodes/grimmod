use std::ptr;

use windows::Win32::System::Diagnostics::Debug::IMAGE_NT_HEADERS32;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::System::Memory::{
    VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE_READ,
};
use windows::Win32::System::SystemServices::{IMAGE_DOS_HEADER, IMAGE_DOS_SIGNATURE};

use crate::raw::memory::{HookError, BASE_ADDRESS};
use crate::raw::{gl, grim, process, sdl};
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

fn startup() -> Result<(), String> {
    process::bind_get_proc_address().string_err()?;
    sdl::bind_static_fns().string_err()?;
    gl::bind_static_fns().string_err()?;
    gl::bind_glew_fns().string_err()?;
    init_features().string_err()?;

    grim::init_renderers.hook(init_renderers).string_err()?;

    Ok(())
}

pub fn init_features() -> Result<(), HookError> {
    feature::mods()?;
    feature::hq_assets()?;
    feature::always_on()?;
    feature::vsync()?;
    feature::hdpi_fix()?;

    Ok(())
}

/// Wraps the application entry to locate and bind now-loaded functions
extern "stdcall" fn application_entry() {
    match startup() {
        Ok(_) => debug::info("Successfully initiated GrimMod feature hooks"),
        Err(err) => debug::error(format!("GrimMod feature hooks failed to attach: {}", err)),
    };

    grim::entry();
}

/// Wraps the renderers init function to execute some code that needs
/// to run after gfx setup is done
pub extern "C" fn init_renderers() {
    // since this function executes after the window has been initialized, binding
    // OpenGL function dynamically using (a dynamic) GetProcessAddress will allow
    // tools like RenderDoc to intercept the call
    match gl::bind_dynamic_fns().string_err() {
        Ok(_) => {
            video_cutouts::create_stencil_buffer();
            misc::validate_mods();
        }
        Err(err) => {
            debug::error(format!(
                "Loading auxiliary OpenGL functions failed: {}",
                err
            ));
        }
    };

    grim::init_renderers();
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
