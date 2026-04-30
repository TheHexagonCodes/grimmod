// Core hooking and function binding infrastructure for Linux.
//
// Direct hooks use inline patching: overwrite the function prologue with a
// JMP to the replacement, saving the original bytes for a trampoline.
// Indirect hooks overwrite GOT (Global Offset Table) entries.
// Memory protection changes use `mprotect` instead of `VirtualProtect`.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::debug;

/// Page size for mprotect alignment.
/// Detected at runtime since box64 on PPC64LE has 64KB pages even though
/// the emulated x86 binary expects 4KB pages.
fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

/// Size of the inline JMP patch: `JMP rel32` = 5 bytes on i386.
const JMP_PATCH_SIZE: usize = 5;

/// Maximum prologue bytes we might need to copy (generous upper bound).
const MAX_PROLOGUE_COPY: usize = 16;

// ---------- x86-32 instruction length decoder ----------

/// Decode the length of a single x86-32 instruction at `code`.
///
/// Returns `Some(len)` for recognized instructions, `None` for unknown opcodes.
///
/// # Safety
/// `code` must point to at least 16 readable bytes.
unsafe fn x86_insn_len(code: *const u8) -> Option<usize> {
    let op = *code;
    match op {
        // Single-byte instructions
        0x50..=0x57 => Some(1), // PUSH reg (EAX..EDI)
        0x58..=0x5F => Some(1), // POP reg
        0x90 => Some(1),        // NOP
        0xC3 => Some(1),        // RET
        0xCC => Some(1),        // INT3
        0xF4 => Some(1),        // HLT
        0x99 => Some(1),        // CDQ
        0x9C => Some(1),        // PUSHFD
        0x9D => Some(1),        // POPFD

        // CALL rel32, JMP rel32
        0xE8 | 0xE9 => Some(5),

        // JMP rel8, Jcc rel8
        0xEB => Some(2),
        0x70..=0x7F => Some(2), // Jcc short

        // MOV reg, imm32 (0xB8+rd)
        0xB8..=0xBF => Some(5),

        // PUSH imm8
        0x6A => Some(2),
        // PUSH imm32
        0x68 => Some(5),

        // Two-byte opcode prefix (0x0F)
        0x0F => {
            let op2 = *code.add(1);
            match op2 {
                // Jcc rel32 (near conditional jumps)
                0x80..=0x8F => Some(6),
                // MOVAPS/MOVUPS xmm, xmm/m128 or reverse
                0x28 | 0x29 | 0x10 | 0x11 => Some(2 + modrm_extra_len(code.add(2))),
                // Other 0F xx with ModRM
                _ => Some(2 + modrm_extra_len(code.add(2))),
            }
        }

        // Instructions with ModR/M byte
        0x00..=0x03
        | 0x08..=0x0B
        | 0x10..=0x13
        | 0x18..=0x1B
        | 0x20..=0x23
        | 0x28..=0x2B
        | 0x30..=0x33
        | 0x38..=0x3B => Some(1 + modrm_extra_len(code.add(1))),

        // ALU AL, imm8
        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => Some(2),
        // ALU EAX, imm32
        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => Some(5),

        // TEST r/m, r
        0x84 | 0x85 => Some(1 + modrm_extra_len(code.add(1))),

        // XCHG, MOV r/m,r / r,r/m
        0x86..=0x8B => Some(1 + modrm_extra_len(code.add(1))),

        // LEA r, m
        0x8D => Some(1 + modrm_extra_len(code.add(1))),

        // MOV r/m, imm
        0xC6 => Some(1 + modrm_extra_len(code.add(1)) + 1), // + imm8
        0xC7 => Some(1 + modrm_extra_len(code.add(1)) + 4), // + imm32

        // Group 1: op r/m, imm8
        0x80 | 0x83 => Some(1 + modrm_extra_len(code.add(1)) + 1),
        // Group 1: op r/m, imm32
        0x81 => Some(1 + modrm_extra_len(code.add(1)) + 4),
        // Group 1: op r/m, imm8 (rare)
        0x82 => Some(1 + modrm_extra_len(code.add(1)) + 1),

        // Shift/rotate group
        0xC0 => Some(1 + modrm_extra_len(code.add(1)) + 1),
        0xC1 => Some(1 + modrm_extra_len(code.add(1)) + 1),
        0xD0..=0xD3 => Some(1 + modrm_extra_len(code.add(1))),

        // INC/DEC r32
        0x40..=0x4F => Some(1),

        // TEST EAX, imm32
        0xA9 => Some(5),
        // TEST AL, imm8
        0xA8 => Some(2),

        // MOV EAX, moffs32 / MOV moffs32, EAX
        0xA1 | 0xA3 => Some(5),
        0xA0 | 0xA2 => Some(5),

        // FF group (CALL/JMP r/m32 etc.)
        0xFF => Some(1 + modrm_extra_len(code.add(1))),

        // NOT, NEG, MUL, IMUL, DIV, IDIV
        0xF6 => {
            let modrm = *code.add(1);
            let reg = (modrm >> 3) & 7;
            let extra = modrm_extra_len(code.add(1));
            if reg == 0 || reg == 1 {
                Some(1 + extra + 1)
            } else {
                Some(1 + extra)
            }
        }
        0xF7 => {
            let modrm = *code.add(1);
            let reg = (modrm >> 3) & 7;
            let extra = modrm_extra_len(code.add(1));
            if reg == 0 || reg == 1 {
                Some(1 + extra + 4)
            } else {
                Some(1 + extra)
            }
        }

        // RET imm16
        0xC2 => Some(3),

        // LEAVE
        0xC9 => Some(1),

        _ => None,
    }
}

