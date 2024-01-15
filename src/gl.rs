#![allow(non_upper_case_globals)]

use std::ffi::{c_int, c_uint};

use crate::process::IndirectFn;

pub static mut sampler_parameteri: IndirectFn<SamplerParameteri> =
    IndirectFn::new("gl_sampler_parameteri", 0x2E84064);

pub static mut sdl_set_swap_interval: IndirectFn<SdlSetSwapInterval> =
    IndirectFn::new("sdl_gl_set_swap_interval", 0x17147C);

pub type Uint = c_uint;
pub type Int = c_int;
pub type Enum = c_uint;

pub type SamplerParameteri = extern "stdcall" fn(Uint, Uint, Int);

pub type SdlSetSwapInterval = extern "C" fn(c_int) -> c_int;

pub const LINEAR: Enum = 0x2601;
pub const TEXTURE_MAG_FILTER: Enum = 0x2800;
pub const TEXTURE_MIN_FILTER: Enum = 0x2801;
