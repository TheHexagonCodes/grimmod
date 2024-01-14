use lazy_static::lazy_static;
use retour::RawDetour;
use std::ffi::c_void;
use std::ffi::{c_char, c_int, c_uint, CStr};
use std::path::Path;
use std::sync::Mutex;

use crate::debug;
use crate::file;
use crate::grim;
use crate::process;

lazy_static! {
    pub static ref HQ_IMAGES: Mutex<Vec<HqImage>> = Mutex::new(Vec::new());
    pub static ref OPEN_BM_IMAGE_HOOK: RawDetour = unsafe {
        RawDetour::new(
            *grim::address::OPEN_BM_IMAGE as *const (),
            open_bm_image as *const (),
        )
        .unwrap()
    };
    pub static ref SURFACE_UPLOAD_HOOK: RawDetour = unsafe {
        RawDetour::new(
            *grim::address::SURFACE_UPLOAD as *const (),
            surface_upload as *const (),
        )
        .unwrap()
    };
    pub static ref DECOMPRESS_IMAGE_HOOK: RawDetour = unsafe {
        RawDetour::new(
            *grim::address::DECOMPRESS_IMAGE as *const (),
            decompress_image as *const (),
        )
        .unwrap()
    };
    pub static ref COPY_IMAGE_HOOK: RawDetour = unsafe {
        RawDetour::new(
            *grim::address::COPY_IMAGE as *const (),
            copy_image as *const (),
        )
        .unwrap()
    };
    pub static ref MANAGE_RESOURCE_HOOK: RawDetour = unsafe {
        RawDetour::new(
            *grim::address::MANAGE_RESOURCE as *const (),
            manage_resource as *const (),
        )
        .unwrap()
    };
}

pub struct HqImage {
    pub path: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub buffer: Vec<u8>,
    pub original_image: usize,
    pub active: bool,
    pub decompressing: bool,
}

/// Return the address for the background render pass's surface
fn bitmap_underlays_surface() -> usize {
    unsafe {
        let render_pass =
            *(*grim::address::BITMAP_UNDERLAYS_RENDER_PASS_PTR as *const *const grim::RenderPass);
        if render_pass.is_null() {
            return 0;
        }
        let render_pass_data = (*render_pass).data;
        if render_pass_data.is_null() {
            return 0;
        }
        let bitmap_underlays_surface = (*render_pass_data).surface;

        bitmap_underlays_surface as usize
    }
}

/// Loads the contents of a BM file into an image container
///
/// This is a overload for a native function that will be hooked
pub extern "C" fn open_bm_image(
    raw_filename: *const c_char,
    param_2: u32,
    param_3: u32,
) -> *mut grim::ImageContainer {
    let image_container = unsafe { grim::open_bm_image(raw_filename, param_2, param_3) };

    if let Ok(filename) = unsafe { CStr::from_ptr(raw_filename) }.to_str() {
        debug::info(format!("Opening BM: {}", filename));

        if let Some(hq_image) = open_hq_image(filename, image_container as usize) {
            HQ_IMAGES.lock().unwrap().push(hq_image);
        }
    };

    image_container
}

fn open_hq_image(filename: &str, image_container_addr: usize) -> Option<HqImage> {
    if !filename.ends_with(".bm") && !filename.ends_with(".BM") {
        return None;
    }

    let file_stem = Path::new(filename).file_stem()?.to_str()?;
    let modded_filename = format!("{}.png", file_stem);
    let modded_path = file::find_modded(modded_filename.as_str())?;

    debug::info(format!("Opening HQ Image: {}", modded_path.display()));

    let image = image::open(modded_path).ok()?;

    Some(HqImage {
        name: filename.to_lowercase(),
        path: modded_filename,
        width: image.width(),
        height: image.height(),
        buffer: image.to_rgb8().into_vec(),
        original_image: image_container_addr,
        active: false,
        decompressing: false,
    })
}

/// Decompresses an image into the global decompression buffer
///
/// This is a overload for a native function that will be hooked
pub extern "C" fn decompress_image(image: *const grim::Image) {
    // check for an image with an associated hq image getting decompressed
    // it will shortly be copied to the clean buffer and soon rendered
    for hq_image in HQ_IMAGES.lock().unwrap().iter_mut() {
        unsafe {
            process::with_mut_ref(
                hq_image.original_image,
                |image_container: &mut grim::ImageContainer| {
                    hq_image.decompressing = *(image_container.images) as usize == image as usize;
                },
            );
        }
    }

    unsafe { grim::decompress_image(image) }
}

/// Copy an image and surface from a source to a pre-allocated destination
///
/// This is a overload for a native function that will be hooked
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
    if dst_image as usize == unsafe { *(*grim::address::CLEAN_BUFFER_PTR as *const usize) } {
        for hq_image in HQ_IMAGES.lock().unwrap().iter_mut() {
            hq_image.active =
                hq_image.decompressing || hq_image.original_image == src_image as usize;
            hq_image.decompressing = false;
        }
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

extern "stdcall" {
    pub fn glTexImage2D(
        target: c_uint,
        level: c_int,
        internalformat: c_int,
        width: c_int,
        height: c_int,
        border: c_int,
        format: c_uint,
        typ: c_uint,
        data: *const c_void,
    );

    pub fn glBindTexture(target: c_uint, texture: c_uint);

    pub fn glPixelStorei(pname: c_uint, param: c_int);
}

/// Prepare a surface (aka texture) for uploading to the GPU or upload it now
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
    for hq_image in HQ_IMAGES.lock().unwrap().iter() {
        if !hq_image.active {
            continue;
        }

        debug::info(format!(
            "Surface Address: 0x{:x}, Image Data: 0x{:x}",
            surface as usize, image_data as usize
        ));

        let hq_image_data: *const [u8] = hq_image.buffer.as_ref();

        unsafe {
            glPixelStorei(0xCF5, 1);
            glBindTexture(0xDE1, (*surface).texture_id);
            glPixelStorei(0xCF2, hq_image.width as i32);
            glTexImage2D(
                0xDE1,
                0,
                0x1907,
                hq_image.width as i32,
                hq_image.height as i32,
                0,
                0x1907,
                0x1401,
                hq_image_data as *const c_void,
            );
        }

        break;
    }
}

pub extern "C" fn manage_resource(resource: *mut grim::Resource) -> c_int {
    let state = unsafe { (*resource).state };
    let asset = unsafe { (*resource).asset as usize };

    if state == 2 {
        HQ_IMAGES
            .lock()
            .unwrap()
            .retain(|hq_image| hq_image.original_image != asset);
    }

    unsafe { grim::manage_resource(resource) }
}
