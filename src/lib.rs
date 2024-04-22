#![feature(fn_traits, let_chains, tuple_trait, unboxed_closures)]

mod animation;
mod debug;
mod file;
mod gl;
mod grim;
mod image;
mod misc;
mod process;
mod proxy;

use std::ffi::c_void;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

fn main() {
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
        grim::read_all.hook(file::read_all as grim::ReadAll);
    }

    unsafe {
        grim::open_bm_image.hook(image::open_bm_image as grim::OpenBmImage);
        grim::surface_upload.hook(image::surface_upload as grim::SurfaceUpload);
        grim::surface_allocate.hook(image::surface_allocate as grim::SurfaceAllocate);
        grim::surface_bind_existing.hook(image::surface_bind_existing as grim::SurfaceBindExisting);
        grim::copy_image.hook(image::copy_image as grim::CopyImage);
        grim::decompress_image.hook(image::decompress_image as grim::DecompressImage);
        grim::manage_resource.hook(image::manage_resource as grim::ManageResource);
        grim::setup_draw.hook(image::setup_draw as grim::SetupDraw);
    };

    unsafe {
        gl::sdl_create_window.hook(misc::sdl_create_window as gl::SdlCreateWindow);
        gl::sdl_set_swap_interval.hook(misc::sdl_gl_set_swap_interval as gl::SdlSetSwapInterval);
        grim::draw_software_scene.hook(misc::draw_software_scene as grim::DrawSoftwareScene);
        gl::sdl_get_display_bounds.hook(misc::sdl_get_display_bounds as gl::SdlGetDisplayBounds);
        gl::sdl_get_current_display_mode
            .hook(misc::sdl_get_current_display_mode as gl::SdlGetCurrentDisplayMode);
    }
}

#[no_mangle]
pub extern "system" fn DllMain(
    hinstance: HMODULE,
    fdw_reason: u32,
    _lp_reserved: *mut c_void,
) -> BOOL {
    if fdw_reason == DLL_PROCESS_ATTACH {
        unsafe {
            let _ = DisableThreadLibraryCalls(hinstance);
            proxy::attach();
        }
        main();
    }
    BOOL(1)
}
