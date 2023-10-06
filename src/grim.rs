use std::ffi::{c_char, c_int, c_void};

pub mod addresses {
    // file operation functions that work with LAB packed files
    pub const OPEN_FILE: usize = 0x0034EF80;
    pub const CLOSE_FILE: usize = 0x0034C870;
    pub const READ_FILE: usize = 0x0034E050;
}

pub const OPEN_FILE: *const extern "C" fn(*mut c_char, *mut c_char) -> *mut c_void =
    addresses::OPEN_FILE as _;
pub const CLOSE_FILE: *const extern "C" fn(*mut c_void) -> c_int = addresses::CLOSE_FILE as _;
pub const READ_FILE: *const extern "C" fn(*mut c_void, *mut c_void, usize) -> usize =
    addresses::READ_FILE as _;

#[inline(always)]
pub fn open_file(filename: *mut c_char, mode: *mut c_char) -> *mut c_void {
    unsafe { (*OPEN_FILE)(filename, mode) }
}

#[inline(always)]
pub fn close_file(file: *mut c_void) -> c_int {
    unsafe { (*CLOSE_FILE)(file) }
}

#[inline(always)]
pub fn read_file(file: *mut c_void, dst: *mut c_void, size: usize) -> usize {
    unsafe { (*READ_FILE)(file, dst, size) }
}
