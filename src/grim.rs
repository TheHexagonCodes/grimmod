#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_void, CStr};

use crate::gl;
use crate::process::{DirectFn, Value};

// file operation functions that work with LAB packed files
pub static mut open_file: DirectFn<OpenFile> = DirectFn::new("open_file", 0x1EF80);
pub static mut close_file: DirectFn<CloseFile> = DirectFn::new("close_file", 0x1C870);
pub static mut read_file: DirectFn<ReadFile> = DirectFn::new("read_file", 0x1E050);
pub static mut read_all: DirectFn<ReadAll> = DirectFn::new("read_all", 0xE6700);

// functions for loading images and preparing textures
pub static mut open_bm_image: DirectFn<OpenBmImage> = DirectFn::new("open_bm_image", 0xDADE0);
pub static mut surface_upload: DirectFn<SurfaceUpload> = DirectFn::new("surface_upload", 0xE8A80);
pub static mut surface_allocate: DirectFn<SurfaceAllocate> =
    DirectFn::new("surface_allocate", 0xECF70);
pub static mut surface_bind_existing: DirectFn<SurfaceBindExisting> =
    DirectFn::new("surface_bind_existing", 0x12ED20);
pub static mut copy_image: DirectFn<CopyImage> = DirectFn::new("copy_image", 0xE5EC0);
pub static mut decompress_image: DirectFn<DecompressImage> =
    DirectFn::new("decompress_image", 0x24D20);
pub static mut manage_resource: DirectFn<ManageResource> =
    DirectFn::new("manage_resource", 0x2B340);
pub static mut setup_draw: DirectFn<SetupDraw> = DirectFn::new("setup_draw", 0xF3540);
pub static mut compile_shader: DirectFn<CompileShader> = DirectFn::new("compile_shader", 0xF2000);
pub static mut draw_software_scene: DirectFn<DrawSoftwareScene> =
    DirectFn::new("draw_software_scene", 0xF91C0);

// buffers used for backgrounds and overlays
pub static mut CLEAN_BUFFER: Value<*const *const Image> = Value::new(0x1691C7C);
pub static mut BACK_BUFFER: Value<*const Image> = Value::new(0x31B4DA0);
// backgrounds' render pass data
pub static mut BITMAP_UNDERLAYS_RENDER_PASS: Value<*const *const RenderPass> =
    Value::new(0x30861E4);
pub static mut RENDERING_MODE: Value<*const f32> = Value::new(0x2E81230);

pub static mut marker: DirectFn<extern "C" fn(len: usize, message: *const c_char)> =
    DirectFn::new("marker", 0xEA1B0);

pub type OpenFile = extern "C" fn(*mut c_char, *mut c_char) -> *mut c_void;
pub type CloseFile = extern "C" fn(*mut c_void) -> c_int;
pub type ReadFile = extern "C" fn(*mut c_void, *mut c_void, usize) -> usize;
pub type ReadAll =
    extern "C" fn(dst: *mut *const c_void, size: *mut usize, filename: *const c_char);

pub type OpenBmImage = extern "C" fn(*const c_char, u32, u32) -> *mut ImageContainer;
pub type CopyImage =
    extern "C" fn(*mut Image, *mut c_void, *mut Image, *mut c_void, u32, u32, u32, u32);
pub type SurfaceUpload = extern "C" fn(*mut Surface, *mut c_void);
pub type SurfaceAllocate =
    extern "C" fn(width: c_int, height: c_int, format: c_uint, param_4: c_int) -> *const Surface;
pub type SurfaceBindExisting = extern "C" fn(
    surface: *mut Surface,
    image: *const Image,
    width: i32,
    height: i32,
    param_4: u32,
    param_5: u32,
    param_6: u32,
    param_7: u32,
    texture_id: gl::Uint,
);
pub type DecompressImage = extern "C" fn(*const Image);
pub type ManageResource = extern "C" fn(*mut Resource) -> c_int;
pub type SetupDraw = extern "C" fn(*mut Draw, *const c_void);
pub type CompileShader = extern "C" fn(name: *const c_char) -> *const Shader;
pub type DrawSoftwareScene =
    extern "C" fn(draw: *const Draw, software_surface: *const Surface, transition: f32);

