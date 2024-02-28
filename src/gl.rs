#![allow(non_upper_case_globals)]

use std::ffi::{c_int, c_uint, c_void};

use crate::process::IndirectFn;

pub static mut tex_image_2d: IndirectFn<TexImage2D> = IndirectFn::new("gl_text_image_2d", 0x1713E0);

// pub static mut use_program: IndirectFn<UseProgram> = IndirectFn::new("gl_use_program", 0x2E837E4);

pub static mut pixel_storei: IndirectFn<PixelStorei> = IndirectFn::new("gl_pixel_storei", 0x1713EC);
pub static mut sampler_parameteri: IndirectFn<SamplerParameteri> =
    IndirectFn::new("gl_sampler_parameteri", 0x2E84064);

pub static mut sdl_set_swap_interval: IndirectFn<SdlSetSwapInterval> =
    IndirectFn::new("sdl_gl_set_swap_interval", 0x17147C);

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

pub type SdlSetSwapInterval = extern "C" fn(interval: c_int) -> c_int;

pub const UNPACK_ROW_LENGTH: Enum = 0x0CF2;
pub const TEXTURE_2D: Enum = 0x0DE1;
pub const UNSIGNED_BYTE: Enum = 0x1401;
pub const RGBA: Enum = 0x1908;
pub const LINEAR: Enum = 0x2601;
pub const TEXTURE_MAG_FILTER: Enum = 0x2800;
pub const TEXTURE_MIN_FILTER: Enum = 0x2801;
pub const RGBA8: Enum = 0x8058;