/// Calculate extra bytes consumed by a ModR/M byte (and optional SIB + displacement).
///
/// # Safety
/// `modrm_ptr` must point to at least 6 readable bytes.
unsafe fn modrm_extra_len(modrm_ptr: *const u8) -> usize {
    let modrm = *modrm_ptr;
    let mode = modrm >> 6;
    let rm = modrm & 7;

    match mode {
        0b00 => {
            if rm == 0b100 {
                let sib = *modrm_ptr.add(1);
                let base = sib & 7;
                if base == 0b101 {
                    1 + 1 + 4 // ModRM + SIB + disp32
                } else {
                    1 + 1 // ModRM + SIB
                }
            } else if rm == 0b101 {
                1 + 4 // ModRM + disp32
            } else {
                1 // ModRM only
            }
        }
        0b01 => {
            if rm == 0b100 {
                1 + 1 + 1 // ModRM + SIB + disp8
            } else {
                1 + 1 // ModRM + disp8
            }
        }
        0b10 => {
            if rm == 0b100 {
                1 + 1 + 4 // ModRM + SIB + disp32
            } else {
                1 + 4 // ModRM + disp32
            }
        }
        0b11 => {
            1 // ModRM only (register-register)
        }
        _ => unreachable!(),
    }
}

/// Calculate prologue copy length (at least JMP_PATCH_SIZE, instruction-aligned).
///
/// # Safety
/// `addr` must point to readable executable memory with at least MAX_PROLOGUE_COPY bytes.
unsafe fn prologue_copy_len(addr: usize) -> Result<usize, String> {
    let code = addr as *const u8;
    let mut offset = 0usize;

    while offset < JMP_PATCH_SIZE {
        if offset >= MAX_PROLOGUE_COPY {
            return Err(format!(
                "Prologue at 0x{:08x} too long to copy (>{} bytes without reaching patch size)",
                addr, MAX_PROLOGUE_COPY
            ));
        }
        match x86_insn_len(code.add(offset)) {
            Some(len) => offset += len,
            None => {
                return Err(format!(
                    "Unknown x86 opcode 0x{:02x} at 0x{:08x}+{} while scanning prologue",
                    *code.add(offset),
                    addr,
                    offset
                ));
            }
        }
    }

    Ok(offset)
}

/// Make a memory region writable+executable using mprotect.
///
/// # Safety
/// Caller must ensure `addr` and `len` describe a valid memory region.
unsafe fn make_writable(addr: usize, len: usize) -> Result<(), String> {
    let ps = page_size();
    let page_start = addr & !(ps - 1);
    let page_end = (addr + len + ps - 1) & !(ps - 1);
    let result = libc::mprotect(
        page_start as *mut libc::c_void,
        page_end - page_start,
        libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
    );
    if result != 0 {
        Err(format!(
            "mprotect failed at 0x{:08x} (len {}): errno {}",
            addr,
            len,
            *libc::__errno_location()
        ))
    } else {
        Ok(())
    }
}

