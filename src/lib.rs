#![feature(fn_traits, tuple_trait, unboxed_closures)]

mod config;
mod debug;
mod file;
mod init;
mod macros;
mod misc;
mod perf;
mod raw;
mod renderer;

// Windows entry point: DLL proxy hijacking via glu32.dll
#[cfg(target_os = "windows")]
mod windows_entry {
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
                crate::raw::glu32::bind_fns().ok();
            }
            crate::init::main();
        }
        BOOL(1)
    }
}

// Linux entry point: LD_PRELOAD constructor
#[cfg(target_os = "linux")]
#[ctor::ctor]
fn grimmod_init() {
    init::main();
}
