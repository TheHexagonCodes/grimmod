use crate::process::{self, Address};

use std::ffi::{c_char, c_int, c_void};
use std::mem;

pub mod address {
    use crate::process::Address;

    // file operation functions that work with LAB packed files
    pub const OPEN_FILE: Address = Address::Relative(0x1EF80);
    pub const CLOSE_FILE: Address = Address::Relative(0x1C870);
    pub const READ_FILE: Address = Address::Relative(0x1E050);

    // contains the address for the RuntimeContext in use by the game
    pub const RUNTIME_CONTEXT_PTR: Address = Address::Relative(0x31B2CD8);
}

type FileOpener = extern "C" fn(*mut c_char, *mut c_char) -> *mut c_void;
type FileCloser = extern "C" fn(*mut c_void) -> c_int;
type FileReader = extern "C" fn(*mut c_void, *mut c_void, usize) -> usize;

#[inline(always)]
pub fn open_file(filename: *mut c_char, mode: *mut c_char) -> *mut c_void {
    unsafe {
        let f: FileOpener = mem::transmute(address::OPEN_FILE.absolute());
        f(filename, mode)
    }
}

#[inline(always)]
pub fn close_file(file: *mut c_void) -> c_int {
    unsafe {
        let f: FileCloser = mem::transmute(address::CLOSE_FILE.absolute());
        f(file)
    }
}

#[inline(always)]
pub fn read_file(file: *mut c_void, dst: *mut c_void, size: usize) -> usize {
    unsafe {
        let f: FileReader = mem::transmute(address::READ_FILE.absolute());
        f(file, dst, size)
    }
}

/// Stores a set of IO functions the game uses at runtime
///
/// This is used to make what functions are used at runtime configurable.
/// e.g. read files from a simple directory in debug but from LAB archives in release.
#[repr(C)]
pub struct RuntimeContext {
    _ignore_pre: [u32; 12],
    pub open_file: *const FileOpener,
    pub close_file: *const FileCloser,
    pub read_file: *const FileReader,
    _ignore_post: [u32; 17],
}

#[inline(always)]
pub fn with_runtime_context<F: FnOnce(&mut RuntimeContext)>(f: F) {
    unsafe {
        let runtime_context = process::read::<usize>(address::RUNTIME_CONTEXT_PTR);
        process::with_mut_ref(Address::Absolute(runtime_context), f)
    }
}