/// Read a value from a raw memory address.
///
/// # Safety
/// Caller must ensure `address` points to valid, readable memory of type T.
pub unsafe fn read<T: Sized + Clone + Default>(address: usize) -> T {
    let ptr = address as *const T;
    if ptr.is_null() {
        T::default()
    } else {
        (*ptr).clone()
    }
}

/// Write a value to a raw memory address, temporarily making it writable.
///
/// # Safety
/// Caller must ensure `address` points to valid memory that can hold a T.
pub unsafe fn write<T: Sized>(address: usize, value: T) {
    make_writable(address, std::mem::size_of::<T>()).ok();
    let ptr = address as *mut T;
    std::ptr::write(ptr, value);
}

/// A trampoline that allows calling the original function after it's been hooked.
struct Trampoline {
    code: *mut u8,
    size: usize,
    prologue_len: usize,
}

impl Trampoline {
    /// # Safety
    /// `target_addr` must point to a valid function with readable prologue bytes.
    unsafe fn new(target_addr: usize, name: &str) -> Result<Self, String> {
        let prologue_len =
            prologue_copy_len(target_addr).map_err(|e| format!("{} (function: {})", e, name))?;

        let alloc_size = prologue_len + 5;
        debug::debug(format!(
            "Trampoline for '{}': copying {} prologue bytes from 0x{:08x}",
            name, prologue_len, target_addr
        ));

        let code = libc::mmap(
            std::ptr::null_mut(),
            alloc_size.max(page_size()),
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        if code == libc::MAP_FAILED {
            return Err(format!("mmap failed for trampoline (function: {})", name));
        }
        let code = code as *mut u8;

        // Copy original prologue bytes
        std::ptr::copy_nonoverlapping(target_addr as *const u8, code, prologue_len);

        // Relocate relative CALL/JMP instructions
        {
            let src = target_addr as *const u8;
            let mut offset = 0usize;
            while offset < prologue_len {
                let op = *src.add(offset);
                let insn_len = x86_insn_len(src.add(offset)).unwrap();

                if (op == 0xE8 || op == 0xE9) && insn_len == 5 {
                    let orig_rel32 = std::ptr::read_unaligned(src.add(offset + 1) as *const i32);
                    let call_target = (target_addr + offset + 5) as isize + orig_rel32 as isize;
                    let tramp_insn_end = code as usize + offset + 5;
                    let new_rel32 = call_target - tramp_insn_end as isize;

                    if new_rel32 > i32::MAX as isize || new_rel32 < i32::MIN as isize {
                        libc::munmap(code as *mut libc::c_void, alloc_size.max(page_size()));
                        return Err(format!(
                            "Cannot relocate {} at 0x{:08x}+{}: target 0x{:x} too far from trampoline 0x{:x} (function: {})",
                            if op == 0xE8 { "CALL" } else { "JMP" },
                            target_addr, offset, call_target, tramp_insn_end, name
                        ));
                    }

                    std::ptr::write_unaligned(code.add(offset + 1) as *mut i32, new_rel32 as i32);
                    debug::debug(format!(
                        "  Relocated {} at +{}: rel32 0x{:08x} -> 0x{:08x} (target 0x{:08x})",
                        if op == 0xE8 { "CALL" } else { "JMP" },
                        offset,
                        orig_rel32,
                        new_rel32,
                        call_target
                    ));
                }

                offset += insn_len;
            }
        }

        // Append JMP rel32 back to original function (past the copied prologue)
        let return_addr = target_addr + prologue_len;
        let jmp_from = code as usize + prologue_len + 5;
        let rel32 = (return_addr as isize - jmp_from as isize) as i32;

        *code.add(prologue_len) = 0xE9;
        std::ptr::write_unaligned(code.add(prologue_len + 1) as *mut i32, rel32);

        Ok(Trampoline {
            code,
            size: alloc_size.max(page_size()),
            prologue_len,
        })
    }

    fn addr(&self) -> usize {
        self.code as usize
    }
}

impl Drop for Trampoline {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.code as *mut libc::c_void, self.size);
        }
    }
}

struct InlineHook {
    trampoline: Trampoline,
    saved_bytes: Vec<u8>,
    target_addr: usize,
}

enum FnHook {
    Direct(Mutex<Option<InlineHook>>),
    Indirect(Mutex<Option<usize>>),
}

impl FnHook {
    fn is_hooked(&self) -> bool {
        match self {
            FnHook::Direct(mutex) => mutex.lock().unwrap().is_some(),
            FnHook::Indirect(mutex) => mutex.lock().unwrap().is_some(),
        }
    }
}

