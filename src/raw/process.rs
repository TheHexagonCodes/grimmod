// Platform-specific symbol resolution and process introspection.
//
// Windows: PE/IAT parsing, VirtualQuery, GetProcAddress
// Linux:   ELF symtab parsing via goblin, dlsym, GOT entry lookup

#[cfg(target_os = "windows")]
mod platform {
    include!("process_windows.rs");
}

#[cfg(target_os = "linux")]
mod platform {
    include!("process_linux.rs");
}

pub use platform::*;
