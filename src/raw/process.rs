// Platform-specific symbol resolution and process introspection.
//
// Windows: PE/IAT parsing, VirtualQuery, GetProcAddress
// Linux:   ELF symtab parsing via goblin, dlsym, GOT entry lookup
// macOS:   Mach-O nlist/LC_SYMTAB parsing via goblin, dlsym, lazy/non-lazy
//          symbol pointer lookup, ASLR slide handling

#[cfg(target_os = "windows")]
mod platform {
    include!("process_windows.rs");
}

#[cfg(target_os = "linux")]
mod platform {
    include!("process_linux.rs");
}

#[cfg(target_os = "macos")]
mod platform {
    include!("process_macos.rs");
}

pub use platform::*;