/// Everything associated with a render pass (background, z-buffer, shadows, etc.)
///
/// This struct actually contains some kind of vector but it's not necessary to
/// properly model it yet.
#[repr(C)]
pub struct RenderPass {
    pub name: *const c_char,
    pub data: *const RenderPassData,
    pub field_3: *const c_void,
    pub field_4: *const c_void,
}

/// Data associated with a render pass
///
/// The exact structure, size, and purpose of this struct is not fully understood
/// but only surface is required now.
#[repr(C)]
pub struct RenderPassData {
    pub field_1: u32,
    pub shader_pipeline: *const c_void,
    pub field_3: u32,
    pub field_4: *const c_void,

    pub field_5: *const c_void,
    pub field_6: u32,
    pub field_7: *const c_void,
    pub field_8: *const c_void,

    pub field_9: u32,
    pub field_10: u32,
    pub field_11: u32,
    pub field_12: u32,

    pub field_13: u32,
    pub surface: *const Surface,
}

/// Data used to setup the next draw call
///
/// The actual structure is much larger but it's not needed yet
#[repr(C)]
pub struct Draw {
    pub field_1: u32,
    pub field_2: u32,
    pub render_target: *const c_void,
    pub depth_drawbuffer: c_int,

    pub fields_4_10: [u32; 6],

    pub framebuffer: c_uint,
    pub samplers: [c_uint; 8],
    pub shader: *const Shader,

    pub fields_14_21: [u32; 7],

    pub surfaces: [*const Surface; 8],
}

/// A compiled shader program with vertex and fragment shaders attached
#[repr(C)]
pub struct Shader {
    pub name: [c_char; 512],
    pub vertex_shader: gl::Uint,
    pub fragment_shader: gl::Uint,
    pub program: gl::Uint,
    pub fragment_constants_index: gl::Uint,
    pub vertex_constants_index: gl::Uint,
    pub param_7: u32,
    pub param_8: *const c_void,
    pub param_9: u32,
    pub param_10: u32,
}

/// Common image attributes extracted to struct
#[repr(C)]
pub struct ImageAttributes {
    pub width: i32,
    pub height: i32,
    pub size: usize,
    bytes_per_row: usize,
    calculated_width: usize,
    _ignore: u32,
    bits_per_pixel: u32,
    rgb_bits: [u32; 3],
    rgb_shift: [u32; 3],
    rgb_loss: [u32; 3],
    _ignore_post: [u32; 3],
}

/// A single image or animation frame
#[repr(C)]
pub struct Image {
    pub param_1: u32,
    pub param_2: u32,
    pub param_3: u32,
    pub attributes: ImageAttributes,
    pub param_5: u32,
    pub data: *mut c_void,
    pub param_7: u32,
    pub name: *mut CStr,
    pub surface: *mut c_void,
}

/// A static or animated image container
#[repr(C)]
pub struct ImageContainer {
    pub name: [c_char; 32],
    pub codec: u32,
    pub palette_included: u32,
    pub format: u32,
    pub bits_per_pixel: u32,
    pub rgb_bits: [u32; 3],
    pub rgb_shift: [u32; 3],
    pub rgb_loss: [u32; 3],
    pub param_9: u32,
    pub param_10: u32,
    pub param_11: u32,
    pub param_12: u32,
    pub image_count: u32,
    pub param_14: u32,
    pub x: u32,
    pub y: u32,
    pub transparent_color: u32,
    pub images: *const *const Image,
}

/// A texture in the OpenGL renderer
#[repr(C)]
pub struct Surface {
    pub width: i32,
    pub height: i32,
    pub param_3: u32,
    pub format: u32,
    pub uploaded: u32,
    pub param_6: u32,
    pub image_data: *mut c_void,
    pub render_target: u32,
    pub texture_id: u32,
    pub param_10: u32,
    pub param_11: u32,
    pub param_12: u32,
}

#[repr(C)]
pub struct Resource {
    pub state: u32,
    pub filename: *const c_char,
    pub kind: *const c_char,
    pub image_container: *const ImageContainer,
    pub size: isize,
}
