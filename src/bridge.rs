use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, CStr};
use std::sync::Mutex;

use crate::debug;
use crate::gl;
use crate::grim;
use crate::image;

pub static DECOMPRESSED: Mutex<Option<ImageAddr>> = Mutex::new(None);
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

    pub fn underlying(&self) -> usize {
        self.0
    }

    pub fn image(&self) -> Option<Image> {
        Image::from_raw(self.0 as *const _)
    }

    /// Gets the original image address before decompression
    pub fn original(&self) -> ImageAddr {
        if self.is_decompression_buffer() {
            DECOMPRESSED.lock().unwrap().unwrap_or(*self)
        } else {
            *self
        }
    }

    pub fn is_decompression_buffer(&self) -> bool {
        self.0 == unsafe { grim::DECOMPRESSION_BUFFER.inner_addr() }
    }

    pub fn is_clean_buffer(&self) -> bool {
        self.0 == unsafe { grim::CLEAN_BUFFER.inner_addr() }
    }

    pub fn is_clean_z_buffer(&self) -> bool {
        self.0 == unsafe { grim::CLEAN_Z_BUFFER.inner_addr() }
    }

    pub fn is_back_buffer(&self) -> bool {
        self.0 == unsafe { grim::BACK_BUFFER.addr() }
    }

    pub fn is_smush_buffer(&self) -> bool {
        self.0 == unsafe { grim::SMUSH_BUFFER.inner_addr() }
    }

    pub fn name(&self) -> String {
        if self.is_decompression_buffer() {
            format!("DECOMPRESSION_BUFFER aka {}", self.original().name())
        } else if self.is_clean_buffer() {
            "CLEAN_BUFFER".to_string()
        } else if self.is_clean_z_buffer() {
            "CLEAN_Z_BUFFER".to_string()
        } else if self.is_back_buffer() {
            "BACK_BUFFER".to_string()
        } else if self.is_smush_buffer() {
            "SMUSH_BUFFER".to_string()
        } else if let Some(name) = image::HqImage::name(*self) {
            name
        } else {
            format!("unknown/dynamic buffer (0x{:x})", self.0)
        }
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
    pub addr: ImageAddr,
    pub width: i32,
    pub height: i32,
}

impl Image {
    pub fn from_raw(image: *const grim::Image) -> Option<Image> {
        let addr = ImageAddr::from_ptr(image);
        unsafe { image.as_ref() }.map(|raw| {
            let (width, height) = (raw.attributes.width, raw.attributes.height);
            Image {
                addr,
                width,
                height,
            }
        })
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
    if debug::verbose() {
        debug::info(format!(
            "Decompressing {}",
            ImageAddr::from_ptr(image).name()
        ));
    }

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
    dst_surface: *mut grim::Surface,
    src_image: *mut grim::Image,
    src_surface: *mut grim::Surface,
    x: u32,
    y: u32,
    param_7: u32,
    param_8: u32,
) {
    let src_image_addr = ImageAddr::from_ptr(src_image);
    let dst_image_addr = ImageAddr::from_ptr(dst_image);

    if debug::verbose() {
        debug::info(format!(
            "Copying {} to {}",
            src_image_addr.name(),
            dst_image_addr.name(),
        ));
    }

    let src_image_addr = ImageAddr::from_ptr(src_image).original();

    // an image being copied to the clean buffer first means it is a background (or draws
    // over the background) about to be rendered
    if dst_image_addr.is_clean_buffer() {
        if let Some(src_image) = src_image_addr.image() {
            image::Background::write(src_image, x, y);
        }
    }

    if dst_image_addr.is_back_buffer() {
        // remove any HQ background if a fullscreen video plays as it will cover it
        // these are not true cutscenes and don't change the scene
        if src_image_addr.is_smush_buffer() && active_smush_frame_size() == Some((640, 480)) {
            *image::BACKGROUND.lock().unwrap() = None;
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

/// Hooks surface binding to associate surfaces with HQ overlays
pub extern "C" fn bind_image_surface(
    image: *mut grim::Image,
    param_2: u32,
    param_3: u32,
    param_4: u32,
) -> *mut grim::Surface {
    let image_addr = ImageAddr::from_ptr(image).original();
    let is_hq = image::HqImage::is_loaded(image_addr);
    let surface = unsafe { grim::bind_image_surface(image, param_2, param_3, param_4) };
    let surface_addr = SurfaceAddr::from_ptr(surface);

    if is_hq {
        if debug::verbose() {
            debug::info(format!(
                "Binding {} to surface 0x{:x}",
                image_addr.name(),
                surface_addr.0
            ));
        }
        OVERLAYS.lock().unwrap().insert(surface_addr, image_addr);
    } else {
        OVERLAYS.lock().unwrap().remove(&surface_addr);
    }

    surface
}

/// Hooks texture deletion to clear out any bound HQ overlays
pub extern "stdcall" fn delete_textures(n: gl::Sizei, textures: *const gl::Uint) {
    let surface_addr = SurfaceAddr(textures as usize - 0x20);
    OVERLAYS.lock().unwrap().remove(&surface_addr);

    unsafe { gl::delete_textures(n, textures) };
}
