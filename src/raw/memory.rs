// Platform-specific implementations of the hooking and memory infrastructure.
//
// Windows: Uses retour (RawDetour) for inline hooks, VirtualProtect for memory
//          protection, lightningscanner for byte-pattern scanning, IAT overwriting
//          for indirect hooks.
//
// Linux:   Uses custom inline x86 prologue patching with mmap'd trampolines,
//          mprotect for memory protection, ELF symtab resolution, GOT overwriting
//          for indirect hooks.
//
// All platforms export the same public API: BoundFn<F>, Value<T,F>, BindError,
// HookError, UnhookError, read(), write(), and Fn trait implementations for BoundFn.
//
// macOS:  Uses custom inline x86-64 prologue patching with mmap'd trampolines,
//         mprotect/MAP_JIT for memory protection, Mach-O symtab resolution,
//         lazy/non-lazy symbol pointer overwriting for indirect hooks.

#[cfg(target_os = "windows")]
mod platform {
    include!("memory_windows.rs");
}

#[cfg(target_os = "linux")]
mod platform {
    include!("memory_linux.rs");
}

#[cfg(target_os = "macos")]
mod platform {
    include!("memory_macos.rs");
}

pub use platform::*;
