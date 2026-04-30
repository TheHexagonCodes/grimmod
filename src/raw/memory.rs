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
// Both platforms export the same public API: BoundFn<F>, Value<T,F>, BindError,
// HookError, UnhookError, read(), write(), and Fn trait implementations for BoundFn.

#[cfg(target_os = "windows")]
mod platform {
    include!("memory_windows.rs");
}

#[cfg(target_os = "linux")]
mod platform {
    include!("memory_linux.rs");
}

pub use platform::*;
