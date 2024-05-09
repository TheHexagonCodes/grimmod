use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::sync::Mutex;

use crate::gl;
use crate::grim;
use crate::image;

pub static DECOMPRESSED: Mutex<Option<ImageAddr>> = Mutex::new(None);
pub static PREPARING_OVERLAY: Mutex<Option<ImageAddr>> = Mutex::new(None);
pub static OVERLAYS: Lazy<Mutex<HashMap<SurfaceAddr, ImageAddr>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Copy, Default, Hash, Eq, PartialEq)]
pub struct ImageContainerAddr(usize);

impl ImageContainerAddr {
    pub fn from_ptr(ptr: *const grim::ImageContainer) -> ImageContainerAddr {
        ImageContainerAddr(ptr as usize)
    }
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct ImageAddr(usize);

impl ImageAddr {
    pub fn from_ptr(ptr: *const grim::Image) -> ImageAddr {
        ImageAddr(ptr as usize)
    }

    /// Gets the original image address before decompression
    pub fn original(image: *const grim::Image) -> Option<ImageAddr> {
        let image_addr = ImageAddr::from_ptr(image);
        if image_addr.is_decompression_buffer() {
            extract(&DECOMPRESSED)
        } else {
            Some(image_addr)
        }
    }

    pub fn is_decompression_buffer(&self) -> bool {
        self.0 == unsafe { grim::DECOMPRESSION_BUFFER.inner_addr() }
    }

    pub fn is_clean_buffer(&self) -> bool {
        self.0 == unsafe { grim::CLEAN_BUFFER.inner_addr() }
    }

    pub fn is_back_buffer(&self) -> bool {
        self.0 == unsafe { grim::BACK_BUFFER.addr() }
    }

    pub fn is_smush_buffer(&self) -> bool {
        self.0 == unsafe { grim::SMUSH_BUFFER.inner_addr() }
    }
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct SurfaceAddr(usize);

impl SurfaceAddr {
    pub fn from_ptr(ptr: *const grim::Surface) -> SurfaceAddr {
        SurfaceAddr(ptr as usize)
    }
}

pub struct ImageContainer<'a> {
    raw: &'a grim::ImageContainer,
    images: Vec<Image>,
    pub original_addr: ImageContainerAddr,
}

impl<'a> ImageContainer<'a> {
    pub fn from_raw(image_container: *const grim::ImageContainer) -> Option<ImageContainer<'a>> {
        let original_addr = ImageContainerAddr::from_ptr(image_container);
        unsafe { image_container.as_ref() }.map(|raw| {
            let image_ptrs =
                unsafe { std::slice::from_raw_parts(raw.images, raw.image_count as usize) };
            let images = image_ptrs
                .iter()
                .filter_map(|&image_ptr| Image::from_raw(image_ptr))
                .collect();
            ImageContainer {
                raw,
                images,
                original_addr,
            }
        })
    }

    pub fn name(&self) -> &str {
        let cstr = unsafe { CStr::from_ptr(self.raw.name.as_ptr()) };
        cstr.to_str().unwrap_or("")
    }

    pub fn images(&self) -> Vec<&Image> {
        self.images.iter().collect()
    }

    pub fn image_addrs(&self) -> Vec<ImageAddr> {
        let image_addrs = unsafe {
            std::slice::from_raw_parts(
                self.raw.images as *const usize,
                self.raw.image_count as usize,
            )
        };
        image_addrs.iter().map(|&addr| ImageAddr(addr)).collect()
    }
}

pub struct Image {
    pub original_addr: ImageAddr,
    pub width: i32,
    pub height: i32,
}

impl Image {
    pub fn from_raw(image: *const grim::Image) -> Option<Image> {
        let original_addr = ImageAddr::from_ptr(image);
        unsafe { image.as_ref() }.map(|raw| {
            let (width, height) = (raw.attributes.width, raw.attributes.height);
            Image {
                original_addr,
                width,
                height,
            }
        })
    }
}

fn extract<T>(mutex: &Mutex<Option<T>>) -> Option<T> {
    mutex.lock().ok()?.take()
}

