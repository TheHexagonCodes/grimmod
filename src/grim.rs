use crate::process;

use std::ffi::{c_char, c_int, c_void, CStr};
use std::mem;

pub mod address {
    use crate::process::relative_address as relative;
    use lazy_static::lazy_static;

    lazy_static! {
        // file operation functions that work with LAB packed files
        pub static ref OPEN_FILE: usize = relative(0x1EF80);
        pub static ref CLOSE_FILE: usize = relative(0x1C870);
        pub static ref READ_FILE: usize = relative(0x1E050);

        // functions for loading images and preparing textures
        pub static ref OPEN_BM_IMAGE: usize = relative(0xDADE0);
        pub static ref SURFACE_UPLOAD: usize = relative(0xE8A80);
        pub static ref COPY_IMAGE: usize = relative(0xE5EC0);
        pub static ref DECOMPRESS_IMAGE: usize = relative(0x24D20);

        // various buffers used for rendering textures
        pub static ref DECOMPRESSION_BUFFER_PTR: usize = relative(0x1691C78);
        pub static ref CLEAN_BUFFER_PTR: usize = relative(0x1691C7C);

        // background render pass data
        pub static ref BITMAP_UNDERLAYS_RENDER_PASS_PTR: usize = relative(0x30861E4);

        // contains the address for the RuntimeContext in use by the game
        pub static ref RUNTIME_CONTEXT_PTR: usize = relative(0x31B2CD8);
    }
}

type FileOpener = extern "C" fn(*mut c_char, *mut c_char) -> *mut c_void;
type FileCloser = extern "C" fn(*mut c_void) -> c_int;
type FileReader = extern "C" fn(*mut c_void, *mut c_void, usize) -> usize;

#[inline(always)]
pub fn open_file(filename: *mut c_char, mode: *mut c_char) -> *mut c_void {
    unsafe {
        let f: FileOpener = mem::transmute(*address::OPEN_FILE);
        f(filename, mode)
    }
}

#[inline(always)]
pub fn close_file(file: *mut c_void) -> c_int {
    unsafe {
        let f: FileCloser = mem::transmute(*address::CLOSE_FILE);
        f(file)
    }
}

#[inline(always)]
pub fn read_file(file: *mut c_void, dst: *mut c_void, size: usize) -> usize {
    unsafe {
        let f: FileReader = mem::transmute(*address::READ_FILE);
        f(file, dst, size)
    }
}

/// Stores a set of IO functions the game uses at runtime
///
/// This is used to make what functions are used at runtime configurable.
/// e.g. read files from a simple directory in debug but from LAB archives in release.
#[repr(C)]
pub struct RuntimeContext {
    _ignore_pre: [u32; 12],
    pub open_file: *const FileOpener,
    pub close_file: *const FileCloser,
    pub read_file: *const FileReader,
    _ignore_post: [u32; 17],
}

#[inline(always)]
pub fn with_runtime_context<F: FnOnce(&mut RuntimeContext)>(f: F) {
    unsafe {
        let runtime_context = process::read::<usize>(*address::RUNTIME_CONTEXT_PTR);
        process::with_mut_ref(runtime_context, f)
    }
}

pub type OpenBmImage = extern "C" fn(*const c_char, u32, u32) -> *mut ImageContainer;

pub type CopyImage =
    extern "C" fn(*mut Image, *mut c_void, *mut Image, *mut c_void, u32, u32, u32, u32);

pub type SurfaceUpload = extern "C" fn(*mut Surface, *mut c_void);

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
