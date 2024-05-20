#![feature(fn_traits, if_let_guard, let_chains, tuple_trait, unboxed_closures)]

mod config;
mod debug;
mod feature;
mod file;
mod init;
mod macros;
mod misc;
mod raw;
mod renderer;

use std::ffi::c_void;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

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
        init::main();
    }
    BOOL(1)
}
