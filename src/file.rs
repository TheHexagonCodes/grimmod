use glob::glob;
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::debug;
use crate::grim;
use crate::image;

lazy_static! {
    // The game guards the file handle list with a mutex so that is replicated here out of caution.
    // It also guards every individual file access with a mutex but that isn't needed here.
    static ref HANDLES: Mutex<HashSet<usize>> = Mutex::new(HashSet::new());
}

extern "C" {
    pub fn fopen(filename: *const c_char, mode: *const c_char) -> *mut c_void;
    pub fn fclose(file: *mut c_void) -> i32;
    pub fn fread(file: *mut c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
}

/// Finds the path to a modded resource, if one exists
pub fn find_modded(filename: &str) -> Option<PathBuf> {
    let mut entries = glob(format!("./Mods/*/resources/{}", filename).as_ref()).unwrap();
    entries.next().map(Result::unwrap)
}

/// Enhances the game's open file function, opening modded files if found
pub extern "C" fn open(raw_filename: *mut c_char, mode: *mut c_char) -> *mut c_void {
    let Ok(filename) = unsafe { CStr::from_ptr(raw_filename) }.to_str() else {
        return std::ptr::null_mut();
    };

    match find_modded(filename) {
        None => unsafe { grim::open_file(raw_filename, mode) },
        Some(path) => {
            debug::info(format!("Opening modded file: {}", path.display()));

            let raw_path = CString::new(path.to_str().unwrap()).unwrap().into_raw();
            let file = unsafe { fopen(raw_path, mode) };

            HANDLES.lock().unwrap().insert(file as usize);

            file
        }
    }
}

/// Closes original or modded files
pub extern "C" fn close(file: *mut c_void) -> i32 {
    let handle = file as usize;
    let modded = HANDLES.lock().unwrap().contains(&handle);

    if !modded {
        unsafe { grim::close_file(file) }
    } else {
        HANDLES.lock().unwrap().remove(&handle);
        unsafe { fclose(file) }
    }
}

/// Reads from original or modded files
pub extern "C" fn read(file: *mut c_void, dst: *mut c_void, size: usize) -> usize {
    let handle = file as usize;
    let modded = HANDLES.lock().unwrap().contains(&handle);

    if !modded {
        unsafe { grim::read_file(file, dst, size) }
    } else {
        unsafe { fread(dst, 1, size, file) }
    }
}

/// Opens a file and reads its entire contents
///
/// This is a overload for a native function that will be hooked
pub extern "C" fn read_all(dst: *mut *const c_void, size: *mut usize, raw_filename: *const c_char) {
    let filename = unsafe { CStr::from_ptr(raw_filename) }
        .to_str()
        .unwrap_or("");

    if filename.starts_with("x86/shaders/compiled/grimmod_background") {
        let mut shader = if filename.ends_with("v.glsl") {
            image::BACKGROUND_V_SHADER.as_bytes().to_vec()
        } else {
            image::BACKGROUND_P_SHADER.as_bytes().to_vec()
        };
        shader.push(0);

        unsafe {
            if let (Some(dst), Some(size)) = (dst.as_mut(), size.as_mut()) {
                *dst = shader.as_ptr() as *const c_void;
                *size = shader.len();
            };
        }

        std::mem::forget(shader);
    } else {
        unsafe { grim::read_all(dst, size, raw_filename) }
    }
}
