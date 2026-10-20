//! Minimal x86-64 assembler for plugin code caves.

use std::collections::HashMap;

/// Just enough of an assembler for the cave: raw bytes plus rel32 fix-ups.
pub struct Asm {
    base: usize,
    code: Vec<u8>,
    labels: HashMap<&'static str, usize>,
    /// (offset of the rel32 field, label it must reach)
    pending: Vec<(usize, &'static str)>,
}

impl Asm {
    pub fn new(base: usize) -> Asm {
        Asm { base, code: Vec::new(), labels: HashMap::new(), pending: Vec::new() }
    }

    pub fn raw(&mut self, bytes: &[u8]) {
        self.code.extend(bytes);
    }

    /// Bytes emitted so far: the offset of the next instruction.
    pub fn len(&self) -> usize {
        self.code.len()
    }

    pub fn label(&mut self, name: &'static str) {
        self.labels.insert(name, self.code.len());
    }

    /// `opcode rel32` to a label.
    pub fn jump(&mut self, opcode: &[u8], label: &'static str) {
        self.code.extend(opcode);
        self.pending.push((self.code.len(), label));
        self.code.extend([0; 4]);
    }

    /// `opcode rel32` to an absolute address.
    pub fn absolute(&mut self, opcode: &[u8], target: usize) {
        self.rip(opcode, target, &[]);
    }

    /// `opcode disp32 trailing`, rip-relative to an absolute address. The
    /// displacement counts from the end of the whole instruction.
    pub fn rip(&mut self, opcode: &[u8], target: usize, trailing: &[u8]) {
        self.code.extend(opcode);
        let end = self.base + self.code.len() + 4 + trailing.len();
        self.code.extend(((target as i64 - end as i64) as i32).to_le_bytes());
        self.code.extend(trailing);
    }

    pub fn finish(mut self) -> Vec<u8> {
        for (field, label) in self.pending {
            let rel = (self.labels[label] as i64 - (field as i64 + 4)) as i32;
            self.code[field..field + 4].copy_from_slice(&rel.to_le_bytes());
        }
        self.code
    }
}
