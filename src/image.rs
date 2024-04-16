use lazy_static::lazy_static;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::{c_char, c_int, c_uint, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::animation;
use crate::debug;
use crate::file;
use crate::gl;
use crate::grim;

#[derive(Clone, Copy, Default, Hash, Eq, PartialEq)]
pub struct ImageContainerAddr(usize);

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct ImageAddr(usize);

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct SurfaceAddr(usize);

pub static DECOMPRESSED: Mutex<Option<ImageAddr>> = Mutex::new(None);
pub static OVERLAY: Mutex<Option<ImageAddr>> = Mutex::new(None);
pub static BACKGROUND: Mutex<Option<HqImage>> = Mutex::new(None);
pub static TARGET: Mutex<Option<Target>> = Mutex::new(None);
pub static ORIGINAL_BG: Mutex<Option<ImageAddr>> = Mutex::new(None);

lazy_static! {
    pub static ref HQ_IMAGES: Mutex<Vec<HqImageContainer>> = Mutex::new(Vec::new());
    pub static ref BACKGROUND_SHADER: usize = compile_background_shader() as usize;
    pub static ref OVERLAYS: Mutex<HashMap<SurfaceAddr, ImageAddr>> = Mutex::new(HashMap::new());
}

#[derive(Debug)]
pub enum Target {
    Background,
    Image(ImageAddr),
}

#[derive(Clone)]
pub struct HqImageContainer {
    pub name: String,
    pub original_addr: ImageContainerAddr,
    pub images: Vec<HqImage>,
}

#[derive(Clone)]
pub struct HqImage {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub path: String,
    pub original_addr: ImageAddr,
    pub has_alpha: bool,
    pub buffer: Vec<u8>,
}

impl HqImageContainer {
    unsafe fn from_raw(
        raw_image_container: *const grim::ImageContainer,
    ) -> Option<HqImageContainer> {
        let image_container = raw_image_container.as_ref()?;
        let name = image_container.name.as_ptr();
        let n_images = image_container.image_count as usize;
        let image_refs = std::slice::from_raw_parts(image_container.images, n_images);
        let images: Vec<_> = image_refs
            .iter()
            .filter_map(|image| image.as_ref())
            .collect();
        let filename = CStr::from_ptr(name).to_str().ok()?;
        if filename.to_lowercase().ends_with(".bm") {
            HqImageContainer::open(filename, image_container, images)
        } else {
            None
        }
    }

    fn open(
        bm_filename: &str,
        image_container: &grim::ImageContainer,
        images: Vec<&grim::Image>,
    ) -> Option<HqImageContainer> {
        let name = Path::new(bm_filename).file_stem()?.to_str()?;
        let hq_images = if let Some(hq_images) = HqImage::open_animation(name, images.clone()) {
            hq_images
        } else {
            let image_paths = HqImage::find_modded_paths(name)?;
            HqImage::open_all(name, image_paths, images)?
        };

        Some(HqImageContainer {
            name: name.to_string(),
            original_addr: ImageContainerAddr(image_container as *const _ as usize),
            images: hq_images,
        })
    }

    fn deallocate(index: usize, hq_image_containers: &mut MutexGuard<'_, Vec<HqImageContainer>>) {
        let hq_image_container = hq_image_containers.remove(index);
        let mut overlays = OVERLAYS.lock().unwrap();
        for hq_image in hq_image_container.images.iter() {
            overlays.retain(|_, image_addr| image_addr != &hq_image.original_addr);
        }
    }
}

impl HqImage {
    fn open_all(
        name: &str,
        paths: Vec<PathBuf>,
        images: Vec<&grim::Image>,
    ) -> Option<Vec<HqImage>> {
        if paths.len() != images.len() {
            debug::error(format!(
                "Could not open {} image: Expected {} frames, found {}",
                name,
                images.len(),
                paths.len()
            ));
            return None;
        }

        let try_pngs: Result<Vec<_>, _> = paths.iter().map(image::open).collect();
        let pngs = match try_pngs {
            Ok(pngs) => Some(pngs),
            Err(error) => {
                debug::error(format!("Could not open {} image: {}", name, error));
                None
            }
        }?;

        Some(
            pngs.into_iter()
                .zip(images)
                .zip(paths)
                .enumerate()
                .map(|(i, ((png, image), path))| HqImage {
                    name: format!("{} ({:02})", name, i),
                    width: png.width(),
                    height: png.height(),
                    scale: png.width() / image.attributes.width as u32,
                    path: path.display().to_string(),
                    original_addr: ImageAddr(image as *const _ as usize),
                    has_alpha: png.color().has_alpha(),
                    buffer: png.to_rgba8().into_vec(),
                })
                .collect(),
        )
    }

    fn open_animation(name: &str, images: Vec<&grim::Image>) -> Option<Vec<HqImage>> {
        let path = file::find_modded(&format!("{}.mkv", name))?;
        let frames = animation::open(path)?;

        Some(
            frames
                .into_iter()
                .zip(images)
                .enumerate()
                .map(|(i, (frame, image))| HqImage {
                    name: format!("{} ({:02})", name, i),
                    width: frame.width,
                    height: frame.height,
                    scale: frame.width / image.attributes.width as u32,
                    path: "".to_string(),
                    original_addr: ImageAddr(image as *const _ as usize),
                    has_alpha: frame.has_alpha,
                    buffer: frame.buffer,
                })
                .collect(),
        )
    }

    fn find_modded_paths(name: &str) -> Option<Vec<PathBuf>> {
        let is_animated = file::find_modded(&format!("{}/01.png", name)).is_some();
        let mut image_paths = if is_animated {
            Some(file::find_all_modded(&format!("{}/*.png", name)).collect())
        } else {
            file::find_modded(&format!("{}.png", name)).map(|image| vec![image])
        }?;
        image_paths.sort();
        Some(image_paths)
    }

    fn find_loaded<'a>(
        original_addr: ImageAddr,
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
fn bitmap_underlays_surface() -> Option<SurfaceAddr> {
    unsafe {
        let render_pass = grim::BITMAP_UNDERLAYS_RENDER_PASS.inner_ref();
        let render_pass_data = render_pass.and_then(|render_pass| render_pass.data.as_ref());
        let surface = render_pass_data.map(|render_pass_data| render_pass_data.surface as usize);
        surface.map(SurfaceAddr)
    }
}

fn extract<T>(mutex: &Mutex<Option<T>>) -> Option<T> {
    mutex.lock().ok()?.take()
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

        // instead of allocating a new image, the game sometimes reuses an existing one
        // detect this re-use and treat it as a deallocation for old hq images
        if let Some(image_container) = image_container.as_ref() {
            let image_addrs = std::slice::from_raw_parts(
                image_container.images as *const usize,
                image_container.image_count as usize,
            );
            let mut hq_image_containers = HQ_IMAGES.lock().unwrap();
            let index = hq_image_containers.iter().position(|image_container| {
                image_container
                    .images
                    .iter()
                    .any(|image| image_addrs.contains(&image.original_addr.0))
            });
            if let Some(index) = index {
                HqImageContainer::deallocate(index, &mut hq_image_containers);
            }
        }

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
    *DECOMPRESSED.lock().unwrap() = Some(ImageAddr(image as usize));

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
    x: u32,
    y: u32,
    param_7: u32,
    param_8: u32,
) {
    // detect an image with an associated hq image being copied to the clean buffer
    // an image being copied to the clean first buffer means it is a background (or about to draw
    // over the background) to be rendered
    if dst_image as usize == unsafe { grim::CLEAN_BUFFER.inner_addr() } {
        let hq_images = HQ_IMAGES.lock().unwrap();
        let image_addr = if src_image as usize == unsafe { grim::DECOMPRESSION_BUFFER.inner_addr() }
        {
            extract(&DECOMPRESSED)
        } else {
            Some(ImageAddr(src_image as usize))
        };
        let hq_image =
            image_addr.and_then(|image_addr| HqImage::find_loaded(image_addr, &hq_images));
        if x == 0 && y == 0 {
            *BACKGROUND.lock().unwrap() = hq_image.cloned();
            *ORIGINAL_BG.lock().unwrap() = hq_image.map(|hq_image| hq_image.original_addr);
        } else if let Some(frame) = hq_image {
            let mut background = BACKGROUND.lock().unwrap();
            if let Some(background) = background.as_mut()
                && background.scale == frame.scale
            {
                let bytes_per_pixel = 4;
                let x = (x * frame.scale) as usize;
                let y = (y * frame.scale) as usize;
                let width = frame.width as usize;
                let bytes_width = width * bytes_per_pixel;
                if !frame.has_alpha {
                    for i in 0..frame.height as usize {
                        let src_start = i * bytes_width;
                        let src_slice = &frame.buffer[src_start..src_start + bytes_width];
                        let dst_start = (background.width as usize * (y + i) + x) * bytes_per_pixel;
                        let dst_slice = &mut background.buffer[dst_start..dst_start + bytes_width];
                        dst_slice.copy_from_slice(src_slice);
                    }
                } else {
                    for i in 0..frame.height as usize {
                        for j in 0..width {
                            let dst_start =
                                (background.width as usize * (y + i) + (x + j)) * bytes_per_pixel;
                            let dst_pixel = (
                                background.buffer[dst_start],
                                background.buffer[dst_start + 1],
                                background.buffer[dst_start + 2],
                                background.buffer[dst_start + 3],
                            );
                            let src_start = (width * i + j) * bytes_per_pixel;
                            let src_pixel = (
                                frame.buffer[src_start],
                                frame.buffer[src_start + 1],
                                frame.buffer[src_start + 2],
                                frame.buffer[src_start + 3],
                            );
                            let (r, g, b) = blend_pixels(dst_pixel, src_pixel);
                            background.buffer[dst_start] = r;
                            background.buffer[dst_start + 1] = g;
                            background.buffer[dst_start + 2] = b;
                        }
                    }
                }
            }
        } else if let Some(original_background_addr) = extract(&ORIGINAL_BG) {
            let hq_image = HqImage::find_loaded(original_background_addr, &hq_images);
            *BACKGROUND.lock().unwrap() = hq_image.cloned();
        }
    }

    // if an image with an associated hq image is being written directly to the back buffer
    // then that indicates that it's an overlay
    // store this overlay temporarily to see what surface it is bound to
    if dst_image as usize == unsafe { grim::BACK_BUFFER.addr() } {
        let image_addr = extract(&DECOMPRESSED).unwrap_or(ImageAddr(src_image as usize));
        let hq_images = HQ_IMAGES.lock().unwrap();
        let hq_image = HqImage::find_loaded(image_addr, &hq_images);
        if hq_image.is_some() {
            *OVERLAY.lock().unwrap() = Some(image_addr);
        }
    }

    unsafe {
        grim::copy_image(
            dst_image,
            dst_surface,
            src_image,
            src_surface,
            x,
            y,
            param_7,
            param_8,
        )
    }
}

fn blend_pixels(background: (u8, u8, u8, u8), foreground: (u8, u8, u8, u8)) -> (u8, u8, u8) {
    let (br, bg, bb, _) = background;
    let (fr, fg, fb, fa) = foreground;

    let r = (fr as u16 * fa as u16 + br as u16 * (255 - fa) as u16) / 255;
    let g = (fg as u16 * fa as u16 + bg as u16 * (255 - fa) as u16) / 255;
    let b = (fb as u16 * fa as u16 + bb as u16 * (255 - fa) as u16) / 255;

    (r as u8, g as u8, b as u8)
}

/// Prepare a surface (aka texture) for uploading to the GPU or upload it now
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn surface_upload(surface: *mut grim::Surface, image_data: *mut c_void) {
    let surface_addr = SurfaceAddr(surface as usize);
    let is_paused = unsafe { grim::is_paused().as_bool() };
    let target = if !is_paused {
        get_target(surface_addr)
    } else {
        None
    };

    if target.is_none() {
        return unsafe {
            // call with null to reset the buffer size as it might have been changed by a hq image
            if !image_data.is_null() {
                grim::surface_upload(surface, std::ptr::null_mut());
            }
            grim::surface_upload(surface, image_data);
        };
    }

    if image_data.is_null() {
        return;
    }

    *TARGET.lock().unwrap() = target;
    unsafe {
        gl::tex_image_2d.hook(hq_tex_image_2d as gl::TexImage2D);
        gl::pixel_storei.hook(hq_pixel_storei as gl::PixelStorei);
        grim::surface_upload(surface, std::ptr::null_mut());
        gl::pixel_storei.unhook();
        gl::tex_image_2d.unhook();
    }
    *TARGET.lock().unwrap() = None;
}

fn get_target(surface_addr: SurfaceAddr) -> Option<Target> {
    if bitmap_underlays_surface() == Some(surface_addr) {
        BACKGROUND
            .lock()
            .unwrap()
            .is_some()
            .then_some(Target::Background)
    } else {
        OVERLAYS
            .lock()
            .unwrap()
            .get(&surface_addr)
            .cloned()
            .map(Target::Image)
    }
}

fn with_target_hq_image<F: Fn(&HqImage)>(f: F) {
    let background = BACKGROUND.lock().unwrap();
    let hq_images = HQ_IMAGES.lock().unwrap();
    let hq_image = match TARGET.lock().unwrap().as_ref() {
        Some(Target::Background) => background.as_ref(),
        Some(Target::Image(image_addr)) => HqImage::find_loaded(*image_addr, &hq_images),
        _ => None,
    };
    if let Some(hq_image) = hq_image {
        f(hq_image)
    }
}

extern "stdcall" fn hq_tex_image_2d(
    _target: gl::Enum,
    _level: gl::Int,
    _internalformat: gl::Int,
    _width: gl::Sizei,
    _height: gl::Sizei,
    _border: gl::Int,
    _format: gl::Enum,
    _typ: gl::Enum,
    _data: *const c_void,
) {
    with_target_hq_image(|hq_image| unsafe {
        gl::tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::RGBA8 as gl::Int,
            hq_image.width as gl::Int,
            hq_image.height as gl::Int,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            hq_image.buffer.as_ptr() as *const _,
        )
    })
}

extern "stdcall" fn hq_pixel_storei(pname: gl::Enum, param: gl::Int) {
    with_target_hq_image(|hq_image| unsafe {
        if pname == gl::UNPACK_ROW_LENGTH {
            gl::pixel_storei(gl::UNPACK_ROW_LENGTH, hq_image.width as gl::Int);
        } else {
            gl::pixel_storei(pname, param);
        }
    })
}

/// Allocate a new surface (texture) of a given width/height
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn surface_allocate(
    width: c_int,
    height: c_int,
    format: c_uint,
    param_4: c_int,
) -> *const grim::Surface {
    let surface = unsafe { grim::surface_allocate(width, height, format, param_4) };
    let surface_addr = SurfaceAddr(surface as usize);

    // if a hq overlay was just loaded, store it with its new associated surface
    if let Some(image_addr) = extract(&OVERLAY) {
        OVERLAYS.lock().unwrap().insert(surface_addr, image_addr);
    }

    surface
}

pub extern "C" fn surface_bind_existing(
    surface: *mut grim::Surface,
    image: *const grim::Image,
    width: i32,
    height: i32,
    param_4: u32,
    param_5: u32,
    param_6: u32,
    param_7: u32,
    texture_id: gl::Uint,
) {
    let surface_addr = SurfaceAddr(surface as usize);
    if let Some(overlay) = extract(&OVERLAY) {
        OVERLAYS.lock().unwrap().insert(surface_addr, overlay);
    }

    unsafe {
        grim::surface_bind_existing(
            surface, image, width, height, param_4, param_5, param_6, param_7, texture_id,
        );
    }
}

pub extern "C" fn manage_resource(resource: *mut grim::Resource) -> c_int {
    let state = unsafe { (*resource).state };
    let image_container_addr = ImageContainerAddr(unsafe { (*resource).image_container as usize });

    if state == 2 {
        let mut hq_image_containers = HQ_IMAGES.lock().unwrap();
        let index = hq_image_containers.iter().position(|hq_image_container| {
            hq_image_container.original_addr == image_container_addr
        });
        if let Some(index) = index {
            HqImageContainer::deallocate(index, &mut hq_image_containers);
        }
    }

    unsafe { grim::manage_resource(resource) }
}

fn is_drawing_hq(draw: *const grim::Draw) -> bool {
    let Some(surface) = drawing_surface(draw) else {
        return false;
    };

    (Some(surface) == bitmap_underlays_surface() && BACKGROUND.lock().unwrap().is_some())
        || OVERLAYS.lock().unwrap().contains_key(&surface)
}

fn drawing_surface(draw: *const grim::Draw) -> Option<SurfaceAddr> {
    unsafe { draw.as_ref() }.map(|draw| SurfaceAddr(draw.surfaces[0] as usize))
}

fn drawing_sampler(draw: *const grim::Draw) -> Option<gl::Uint> {
    unsafe { draw.as_ref() }.map(|draw| draw.samplers[0] as c_uint)
}

/// Sets the OpenGL state for the next draw call
///
/// This is an overload for a native function that will be hooked
pub extern "C" fn setup_draw(draw: *mut grim::Draw, index_buffer: *const c_void) {
    let hq = is_drawing_hq(draw);

    // for hq images, use a custom shader that keeps the full resolution
    if hq && let Some(draw) = unsafe { draw.as_mut() } {
        draw.shader = *BACKGROUND_SHADER as *const grim::Shader;
    }

    unsafe {
        grim::setup_draw(draw, index_buffer);
    }

    if hq && let (Some(surface), Some(sampler)) = (drawing_surface(draw), drawing_sampler(draw)) {
        unsafe {
            // for hq images, use a linear texture filter for better quality
            gl::sampler_parameteri(sampler, gl::TEXTURE_MIN_FILTER, gl::LINEAR as gl::Int);
            gl::sampler_parameteri(sampler, gl::TEXTURE_MAG_FILTER, gl::LINEAR as gl::Int);
            gl::blend_func_separate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, 1, 0);
        }

        // once an overlay has been drawn, remove it from the list of overlays
        // if an overlay lasts for many frames, it will get reloaded every frame with a new surface
        OVERLAYS.lock().unwrap().remove(&surface);
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
