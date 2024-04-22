#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_void};
use windows::Win32::Foundation::{BOOL, HMODULE, HWND};
use windows::Win32::Graphics::Gdi::HDC;

use crate::process::IndirectFn;

pub static mut tex_image_2d: IndirectFn<TexImage2D> = IndirectFn::new("gl_text_image_2d", 0x1713E0);

// pub static mut use_program: IndirectFn<UseProgram> = IndirectFn::new("gl_use_program", 0x2E837E4);

pub static mut pixel_storei: IndirectFn<PixelStorei> = IndirectFn::new("gl_pixel_storei", 0x1713EC);
pub static mut sampler_parameteri: IndirectFn<SamplerParameteri> =
    IndirectFn::new("gl_sampler_parameteri", 0x2E84064);
pub static mut blend_func_separate: IndirectFn<BlendFuncSeparate> =
    IndirectFn::new("gl_sampler_parameteri", 0x2E8360C);

pub static mut sdl_set_swap_interval: IndirectFn<SdlSetSwapInterval> =
    IndirectFn::new("sdl_gl_set_swap_interval", 0x17147C);

pub static mut sdl_create_window: IndirectFn<SdlCreateWindow> =
    IndirectFn::new("sdl_create_window", 0x1714F0);

pub static mut sdl_get_window_wminfo: IndirectFn<SdlGetWindowWminfo> =
    IndirectFn::new("sdl_get_window_wminfo", 0x17148C);

pub static mut sdl_get_display_bounds: IndirectFn<SdlGetDisplayBounds> =
    IndirectFn::new("sdl_get_display_bounds", 0x171494);

pub static mut sdl_get_current_display_mode: IndirectFn<SdlGetCurrentDisplayMode> =
    IndirectFn::new("sdl_get_current_display_mode", 0x1714E8);

pub const SDL_WINDOW_ALLOW_HIGHDPI: u32 = 0x00002000;

pub type Uint = c_uint;
pub type Int = c_int;
pub type Enum = c_uint;
pub type Sizei = c_int;

pub type TexImage2D = extern "stdcall" fn(
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

// pub type UseProgram = extern "stdcall" fn(program: Uint);

pub type PixelStorei = extern "stdcall" fn(pname: Enum, param: Int);
pub type SamplerParameteri = extern "stdcall" fn(sampler: Uint, pname: Enum, param: Int);
pub type BlendFuncSeparate =
    extern "stdcall" fn(src_rgb: Enum, dst_rgb: Enum, src_alpha: Enum, dst_alpha: Enum);

pub type SdlSetSwapInterval = extern "C" fn(interval: c_int) -> c_int;
pub type SdlCreateWindow = extern "C" fn(
    title: *const c_char,
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
    flags: u32,
) -> *mut c_void;
pub type SdlGetWindowWminfo = extern "C" fn(window: *mut c_void, info: *mut SysWminfo) -> BOOL;
pub type SdlGetDisplayBounds = extern "C" fn(display_index: c_int, rect: *mut Rect) -> c_int;

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

pub type SdlGetCurrentDisplayMode =
    extern "C" fn(display_index: c_int, mode: *mut DisplayMode) -> c_int;

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
