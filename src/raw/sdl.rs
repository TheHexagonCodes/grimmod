#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_void};

use crate::indirect_fns;
use crate::raw::memory::BindError;

// ---- SDL function declarations ----
// Both platforms use extern "C" for SDL (it's always cdecl).
// However, the return type of get_window_wminfo differs.

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::Win32::Foundation::BOOL;

    indirect_fns! {
        extern "C" fn set_swap_interval(interval: c_int) -> c_int;
        extern "C" fn create_window(
            title: *const c_char,
            x: c_int,
            y: c_int,
            w: c_int,
            h: c_int,
            flags: u32,
        ) -> *mut c_void;
        extern "C" fn get_window_wminfo(window: *mut c_void, info: *mut super::SysWminfo) -> BOOL;
        extern "C" fn get_display_bounds(display_index: c_int, rect: *mut super::Rect) -> c_int;
        extern "C" fn get_current_display_mode(
            display_index: c_int,
            mode: *mut super::DisplayMode
        ) -> c_int;
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;

    indirect_fns! {
        extern "C" fn set_swap_interval(interval: c_int) -> c_int;
        extern "C" fn create_window(
            title: *const c_char,
            x: c_int,
            y: c_int,
            w: c_int,
            h: c_int,
            flags: u32,
        ) -> *mut c_void;
        extern "C" fn get_window_wminfo(window: *mut c_void, info: *mut super::SysWminfo) -> c_int;
        extern "C" fn get_display_bounds(display_index: c_int, rect: *mut super::Rect) -> c_int;
        extern "C" fn get_current_display_mode(
            display_index: c_int,
            mode: *mut super::DisplayMode
        ) -> c_int;
    }
}

pub use platform::*;

// ---- SysWminfo struct ----

#[cfg(target_os = "windows")]
mod wminfo {
    use windows::Win32::Foundation::{HMODULE, HWND};
    use windows::Win32::Graphics::Gdi::HDC;

    #[derive(Default)]
    #[repr(C)]
    pub struct SysWminfo {
        pub version: u32,
        pub subsystem: u32,
        pub window: HWND,
        pub hdc: HDC,
        pub hinstance: HMODULE,
    }
}

#[cfg(target_os = "linux")]
mod wminfo {
    use std::ffi::c_void;

    /// SDL_SysWMinfo for Linux/X11.
    ///
    /// The Windows version has HWND/HDC/HMODULE fields. On Linux under X11,
    /// the relevant fields are the X11 Display* and Window handle.
    #[repr(C)]
    pub struct SysWminfo {
        pub version: u32,
        pub subsystem: u32,
        pub display: *mut c_void,
        pub window: u64, // X11 Window is a 32-bit XID, but padded
    }

    impl Default for SysWminfo {
        fn default() -> Self {
            SysWminfo {
                version: 0,
                subsystem: 0,
                display: std::ptr::null_mut(),
                window: 0,
            }
        }
    }
}

pub use wminfo::SysWminfo;

// ---- Shared types ----

#[repr(C)]
pub struct DisplayMode {
    pub format: u32,
    pub width: c_int,
    pub height: c_int,
    pub refresh_rate: c_int,
    pub driverdata: *mut c_void,
}

#[repr(C)]
pub struct Rect {
    pub x: c_int,
    pub y: c_int,
    pub w: c_int,
    pub h: c_int,
}

pub const WINDOW_ALLOW_HIGHDPI: u32 = 0x00002000;

// ---- Binding functions ----

#[cfg(target_os = "windows")]
pub fn bind_static_fns() -> Result<(), BindError> {
    set_swap_interval.bind_symbol("SDL_GL_SetSwapInterval")?;
    create_window.bind_symbol("SDL_CreateWindow")?;
    get_window_wminfo.bind_symbol("SDL_GetWindowWMInfo")?;
    get_display_bounds.bind_symbol("SDL_GetDisplayBounds")?;
    get_current_display_mode.bind_symbol("SDL_GetCurrentDisplayMode")?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn bind_static_fns() -> Result<(), BindError> {
    set_swap_interval.bind_got_entry("SDL_GL_SetSwapInterval")?;
    create_window.bind_got_entry("SDL_CreateWindow")?;
    get_window_wminfo.bind_got_entry("SDL_GetWindowWMInfo")?;
    get_display_bounds.bind_got_entry("SDL_GetDisplayBounds")?;
    get_current_display_mode.bind_got_entry("SDL_GetCurrentDisplayMode")?;

    Ok(())
}
