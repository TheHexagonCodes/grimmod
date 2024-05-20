#![allow(non_upper_case_globals)]

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use windows::core::PCSTR;
use windows::Win32::Foundation::{BOOL, HMODULE, HWND, PROC};
use windows::Win32::Graphics::Gdi::HDC;

use crate::raw::memory::BoundFn;
use crate::{bound_fns, fn_refs};

use super::wrappers;

// static imports
bound_fns! {
    extern "stdcall" fn get_proc_address(name: PCSTR) -> PROC;
    extern "stdcall" fn get_error() -> Enum;
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
    extern "stdcall" fn pixel_storei(pname: Enum, param: Int);
    extern "stdcall" fn get_integerv(pname: Enum, params: *mut Int);
    extern "stdcall" fn delete_textures(n: Sizei, textures: *const Uint);
    extern "stdcall" fn enable(cap: Enum);
    extern "stdcall" fn disable(cap: Enum);
    extern "stdcall" fn color_mask(red: Uint, green: Uint, blue: Uint, alpha: Uint);
    extern "stdcall" fn depth_mask(flag: Uint);
    extern "stdcall" fn clear(mask: u32);
}

// dynamic imports
bound_fns! {
    extern "stdcall" fn draw_arrays(mode: Enum, first: Int, count: Sizei);
    extern "stdcall" fn stencil_func(func: Enum, ref_value: Int, mask: Uint);
    extern "stdcall" fn stencil_op(sfail: Enum, dpfail: Enum, dppass: Enum);
    extern "stdcall" fn stencil_mask(mask: Uint);
}

// glew imports
bound_fns! {
    extern "stdcall" fn sampler_parameteri(sampler: Uint, pname: Enum, param: Int);
    extern "stdcall" fn blend_func_separate(
        src_rgb: Enum,
        dst_rgb: Enum,
        src_alpha: Enum,
        dst_alpha: Enum
    );
    extern "stdcall" fn bind_buffer(target: Enum, buffer: Uint);
    extern "stdcall" fn buffer_data(target: Enum, size: Sizei, data: *mut c_void, usage: Enum);
    extern "stdcall" fn gen_buffers(n: Sizei, buffers: *mut Uint);
    extern "stdcall" fn vertex_attrib_pointer(
        index: Uint,
        size: Int,
        typ: Enum,
        normalized: Uint,
        stride: Sizei,
        pointer: *const c_void
    );
    extern "stdcall" fn enable_vertex_attrib_array(index: Uint);
    extern "stdcall" fn draw_elements_base_vertex(mode: Enum, count: Sizei, typ: Enum, indicies: *mut c_void, basevertex: Int);
    extern "stdcall" fn gen_vertex_arrays(n: Sizei, arrays: *mut Uint);
    extern "stdcall" fn bind_vertex_array(array: Uint);
    extern "stdcall" fn gen_renderbuffers(n: Sizei, renderbuffers: *mut Uint);
    extern "stdcall" fn bind_renderbuffer(target: Enum, renderbuffer: Uint);
    extern "stdcall" fn renderbuffer_storage(target: Enum, internalformat: Enum, width: Sizei, height: Sizei);
    extern "stdcall" fn framebuffer_renderbuffer(target: Enum, attachment: Enum, renderbuffertarget: Enum, renderbuffer: Uint);
}

