mod debug;
mod process;

use process::Process;

use std::ffi::c_void;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

fn main() {
    debug::init();
    debug::info("GrimMod attached to GrimFandango.exe");

    match Process::init() {
        Some(process) => {
            debug::info(format!(
                "Base memory address found: {:x}",
                process.base_address
            ));
        }
        None => {
            debug::error("Could not find base memory address for GrimFandango.exe");
        }
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
