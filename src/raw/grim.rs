#![allow(improper_ctypes, non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_void, CStr};

use crate::raw::gl;
use crate::raw::memory::Value;
use crate::{bound_fns, fns};

bound_fns! {
    extern "stdcall" fn entry();
}

fns! {
    // file operation functions that work with LAB packed files
    #[address(0x1EF80)]
    extern "C" fn open_file(filename: *mut c_char, mode: *mut c_char) -> *mut c_void;
    #[address(0x1C870)]
    extern "C" fn close_file(file: *mut c_void) -> c_int;
    #[address(0x1E050)]
    extern "C" fn read_file(file: *mut c_void, dst: *mut c_void, size: usize) -> usize;

    // Reads and parses a bitmap (.bm/.zbm) image into unified image container
    #[address(0xDADE0)]
    extern "C" fn open_bm_image(
        filename: *const c_char,
        param_2: u32,
        param_3: u32,
    ) -> *mut ImageContainer;

    // Copy an image and surface from a source to a pre-allocated destination
    #[address(0xE5EC0)]
    extern "C" fn copy_image(
        dst_image: *mut Image,
        dst_surface: *mut Surface,
        src_image: *mut Image,
        src_surface: *mut Surface,
        x: u32,
        y: u32,
        param_7: u32,
        param_8: u32,
    );

    // Decompresses an image into the global decompression buffer
    #[address(0x24D20)]
    extern "C" fn decompress_image(image: *const Image);

    #[address(0x2B340)]
    extern "C" fn manage_resource(resource: *mut Resource) -> c_int;

    // Gets the surface for an image, creating it if necessary
    #[address(0x13E010)]
    extern "C" fn bind_image_surface(image: *mut Image, param_2: u32, param_3: u32, param_4: u32) -> *mut Surface;

    // Prepare a surface (aka texture) for uploading to the GPU or upload it now
    #[address(0xE8A80)]
    extern "C" fn surface_upload(surface: *mut Surface, image_data: *mut c_void);

    #[address(0xE8FB0)]
    extern "C" fn set_draw_shader(draw: *mut Draw, shader: *mut Shader);

    // Sets all the OpenGL state for the next draw call
    #[address(0xF3540)]
    extern "C" fn setup_draw(draw: *mut Draw, index_buffer: *const c_void);

    #[address(0xF91C0)]
    extern "C" fn draw_software_scene(
        draw: *const Draw,
        software_surface: *const Surface,
        transition: f32
    );

    // Draw the selected indexed primitives
    #[address(0xF3890)]
    extern "C" fn draw_indexed_primitives(
        draw: *mut Draw,
        param_2: u32,
        param_3: u32,
        param_4: u32,
        param_5: u32
    );

    // Initialize the basic graphics components
    #[address(0xF2960)]
    extern "C" fn init_gfx() -> u8;

    #[address(0xEA1B0)]
    extern "C" fn marker(len: usize, message: *const c_char);
}

// buffers used for backgrounds and overlays
pub static mut DECOMPRESSION_BUFFER: Value<*const Image> = Value::new(0x1691C78);
pub static mut CLEAN_BUFFER: Value<*const Image> = Value::new(0x1691C7C);
pub static mut CLEAN_Z_BUFFER: Value<*const Image> = Value::new(0x1691C80);
pub static mut BACK_BUFFER: Value<Image> = Value::new(0x31B4DA0);
pub static mut SMUSH_BUFFER: Value<*const Image> = Value::new(0x16A8474);
pub static mut ACTIVE_SMUSH_FRAME: Value<*const SmushFrame> = Value::new(0x1714B98);
// backgrounds' render pass data
pub static mut BITMAP_UNDERLAYS_RENDER_PASS: Value<*const RenderPass> = Value::new(0x30861E4);
pub static mut RENDERING_MODE: Value<f32> = Value::new(0x2E81230);
pub static mut GAME_WINDOW: Value<*const c_void> = Value::new(0x2E81244);

pub static mut TEXTURED_QUAD_SHADER: Value<*const Shader> = Value::new(0x2E81848);

/// LLVM's libc++ std::vector
#[repr(C)]
pub struct Vector<T> {
    pub start: *mut T,
    pub end: *mut T,
    pub capacity_end: *mut T,
    phantom: std::marker::PhantomData<T>,
}

impl<T: Sized> Vector<T> {
    pub unsafe fn data(&self) -> &[T] {
        std::slice::from_raw_parts(self.start, self.len())
    }

    pub fn len(&self) -> usize {
        if self.start.is_null() {
            0
        } else {
            let span = self.end as usize - self.start as usize;
            1 + (span / std::mem::size_of::<T>())
        }
    }
}

/// A named render pass with all its associated entities
#[repr(C)]
pub struct RenderPass {
    pub name: *const c_char,
    pub entities: Vector<RenderPassEntity>,
    pub field_3: *const c_void,
}

/// An entity that will be drawn when associated with a render pass
#[repr(C)]
pub struct RenderPassEntity {
    pub field_1: u32,
    pub shader_pipeline: *const c_void,
    pub field_3: u32,
    pub field_4: *const c_void,

    pub fields_5_12: [u32; 8],

    pub field_13: u32,
    pub surface: *const Surface,
    pub field_15: u32,
    pub field_16: u32,

    pub fields_17_31: [u32; 15],
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

#[repr(C)]
pub struct SmushFrame {
    pub buffer: *mut c_void,
    pub attributes: ImageAttributes,
}