pub struct BoundFn<F> {
    pub name: &'static str,
    pub addr: Mutex<usize>,
    hook: FnHook,
    pub symbol: Option<&'static str>,
    original_fn_cache: AtomicUsize,
    fn_type: PhantomData<F>,
}

unsafe impl<F> Sync for BoundFn<F> {}
unsafe impl<F> Send for BoundFn<F> {}

impl<F> BoundFn<F> {
    pub const fn direct(name: &'static str, symbol: Option<&'static str>) -> BoundFn<F> {
        BoundFn {
            name,
            addr: Mutex::new(0),
            hook: FnHook::Direct(Mutex::new(None)),
            symbol,
            original_fn_cache: AtomicUsize::new(0),
            fn_type: PhantomData,
        }
    }

    pub const fn indirect(name: &'static str) -> BoundFn<F> {
        BoundFn {
            name,
            addr: Mutex::new(0),
            hook: FnHook::Indirect(Mutex::new(None)),
            symbol: None,
            original_fn_cache: AtomicUsize::new(0),
            fn_type: PhantomData,
        }
    }

    pub fn get_addr(&self) -> usize {
        *self.addr.lock().unwrap()
    }

    pub fn bind(&self, addr: usize) -> Result<(), BindError> {
        let mut addr_guard = self.addr.lock().unwrap();
        if *addr_guard == 0 {
            *addr_guard = addr;
            let cache_addr = match &self.hook {
                FnHook::Direct(_) => addr,
                FnHook::Indirect(_) => unsafe { *(addr as *const usize) },
            };
            self.original_fn_cache.store(cache_addr, Ordering::Release);
            Ok(())
        } else {
            Err(BindError::AlreadyBound(self.name.to_string()))
        }
    }

    pub fn bind_dlsym(&self, sym_name: &str) -> Result<(), BindError> {
        let addr = crate::raw::process::dlsym_lookup(sym_name).ok_or_else(|| self.not_found())?;
        self.bind(addr)
    }

    pub fn bind_elf_symbol(&self, sym_name: &str) -> Result<(), BindError> {
        let addr =
            crate::raw::process::elf_symbol_lookup(sym_name).ok_or_else(|| self.not_found())?;
        self.bind(addr)
    }

    pub fn bind_got_entry(&self, sym_name: &str) -> Result<(), BindError> {
        if let Some(addr) = crate::raw::process::got_entry_lookup(sym_name) {
            return self.bind(addr);
        }
        if let Some(addr) = crate::raw::process::dlsym_lookup(sym_name) {
            return self.bind(addr);
        }
        if let Some(addr) = crate::raw::process::elf_symbol_lookup(sym_name) {
            return self.bind(addr);
        }
        Err(self.not_found())
    }

    pub fn bind_symbol(&self, sym_name: &str) -> Result<(), BindError> {
        if let Some(addr) = crate::raw::process::dlsym_lookup(sym_name) {
            return self.bind(addr);
        }
        if let Some(addr) = crate::raw::process::elf_symbol_lookup(sym_name) {
            return self.bind(addr);
        }
        Err(self.not_found())
    }

    pub fn hook(&self, replacement: F) -> Result<(), HookError> {
        let addr = self.get_addr();
        if addr == 0 {
            return Err(HookError::Unbound(self.name.to_string()));
        }

        if self.hook.is_hooked() {
            return Err(self.already_hooked());
        }

        let replacement_addr = unsafe { *(&replacement as *const F as *const usize) };

        match &self.hook {
            FnHook::Direct(mutex) => unsafe {
                let trampoline = Trampoline::new(addr, self.name)
                    .map_err(|e| HookError::Hook(self.name.to_string(), e))?;

                let prologue_len = trampoline.prologue_len;

                let mut saved_bytes = vec![0u8; prologue_len];
                std::ptr::copy_nonoverlapping(
                    addr as *const u8,
                    saved_bytes.as_mut_ptr(),
                    prologue_len,
                );

                make_writable(addr, prologue_len)
                    .map_err(|e| HookError::Hook(self.name.to_string(), e))?;

                let jmp_from = addr + JMP_PATCH_SIZE;
                let rel32 = (replacement_addr as isize - jmp_from as isize) as i32;
                *(addr as *mut u8) = 0xE9;
                std::ptr::write_unaligned((addr + 1) as *mut i32, rel32);

                for i in JMP_PATCH_SIZE..prologue_len {
                    *(addr as *mut u8).add(i) = 0x90;
                }

                let trampoline_addr = trampoline.addr();
                *mutex.lock().unwrap() = Some(InlineHook {
                    trampoline,
                    saved_bytes,
                    target_addr: addr,
                });
                self.original_fn_cache
                    .store(trampoline_addr, Ordering::Release);
            },
            FnHook::Indirect(mutex) => unsafe {
                let original_addr = *(addr as *const usize);
                write(addr, replacement_addr);
                *mutex.lock().unwrap() = Some(original_addr);
                self.original_fn_cache
                    .store(original_addr, Ordering::Release);
            },
        }

        Ok(())
    }

