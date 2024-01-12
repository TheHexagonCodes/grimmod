use lazy_static::lazy_static;

use crate::debug;
use crate::file;
use crate::grim;

use retour::RawDetour;
use std::ffi::{c_char, CStr};
use std::path::Path;
use std::sync::Mutex;

lazy_static! {
    pub static ref HQ_IMAGES: Mutex<Vec<HqImage>> = Mutex::new(Vec::new());
    pub static ref OPEN_BM_IMAGE_HOOK: RawDetour = unsafe {
        RawDetour::new(
            *grim::address::OPEN_BM_IMAGE as *const (),
            open_bm_image as *const (),
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
}

pub extern "C" fn open_bm_image(
    filename: *const c_char,
    param_2: u32,
    param_3: u32,
) -> *mut grim::ImageContainer {
    let original: grim::OpenBmImage =
        unsafe { std::mem::transmute(OPEN_BM_IMAGE_HOOK.trampoline()) };
    let image_container = original(filename, param_2, param_3);

    if let Some(hq_image) = open_hq_image(filename, image_container) {
        HQ_IMAGES.lock().unwrap().push(hq_image);
    }

    image_container
}

fn open_hq_image(
    raw_filename: *const c_char,
    image_container: *const grim::ImageContainer,
) -> Option<HqImage> {
    let filename = unsafe { CStr::from_ptr(raw_filename) }.to_str().ok()?;

    debug::info(format!("Opening BM: {}", filename));

    if !filename.ends_with(".bm") && !filename.ends_with(".BM") {
        return None;
    }

    let file_stem = Path::new(filename).file_stem()?.to_str()?;
    let modded_filename = format!("{}.png", file_stem);
    let modded_path = file::find_modded(modded_filename.as_str())?;
    let image = image::open(modded_path).ok()?;

    Some(HqImage {
        name: filename.to_lowercase(),
        path: modded_filename,
        width: image.width(),
        height: image.height(),
        buffer: image.to_rgb8().into_vec(),
        original_image: image_container as usize,
    })
}
