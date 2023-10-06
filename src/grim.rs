use std::ffi::{c_char, c_int, c_void};
use std::mem;

pub mod addresses {
    // file operation functions that work with LAB packed files
    pub const OPEN_FILE: usize = 0x0034EF80;
    pub const CLOSE_FILE: usize = 0x0034C870;
    pub const READ_FILE: usize = 0x0034E050;
}

type FileOpener = extern "C" fn(*mut c_char, *mut c_char) -> *mut c_void;
type FileCloser = extern "C" fn(*mut c_void) -> c_int;
type FileReader = extern "C" fn(*mut c_void, *mut c_void, usize) -> usize;

#[inline(always)]
pub fn open_file(filename: *mut c_char, mode: *mut c_char) -> *mut c_void {
    unsafe {
        let f: FileOpener = mem::transmute(addresses::OPEN_FILE);
        f(filename, mode)
    }
}

#[inline(always)]
pub fn close_file(file: *mut c_void) -> c_int {
    unsafe {
        let f: FileCloser = mem::transmute(addresses::CLOSE_FILE);
        f(file)
    }
}

#[inline(always)]
pub fn read_file(file: *mut c_void, dst: *mut c_void, size: usize) -> usize {
    unsafe {
        let f: FileReader = mem::transmute(addresses::READ_FILE);
        f(file, dst, size)
    }
}
