use glob::glob;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::debug;
use crate::raw::grim;

// The game guards the file handle list with a mutex so that is replicated here out of caution.
// It also guards every individual file access with a mutex but that isn't needed here.
static HANDLES: Lazy<Mutex<HashSet<usize>>> = Lazy::new(|| Mutex::new(HashSet::new()));

extern "C" {
    pub fn fopen(filename: *const c_char, mode: *const c_char) -> *mut c_void;
    pub fn fclose(file: *mut c_void) -> i32;
    pub fn fread(file: *mut c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
}

/// Finds the path to a modded resource, if one exists
pub fn find_modded(filename: &str) -> Option<PathBuf> {
    let mut entries = glob(&format!("./Mods/*/assets/*/{}", filename.to_lowercase())).ok()?;
    entries.next().map(Result::unwrap)
}

/// Enhances the game's open file function, opening modded files if found
pub extern "C" fn open(raw_filename: *mut c_char, mode: *mut c_char) -> *mut c_void {
    let Ok(filename) = unsafe { CStr::from_ptr(raw_filename) }.to_str() else {
        return std::ptr::null_mut();
    };

    match find_modded(filename) {
        None => grim::open_file(raw_filename, mode),
        Some(path) => {
            if debug::verbose() {
                debug::info(format!("Opening modded {} file", path.display()));
            }

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
        grim::close_file(file)
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
        grim::read_file(file, dst, size)
    } else {
        unsafe { fread(dst, 1, size, file) }
    }
}
