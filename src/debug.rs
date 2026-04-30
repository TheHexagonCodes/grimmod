use std::ffi::CString;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::LazyLock;

use crate::config::Config;
use crate::raw::{gl, grim};

const LOG_FILENAME: &str = "grimmod.log";

static LOG_FILE: LazyLock<Option<File>> = LazyLock::new(|| {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(LOG_FILENAME)
        .ok()
});

fn write(message: fmt::Arguments) -> Option<()> {
    // On Linux, also print to stderr for easy debugging under LD_PRELOAD
    #[cfg(target_os = "linux")]
    eprintln!("[grimmod] {}", message);

    if let Some(mut log_file) = LOG_FILE.as_ref() {
        writeln!(log_file, "{}", message).ok()?;
    }
    Some(())
}

pub fn info<T: AsRef<str>>(message: T) -> Option<()> {
    if !Config::get().logging.enabled {
        return None;
    }
    write(format_args!("[INFO] {}", message.as_ref()))
}

/// Debug-level messages — only logged when `logging.debug = true` in grimmod.toml.
pub fn debug<T: AsRef<str>>(message: T) -> Option<()> {
    if !verbose() {
        return None;
    }
    write(format_args!("[DEBUG] {}", message.as_ref()))
}

pub fn error<T: AsRef<str>>(message: T) -> Option<()> {
    if !Config::get().logging.enabled {
        return None;
    }
    write(format_args!("[ERROR] {}", message.as_ref()))
}

#[allow(dead_code)]
pub fn gl<T: AsRef<str>>(message: T) -> Option<()> {
    let cmessage = CString::new(message.as_ref()).ok()?;
    grim::marker(message.as_ref().len(), cmessage.as_ptr());
    Some(())
}

#[allow(dead_code)]
pub fn gl_error() -> Option<u32> {
    let gl_err = gl::get_error();
    if gl_err != 0 {
        error(format!("OpenGL: {}", gl_err));
        Some(gl_err)
    } else {
        None
    }
}

pub fn verbose() -> bool {
    Config::get().logging.debug
}
