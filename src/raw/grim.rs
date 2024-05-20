#![allow(improper_ctypes, non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_void, CStr};

use crate::bound_fns;
use crate::raw::gl;
use crate::raw::memory::{BindError, Value};

bound_fns! {
    extern "stdcall" fn entry();

    // Initialize the basic graphics components
    #[pattern("55 8b ec 51 c6 05 ?? ?? ?? ?? 01", 0x0)]
    extern "C" fn init_gfx() -> u8;

    // file operation functions that work with LAB packed files
    #[pattern("55 8b ec 81 ec 20 02 00 00 a1 ?? ?? ?? ?? 33 c5", 0x0)]
    extern "C" fn open_file(filename: *mut c_char, mode: *mut c_char) -> *mut c_void;
    #[pattern("55 8b ec 8b 45 08 8b c8 69 c9 30 10 00 00 56", 0x0)]
    extern "C" fn close_file(file: *mut c_void) -> c_int;
    #[pattern("7e 08 81 fb 80 00 00 00 7e 1d 8b 0d ?? ?? ?? ?? 8b 51 18 68 c6 07 00 00", 0x19)]
    extern "C" fn read_file(file: *mut c_void, dst: *mut c_void, size: usize) -> usize;

    // Reads and parses a bitmap (.bm/.zbm) image into unified image container
    #[pattern("55 8b ec a1 ?? ?? ?? ?? 8b 48 20 56 6a 41", 0x0)]
    extern "C" fn open_bm_image(
        filename: *const c_char,
        param_2: u32,
        param_3: u32,
    ) -> *mut ImageContainer;

    // Copy an image and surface from a source to a pre-allocated destination
    #[pattern("8b 04 8d ?? ?? ?? ?? 8b 40 18 85 c0 74 1f", 0x20)]
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
    #[pattern("55 8b ec 53 8b 5d 08 8b 43 0c 0f af 43 10", 0x0)]
    extern "C" fn decompress_image(image: *const Image);

    // Manage a resource based on its state
    #[pattern("55 8b ec 51 56 8b 75 08 33 c0 81 7e 08 42 4b 4e 44", 0x0)]
    extern "C" fn manage_resource(resource: *mut Resource) -> c_int;

    // Gets the surface for an image, creating it if necessary
    #[pattern("52 8d 4d f4 51 8d 55 08 52 8d 4d f0 51 8d 55 ec 52", 0x1C)]
    extern "C" fn bind_image_surface(
        image: *mut Image,
        param_2: u32,
        param_3: u32,
        param_4: u32
    ) -> *mut Surface;

    // Prepare a surface (aka texture) for uploading to the GPU or upload it now

    #[pattern("55 8b ec 83 ec 18 53 56 8b 75 08 8b 46 0c 57 83 f8 12", 0x0)]
    extern "C" fn surface_upload(surface: *mut Surface, image_data: *mut c_void);

    // Sets all the OpenGL state for the next draw call
    #[pattern("55 8b ec 51 53 56 8b 75 08 80 be 32 01 00 00 00 57 74 09", 0x0)]
    extern "C" fn setup_draw(draw: *mut Draw, index_buffer: *const c_void);

    // Sets the shader for the next draw call
    #[pattern("55 8b ec 8b 45 08 8b 4d 0c 3b 48 4c 74 1a", 0x0)]
    extern "C" fn set_draw_shader(draw: *mut Draw, shader: *mut Shader);

    // Draws the scene with the software renderer
    #[pattern("55 8b ec 81 ec 40 02 00 00 a1 ?? ?? ?? ?? 33 c5 89 45 fc", 0x0)]
    extern "C" fn draw_software_scene(
        draw: *const Draw,
        software_surface: *const Surface,
        transition: f32
    );

    // Draw the selected indexed primitives
    #[pattern("55 8b ec 56 8b 75 18 57 8b 7d 10 83 fe fe 75 05", 0x0)]
    extern "C" fn draw_indexed_primitives(
        draw: *mut Draw,
        param_2: u32,
        param_3: u32,
        param_4: u32,
        param_5: u32
    );

    // Leaves a marker for debugging OpenGL calls
    #[pattern("55 8b ec 57 8b 3d ?? ?? ?? ?? 85 ff 74 17", 0x0)]
    extern "C" fn marker(len: usize, message: *const c_char);
}

pub fn find_fns(code_addr: usize, code_size: usize) -> Result<(), BindError> {
    init_gfx.find(code_addr, code_size)?;
    open_file.find(code_addr, code_size)?;
    close_file.find(code_addr, code_size)?;
    read_file.find(code_addr, code_size)?;
    open_bm_image.find(code_addr, code_size)?;
    copy_image.find(code_addr, code_size)?;
    decompress_image.find(code_addr, code_size)?;
    manage_resource.find(code_addr, code_size)?;
    bind_image_surface.find(code_addr, code_size)?;
    surface_upload.find(code_addr, code_size)?;
    setup_draw.find(code_addr, code_size)?;
    set_draw_shader.find(code_addr, code_size)?;
    draw_software_scene.find(code_addr, code_size)?;
    draw_indexed_primitives.find(code_addr, code_size)?;
    marker.find(code_addr, code_size)?;

    Ok(())
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
