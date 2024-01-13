use glob::glob;
use lazy_static::lazy_static;
use retour::RawDetour;
use std::collections::HashSet;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::debug;
use crate::grim;

// The game guards the file handle list with a mutex so that is replicated here out of caution.
// It also guards every individual file access with a mutex but that doesn't appear necessary here.
static HANDLES: Mutex<Option<HashSet<usize>>> = Mutex::new(None);

lazy_static! {
    pub static ref OPEN_FILE_HOOK: RawDetour = unsafe {
        RawDetour::new(*grim::address::OPEN_FILE as *const (), open as *const ()).unwrap()
    };
    pub static ref CLOSE_FILE_HOOK: RawDetour = unsafe {
        RawDetour::new(*grim::address::CLOSE_FILE as *const (), close as *const ()).unwrap()
    };
    pub static ref READ_FILE_HOOK: RawDetour = unsafe {
        RawDetour::new(*grim::address::READ_FILE as *const (), read as *const ()).unwrap()
    };
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
    let original: grim::FileOpener = unsafe { std::mem::transmute(OPEN_FILE_HOOK.trampoline()) };
    let Ok(filename) = unsafe { CStr::from_ptr(raw_filename) }.to_str() else {
        return std::ptr::null_mut();
    };

    match find_modded(filename) {
        None => original(raw_filename, mode),
        Some(path) => {
            debug::info(format!("Opening modded file: {}", path.display()));

            let raw_path = CString::new(path.to_str().unwrap()).unwrap().into_raw();
            let file = unsafe { fopen(raw_path, mode) };

            with_handles(|handles| {
                handles.insert(file as usize);
            });

            file
        }
    }
}

/// Closes original or modded files
pub extern "C" fn close(file: *mut c_void) -> i32 {
    let original: grim::FileCloser = unsafe { std::mem::transmute(CLOSE_FILE_HOOK.trampoline()) };
    let handle = file as usize;
    with_handles(|handles| {
        if !handles.contains(&handle) {
            original(file)
        } else {
            handles.remove(&handle);
            unsafe { fclose(file) }
        }
    })
}

/// Reads from original or modded files
pub extern "C" fn read(file: *mut c_void, dst: *mut c_void, size: usize) -> usize {
    let original: grim::FileReader = unsafe { std::mem::transmute(READ_FILE_HOOK.trampoline()) };
    let handle = file as usize;
    let modded = with_handles(|handles| handles.contains(&handle));

    if !modded {
        original(file, dst, size)
    } else {
        unsafe { fread(dst, 1, size, file) }
    }
}

fn with_handles<F, T>(f: F) -> T
where
    F: FnOnce(&mut HashSet<usize>) -> T,
{
    let mut guard = HANDLES.lock().unwrap();

    match guard.as_mut() {
        None => {
            let mut handles = HashSet::new();
            let value = f(&mut handles);
            *guard = Some(handles);
            value
        }
        Some(handles) => f(handles),
    }
}
