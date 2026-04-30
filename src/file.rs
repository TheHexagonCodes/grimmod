use glob::glob;
use std::collections::HashSet;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;

use crate::debug;
use crate::raw::grim;

// The game guards the file handle list with a mutex so that is replicated here out of caution.
// It also guards every individual file access with a mutex but that isn't needed here.
static HANDLES: LazyLock<Mutex<HashSet<usize>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

extern "C" {
    pub fn fopen(filename: *const c_char, mode: *const c_char) -> *mut c_void;
    pub fn fclose(file: *mut c_void) -> i32;
    pub fn fread(file: *mut c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
}

/// Finds the path to a modded resource, if one exists
pub fn find_modded(filename: &str) -> Option<PathBuf> {
    let mut entries = glob(&format!("./Mods/*/assets/*/{}", filename.to_lowercase())).ok()?;
    entries.next().and_then(Result::ok)
}

/// Enhances the game's open file function, opening modded files if found
pub extern "C" fn open(raw_filename: *mut c_char, mode: *mut c_char) -> *mut c_void {
    let Ok(filename) = (unsafe { CStr::from_ptr(raw_filename) }).to_str() else {
        return std::ptr::null_mut();
    };

    if debug::verbose() {
        debug::info(format!("File open: '{}'", filename));
    }

    match find_modded(filename) {
        None => grim::open_file(raw_filename, mode),
        Some(path) => {
            if debug::verbose() {
                debug::info(format!("Opening modded {} file", path.display()));
            }

            let Some(path_str) = path.to_str() else {
                debug::error(format!("Non-UTF-8 path: {}", path.display()));
                return grim::open_file(raw_filename, mode);
            };
            let Ok(c_path) = CString::new(path_str) else {
                debug::error(format!("Path contains null byte: {}", path.display()));
                return grim::open_file(raw_filename, mode);
            };

            let file = unsafe { fopen(c_path.as_ptr(), mode) };

            HANDLES
                .lock()
                .expect("HANDLES lock poisoned")
                .insert(file as usize);

            file
        }
    }
}

/// Closes original or modded files
pub extern "C" fn close(file: *mut c_void) -> i32 {
    let handle = file as usize;
    let modded = HANDLES
        .lock()
        .expect("HANDLES lock poisoned")
        .remove(&handle);

    if !modded {
        grim::close_file(file)
    } else {
        unsafe { fclose(file) }
    }
}

/// Reads from original or modded files
pub extern "C" fn read(file: *mut c_void, dst: *mut c_void, size: usize) -> usize {
    let handle = file as usize;
    let modded = HANDLES
        .lock()
        .expect("HANDLES lock poisoned")
        .contains(&handle);

    if !modded {
        grim::read_file(file, dst, size)
    } else {
        unsafe { fread(dst, 1, size, file) }
    }
}