    pub fn unhook(&self) -> Result<(), UnhookError> {
        match &self.hook {
            FnHook::Direct(mutex) => {
                let hook = mutex
                    .lock()
                    .unwrap()
                    .take()
                    .ok_or_else(|| self.not_hooked())?;

                unsafe {
                    let len = hook.saved_bytes.len();
                    make_writable(hook.target_addr, len)
                        .map_err(|e| UnhookError::Hook(self.name.to_string(), e))?;
                    std::ptr::copy_nonoverlapping(
                        hook.saved_bytes.as_ptr(),
                        hook.target_addr as *mut u8,
                        len,
                    );
                }
                self.original_fn_cache
                    .store(hook.target_addr, Ordering::Release);
                Ok(())
            }
            FnHook::Indirect(mutex) => {
                let original_addr = mutex
                    .lock()
                    .unwrap()
                    .take()
                    .ok_or_else(|| self.not_hooked())?;
                unsafe { write(self.get_addr(), original_addr) };
                self.original_fn_cache
                    .store(original_addr, Ordering::Release);
                Ok(())
            }
        }
    }

    pub fn original_fn_addr(&self) -> Option<usize> {
        let addr = self.get_addr();
        let unhooked = || (addr != 0).then_some(addr);
        match &self.hook {
            FnHook::Direct(mutex) => mutex
                .lock()
                .unwrap()
                .as_ref()
                .map(|hook| hook.trampoline.addr())
                .or_else(unhooked),
            FnHook::Indirect(mutex) => (*mutex.lock().unwrap())
                .or_else(|| unhooked().map(|addr| unsafe { *(addr as *const usize) })),
        }
    }

    #[inline(always)]
    pub fn original_fn_addr_fast(&self) -> usize {
        self.original_fn_cache.load(Ordering::Acquire)
    }

    pub fn original_fn_addr_or_panic(&self) -> usize {
        let cached = self.original_fn_addr_fast();
        if cached != 0 {
            return cached;
        }
        if let Some(f) = self.original_fn_addr() {
            f
        } else {
            let error = format!("Tried to call unbound function '{}'", self.name);
            debug::error(&error);
            panic!("{}", &error)
        }
    }

    pub fn not_found(&self) -> BindError {
        BindError::NotFound(self.name.to_string())
    }

    pub fn already_hooked(&self) -> HookError {
        HookError::AlreadyHooked(self.name.to_string())
    }

    pub fn not_hooked(&self) -> UnhookError {
        UnhookError::NotHooked(self.name.to_string())
    }
}

pub enum BindError {
    AlreadyBound(String),
    NotFound(String),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindError::AlreadyBound(func) => {
                write!(f, "Tried to find '{}' but it has already been found", func)
            }
            BindError::NotFound(func) => {
                write!(f, "Could not find '{}'", func)
            }
        }
    }
}

pub enum HookError {
    Unbound(String),
    AlreadyHooked(String),
    Hook(String, String),
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookError::Unbound(func) => write!(f, "Tried to hook '{}' before it was found", func),
            HookError::AlreadyHooked(func) => {
                write!(f, "Tried to hook '{}' while already hooked", func)
            }
            HookError::Hook(func, err) => {
                write!(f, "Low-level error while hooking '{}': {}", func, err)
            }
        }
    }
}

pub enum UnhookError {
    NotHooked(String),
    Hook(String, String),
}

impl std::fmt::Display for UnhookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnhookError::NotHooked(name) => write!(f, "{} is not hooked", name),
            UnhookError::Hook(name, err) => write!(f, "failed to unhook {}: {}", name, err),
        }
    }
}

