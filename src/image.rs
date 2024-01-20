use lazy_static::lazy_static;
use std::ffi::c_void;
use std::ffi::{c_char, c_int, c_uint, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::debug;
use crate::file;
use crate::gl;
use crate::grim;

pub static DECOMPRESSED: Mutex<usize> = Mutex::new(0);
pub static BACKGROUND: Mutex<Option<HqImage>> = Mutex::new(None);

lazy_static! {
    pub static ref HQ_IMAGES: Mutex<Vec<HqImageContainer>> = Mutex::new(Vec::new());
    pub static ref BACKGROUND_SHADER: usize = compile_background_shader() as usize;
}

#[derive(Clone)]
pub struct HqImageContainer {
    pub name: String,
    pub original_addr: usize,
    pub images: Vec<HqImage>,
}

#[derive(Clone)]
pub struct HqImage {
    pub width: u32,
    pub height: u32,
    pub path: String,
    pub original_addr: usize,
    pub buffer: Vec<u8>,
}

impl HqImageContainer {
    unsafe fn from_raw(
        raw_image_container: *const grim::ImageContainer,
    ) -> Option<HqImageContainer> {
        let image_container = raw_image_container.as_ref()?;
        let name = image_container.name.as_ptr();
        let image_refs = std::slice::from_raw_parts(image_container.images, 1);
        let filename = CStr::from_ptr(name).to_str().ok()?;
        HqImageContainer::open(filename, image_container, image_refs[0].as_ref()?)
    }

    fn open(
        bm_filename: &str,
        image_container: &grim::ImageContainer,
        image: &grim::Image,
    ) -> Option<HqImageContainer> {
        let name = Path::new(bm_filename).file_stem()?.to_str()?;
        let image_path = HqImage::find_modded_path(name)?;
        let hq_image = HqImage::open_all(name, image_path, image)?;

        Some(HqImageContainer {
            name: name.to_string(),
            original_addr: image_container as *const _ as usize,
            images: vec![hq_image],
        })
    }
}

impl HqImage {
    fn open_all(name: &str, path: PathBuf, image: &grim::Image) -> Option<HqImage> {
        let png = match image::open(&path) {
            Ok(pngs) => Some(pngs),
            Err(error) => {
                debug::error(format!("Could not open {} image: {}", name, error));
                None
            }
        }?;

        Some(HqImage {
            width: png.width(),
            height: png.height(),
            path: path.display().to_string(),
            original_addr: image as *const _ as usize,
            buffer: png.to_rgb8().into_vec(),
        })
    }

    fn find_modded_path(name: &str) -> Option<PathBuf> {
        file::find_modded(&format!("{}.png", name))
    }

    fn find_loaded<'a>(
        original_addr: usize,
        hq_images: &'a MutexGuard<'a, Vec<HqImageContainer>>,
    ) -> Option<&'a HqImage> {
        hq_images.iter().find_map(|hq_image_container| {
            hq_image_container
                .images
                .iter()
                .find(|hq_image| hq_image.original_addr == original_addr)
        })
    }
}

/// Return the address for the background render pass's surface
fn bitmap_underlays_surface() -> usize {
    unsafe {
        let render_pass = grim::BITMAP_UNDERLAYS_RENDER_PASS.inner_ref();
        let render_pass_data = render_pass.and_then(|render_pass| render_pass.data.as_ref());
        let surface = render_pass_data.map(|render_pass_data| render_pass_data.surface as usize);
        surface.unwrap_or(0)
    }
}

/// Loads the contents of a BM file into an image container
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn open_bm_image(
    raw_filename: *const c_char,
    param_2: u32,
    param_3: u32,
) -> *mut grim::ImageContainer {
    unsafe {
        let image_container = grim::open_bm_image(raw_filename, param_2, param_3);

        if let Some(hq_image_container) = HqImageContainer::from_raw(image_container) {
            HQ_IMAGES.lock().unwrap().push(hq_image_container);
        }

        image_container
    }
}

/// Decompresses an image into the global decompression buffer
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn decompress_image(image: *const grim::Image) {
    // store the address of the last image decompressed
    // it will shortly be copied to the clean buffer and rendered
    *DECOMPRESSED.lock().unwrap() = image as usize;

    unsafe { grim::decompress_image(image) }
}

/// Copy an image and surface from a source to a pre-allocated destination
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn copy_image(
    dst_image: *mut grim::Image,
    dst_surface: *mut c_void,
    src_image: *mut grim::Image,
    src_surface: *mut c_void,
    param_5: u32,
    param_6: u32,
    param_7: u32,
    param_8: u32,
) {
    // detect an image with an associated hq image being copied to the clean buffer
    // either directly or from a recent decompression
    // an image being copied to the clean buffer means it is about to get rendered
    if dst_image as usize == unsafe { grim::CLEAN_BUFFER.inner_addr() } {
        let mut decompressed = DECOMPRESSED.lock().unwrap();
        let image_addr = if *decompressed != 0 {
            *decompressed
        } else {
            src_image as usize
        };

        let hq_images = HQ_IMAGES.lock().unwrap();
        let hq_image = HqImage::find_loaded(image_addr, &hq_images);
        *BACKGROUND.lock().unwrap() = hq_image.cloned();

        *decompressed = 0;
    }

    unsafe {
        grim::copy_image(
            dst_image,
            dst_surface,
            src_image,
            src_surface,
            param_5,
            param_6,
            param_7,
            param_8,
        )
    }
}

