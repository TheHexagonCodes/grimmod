// Core hooking and function binding infrastructure for macOS (x86_64).
//
// Direct hooks use inline patching: overwrite the function prologue with a
// JMP to the replacement, saving the original bytes for a trampoline.
// Indirect hooks overwrite Mach-O lazy/non-lazy symbol pointer entries
// (equivalent of GOT on ELF).
//
// Key differences from the Linux (i386) version:
// - x86-64 instruction decoder (REX prefixes, RIP-relative addressing)
// - 64-bit trampolines: JMP rel32 can only reach +/-2GB, so we allocate
//   trampoline memory within 2GB of the target and use indirect jumps
//   (`JMP [RIP+0]; .quad addr`) for the final jump back when needed.
// - macOS MAP_JIT flag for mmap'd executable memory (W^X policy)
// - No __errno_location — use libc::__error() on macOS for errno

use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::debug;

/// Page size — always 4KB on x86_64 macOS.
fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

/// Size of the inline JMP patch.
/// On x86-64, a `JMP rel32` is still 5 bytes but can only reach +/-2GB.
/// We use `JMP rel32` when the replacement is within 2GB, otherwise we use
/// an indirect jump: `FF 25 00 00 00 00 <8-byte addr>` (14 bytes total).
const JMP_REL32_SIZE: usize = 5;
const JMP_ABS64_SIZE: usize = 14; // FF 25 00 00 00 00 + 8 bytes

/// Maximum prologue bytes we might need to copy (generous upper bound).
const MAX_PROLOGUE_COPY: usize = 24;

/// 2GB range for rel32 addressing.
const REL32_RANGE: isize = 0x7FFF_FFFF;

/// Determine the patch size needed to jump from `from` to `to`.
fn jmp_patch_size(from: usize, to: usize) -> usize {
    let distance = (to as isize).wrapping_sub(from as isize + JMP_REL32_SIZE as isize);
    if distance.abs() <= REL32_RANGE {
        JMP_REL32_SIZE
    } else {
        JMP_ABS64_SIZE
    }
}

// ---------- x86-64 instruction length decoder ----------

