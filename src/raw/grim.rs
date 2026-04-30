#![allow(improper_ctypes, non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_void, CStr};

use crate::direct_fns;
use crate::raw::gl;
use crate::raw::memory::Value;

// ---- Application entry ----

#[cfg(target_os = "windows")]
direct_fns! {
    // The main application entry point, after DLL initialization
    extern "stdcall" fn entry();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
direct_fns! {
    // The main application entry point
    #[symbol("main")]
    extern "C" fn entry(argc: c_int, argv: *const *const c_char);
}

// ---- Game functions ----

#[cfg(target_os = "windows")]
direct_fns! {
    #![bind_with(find_fns)]

    // Initializes the 3 renderers (software, deferred, hardware)
    #[pattern("c7 00 00 00 00 00 8b 0d ?? ?? ?? ?? c7 01 01 00 00 00 8b 15 ?? ?? ?? ?? c7", 0x14)]
    extern "C" fn init_renderers();

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

    // Draws the scene with either renderer (software, deferred)
    #[pattern("55 8b ec 81 ec 40 02 00 00 a1 ?? ?? ?? ?? 33 c5 89 45 fc", 0x0)]
    extern "C" fn render_scene(
        draw: *const Draw,
        surface: *const Surface,
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

    // The following functions are only used to find static values

    // Allocates the base image buffer
    #[pattern("55 8b ec 8b 15 ?? ?? ?? ?? 83 ec 08 56 8d 45 f8 50 8d 4d fc 51 52", 0x0)]
    extern "C" fn init_base_buffer() -> c_uint;

    // Intializes main software front/back buffers
    #[pattern("55 8b ec 8b 45 08 3b 05 ?? ?? ?? ?? 72 04 33 c0 5d c3", 0x0)]
    extern "C" fn init_software_buffers();

    // Intializes the buffer used to decode smush video frames
    #[pattern("75 07 b8 01 00 00 00 5d c3 a1 ?? ?? ?? ?? 56 8b 75 08", 0xA)]
    extern "C" fn init_smush_buffer();

    // Resets some of the buffers used for temporary work
    #[pattern("a1 ?? ?? ?? ?? 56 33 f6 3b c6 74 0f 50", 0x0)]
    extern "C" fn reset_intermediate_buffers();

    // Decodes the next smush frame (not fully known, unnecessary)
    #[pattern("55 8b ec 81 ec 7c 04 00 00 a1 ?? ?? ?? ?? 33 c5", 0x0)]
    extern "C" fn decode_smush_frame();

    // Defines and inits most of the shaders and render steps for the remaster
    #[pattern("a3 ?? ?? ?? ?? e8 ?? ?? ?? ?? 83 c4 04 a3 ?? ?? ?? ?? c3", 0x359)]
    extern "C" fn init_shaders_and_render_passes();

    // Begins toggling between the software and remaster renderer
    #[pattern("55 8b ec 83 ec 08 56 57 8b 7d 08 33 f6 3b fe 0f 85 3c 01 00 00", 0x0)]
    extern "C" fn toggle_renderers();
}

#[cfg(target_os = "linux")]
direct_fns! {
    #![bind_with(find_fns)]

    #[symbol("zg_Render_Initialize")]
    extern "C" fn init_renderers();

    #[symbol("stdFileOpen")]
    extern "C" fn open_file(filename: *mut c_char, mode: *mut c_char) -> *mut c_void;
    #[symbol("stdFileClose")]
    extern "C" fn close_file(file: *mut c_void) -> c_int;
    #[symbol("stdFileRead")]
    extern "C" fn read_file(file: *mut c_void, dst: *mut c_void, size: usize) -> usize;

    #[symbol("stdBitmap_Load")]
    extern "C" fn open_bm_image(
        filename: *const c_char,
        param_2: u32,
        param_3: u32,
    ) -> *mut ImageContainer;

    #[symbol("zg_Render_BufferCopyImpl")]
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

    #[symbol("sputRender_Decompress")]
    extern "C" fn decompress_image(image: *const Image);

    #[symbol("sputResource_BackgroundHandler")]
    extern "C" fn manage_resource(resource: *mut Resource) -> c_int;

    #[symbol("zg_RendererDeferred_GetCachedTextureFromColorVBuffer")]
    extern "C" fn bind_image_surface(
        image: *mut Image,
        param_2: u32,
        param_3: u32,
        param_4: u32
    ) -> *mut Surface;

    #[symbol("zg_Surface_Upload")]
    extern "C" fn surface_upload(surface: *mut Surface, image_data: *mut c_void);

    #[symbol("zg_RenderContext_DrawSetup")]
    extern "C" fn setup_draw(draw: *mut Draw, index_buffer: *const c_void);

    #[symbol("zg_Shader_Apply")]
    extern "C" fn set_draw_shader(draw: *mut Draw, shader: *mut Shader);

    #[symbol("zg_Surface_Present")]
    extern "C" fn render_scene(
        draw: *const Draw,
        surface: *const Surface,
        transition: f32
    );

    #[symbol("zg_RenderContext_DrawIndexedPrimitives")]
    extern "C" fn draw_indexed_primitives(
        draw: *mut Draw,
        param_2: u32,
        param_3: u32,
        param_4: u32,
        param_5: u32
    );

    #[symbol("zg_RenderContext_PushMarker")]
    extern "C" fn marker(len: usize, message: *const c_char);

    #[symbol("allocateDefaultBuffers")]
    extern "C" fn init_base_buffer() -> c_uint;

    #[symbol("zg_RendererSoftware_SetBuffers")]
    extern "C" fn init_software_buffers();

    #[symbol("sputSmush_Initialize")]
    extern "C" fn init_smush_buffer();

    #[symbol("sputRender_Close")]
    extern "C" fn reset_intermediate_buffers();

    #[symbol("SmushPlay_UpdateMovie")]
    extern "C" fn decode_smush_frame();

    #[symbol("loadRenderResources")]
    extern "C" fn init_shaders_and_render_passes();

    #[symbol("sputRender_ControlHandler")]
    extern "C" fn toggle_renderers();
}

// macOS: same engine, same symbol names as Linux, but 64-bit. The macOS Mach-O binary has
// identical function names (some plain C, some C++ mangled — our Mach-O symbol
// resolver handles both via demangling and _-prefix stripping).
//
// CRITICAL: draw_indexed_primitives param_3 is a zgIndexBuffer* (64-bit pointer).
// On 32-bit Linux it was declared u32 (same size as pointer), but on 64-bit macOS
// declaring it as u32 would truncate the pointer, causing a crash in DrawSetup
// when the game tries to dereference the truncated index buffer pointer.
//
// CRITICAL: copy_image param_7 is LECRECT* (64-bit pointer on macOS).
// C++ mangled: zg_RendererSoftware_BufferCopyImpl(..., LECRECT*, int)
// Declaring as u32 truncates the pointer on 64-bit, causing SIGSEGV.
#[cfg(target_os = "macos")]
direct_fns! {
    #![bind_with(find_fns)]

    #[symbol("zg_Render_Initialize")]
    extern "C" fn init_renderers();

    #[symbol("stdFileOpen")]
    extern "C" fn open_file(filename: *mut c_char, mode: *mut c_char) -> *mut c_void;
    #[symbol("stdFileClose")]
    extern "C" fn close_file(file: *mut c_void) -> c_int;
    #[symbol("stdFileRead")]
    extern "C" fn read_file(file: *mut c_void, dst: *mut c_void, size: usize) -> usize;

    #[symbol("stdBitmap_Load")]
    extern "C" fn open_bm_image(
        filename: *const c_char,
        param_2: u32,
        param_3: u32,
    ) -> *mut ImageContainer;

    #[symbol("zg_Render_BufferCopyImpl")]
    extern "C" fn copy_image(
        dst_image: *mut Image,
        dst_surface: *mut Surface,
        src_image: *mut Image,
        src_surface: *mut Surface,
        x: u32,
        y: u32,
        param_7: usize,
        param_8: u32,
    );

    #[symbol("sputRender_Decompress")]
    extern "C" fn decompress_image(image: *const Image);

    #[symbol("sputResource_BackgroundHandler")]
    extern "C" fn manage_resource(resource: *mut Resource) -> c_int;

    #[symbol("zg_RendererDeferred_GetCachedTextureFromColorVBuffer")]
    extern "C" fn bind_image_surface(
        image: *mut Image,
        param_2: u32,
        param_3: u32,
        param_4: u32
    ) -> *mut Surface;

    #[symbol("zg_Surface_Upload")]
    extern "C" fn surface_upload(surface: *mut Surface, image_data: *mut c_void);

    #[symbol("zg_RenderContext_DrawSetup")]
    extern "C" fn setup_draw(draw: *mut Draw, index_buffer: *const c_void);

    #[symbol("zg_Shader_Apply")]
    extern "C" fn set_draw_shader(draw: *mut Draw, shader: *mut Shader);

    #[symbol("zg_Surface_Present")]
    extern "C" fn render_scene(
        draw: *const Draw,
        surface: *const Surface,
        transition: f32
    );

    // param_2 (rsi) and param_3 (rdx) must be pointer-sized on 64-bit.
    // param_3 is actually a zgIndexBuffer* passed through to DrawSetup.
    #[symbol("zg_RenderContext_DrawIndexedPrimitives")]
    extern "C" fn draw_indexed_primitives(
        draw: *mut Draw,
        param_2: usize,
        param_3: usize,
        param_4: u32,
        param_5: u32
    );

    #[symbol("zg_RenderContext_PushMarker")]
    extern "C" fn marker(len: usize, message: *const c_char);

    #[symbol("allocateDefaultBuffers")]
    extern "C" fn init_base_buffer() -> c_uint;

    #[symbol("zg_RendererSoftware_SetBuffers")]
    extern "C" fn init_software_buffers();

    #[symbol("sputSmush_Initialize")]
    extern "C" fn init_smush_buffer();

    #[symbol("sputRender_Close")]
    extern "C" fn reset_intermediate_buffers();

    #[symbol("SmushPlay_UpdateMovie")]
    extern "C" fn decode_smush_frame();

    #[symbol("loadRenderResources")]
    extern "C" fn init_shaders_and_render_passes();

    #[symbol("sputRender_ControlHandler")]
    extern "C" fn toggle_renderers();
}

// ---- Static game values ----

#[cfg(target_os = "windows")]
pub static mut BACK_BUFFER: Value<Image, InitSoftwareBuffers> =
    Value::new("BACK_BUFFER", &init_software_buffers, 0xD1);
#[cfg(target_os = "windows")]
pub static mut SMUSH_BUFFER: Value<*const Image, InitSmushBuffer> =
    Value::new("SMUSH_BUFFER", &init_smush_buffer, 0x14);
#[cfg(target_os = "windows")]
pub static mut DECOMPRESSION_BUFFER: Value<*const Image, ResetIntermediateBuffers> =
    Value::new("DECOMPRESSION_BUFFER", &reset_intermediate_buffers, 0x4C);
#[cfg(target_os = "windows")]
pub static mut CLEAN_BUFFER: Value<*const Image, ResetIntermediateBuffers> =
    Value::new("CLEAN_BUFFER", &reset_intermediate_buffers, 0x1C);
#[cfg(target_os = "windows")]
pub static mut CLEAN_Z_BUFFER: Value<*const Image, ResetIntermediateBuffers> =
    Value::new("CLEAN_Z_BUFFER", &reset_intermediate_buffers, 0x34);
#[cfg(target_os = "windows")]
pub static mut ACTIVE_SMUSH_FRAME: Value<*const SmushFrame, DecodeSmushFrame> =
    Value::new("ACTIVE_SMUSH_FRAME", &decode_smush_frame, 0x7C);
#[cfg(target_os = "windows")]
pub static mut BITMAP_UNDERLAYS_RENDER_PASS: Value<*const RenderPass, InitShadersAndRenderPasses> =
    Value::new(
        "BITMAP_UNDERLAYS_RENDER_PASS",
        &init_shaders_and_render_passes,
        0x30C,
    );
#[cfg(target_os = "windows")]
pub static mut TEXTURED_QUAD_SHADER: Value<*const Shader, InitShadersAndRenderPasses> = Value::new(
    "TEXTURED_QUAD_SHADER",
    &init_shaders_and_render_passes,
    0x1F,
);
#[cfg(target_os = "windows")]
pub static mut GAME_WINDOW: Value<*const c_void, InitBaseBuffer> =
    Value::new("GAME_WINDOW", &init_base_buffer, 0x5);
#[cfg(target_os = "windows")]
pub static mut RENDERING_MODE: Value<f32, ToggleRenderers> =
    Value::new("RENDERING_MODE", &toggle_renderers, 0x5C);

#[cfg(target_os = "linux")]
pub static BACK_BUFFER: Value<*const Image, ()> =
    Value::from_symbol("BACK_BUFFER", "sputRender_pDrawBuffer");
#[cfg(target_os = "linux")]
pub static SMUSH_BUFFER: Value<*const Image, ()> = Value::from_symbol("SMUSH_BUFFER", "SmushBuf");
#[cfg(target_os = "linux")]
pub static DECOMPRESSION_BUFFER: Value<*const Image, ()> =
    Value::from_symbol("DECOMPRESSION_BUFFER", "sputRender_pDecompressionBuffer");
#[cfg(target_os = "linux")]
pub static CLEAN_BUFFER: Value<*const Image, ()> =
    Value::from_symbol("CLEAN_BUFFER", "sputRender_pCleanBuffer");
#[cfg(target_os = "linux")]
pub static CLEAN_Z_BUFFER: Value<*const Image, ()> =
    Value::from_symbol("CLEAN_Z_BUFFER", "sputRender_pCleanZBuffer");
#[cfg(target_os = "linux")]
pub static ACTIVE_SMUSH_FRAME: Value<*const SmushFrame, ()> =
    Value::from_symbol("ACTIVE_SMUSH_FRAME", "smush_pInternalBitmap");
#[cfg(target_os = "linux")]
pub static BITMAP_UNDERLAYS_RENDER_PASS: Value<*const RenderPass, ()> =
    Value::from_symbol("BITMAP_UNDERLAYS_RENDER_PASS", "passBitmapUnderlays");
#[cfg(target_os = "linux")]
pub static TEXTURED_QUAD_SHADER: Value<*const Shader, ()> =
    Value::from_symbol("TEXTURED_QUAD_SHADER", "pTexturedQuadShader");
#[cfg(target_os = "linux")]
pub static GAME_WINDOW: Value<*const c_void, ()> = Value::from_symbol("GAME_WINDOW", "pWindow");
#[cfg(target_os = "linux")]
pub static RENDERING_MODE: Value<f32, ()> =
    Value::from_symbol("RENDERING_MODE", "zg_Render_useSoftwareRenderer");

// macOS: same static game values as Linux — resolved by symbol name.
// The macOS binary exports the same global variable symbols.
#[cfg(target_os = "macos")]
pub static BACK_BUFFER: Value<*const Image, ()> =
    Value::from_symbol("BACK_BUFFER", "sputRender_pDrawBuffer");
#[cfg(target_os = "macos")]
pub static SMUSH_BUFFER: Value<*const Image, ()> = Value::from_symbol("SMUSH_BUFFER", "SmushBuf");
#[cfg(target_os = "macos")]
pub static DECOMPRESSION_BUFFER: Value<*const Image, ()> =
    Value::from_symbol("DECOMPRESSION_BUFFER", "sputRender_pDecompressionBuffer");
#[cfg(target_os = "macos")]
pub static CLEAN_BUFFER: Value<*const Image, ()> =
    Value::from_symbol("CLEAN_BUFFER", "sputRender_pCleanBuffer");
#[cfg(target_os = "macos")]
pub static CLEAN_Z_BUFFER: Value<*const Image, ()> =
    Value::from_symbol("CLEAN_Z_BUFFER", "sputRender_pCleanZBuffer");
#[cfg(target_os = "macos")]
pub static ACTIVE_SMUSH_FRAME: Value<*const SmushFrame, ()> =
    Value::from_symbol("ACTIVE_SMUSH_FRAME", "smush_pInternalBitmap");
#[cfg(target_os = "macos")]
pub static BITMAP_UNDERLAYS_RENDER_PASS: Value<*const RenderPass, ()> =
    Value::from_symbol("BITMAP_UNDERLAYS_RENDER_PASS", "passBitmapUnderlays");
#[cfg(target_os = "macos")]
pub static TEXTURED_QUAD_SHADER: Value<*const Shader, ()> =
    Value::from_symbol("TEXTURED_QUAD_SHADER", "pTexturedQuadShader");
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub static GAME_WINDOW: Value<*const c_void, ()> = Value::from_symbol("GAME_WINDOW", "pWindow");
#[cfg(target_os = "macos")]
pub static RENDERING_MODE: Value<f32, ()> =
    Value::from_symbol("RENDERING_MODE", "zg_Render_useSoftwareRenderer");

// ---- Struct definitions ----
//
// These are #[repr(C)] and must match the game binary's ABI.
// On 32-bit (Windows/Linux), pointer fields are 4 bytes.
// On 64-bit (macOS), pointer fields are 8 bytes and alignment/padding differs.
// Structs with pointer fields or platform-dependent padding have separate
// definitions for 32-bit and 64-bit targets.

/// LLVM's libc++ std::vector — works on both 32-bit and 64-bit
#[repr(C)]
pub struct Vector<T> {
    pub start: *mut T,
    pub end: *mut T,
    pub capacity_end: *mut T,
    phantom: std::marker::PhantomData<T>,
}

impl<T: Sized> Vector<T> {
    pub unsafe fn data(&self) -> &[T] {
        if self.start.is_null() {
            &[]
        } else {
            std::slice::from_raw_parts(self.start, self.len())
        }
    }

    pub fn len(&self) -> usize {
        if self.start.is_null() {
            0
        } else {
            let span = self.end as usize - self.start as usize;
            // end points past the last element (libc++ vector convention)
            span / std::mem::size_of::<T>()
        }
    }
}

/// A named render pass with all its associated entities — works on both 32/64-bit.
/// All fields are pointers or Vector (which is 3 pointers), so #[repr(C)] handles it.
#[repr(C)]
pub struct RenderPass {
    pub name: *const c_char,
    pub entities: Vector<RenderPassEntity>,
    pub field_3: *const c_void,
}

// ---- RenderPassEntity (zgHardwareDrawCall) ----
//
// 32-bit (Windows/Linux): 252 bytes (0xFC)
// 64-bit (macOS): ~332 bytes — pointers widen from 4→8 bytes, alignment padding changes.
//
// grimmod accesses: entity.surface

#[cfg(not(target_os = "macos"))]
#[repr(C)]
pub struct RenderPassEntity {
    pub draw_type: u32,                    // +0x00
    pub shader_pipeline: *const c_void,    // +0x04
    pub constant_buf_1: [u32; 3],          // +0x08
    pub constant_buf_2: [u32; 3],          // +0x14
    pub depth_func: u32,                   // +0x20
    pub depth_enable: u32,                 // +0x24
    pub blend_src: u32,                    // +0x28
    pub blend_dst: u32,                    // +0x2C
    pub texture_count: u32,                // +0x30
    pub surface: *const Surface,           // +0x34 (textures[0])
    pub textures_1_7: [*const Surface; 7], // +0x38
    pub sampler_count: u32,                // +0x54
    pub samplers: [u32; 8],                // +0x58
    pub _unknown_78: [u32; 24],            // +0x78 (96 bytes)
    pub draw_param_1: u32,                 // +0xD8
    pub draw_param_2: u32,                 // +0xDC
    pub vertex_buffer: *const c_void,      // +0xE0
    pub indexed_flag: u32,                 // +0xE4
    pub scissor_enabled: u32,              // +0xE8
    pub scissor_rect: [u32; 4],            // +0xEC
}

/// macOS 64-bit layout of zgHardwareDrawCall, reversed from zg_RendererHardware_Draw_Issue.
///
/// Key offsets from disassembly:
///   +0x00: draw_type       +0x08: shader_pipeline (ptr)
///   +0x10: cbuf1[0]        +0x18: cbuf1[1]        +0x20: cbuf1[2]
///   +0x28: cbuf2[0]        +0x30: cbuf2[1]        +0x38: cbuf2[2]
///   +0x40: depth_func      +0x44: depth_enable
///   +0x48: blend_src       +0x4C: blend_dst
///   +0x50: texture_count
///   +0x58: textures[0..8] (8 × 8-byte pointers) → surface is textures[0]
///   +0x98: sampler_count
///   +0x9C: samplers[8] (each 4 bytes)
///   +0xBC..0x11F: unknown (100 bytes, 25 u32s)
///   +0x120: draw_param_1   +0x124: draw_param_2
///   +0x128: vertex_buffer (ptr)
///   +0x130: indexed (ptr-sized)
///   +0x138: scissor_enabled
///   +0x13C: scissor_rect[4]
#[cfg(target_os = "macos")]
#[repr(C)]
pub struct RenderPassEntity {
    pub draw_type: u32,                    // +0x00
    _pad_04: u32,                          // +0x04 (alignment padding)
    pub shader_pipeline: *const c_void,    // +0x08
    pub constant_buf_1: [usize; 3],        // +0x10 (3 × 8-byte values)
    pub constant_buf_2: [usize; 3],        // +0x28
    pub depth_func: u32,                   // +0x40
    pub depth_enable: u32,                 // +0x44
    pub blend_src: u32,                    // +0x48
    pub blend_dst: u32,                    // +0x4C
    pub texture_count: u32,                // +0x50
    _pad_54: u32,                          // +0x54 (alignment before pointer array)
    pub surface: *const Surface,           // +0x58 (textures[0])
    pub textures_1_7: [*const Surface; 7], // +0x60
    pub sampler_count: u32,                // +0x98
    pub samplers: [u32; 8],                // +0x9C
    pub _unknown_bc: [u32; 25],            // +0xBC (100 bytes)
    pub draw_param_1: u32,                 // +0x120
    pub draw_param_2: u32,                 // +0x124
    pub vertex_buffer: *const c_void,      // +0x128
    pub indexed_flag: usize,               // +0x130 (ptr-sized)
    pub scissor_enabled: u32,              // +0x138
    pub scissor_rect: [u32; 4],            // +0x13C
}

// ---- Draw (zgRenderContext) ----
//
// 32-bit: samplers at +0x28, shader at +0x48, surfaces at +0x68
// 64-bit: shader at +0x68, surfaces at +0xa8, samplers at +0xe8
//
// grimmod accesses: draw.surfaces[0], draw.samplers[0], draw.shader (via set_draw_shader)

#[cfg(not(target_os = "macos"))]
#[repr(C)]
pub struct Draw {
    pub field_1: u32,
    pub field_2: u32,
    pub render_target: *const c_void,
    pub depth_drawbuffer: c_int,

    pub _fields_10_27: [u32; 6], // +0x10 to +0x27

    pub samplers: [c_uint; 8], // +0x28
    pub shader: *const Shader, // +0x48

    pub _fields_4c_67: [u32; 7], // +0x4C to +0x67

    pub surfaces: [*const Surface; 8], // +0x68
}

/// macOS 64-bit layout of zgRenderContext, reversed from bindTextures/SetTextures/SetSamplers.
///
/// Key offsets from disassembly:
///   +0x18: vertex_buffer (ptr, from SetVertexBuffer)
///   +0x44: sampler_ids[0..8] (8 × u32, from bindTextures: `movl 0x44(%r12,%rbx,4), %r13d`)
///   +0x68: shader (*const Shader, from zg_Shader_Apply)
///   +0xa0: texture_count (u32)
///   +0xa8: surfaces[0..8] (8 × 8-byte pointers)
///   +0xe8: sampler_state[0..8] (each 16 bytes, from applySamplerState)
///   +0x19c-0x1a2: dirty flags
#[cfg(target_os = "macos")]
#[repr(C)]
pub struct Draw {
    pub _fields_00_43: [u8; 0x44], // +0x00 to +0x43

    pub samplers: [c_uint; 8], // +0x44 (8 × 4 = 32 bytes, sampler GL IDs)
    // +0x64 end of samplers
    pub _pad_64_67: [u8; 4], // +0x64 to +0x67 (padding)

    pub shader: *const Shader, // +0x68

    pub _fields_70_9f: [u8; 0x30], // +0x70 to +0x9f (48 bytes)

    pub _texture_count: u32, // +0xa0
    _pad_a4: u32,            // +0xa4 (alignment)
    pub surfaces: [*const Surface; 8], // +0xa8 (8 × 8 = 64 bytes, ends at +0xe8)

                             // +0xe8: sampler state data (16 bytes per entry, 8 entries = 128 bytes)
                             // grimmod does not need to access this
}

/// A compiled shader program — mostly fixed-size fields, same on 32/64-bit
/// except for the pointer field. grimmod accesses shader indirectly through
/// set_draw_shader and TEXTURED_QUAD_SHADER.
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

/// Common image attributes extracted to struct.
/// Contains `usize` fields (size, bytes_per_row, calculated_width) which are
/// pointer-width dependent but identical in meaning on both platforms.
#[repr(C)]
pub struct ImageAttributes {
    pub width: i32,
    pub height: i32,
    pub size: u32,
    bytes_per_row: u32,
    calculated_width: u32,
    _ignore: u32,
    bits_per_pixel: u32,
    rgb_bits: [u32; 3],
    rgb_shift: [u32; 3],
    rgb_loss: [u32; 3],
    _ignore_post: [u32; 3],
}

/// A single image or animation frame.
/// All pointer fields are properly typed, so #[repr(C)] handles 32/64-bit.
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

/// A static or animated image container.
/// The `images` field is a pointer, properly typed.
/// On macOS 64-bit, `param_12` is a pointer (palette data), which shifts
/// subsequent field offsets: image_count at +0x68, images at +0x80 (vs +0x64/+0x78 on 32-bit).
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
    #[cfg(target_os = "macos")]
    pub param_12: *const c_void,
    #[cfg(not(target_os = "macos"))]
    pub param_12: u32,
    pub image_count: u32,
    pub param_14: u32,
    pub x: u32,
    pub y: u32,
    pub transparent_color: u32,
    pub images: *const *const Image,
}

/// A texture in the OpenGL renderer.
/// The `image_data` field is a pointer.
/// On macOS 64-bit, `render_target` is a pointer (8 bytes), pushing `texture_id` to +0x28.
#[repr(C)]
pub struct Surface {
    pub width: i32,
    pub height: i32,
    pub param_3: u32,
    pub format: u32,
    pub uploaded: u32,
    pub param_6: u32,
    pub image_data: *mut c_void,
    #[cfg(target_os = "macos")]
    pub render_target: *mut c_void,
    #[cfg(not(target_os = "macos"))]
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