fn_refs! {
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

pub const TRIANGLES: Enum = 0x0004;
pub const UNPACK_ROW_LENGTH: Enum = 0x0CF2;
pub const TEXTURE_2D: Enum = 0x0DE1;
pub const UNSIGNED_BYTE: Enum = 0x1401;
pub const FLOAT: Enum = 0x1406;
pub const RGBA: Enum = 0x1908;
pub const LINEAR: Enum = 0x2601;
pub const TEXTURE_MAG_FILTER: Enum = 0x2800;
pub const TEXTURE_MIN_FILTER: Enum = 0x2801;
pub const SRC_ALPHA: Enum = 0x0302;
pub const ONE_MINUS_SRC_ALPHA: Enum = 0x0303;
pub const RGBA8: Enum = 0x8058;
pub const ARRAY_BUFFER: Enum = 0x8892;
pub const STATIC_DRAW: Enum = 0x88E4;
pub const VIEWPORT: Enum = 0x0BA2;
pub const STENCIL_TEST: Enum = 0x0B90;
pub const STENCIL_BUFFER_BIT: u32 = 0x00000400;
pub const KEEP: Enum = 0x1E00;
pub const REPLACE: Enum = 0x1E01;
pub const ALWAYS: Enum = 0x0207;
pub const EQUAL: Enum = 0x0202;
pub const STENCIL_ATTACHMENT: Enum = 0x8D20;
pub const FRAMEBUFFER: Enum = 0x8D40;
pub const RENDERBUFFER: Enum = 0x8D41;
pub const DEPTH24_STENCIL8: Enum = 0x88F0;
pub const VERTEX_ARRAY_BINDING: Enum = 0x85B5;

pub fn bind_static_fn<F>(
    bound_fn: &BoundFn<F>,
    name: &str,
    import_map: &HashMap<String, usize>,
) -> Result<(), String> {
    let import = import_map.get(name).ok_or_else(|| name.to_string())?;
    bound_fn.bind(*import);
    Ok(())
}

pub fn bind_static_fns(import_map: &HashMap<String, usize>) -> Result<(), String> {
    bind_static_fn(&get_proc_address, "wglGetProcAddress", import_map)?;
    bind_static_fn(&get_error, "glGetError", import_map)?;
    bind_static_fn(&tex_image_2d, "glTexImage2D", import_map)?;
    bind_static_fn(&pixel_storei, "glPixelStorei", import_map)?;
    bind_static_fn(&get_integerv, "glGetIntegerv", import_map)?;
    bind_static_fn(&delete_textures, "glDeleteTextures", import_map)?;
    bind_static_fn(&enable, "glEnable", import_map)?;
    bind_static_fn(&disable, "glDisable", import_map)?;
    bind_static_fn(&color_mask, "glColorMask", import_map)?;
    bind_static_fn(&depth_mask, "glDepthMask", import_map)?;
    bind_static_fn(&clear, "glClear", import_map)?;

    Ok(())
}

pub fn bind_dynamic_fns() -> Result<(), wrappers::DllError> {
    wrappers::with_system_dll("opengl32.dll", |dll| {
        dll.bind(&draw_arrays, "glDrawArrays")?;
        dll.bind(&stencil_func, "glStencilFunc")?;
        dll.bind(&stencil_op, "glStencilOp")?;
        dll.bind(&stencil_mask, "glStencilMask")?;

        Ok(())
    })
}

pub fn bind_glew_fn<F>(bound_fn: &BoundFn<F>, name: &str) -> Result<(), String> {
    let cname = CString::new(name).map_err(|_| name.to_string())?;
    let func =
        get_proc_address(PCSTR(cname.as_ptr() as *const u8)).ok_or_else(|| name.to_string())?;
    bound_fn.bind(func as usize);
    Ok(())
}

pub fn bind_glew_fns() -> Result<(), String> {
    bind_glew_fn(&sampler_parameteri, "glSamplerParameteri")?;
    bind_glew_fn(&blend_func_separate, "glBlendFuncSeparate")?;
    bind_glew_fn(&gen_buffers, "glGenBuffers")?;
    bind_glew_fn(&bind_buffer, "glBindBuffer")?;
    bind_glew_fn(&buffer_data, "glBufferData")?;
    bind_glew_fn(&vertex_attrib_pointer, "glVertexAttribPointer")?;
    bind_glew_fn(&enable_vertex_attrib_array, "glEnableVertexAttribArray")?;
    bind_glew_fn(&draw_elements_base_vertex, "glDrawElementsBaseVertex")?;
    bind_glew_fn(&gen_vertex_arrays, "glGenVertexArrays")?;
    bind_glew_fn(&bind_vertex_array, "glBindVertexArray")?;
    bind_glew_fn(&gen_renderbuffers, "glGenRenderbuffers")?;
    bind_glew_fn(&bind_renderbuffer, "glBindRenderbuffer")?;
    bind_glew_fn(&renderbuffer_storage, "glRenderbufferStorage")?;
    bind_glew_fn(&framebuffer_renderbuffer, "glFramebufferRenderbuffer")?;

    Ok(())
}