/// Decode the length of a single x86-64 instruction at `code`.
///
/// Returns `Some(len)` for recognized instructions, `None` for unknown opcodes.
///
/// Key x86-64 differences from x86-32:
/// - REX prefixes (0x40-0x4F) are no longer INC/DEC r32
/// - RIP-relative addressing mode (ModRM mod=00, r/m=101 means [RIP+disp32])
/// - Default operand size is 32-bit; REX.W makes it 64-bit
/// - Address size is 64-bit by default
///
/// # Safety
/// `code` must point to at least 24 readable bytes.
unsafe fn x86_64_insn_len(code: *const u8) -> Option<usize> {
    let mut offset = 0usize;

    // --- Legacy prefixes ---
    loop {
        let b = *code.add(offset);
        match b {
            // Group 1: LOCK, REP, REPNE
            0xF0 | 0xF2 | 0xF3 |
            // Group 2: segment overrides (mostly ignored in 64-bit mode) + branch hints
            0x2E | 0x36 | 0x3E | 0x26 | 0x64 | 0x65 |
            // Group 3: operand-size override
            0x66 |
            // Group 4: address-size override
            0x67 => {
                offset += 1;
            }
            _ => break,
        }
    }

    // --- REX prefix (0x40-0x4F) ---
    let mut has_rex = false;
    let _rex_w;
    let b = *code.add(offset);
    if b >= 0x40 && b <= 0x4F {
        has_rex = true;
        _rex_w = (b & 0x08) != 0;
        offset += 1;
    } else {
        _rex_w = false;
    }

    let op = *code.add(offset);
    let opcode_start = offset;
    offset += 1;

    match op {
        // Single-byte instructions
        0x50..=0x57 => Some(offset - opcode_start + opcode_start), // PUSH reg
        0x58..=0x5F => Some(offset),                               // POP reg
        0x90 => Some(offset),                                      // NOP
        0xC3 => Some(offset),                                      // RET
        0xCC => Some(offset),                                      // INT3
        0xF4 => Some(offset),                                      // HLT
        0x99 => Some(offset),                                      // CDQ/CQO
        0x9C => Some(offset),                                      // PUSHFQ
        0x9D => Some(offset),                                      // POPFQ
        0xC9 => Some(offset),                                      // LEAVE
        0x98 => Some(offset),                                      // CBW/CWDE/CDQE

        // CALL rel32, JMP rel32
        0xE8 | 0xE9 => Some(offset + 4),

        // JMP rel8, Jcc rel8
        0xEB => Some(offset + 1),
        0x70..=0x7F => Some(offset + 1), // Jcc short

        // MOV reg, imm32 (REX.W extends to imm64 for 0xB8+rd)
        0xB8..=0xBF => {
            if _rex_w {
                Some(offset + 8) // MOV r64, imm64
            } else {
                Some(offset + 4) // MOV r32, imm32
            }
        }

        // MOV r8, imm8
        0xB0..=0xB7 => Some(offset + 1),

        // PUSH imm8
        0x6A => Some(offset + 1),
        // PUSH imm32
        0x68 => Some(offset + 4),

        // Two-byte opcode prefix (0x0F)
        0x0F => {
            let op2 = *code.add(offset);
            offset += 1;
            match op2 {
                // Jcc rel32 (near conditional jumps)
                0x80..=0x8F => Some(offset + 4),
                // MOVAPS/MOVUPS xmm, xmm/m128 or reverse
                0x28 | 0x29 | 0x10 | 0x11 => Some(offset + modrm_extra_len_64(code.add(offset))),
                // CMOVcc r, r/m
                0x40..=0x4F => Some(offset + modrm_extra_len_64(code.add(offset))),
                // SETcc r/m8
                0x90..=0x9F => Some(offset + modrm_extra_len_64(code.add(offset))),
                // MOVZX r, r/m8 / MOVZX r, r/m16
                0xB6 | 0xB7 => Some(offset + modrm_extra_len_64(code.add(offset))),
                // MOVSX r, r/m8 / MOVSX r, r/m16
                0xBE | 0xBF => Some(offset + modrm_extra_len_64(code.add(offset))),
                // NOP r/m (multi-byte NOP)
                0x1F => Some(offset + modrm_extra_len_64(code.add(offset))),
                // IMUL r, r/m
                0xAF => Some(offset + modrm_extra_len_64(code.add(offset))),
                // XORPS/ANDPS/ORPS etc.
                0x57 | 0x54 | 0x56 => Some(offset + modrm_extra_len_64(code.add(offset))),
                // Other 0F xx with ModRM — default handler
                _ => Some(offset + modrm_extra_len_64(code.add(offset))),
            }
        }

        // Instructions with ModR/M byte (ALU r/m, r / r, r/m)
        0x00..=0x03
        | 0x08..=0x0B
        | 0x10..=0x13
        | 0x18..=0x1B
        | 0x20..=0x23
        | 0x28..=0x2B
        | 0x30..=0x33
        | 0x38..=0x3B => Some(offset + modrm_extra_len_64(code.add(offset))),

        // ALU AL, imm8
        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => Some(offset + 1),
        // ALU rAX, imm32
        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => Some(offset + 4),

        // TEST r/m, r
        0x84 | 0x85 => Some(offset + modrm_extra_len_64(code.add(offset))),

        // XCHG, MOV r/m,r / r,r/m
        0x86..=0x8B => Some(offset + modrm_extra_len_64(code.add(offset))),

        // LEA r, m
        0x8D => Some(offset + modrm_extra_len_64(code.add(offset))),

        // MOV r/m, imm
        0xC6 => Some(offset + modrm_extra_len_64(code.add(offset)) + 1), // + imm8
        0xC7 => {
            // With REX.W, still imm32 (sign-extended to 64-bit)
            Some(offset + modrm_extra_len_64(code.add(offset)) + 4)
        }

        // Group 1: op r/m, imm8
        0x80 | 0x83 => Some(offset + modrm_extra_len_64(code.add(offset)) + 1),
        // Group 1: op r/m, imm32
        0x81 => Some(offset + modrm_extra_len_64(code.add(offset)) + 4),

        // Shift/rotate group
        0xC0 => Some(offset + modrm_extra_len_64(code.add(offset)) + 1),
        0xC1 => Some(offset + modrm_extra_len_64(code.add(offset)) + 1),
        0xD0..=0xD3 => Some(offset + modrm_extra_len_64(code.add(offset))),

        // TEST rAX, imm32
        0xA9 => Some(offset + 4),
        // TEST AL, imm8
        0xA8 => Some(offset + 1),

        // MOV AL/rAX, moffs64 / MOV moffs64, AL/rAX
        // In 64-bit mode these use 8-byte absolute addresses (with 0x67 prefix: 4 bytes)
        0xA0 | 0xA1 | 0xA2 | 0xA3 => Some(offset + 8),

        // FF group (INC/DEC/CALL/JMP r/m64 etc.)
        0xFF => Some(offset + modrm_extra_len_64(code.add(offset))),

        // NOT, NEG, MUL, IMUL, DIV, IDIV (F6/F7 group)
        0xF6 => {
            let modrm = *code.add(offset);
            let reg = (modrm >> 3) & 7;
            let extra = modrm_extra_len_64(code.add(offset));
            if reg == 0 || reg == 1 {
                Some(offset + extra + 1) // TEST r/m8, imm8
            } else {
                Some(offset + extra)
            }
        }
        0xF7 => {
            let modrm = *code.add(offset);
            let reg = (modrm >> 3) & 7;
            let extra = modrm_extra_len_64(code.add(offset));
            if reg == 0 || reg == 1 {
                Some(offset + extra + 4) // TEST r/m32, imm32
            } else {
                Some(offset + extra)
            }
        }

        // RET imm16
        0xC2 => Some(offset + 2),

        // 0x40..=0x4F in 64-bit mode are REX prefixes, handled above.
        // If we reach here without has_rex it means something is wrong.
        // But we already consumed REX above, so these shouldn't appear.
        _ => {
            // Before giving up, check if this might be a VEX-prefixed instruction etc.
            // For now, return None for unrecognized opcodes.
            let _ = has_rex;
            None
        }
    }
}

