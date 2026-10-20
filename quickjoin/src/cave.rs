//! A detour that calls Rust with every register: the cave saves the flags,
//! all general registers and xmm0-5, calls a function with a pointer to the
//! saved registers (which it may change), restores them, runs the overwritten
//! instructions and jumps back. While the flag byte is 0 it skips the call.

use shroudkit_api::{Kit, asm::Asm};

/// Registers as the cave pushed them (lowest address first). Writes to these
/// fields are restored into the registers.
#[repr(C)]
pub struct Regs {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    pub flags: u64,
}

pub type Hook = extern "C" fn(*mut Regs);

/// Cave layout: function pointer, flag byte, code.
const FUNCTION: usize = 0;
const FLAG: usize = 8;
const CODE: usize = 16;
const CAVE_SIZE: usize = 256;

/// Detours `site`, whose first bytes must be `original` (whole instructions,
/// position independent). Returns the jump patch (created disabled) and the
/// address of the flag byte.
pub fn install(kit: &Kit, what: &str, site: usize, original: &[u8], function: Hook) -> Result<(shroudkit_api::Patch, usize), String> {
    let actual = unsafe { std::slice::from_raw_parts(site as *const u8, original.len()) };
    if actual != original {
        return Err(format!("{what}: unexpected bytes at exe+{:#x}", site - kit.module_base()));
    }
    let cave = kit.alloc_near(site, CAVE_SIZE)?;
    let mut image = vec![0u8; CODE];
    image[FUNCTION..FUNCTION + 8].copy_from_slice(&(function as usize).to_le_bytes());
    image.extend(assemble(cave, original, site + original.len()));
    if image.len() > CAVE_SIZE || !kit.write_memory(cave, &image) {
        return Err(format!("{what}: could not write the code cave"));
    }
    let mut entry = shroudkit_api::jmp_rel32(site, cave + CODE).to_vec();
    entry.resize(original.len(), 0x90);
    Ok((kit.patch(site, &entry)?, cave + FLAG))
}

/// The cave code, assembled to run at `cave + CODE`.
fn assemble(cave: usize, original: &[u8], back: usize) -> Vec<u8> {
    let mut asm = Asm::new(cave + CODE);
    asm.rip(&[0x80, 0x3D], cave + FLAG, &[0]); //                 cmp byte [flag], 0
    asm.jump(&[0x0F, 0x84], "original"); //                        je original
    asm.raw(&[0x9C]); //                                           pushfq
    for push in [&[0x50][..], &[0x51], &[0x52], &[0x53], &[0x55], &[0x56], &[0x57]] {
        asm.raw(push); //                                          push rax, rcx, rdx, rbx, rbp, rsi, rdi
    }
    for reg in 0..8u8 {
        asm.raw(&[0x41, 0x50 + reg]); //                           push r8 .. r15
    }
    asm.raw(&[0x48, 0x8B, 0xEC]); //                               mov rbp, rsp          ; rbp = &Regs
    asm.raw(&[0x48, 0x83, 0xEC, 0x60]); //                         sub rsp, 60h
    asm.raw(&[0x48, 0x83, 0xE4, 0xF0]); //                         and rsp, -16
    for xmm in 0..6u8 {
        asm.raw(&[0x0F, 0x29, 0x44 | (xmm << 3), 0x24, xmm * 16]); // movaps [rsp+16*N], xmmN
    }
    asm.raw(&[0x48, 0x83, 0xEC, 0x20]); //                         sub rsp, 20h          ; shadow space
    asm.raw(&[0x48, 0x8B, 0xCD]); //                               mov rcx, rbp
    asm.rip(&[0xFF, 0x15], cave + FUNCTION, &[]); //               call [function]
    asm.raw(&[0x48, 0x83, 0xC4, 0x20]); //                         add rsp, 20h
    for xmm in 0..6u8 {
        asm.raw(&[0x0F, 0x28, 0x44 | (xmm << 3), 0x24, xmm * 16]); // movaps xmmN, [rsp+16*N]
    }
    asm.raw(&[0x48, 0x8B, 0xE5]); //                               mov rsp, rbp
    for reg in (0..8u8).rev() {
        asm.raw(&[0x41, 0x58 + reg]); //                           pop r15 .. r8
    }
    for pop in [&[0x5F][..], &[0x5E], &[0x5D], &[0x5B], &[0x5A], &[0x59], &[0x58]] {
        asm.raw(pop); //                                           pop rdi, rsi, rbp, rbx, rdx, rcx, rax
    }
    asm.raw(&[0x9D]); //                                           popfq
    asm.label("original");
    asm.raw(original); //                                          the overwritten instructions
    asm.absolute(&[0xE9], back); //                                jmp back
    asm.finish()
}

#[cfg(test)]
mod tests {
    /// Printed for checking with capstone: cave at 0x10000000.
    #[test]
    fn dump_cave() {
        let code = super::assemble(0x1000_0000, &[0x48, 0x8B, 0x85, 0x80, 0x00, 0x00, 0x00], 0x1000_9007);
        let hex: Vec<String> = code.iter().map(|byte| format!("{byte:02x}")).collect();
        println!("CAVE {}", hex.join(""));
    }
}
