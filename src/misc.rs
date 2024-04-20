use std::ffi::{c_char, c_int, c_void};
use windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware;

use crate::gl;
use crate::grim;

/// SDL function for controlling the swap interval (vsync)
///
/// This is a overload for a native function that will be hooked
pub extern "C" fn sdl_gl_set_swap_interval(_interval: c_int) -> c_int {
    // for now, always enable vsync
    unsafe { gl::sdl_set_swap_interval(1) }
}

pub extern "C" fn sdl_create_window(
    title: *const c_char,
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
    flags: u32,
) -> *mut c_void {
    unsafe {
        SetProcessDPIAware();
        gl::sdl_create_window(title, x, y, w, h, flags | gl::SDL_WINDOW_ALLOW_HIGHDPI)
    }
}

pub extern "C" fn draw_software_scene(
    draw: *const grim::Draw,
    software_surface: *const grim::Surface,
    transition: f32,
) {
    unsafe {
        let value = if transition == 1.0 {
            1.0
        } else {
            grim::RENDERING_MODE.get()
        };
        grim::draw_software_scene(draw, software_surface, value);
    }
}
