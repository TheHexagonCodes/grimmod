#![feature(fn_traits, if_let_guard, let_chains, tuple_trait, unboxed_closures)]

mod animation;
mod bridge;
mod config;
mod debug;
mod feature;
mod file;
mod gl;
mod grim;
mod image;
mod macros;
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

    feature::mods();
    feature::hq_assets();
    feature::quick_toggle();
    feature::vsync();
    feature::hdpi_fix();
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
