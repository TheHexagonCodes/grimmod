#![feature(fn_traits, if_let_guard, let_chains, tuple_trait, unboxed_closures)]

mod config;
mod debug;
mod feature;
mod file;
mod macros;
mod misc;
mod raw;
mod renderer;

use std::ffi::c_void;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

fn main() {
    debug::info("GrimMod attached to GrimFandango.exe");

    debug::info(format!(
        "Base memory address found: 0x{:x}",
        *raw::memory::BASE_ADDRESS
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
            raw::proxy::attach();
        }
        main();
    }
    BOOL(1)
}
