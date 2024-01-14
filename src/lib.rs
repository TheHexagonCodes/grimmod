#![feature(fn_traits, let_chains, tuple_trait, unboxed_closures)]

mod debug;
mod file;
mod grim;
mod image;
mod misc;
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
        grim::open_file.hook(file::open as grim::OpenFile);
        grim::close_file.hook(file::close as grim::CloseFile);
        grim::read_file.hook(file::read as grim::ReadFile);
    }

    unsafe {
        grim::open_bm_image.hook(image::open_bm_image as grim::OpenBmImage);
        grim::surface_upload.hook(image::surface_upload as grim::SurfaceUpload);
        grim::copy_image.hook(image::copy_image as grim::CopyImage);
        grim::decompress_image.hook(image::decompress_image as grim::DecompressImage);
        grim::manage_resource.hook(image::manage_resource as grim::ManageResource);
    };

    unsafe {
        grim::sdl_gl_set_swap_interval
            .hook(misc::sdl_gl_set_swap_interval as grim::SdlGlSetSwapInterval);
    }
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
