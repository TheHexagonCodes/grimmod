#![allow(non_upper_case_globals)]

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void};
use windows::Win32::Foundation::{BOOL, HMODULE, HWND};
use windows::Win32::Graphics::Gdi::HDC;

use crate::bound_fns;

bound_fns! {
    extern "C" fn set_swap_interval(interval: c_int) -> c_int;
    extern "C" fn create_window(
        title: *const c_char,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        flags: u32,
    ) -> *mut c_void;
    extern "C" fn get_window_wminfo(window: *mut c_void, info: *mut SysWminfo) -> BOOL;
    extern "C" fn get_display_bounds(display_index: c_int, rect: *mut Rect) -> c_int;
    extern "C" fn get_current_display_mode(
        display_index: c_int,
        mode: *mut DisplayMode
    ) -> c_int;
}

#[derive(Default)]
#[repr(C)]
pub struct SysWminfo {
    pub version: u32,
    pub subsystem: u32,
    pub window: HWND,
    pub hdc: HDC,
    pub hinstance: HMODULE,
}

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

pub fn bind_static_fns(import_map: &HashMap<String, usize>) -> Result<(), String> {
    set_swap_interval.bind_from_imports("SDL_GL_SetSwapInterval", import_map)?;
    create_window.bind_from_imports("SDL_CreateWindow", import_map)?;
    get_window_wminfo.bind_from_imports("SDL_GetWindowWMInfo", import_map)?;
    get_display_bounds.bind_from_imports("SDL_GetDisplayBounds", import_map)?;
    get_current_display_mode.bind_from_imports("SDL_GetCurrentDisplayMode", import_map)?;

    Ok(())
}
