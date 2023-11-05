mod debug;
mod file;
mod grim;
mod process;

use std::ffi::c_void;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

fn main() {
    debug::init();
    debug::info("GrimMod attached to GrimFandango.exe");

    if process::init() {
        debug::info(format!(
            "Base memory address found: {:x}",
            process::base_address()
        ));

        // Replace the game's IO functions with the mod loader
        grim::with_runtime_context(|runtime_context| {
            runtime_context.open_file = file::open as *const _;
            runtime_context.close_file = file::close as *const _;
            runtime_context.read_file = file::read as *const _;
        });
    } else {
        debug::error("Could not find base memory address for GrimFandango.exe");
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