/// Calculate extra bytes consumed by a ModR/M byte (and optional SIB + displacement)
/// using x86-64 addressing rules.
///
/// Key difference from x86-32: in mod=00, r/m=101 means [RIP+disp32] instead of [disp32].
/// The byte count is the same (ModRM + disp32), but semantics differ for relocation.
///
/// # Safety
/// `modrm_ptr` must point to at least 6 readable bytes.
unsafe fn modrm_extra_len_64(modrm_ptr: *const u8) -> usize {
    let modrm = *modrm_ptr;
    let mode = modrm >> 6;
    let rm = modrm & 7;

    match mode {
        0b00 => {
            if rm == 0b100 {
                // SIB byte follows
                let sib = *modrm_ptr.add(1);
                let base = sib & 7;
                if base == 0b101 {
                    1 + 1 + 4 // ModRM + SIB + disp32
                } else {
                    1 + 1 // ModRM + SIB
                }
            } else if rm == 0b101 {
                // RIP-relative: [RIP + disp32]
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

/// Calculate prologue copy length (at least `min_patch_size`, instruction-aligned).
///
/// # Safety
/// `addr` must point to readable executable memory with at least MAX_PROLOGUE_COPY bytes.
unsafe fn prologue_copy_len(addr: usize, min_patch_size: usize) -> Result<usize, String> {
    let code = addr as *const u8;
    let mut offset = 0usize;

    while offset < min_patch_size {
        if offset >= MAX_PROLOGUE_COPY {
            return Err(format!(
                "Prologue at 0x{:016x} too long to copy (>{} bytes without reaching patch size {})",
                addr, MAX_PROLOGUE_COPY, min_patch_size
            ));
        }
        match x86_64_insn_len(code.add(offset)) {
            Some(len) => offset += len,
            None => {
                return Err(format!(
                    "Unknown x86-64 opcode 0x{:02x} at 0x{:016x}+{} while scanning prologue",
                    *code.add(offset),
                    addr,
                    offset
                ));
            }
        }
    }

    Ok(offset)
}

/// Make a memory region writable+executable using mach_vm_protect.
///
/// On macOS, the kernel enforces W^X on signed executable pages.
/// `mprotect(PROT_WRITE | PROT_EXEC)` fails with EACCES on `__TEXT` segments.
/// We must use `mach_vm_protect` with the `VM_PROT_COPY` flag, which triggers
/// a copy-on-write of the page, allowing us to make it writable.
///
/// # Safety
/// Caller must ensure `addr` and `len` describe a valid memory region.
unsafe fn make_writable(addr: usize, len: usize) -> Result<(), String> {
    let ps = page_size();
    let page_start = addr & !(ps - 1);
    let page_end = (addr + len + ps - 1) & !(ps - 1);
    let size = page_end - page_start;

    let task = mach2::traps::mach_task_self();
    let prot = mach2::vm_prot::VM_PROT_READ
        | mach2::vm_prot::VM_PROT_WRITE
        | mach2::vm_prot::VM_PROT_EXECUTE
        | mach2::vm_prot::VM_PROT_COPY;

    let kr = mach2::vm::mach_vm_protect(
        task,
        page_start as mach2::vm_types::mach_vm_address_t,
        size as mach2::vm_types::mach_vm_size_t,
        0, // set_maximum = false
        prot,
    );

    if kr != mach2::kern_return::KERN_SUCCESS {
        Err(format!(
            "mach_vm_protect failed at 0x{:016x} (len {}): kern_return {}",
            addr, len, kr
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

/// Try to allocate executable memory near `target_addr` (within +/-2GB).
/// This is needed so that the trampoline's JMP rel32 back to the original
/// function can reach.
///
/// # Safety
/// Uses mmap to allocate memory.
unsafe fn alloc_near(target_addr: usize, size: usize) -> Result<*mut u8, String> {
    let ps = page_size();
    let alloc_size = size.max(ps);

    // Try hints within +/-2GB of target. Start close and expand outward.
    let low = target_addr.saturating_sub(REL32_RANGE as usize);
    let high = target_addr.saturating_add(REL32_RANGE as usize);

    // Try multiple hint addresses
    for offset_mb in (1..=2048u64).step_by(1) {
        for &dir in &[-1i64, 1i64] {
            let hint = if dir < 0 {
                target_addr.wrapping_sub((offset_mb as usize) * 0x10000)
            } else {
                target_addr.wrapping_add((offset_mb as usize) * 0x10000)
            };
            let hint = hint & !(ps - 1);

            if hint < low || hint > high || hint == 0 {
                continue;
            }

            let code = libc::mmap(
                hint as *mut libc::c_void,
                alloc_size,
                libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_JIT,
                -1,
                0,
            );
            if code == libc::MAP_FAILED {
                continue;
            }

            let code_addr = code as usize;
            // Check it's actually within 2GB
            let distance = (code_addr as isize)
                .wrapping_sub(target_addr as isize)
                .abs();
            if distance > REL32_RANGE {
                libc::munmap(code, alloc_size);
                continue;
            }

            return Ok(code as *mut u8);
        }
    }

    // Last resort: try without hint, might work if address space is compact
    let code = libc::mmap(
        std::ptr::null_mut(),
        alloc_size,
        libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_JIT,
        -1,
        0,
    );
    if code == libc::MAP_FAILED {
        return Err("mmap failed for trampoline (all attempts)".to_string());
    }

    Ok(code as *mut u8)
}

/// Write a `JMP rel32` at `write_addr` targeting `target_addr`.
/// The JMP ends at `write_addr + 5`.
unsafe fn write_jmp_rel32(write_addr: *mut u8, target_addr: usize) {
    let jmp_end = write_addr as usize + JMP_REL32_SIZE;
    let rel32 = (target_addr as isize - jmp_end as isize) as i32;
    *write_addr = 0xE9;
    std::ptr::write_unaligned(write_addr.add(1) as *mut i32, rel32);
}

/// Write an absolute indirect JMP at `write_addr`: `FF 25 00 00 00 00` + 8-byte address.
/// Total: 14 bytes. Used when target is beyond +/-2GB from the jump site.
unsafe fn write_jmp_abs64(write_addr: *mut u8, target_addr: usize) {
    // JMP [RIP+0] — the 8-byte address follows immediately
    *write_addr = 0xFF;
    *write_addr.add(1) = 0x25;
    std::ptr::write_unaligned(write_addr.add(2) as *mut i32, 0); // disp32 = 0
    std::ptr::write_unaligned(write_addr.add(6) as *mut u64, target_addr as u64);
}

/// Write a JMP to `target_addr` at `write_addr`, choosing rel32 or abs64 as needed.
/// Returns the number of bytes written.
unsafe fn write_jmp(write_addr: *mut u8, target_addr: usize) -> usize {
    let distance =
        (target_addr as isize).wrapping_sub(write_addr as usize as isize + JMP_REL32_SIZE as isize);
    if distance.abs() <= REL32_RANGE {
        write_jmp_rel32(write_addr, target_addr);
        JMP_REL32_SIZE
    } else {
        write_jmp_abs64(write_addr, target_addr);
        JMP_ABS64_SIZE
    }
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
    unsafe fn new(target_addr: usize, replacement_addr: usize, name: &str) -> Result<Self, String> {
        let patch_size = jmp_patch_size(target_addr, replacement_addr);
        let prologue_len = prologue_copy_len(target_addr, patch_size)
            .map_err(|e| format!("{} (function: {})", e, name))?;

        // The trampoline needs: prologue bytes + a JMP back (up to 14 bytes)
        let alloc_size = prologue_len + JMP_ABS64_SIZE;
        debug::debug(format!(
            "Trampoline for '{}': copying {} prologue bytes from 0x{:016x} (patch_size={})",
            name, prologue_len, target_addr, patch_size
        ));

        let code = alloc_near(target_addr, alloc_size)
            .map_err(|e| format!("{} (function: {})", e, name))?;

        // macOS MAP_JIT: toggle to writable
        #[cfg(target_os = "macos")]
        libc::pthread_jit_write_protect_np(0); // 0 = writable

        // Copy original prologue bytes
        std::ptr::copy_nonoverlapping(target_addr as *const u8, code, prologue_len);

        // Relocate RIP-relative instructions in the copied prologue
        relocate_rip_relative(code, target_addr, prologue_len);

        // Relocate relative CALL/JMP instructions
        {
            let src = target_addr as *const u8;
            let mut off = 0usize;
            while off < prologue_len {
                let insn_op = *src.add(off);
                let insn_len = x86_64_insn_len(src.add(off)).unwrap();

                if (insn_op == 0xE8 || insn_op == 0xE9) && insn_len >= 5 {
                    // Find where the rel32 is — it's always at op+1 for these
                    // But we need to account for possible prefixes
                    let rel32_off = off + insn_len - 4;
                    let orig_rel32 = std::ptr::read_unaligned(src.add(rel32_off) as *const i32);
                    let call_target = (target_addr + off + insn_len) as isize + orig_rel32 as isize;
                    let tramp_insn_end = code as usize + off + insn_len;
                    let new_rel32 = call_target - tramp_insn_end as isize;

                    if new_rel32 > i32::MAX as isize || new_rel32 < i32::MIN as isize {
                        libc::munmap(code as *mut libc::c_void, alloc_size.max(page_size()));
                        return Err(format!(
                            "Cannot relocate {} at 0x{:016x}+{}: target 0x{:x} too far from trampoline 0x{:x} (function: {})",
                            if insn_op == 0xE8 { "CALL" } else { "JMP" },
                            target_addr, off, call_target, tramp_insn_end, name
                        ));
                    }

                    std::ptr::write_unaligned(code.add(rel32_off) as *mut i32, new_rel32 as i32);
                    debug::debug(format!(
                        "  Relocated {} at +{}: rel32 0x{:08x} -> 0x{:08x} (target 0x{:016x})",
                        if insn_op == 0xE8 { "CALL" } else { "JMP" },
                        off,
                        orig_rel32,
                        new_rel32,
                        call_target
                    ));
                }

                off += insn_len;
            }
        }

        // Append JMP back to original function (past the copied prologue)
        let return_addr = target_addr + prologue_len;
        write_jmp(code.add(prologue_len), return_addr);

        // macOS MAP_JIT: toggle back to executable
        #[cfg(target_os = "macos")]
        libc::pthread_jit_write_protect_np(1); // 1 = executable

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

/// Relocate RIP-relative memory accesses in copied prologue bytes.
///
/// When instructions using [RIP+disp32] are copied from the original location
/// to the trampoline, the disp32 needs adjustment because RIP has changed.
unsafe fn relocate_rip_relative(tramp: *mut u8, orig_addr: usize, prologue_len: usize) {
    let mut off = 0usize;
    while off < prologue_len {
        let insn_start = tramp.add(off);
        let orig_insn = orig_addr + off;
        let len = x86_64_insn_len((orig_addr as *const u8).add(off)).unwrap();

        // Check for RIP-relative addressing: look for ModRM with mod=00, r/m=101
        // Skip past any prefixes and REX to find the opcode and ModRM byte
        let mut prefix_len = 0usize;

        // Skip legacy prefixes
        loop {
            let b = *insn_start.add(prefix_len);
            match b {
                0xF0 | 0xF2 | 0xF3 | 0x2E | 0x36 | 0x3E | 0x26 | 0x64 | 0x65 | 0x66 | 0x67 => {
                    prefix_len += 1;
                }
                _ => break,
            }
        }

        // Skip REX prefix
        let b = *insn_start.add(prefix_len);
        if b >= 0x40 && b <= 0x4F {
            prefix_len += 1;
        }

        let opcode = *insn_start.add(prefix_len);

        // Determine where ModRM byte is
        let modrm_offset = if opcode == 0x0F {
            // Two-byte opcode: ModRM follows the second opcode byte
            prefix_len + 2
        } else {
            // One-byte opcode with ModRM
            prefix_len + 1
        };

        // Check if this instruction has a ModRM byte with RIP-relative addressing
        if modrm_offset < len {
            let modrm = *insn_start.add(modrm_offset);
            let mode = modrm >> 6;
            let rm = modrm & 7;

            // RIP-relative: mod=00, r/m=101
            if mode == 0b00 && rm == 0b101 {
                let disp32_offset = modrm_offset + 1;
                if disp32_offset + 4 <= len {
                    let orig_disp32 =
                        std::ptr::read_unaligned(insn_start.add(disp32_offset) as *const i32);
                    let orig_rip_end = orig_insn + len;
                    let tramp_rip_end = tramp as usize + off + len;
                    let target_addr = (orig_rip_end as isize + orig_disp32 as isize) as usize;
                    let new_disp32 = target_addr as isize - tramp_rip_end as isize;

                    if new_disp32 >= i32::MIN as isize && new_disp32 <= i32::MAX as isize {
                        std::ptr::write_unaligned(
                            insn_start.add(disp32_offset) as *mut i32,
                            new_disp32 as i32,
                        );
                    }
                    // If it doesn't fit, we have a problem, but this is unlikely for
                    // trampolines allocated near the original code.
                }
            }
        }

        off += len;
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

#[allow(dead_code)]
pub struct BoundFn<F> {
    pub name: &'static str,
    pub addr: Mutex<usize>,
    hook: FnHook,
    pub symbol: Option<&'static str>,
    /// Cached original-function address for the fast path.
    /// Set to the trampoline address on hook(), restored to the bound address on unhook(),
    /// 0 when unbound. A single Relaxed atomic load replaces two mutex acquisitions
    /// on every call through the Fn impls (~308 calls/frame for hot-path hooks).
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
            // Pre-populate the fast-path cache.
            // For direct hooks: the original function IS at `addr`.
            // For indirect hooks: `addr` is the GOT/stub slot; the actual
            // function pointer is at *addr. Read it now (safe because the
            // dynamic linker has already resolved the symbol by bind time).
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

    #[allow(dead_code)]
    pub fn bind_dlsym(&self, sym_name: &str) -> Result<(), BindError> {
        let addr = crate::raw::process::dlsym_lookup(sym_name).ok_or_else(|| self.not_found())?;
        self.bind(addr)
    }

    /// Bind by looking up a symbol in the Mach-O symbol table.
    #[allow(dead_code)]
    pub fn bind_macho_symbol(&self, sym_name: &str) -> Result<(), BindError> {
        let addr =
            crate::raw::process::macho_symbol_lookup(sym_name).ok_or_else(|| self.not_found())?;
        self.bind(addr)
    }

    /// Bind via Mach-O lazy/non-lazy symbol pointer table (equivalent of GOT on ELF).
    pub fn bind_stub_entry(&self, sym_name: &str) -> Result<(), BindError> {
        if let Some(addr) = crate::raw::process::stub_pointer_lookup(sym_name) {
            return self.bind(addr);
        }
        if let Some(addr) = crate::raw::process::dlsym_lookup(sym_name) {
            return self.bind(addr);
        }
        if let Some(addr) = crate::raw::process::macho_symbol_lookup(sym_name) {
            return self.bind(addr);
        }
        Err(self.not_found())
    }

    /// Alias for bind_stub_entry — provides API compatibility with the Linux
    /// `bind_got_entry` method so that shared macros work on both platforms.
    pub fn bind_got_entry(&self, sym_name: &str) -> Result<(), BindError> {
        self.bind_stub_entry(sym_name)
    }

    pub fn bind_symbol(&self, sym_name: &str) -> Result<(), BindError> {
        if let Some(addr) = crate::raw::process::dlsym_lookup(sym_name) {
            return self.bind(addr);
        }
        if let Some(addr) = crate::raw::process::macho_symbol_lookup(sym_name) {
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
                let trampoline = Trampoline::new(addr, replacement_addr, self.name)
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

                // Write JMP to replacement
                let bytes_written = write_jmp(addr as *mut u8, replacement_addr);

                // NOP out remaining bytes in the prologue
                for i in bytes_written..prologue_len {
                    *(addr as *mut u8).add(i) = 0x90;
                }

                // Update the fast-path cache to point to the trampoline
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
                // Update the fast-path cache to the saved original function pointer
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
                // Restore cache to the original bound address (no longer hooked)
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
                let bound_addr = self.get_addr();
                unsafe { write(bound_addr, original_addr) };
                // Restore cache: for indirect hooks, the original function is what
                // the GOT/stub pointer now points to again. But the *bound* address
                // is the GOT slot address, and reading through it gives original_addr.
                // Store original_addr since that's what callers need when unhooked.
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

    /// Fast path for the Fn impls: single atomic load, no mutex.
    /// Returns 0 if unbound (caller must check).
    #[inline(always)]
    pub fn original_fn_addr_fast(&self) -> usize {
        self.original_fn_cache.load(Ordering::Acquire)
    }

    pub fn original_fn_addr_or_panic(&self) -> usize {
        // Try fast path first
        let cached = self.original_fn_addr_fast();
        if cached != 0 {
            return cached;
        }
        // Fallback to slow path (shouldn't happen in normal operation)
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

#[allow(dead_code)]
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

    #[allow(dead_code)]
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
                    .or_else(|| crate::raw::process::macho_symbol_lookup(sym_name))
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
