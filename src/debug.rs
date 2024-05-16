use once_cell::sync::Lazy;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::config::Config;
use crate::raw::{gl, grim};

const LOG_FILENAME: &str = "grimmod.log";

static LOG_FILE: Lazy<Option<File>> = Lazy::new(|| {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(LOG_FILENAME)
        .ok()
});

pub fn write<T: AsRef<str>>(message: T) -> Option<()> {
    if let Some(mut log_file) = LOG_FILE.as_ref() {
        writeln!(log_file, "{}", message.as_ref()).ok()?;
    }
    Some(())
}

pub fn info<T: AsRef<str>>(message: T) -> Option<()> {
    write(format!("[INFO] {}", message.as_ref()))
}

pub fn error<T: AsRef<str>>(message: T) -> Option<()> {
    write(format!("[ERROR] {}", message.as_ref()))
}

#[allow(dead_code)]
pub fn gl<T: AsRef<str>>(message: T) -> Option<()> {
    let cmessage = CString::new(message.as_ref()).ok()?;
    unsafe {
        grim::marker(message.as_ref().len(), cmessage.as_ptr());
    }
    Some(())
}

#[allow(dead_code)]
pub fn gl_error() -> Option<u32> {
    let gl_err = unsafe { gl::get_error() };
    if gl_err != 0 {
        error(format!("OpenGL: {}", gl_err));
        Some(gl_err)
    } else {
        None
    }
}

pub fn verbose() -> bool {
    Config::get().debug.verbose
}
