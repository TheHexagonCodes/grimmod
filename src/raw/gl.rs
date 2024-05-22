#![allow(non_upper_case_globals)]

use std::ffi::{c_int, c_uint, c_void};
use windows::core::PCSTR;
use windows::Win32::Foundation::PROC;

use crate::raw::memory::BindError;
use crate::raw::wrappers::{with_system_dll, DllError};
use crate::{direct_fns, indirect_fns};

// static imports
indirect_fns! {
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
direct_fns! {
    extern "stdcall" fn draw_arrays(mode: Enum, first: Int, count: Sizei);
    extern "stdcall" fn stencil_func(func: Enum, ref_value: Int, mask: Uint);
    extern "stdcall" fn stencil_op(sfail: Enum, dpfail: Enum, dppass: Enum);
    extern "stdcall" fn stencil_mask(mask: Uint);
}

// glew imports
indirect_fns! {
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

pub type Uint = c_uint;
pub type Int = c_int;
pub type Enum = c_uint;
pub type Sizei = c_int;

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

pub fn bind_static_fns() -> Result<(), BindError> {
    get_proc_address.bind_symbol("wglGetProcAddress")?;
    get_error.bind_symbol("glGetError")?;
    tex_image_2d.bind_symbol("glTexImage2D")?;
    pixel_storei.bind_symbol("glPixelStorei")?;
    get_integerv.bind_symbol("glGetIntegerv")?;
    delete_textures.bind_symbol("glDeleteTextures")?;
    enable.bind_symbol("glEnable")?;
    disable.bind_symbol("glDisable")?;
    color_mask.bind_symbol("glColorMask")?;
    depth_mask.bind_symbol("glDepthMask")?;
    clear.bind_symbol("glClear")?;

    Ok(())
}

pub fn bind_dynamic_fns() -> Result<(), DllError> {
    with_system_dll("opengl32.dll", |dll| {
        dll.bind(&draw_arrays, "glDrawArrays")?;
        dll.bind(&stencil_func, "glStencilFunc")?;
        dll.bind(&stencil_op, "glStencilOp")?;
        dll.bind(&stencil_mask, "glStencilMask")?;

        Ok(())
    })
}

pub fn bind_glew_fns() -> Result<(), BindError> {
    sampler_parameteri.bind_symbol("__glewSamplerParameteri")?;
    blend_func_separate.bind_symbol("__glewBlendFuncSeparate")?;
    gen_buffers.bind_symbol("__glewGenBuffers")?;
    bind_buffer.bind_symbol("__glewBindBuffer")?;
    buffer_data.bind_symbol("__glewBufferData")?;
    vertex_attrib_pointer.bind_symbol("__glewVertexAttribPointer")?;
    enable_vertex_attrib_array.bind_symbol("__glewEnableVertexAttribArray")?;
    draw_elements_base_vertex.bind_symbol("__glewDrawElementsBaseVertex")?;
    gen_vertex_arrays.bind_symbol("__glewGenVertexArrays")?;
    bind_vertex_array.bind_symbol("__glewBindVertexArray")?;
    gen_renderbuffers.bind_symbol("__glewGenRenderbuffers")?;
    bind_renderbuffer.bind_symbol("__glewBindRenderbuffer")?;
    renderbuffer_storage.bind_symbol("__glewRenderbufferStorage")?;
    framebuffer_renderbuffer.bind_symbol("__glewFramebufferRenderbuffer")?;

    Ok(())
}
