#![allow(non_upper_case_globals)]

use std::ffi::{c_int, c_uint, c_void};

use crate::{direct_fns, indirect_fns};

// static imports
indirect_fns! {
    #![bind_with(bind_static_fns)]

    #[symbol(glGetError)]
    extern "stdcall" fn get_error() -> Enum;

    #[symbol(glTexImage2D)]
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

    #[symbol(glPixelStorei)]
    extern "stdcall" fn pixel_storei(pname: Enum, param: Int);

    #[symbol(glGetIntegerv)]
    extern "stdcall" fn get_integerv(pname: Enum, params: *mut Int);

    #[symbol(glDeleteTextures)]
    extern "stdcall" fn delete_textures(n: Sizei, textures: *const Uint);

    #[symbol(glEnable)]
    extern "stdcall" fn enable(cap: Enum);

    #[symbol(glEnable)]
    extern "stdcall" fn disable(cap: Enum);

    #[symbol(glColorMask)]
    extern "stdcall" fn color_mask(red: Uint, green: Uint, blue: Uint, alpha: Uint);

    #[symbol(glDepthMask)]
    extern "stdcall" fn depth_mask(flag: Uint);

    #[symbol(glClear)]
    extern "stdcall" fn clear(mask: u32);
}

// dynamic imports
direct_fns! {
    #![bind_with(bind_dynamic_fns)]

    #[symbol(glDrawArrays, "opengl32.dll")]
    extern "stdcall" fn draw_arrays(mode: Enum, first: Int, count: Sizei);

    #[symbol(glStencilFunc, "opengl32.dll")]
    extern "stdcall" fn stencil_func(func: Enum, ref_value: Int, mask: Uint);

    #[symbol(glStencilOp, "opengl32.dll")]
    extern "stdcall" fn stencil_op(sfail: Enum, dpfail: Enum, dppass: Enum);

    #[symbol(glStencilMask, "opengl32.dll")]
    extern "stdcall" fn stencil_mask(mask: Uint);
}

// glew imports
indirect_fns! {
    #![bind_with(bind_glew_fns)]

    #[symbol(__glewSamplerParameteri)]
    extern "stdcall" fn sampler_parameteri(sampler: Uint, pname: Enum, param: Int);

    #[symbol(__glewBlendFuncSeparate)]
    extern "stdcall" fn blend_func_separate(
        src_rgb: Enum,
        dst_rgb: Enum,
        src_alpha: Enum,
        dst_alpha: Enum
    );

    #[symbol(__glewBindBuffer)]
    extern "stdcall" fn bind_buffer(target: Enum, buffer: Uint);

    #[symbol(__glewBufferData)]
    extern "stdcall" fn buffer_data(target: Enum, size: Sizei, data: *mut c_void, usage: Enum);

    #[symbol(__glewGenBuffers)]
    extern "stdcall" fn gen_buffers(n: Sizei, buffers: *mut Uint);

    #[symbol(__glewVertexAttribPointer)]
    extern "stdcall" fn vertex_attrib_pointer(
        index: Uint,
        size: Int,
        typ: Enum,
        normalized: Uint,
        stride: Sizei,
        pointer: *const c_void
    );

    #[symbol(__glewEnableVertexAttribArray)]
    extern "stdcall" fn enable_vertex_attrib_array(index: Uint);

    #[symbol(__glewDrawElementsBaseVertex)]
    extern "stdcall" fn draw_elements_base_vertex(mode: Enum, count: Sizei, typ: Enum, indicies: *mut c_void, basevertex: Int);

    #[symbol(__glewGenVertexArrays)]
    extern "stdcall" fn gen_vertex_arrays(n: Sizei, arrays: *mut Uint);

    #[symbol(__glewBindVertexArray)]
    extern "stdcall" fn bind_vertex_array(array: Uint);

    #[symbol(__glewGenRenderbuffers)]
    extern "stdcall" fn gen_renderbuffers(n: Sizei, renderbuffers: *mut Uint);

    #[symbol(__glewBindRenderbuffer)]
    extern "stdcall" fn bind_renderbuffer(target: Enum, renderbuffer: Uint);

    #[symbol(__glewRenderbufferStorage)]
    extern "stdcall" fn renderbuffer_storage(target: Enum, internalformat: Enum, width: Sizei, height: Sizei);

    #[symbol(__glewFramebufferRenderbuffer)]
    extern "stdcall" fn framebuffer_renderbuffer(target: Enum, attachment: Enum, renderbuffertarget: Enum, renderbuffer: Uint);

    #[symbol(__glewCompressedTexImage2D)]
    extern "stdcall" fn compressed_tex_image2d(
        target: Enum,
        level: Int,
        internalformat: Enum,
        width: Sizei,
        height: Sizei,
        border: Int,
        image_size: Sizei,
        data: *const c_void
    );

    #[symbol(__glewCompressedTexImage2DARB)]
    extern "stdcall" fn compressed_tex_image2d_arb(
        target: Enum,
        level: Int,
        internalformat: Enum,
        width: Sizei,
        height: Sizei,
        border: Int,
        image_size: Sizei,
        data: *const c_void
    );
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