macro_rules! impl_bound_fn_traits {
    ($($T:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($T,)* R> FnOnce<($($T,)*)> for BoundFn<extern "C" fn($($T,)*) -> R> {
            type Output = R;

            extern "rust-call" fn call_once(self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern "C" fn($($T,)*) -> R = unsafe { std::mem::transmute(self.original_fn_addr_or_panic()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T,)* R> FnMut<($($T,)*)> for BoundFn<extern "C" fn($($T,)*) -> R> {
            extern "rust-call" fn call_mut(&mut self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern "C" fn($($T,)*) -> R = unsafe { std::mem::transmute(self.original_fn_addr_or_panic()) };
                (f)($($T,)*)
            }
        }

        #[allow(non_snake_case)]
        impl<$($T,)* R> Fn<($($T,)*)> for BoundFn<extern "C" fn($($T,)*) -> R> {
            extern "rust-call" fn call(&self, ($($T,)*): ($($T,)*)) -> R {
                let f: extern "C" fn($($T,)*) -> R = unsafe { std::mem::transmute(self.original_fn_addr_or_panic()) };
                (f)($($T,)*)
            }
        }
    };
}

impl_bound_fn_traits!();
impl_bound_fn_traits!(A);
impl_bound_fn_traits!(A, B);
impl_bound_fn_traits!(A, B, C);
impl_bound_fn_traits!(A, B, C, D);
impl_bound_fn_traits!(A, B, C, D, E);
impl_bound_fn_traits!(A, B, C, D, E, F);
impl_bound_fn_traits!(A, B, C, D, E, F, G);
impl_bound_fn_traits!(A, B, C, D, E, F, G, H);
impl_bound_fn_traits!(A, B, C, D, E, F, G, H, I);
impl_bound_fn_traits!(A, B, C, D, E, F, G, H, I, J);

// ---------- Static values ----------

pub struct Value<T, F: 'static> {
    name: &'static str,
    source: ValueSource<F>,
    addr: OnceLock<usize>,
    value_type: PhantomData<T>,
}

enum ValueSource<F: 'static> {
    Symbol(&'static str),
    Relative {
        relative_to: &'static BoundFn<F>,
        offset: usize,
    },
}

unsafe impl<T, F> Sync for Value<T, F> {}
unsafe impl<T, F> Send for Value<T, F> {}

impl<T, F> Value<T, F> {
    pub const fn from_symbol(name: &'static str, symbol: &'static str) -> Value<T, F> {
        Value {
            name,
            source: ValueSource::Symbol(symbol),
            addr: OnceLock::new(),
            value_type: PhantomData,
        }
    }

    pub const fn relative(
        name: &'static str,
        relative_to: &'static BoundFn<F>,
        offset: usize,
    ) -> Value<T, F> {
        Value {
            name,
            source: ValueSource::Relative {
                relative_to,
                offset,
            },
            addr: OnceLock::new(),
            value_type: PhantomData,
        }
    }

    pub fn addr(&self) -> usize {
        *self.addr.get_or_init(|| {
            let addr = match &self.source {
                ValueSource::Symbol(sym_name) => crate::raw::process::dlsym_lookup(sym_name)
                    .or_else(|| crate::raw::process::elf_symbol_lookup(sym_name))
                    .unwrap_or_else(|| {
                        panic!(
                            "Could not resolve symbol '{}' for value '{}'",
                            sym_name, self.name
                        )
                    }),
                ValueSource::Relative {
                    relative_to,
                    offset,
                } => {
                    let ref_addr = relative_to.get_addr() + offset;
                    unsafe { std::ptr::read(ref_addr as *const usize) }
                }
            };
            if debug::verbose() {
                debug::info(format!(
                    "Found address for static {}: 0x{:x}",
                    self.name, addr
                ));
            }
            addr
        })
    }
}

impl<T, F> Value<T, F> {
    pub unsafe fn as_ref<'a>(&self) -> Option<&'a T> {
        (self.addr() as *const T).as_ref()
    }
}

impl<T: Clone, F> Value<T, F> {
    pub unsafe fn get(&self) -> T {
        self.as_ref().cloned().unwrap()
    }
}

impl<T, F> Value<*const T, F> {
    pub unsafe fn inner_addr(&self) -> usize {
        read::<usize>(self.addr())
    }

    pub unsafe fn inner_ref<'a>(&self) -> Option<&'a T> {
        let addr = self.inner_addr();
        (addr as *const T).as_ref()
    }
}
