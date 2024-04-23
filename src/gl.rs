#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_void};
use windows::Win32::Foundation::{BOOL, HMODULE, HWND};
use windows::Win32::Graphics::Gdi::HDC;

use crate::fn_refs;

fn_refs! {
    #[address(0x1713E0)]
    extern "stdcall" fn tex_image_2d(
        target: Enum,
        level: Int,
        internalformat: Int,
        width: Sizei,
        height: Sizei,
        border: Int,
        format: Enum,
        typ: Enum,
        data: *const c_void,
    );
    #[address(0x1713EC)]
    extern "stdcall" fn pixel_storei(pname: Enum, param: Int);
    #[address(0x2E84064)]
    extern "stdcall" fn sampler_parameteri(sampler: Uint, pname: Enum, param: Int);
    #[address(0x2E8360C)]
    extern "stdcall" fn blend_func_separate(
        src_rgb: Enum,
        dst_rgb: Enum,
        src_alpha: Enum,
        dst_alpha: Enum
    );

    #[address(0x17147C)]
    extern "C" fn sdl_set_swap_interval(interval: c_int) -> c_int;
    #[address(0x1714F0)]
    extern "C" fn sdl_create_window(
        title: *const c_char,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        flags: u32,
    ) -> *mut c_void;
    #[address(0x17148C)]
    extern "C" fn sdl_get_window_wminfo(window: *mut c_void, info: *mut SysWminfo) -> BOOL;
    #[address(0x171494)]
    extern "C" fn sdl_get_display_bounds(display_index: c_int, rect: *mut Rect) -> c_int;
    #[address(0x1714E8)]
    extern "C" fn sdl_get_current_display_mode(
        display_index: c_int,
        mode: *mut DisplayMode
    ) -> c_int;
}

pub const SDL_WINDOW_ALLOW_HIGHDPI: u32 = 0x00002000;

pub type Uint = c_uint;
pub type Int = c_int;
pub type Enum = c_uint;
pub type Sizei = c_int;

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
pub struct Rect {
    pub x: c_int,
    pub y: c_int,
    pub w: c_int,
    pub h: c_int,
}

#[repr(C)]
pub struct DisplayMode {
    pub format: u32,
    pub width: c_int,
    pub height: c_int,
    pub refresh_rate: c_int,
    pub driverdata: *mut c_void,
}

pub const UNPACK_ROW_LENGTH: Enum = 0x0CF2;
pub const TEXTURE_2D: Enum = 0x0DE1;
pub const UNSIGNED_BYTE: Enum = 0x1401;
pub const RGBA: Enum = 0x1908;
pub const LINEAR: Enum = 0x2601;
pub const TEXTURE_MAG_FILTER: Enum = 0x2800;
pub const TEXTURE_MIN_FILTER: Enum = 0x2801;
pub const SRC_ALPHA: Enum = 0x0302;
pub const ONE_MINUS_SRC_ALPHA: Enum = 0x0303;
pub const RGBA8: Enum = 0x8058;