/// Prepare a surface (aka texture) for uploading to the GPU or upload it now
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn surface_upload(surface: *mut grim::Surface, image_data: *mut c_void) {
    unsafe {
        // call with null to reset the buffer size as it might have been changed by a hq image
        grim::surface_upload(surface, std::ptr::null_mut());
        grim::surface_upload(surface, image_data);
    }

    // check if the "bitmap underlays" surface's data is being uploaded
    // this signals that the active background is being set
    if image_data.is_null() || surface as usize != bitmap_underlays_surface() {
        return;
    }

    // check if any hq image is "active", if it is attach it as the background
    if let Some(hq_image) = BACKGROUND.lock().unwrap().as_ref() {
        debug::info(format!(
            "Surface Address: 0x{:x}, Image Data: 0x{:x}",
            surface as usize, image_data as usize
        ));

        let texture_id = unsafe { surface.as_ref() }
            .map(|surface| surface.texture_id)
            .unwrap_or(0);
        let hq_image_data: *const [u8] = hq_image.buffer.as_ref();

        unsafe {
            gl::pixel_storei(gl::UNPACK_ALIGNMENT, 1);
            gl::bind_texture(gl::TEXTURE_2D, texture_id);
            gl::pixel_storei(gl::UNPACK_ROW_LENGTH, hq_image.width as gl::Int);
            gl::tex_image_2d(
                gl::TEXTURE_2D,
                0,
                gl::RGB as gl::Int,
                hq_image.width as gl::Int,
                hq_image.height as gl::Int,
                0,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                hq_image_data as *const c_void,
            );
        }
    }
}

pub extern "C" fn manage_resource(resource: *mut grim::Resource) -> c_int {
    let state = unsafe { (*resource).state };
    let asset = unsafe { (*resource).asset as usize };

    if state == 2 {
        HQ_IMAGES
            .lock()
            .unwrap()
            .retain(|hq_image| hq_image.original_addr != asset);
    }

    unsafe { grim::manage_resource(resource) }
}

fn drawing_hq_background(draw: *const grim::Draw) -> bool {
    let Some(surface) = unsafe { draw.as_ref() }.map(|draw| draw.surfaces[0] as usize) else {
        return false;
    };
    surface == bitmap_underlays_surface() && BACKGROUND.lock().unwrap().is_some()
}

/// Sets the OpenGL state for the next draw call
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn setup_draw(draw: *mut grim::Draw, index_buffer: *const c_void) {
    let hq = drawing_hq_background(draw);

    // for hq backgrounds, use a custom shader that keeps the full resolution
    if hq && let Some(draw) = unsafe { draw.as_mut() } {
        draw.shader = *BACKGROUND_SHADER as *const grim::Shader;
    }

    unsafe {
        grim::setup_draw(draw, index_buffer);

        // for hq backgrounds, use a linear texture filter for better quality
        let sampler = draw.as_ref().map(|draw| draw.samplers[0] as c_uint);
        if hq && let Some(sampler) = sampler {
            gl::sampler_parameteri(sampler, gl::TEXTURE_MIN_FILTER, gl::LINEAR as gl::Int);
            gl::sampler_parameteri(sampler, gl::TEXTURE_MAG_FILTER, gl::LINEAR as gl::Int);
        }
    }
}

pub fn compile_background_shader() -> *const grim::Shader {
    let name = CString::new("grimmod_background").unwrap();
    unsafe { grim::compile_shader(name.as_ptr()) }
}

pub static BACKGROUND_V_SHADER: &str = r#"
    #version 330
    layout(std140) uniform VSConstants {
         vec4    MultiplyColor;     vec4    TexelSizeDesaturateGamma;     vec4    ColorKey;
    } constantsV;
    in vec3 vs_Position;
    in vec2 vs_TexCoord;
    out vec2 ps_TexCoordPx;
    void main ()
    {
      gl_Position = vec4(vs_Position, 1.0);
      ps_TexCoordPx = vs_TexCoord;
    }
"#;

pub static BACKGROUND_P_SHADER: &str = r#"
    #version 330
    layout(std140) uniform PSConstants {
         vec4    MultiplyColor;     vec4    TexelSizeDesaturateGamma;     vec4    ColorKey;
    } constantsP;
    uniform sampler2D ps_Texture0;
    in vec2 ps_TexCoordPx;
    out vec4 ps_Result;
    void main ()
    {
      ps_Result = texture(ps_Texture0, ps_TexCoordPx);
    }
"#;
