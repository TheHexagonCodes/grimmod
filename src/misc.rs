use std::ffi::c_int;

use crate::grim;

/// SDL function for controlling the swap interval (vsync)
///
/// This is a overload for a native function that will be hooked
pub extern "C" fn sdl_gl_set_swap_interval(_interval: c_int) -> c_int {
    // for now, always enable vsync
    unsafe { grim::sdl_gl_set_swap_interval(1) }
}
