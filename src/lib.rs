#![feature(fn_traits, let_chains, tuple_trait, unboxed_closures)]

mod debug;
mod file;
mod grim;
mod image;
mod process;

use std::ffi::c_void;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

fn main() {
    debug::init();
    debug::info("GrimMod attached to GrimFandango.exe");

    debug::info(format!(
        "Base memory address found: 0x{:x}",
        *process::BASE_ADDRESS
    ));

    // Hook the game's IO functions to load modded files
    unsafe {
        file::OPEN_FILE_HOOK.enable().ok();
        file::CLOSE_FILE_HOOK.enable().ok();
        file::READ_FILE_HOOK.enable().ok();
    }

    unsafe {
        image::OPEN_BM_IMAGE_HOOK.enable().ok();
        image::SURFACE_UPLOAD_HOOK.enable().ok();
        image::COPY_IMAGE_HOOK.enable().ok();
        image::DECOMPRESS_IMAGE_HOOK.enable().ok();
        image::MANAGE_RESOURCE_HOOK.enable().ok();
    };
}

#[no_mangle]
pub extern "system" fn DllMain(
    _hinstance: usize,
    fdw_reason: u32,
    _lp_reserved: *mut c_void,
) -> BOOL {
    if fdw_reason == DLL_PROCESS_ATTACH {
        main();
    }
    BOOL(1)
}
