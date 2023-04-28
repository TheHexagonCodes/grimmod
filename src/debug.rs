use std::fs::OpenOptions;
use std::io::Write;

static LOG_FILE: &str = "grimmod.log";

pub fn init() -> Option<()> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(LOG_FILE)
        .ok()
        .map(|_| ())
}

pub fn write(message: &str) -> Option<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(LOG_FILE)
        .ok()?;
    writeln!(file, "{}", message).ok()?;
    Some(())
}

pub fn info(message: &str) -> Option<()> {
    write(format!("[INFO] {}", message).as_ref())
}

pub fn error(message: &str) -> Option<()> {
    write(format!("[ERROR] {}", message).as_ref())
}