/// Stores an image/surface pair if an HQ overlay was just prepared
pub fn pair_overlay_surface(surface: *const grim::Surface) {
    let surface_addr = SurfaceAddr(surface as usize);
    if let Some(overlay) = extract(&PREPARING_OVERLAY) {
        OVERLAYS.lock().unwrap().insert(surface_addr, overlay);
    }
}

/// Removes all HQ overlay pairs owned by dropped HQ container
pub fn unpair_overlay_surfaces(hq_image_container: &image::HqImageContainer) {
    let mut overlays = OVERLAYS.lock().unwrap();
    for hq_image in hq_image_container.images.iter() {
        overlays.retain(|_, image_addr| image_addr != &hq_image.original_addr);
    }
}

/// Hooks BM image loading to load a modded HQ version of the image
pub extern "C" fn open_bm_image(
    filename: *const c_char,
    param_2: u32,
    param_3: u32,
) -> *mut grim::ImageContainer {
    unsafe {
        let image_container = grim::open_bm_image(filename, param_2, param_3);

        if let Some(image_container) = ImageContainer::from_raw(image_container) {
            let removed = image::HqImageContainer::load(&image_container);
            if let Some(hq_image_container) = removed {
                unpair_overlay_surfaces(&hq_image_container);
            }
        }

        image_container
    }
}

/// Hooks resource management to drop HQ images with original image
pub extern "C" fn manage_resource(resource: *mut grim::Resource) -> c_int {
    let state = unsafe { (*resource).state };
    let image_container_addr = ImageContainerAddr(unsafe { (*resource).image_container as usize });

    if state == 2 {
        let removed = image::HqImageContainer::unload(image_container_addr);
        if let Some(hq_image_container) = removed {
            unpair_overlay_surfaces(&hq_image_container);
        }
    }

    unsafe { grim::manage_resource(resource) }
}

/// Hooks decompression to track an image through the system
pub extern "C" fn decompress_image(image: *const grim::Image) {
    // store the address of the last image decompressed
    // it will shortly be copied to the clean buffer and rendered
    *DECOMPRESSED.lock().unwrap() = Some(ImageAddr::from_ptr(image));

    unsafe { grim::decompress_image(image) }
}

fn active_smush_frame_size() -> Option<(i32, i32)> {
    let frame = unsafe { grim::ACTIVE_SMUSH_FRAME.inner_ref() }?;
    Some((frame.attributes.width, frame.attributes.height))
}

/// Hooks image copying to detect when a background or overlay is being interacted with
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
    let dst_image_addr = ImageAddr::from_ptr(dst_image);
    let src_image_addr = ImageAddr::original(src_image).unwrap_or(ImageAddr(0));

    // an image being copied to the clean buffer first means it is a background (or draws
    // over the background) about to be rendered
    if dst_image_addr.is_clean_buffer() {
        image::Background::write(src_image_addr, x, y);
    }

    if dst_image_addr.is_back_buffer() {
        // remove any HQ background if a fullscreen video plays as it will cover it
        // these are not true cutscenes and don't change the scene
        if src_image_addr.is_smush_buffer() && active_smush_frame_size() == Some((640, 480)) {
            *image::BACKGROUND.lock().unwrap() = None;
        }
        // if an image is being written directly to the back buffer and it is not a then it's an overlay
        // temporarily store the image address as the next surface bound will be for it
        else {
            *PREPARING_OVERLAY.lock().unwrap() =
                image::HqImage::is_loaded(src_image_addr).then_some(src_image_addr);
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

/// Hooks surface allocation to link dynamic surfaces to their HQ overlays
pub extern "C" fn surface_allocate(
    width: c_int,
    height: c_int,
    format: c_uint,
    param_4: c_int,
) -> *const grim::Surface {
    let surface = unsafe { grim::surface_allocate(width, height, format, param_4) };
    pair_overlay_surface(surface);
    surface
}

/// Hooks surface reuse to link dynamic surfaces to their HQ overlays
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
    pair_overlay_surface(surface);
    unsafe {
        grim::surface_bind_existing(
            surface, image, width, height, param_4, param_5, param_6, param_7, texture_id,
        )
    };
}
