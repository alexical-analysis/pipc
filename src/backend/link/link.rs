use std::collections::HashMap;

use crate::backend::emit::emit::{CompiledFunc, RelocKind};

/// Single-pass linker for pip16 ROM images.
///
/// Functions are laid out sequentially in ROM starting at address 0x0000.
/// Two kinds of relocations are resolved:
///
///   `FuncBase`    — intra-function branch target: add the function's base
///                   address to the local offset already encoded in the LUI+LLI.
///   `ExternalFunc` — cross-function call target: replace the LUI+LLI with the
///                   absolute address of the named function.
pub struct Linker {
    funcs: Vec<CompiledFunc>,
}

impl Linker {
    pub fn new() -> Self {
        Self { funcs: Vec::new() }
    }

    pub fn add(&mut self, func: CompiledFunc) {
        self.funcs.push(func);
    }

    /// Resolve all relocations and produce the flat ROM word stream.
    pub fn link(self) -> Vec<u16> {
        // Pass 1 — build symbol table: name → absolute ROM base address.
        let mut symbol_table: HashMap<String, u16> = HashMap::new();
        let mut cursor = 0u16;
        for func in &self.funcs {
            symbol_table.insert(func.name.clone(), cursor);
            cursor = cursor
                .checked_add(func.words.len() as u16)
                .expect("ROM size exceeds 65535 words");
        }

        // Pass 2 — patch relocations and concatenate.
        let mut rom: Vec<u16> = Vec::with_capacity(cursor as usize);
        cursor = 0;

        for func in self.funcs {
            let base = cursor;
            let mut words = func.words;

            for reloc in &func.relocations {
                let new_addr = match &reloc.kind {
                    RelocKind::FuncBase => {
                        // The emitter stored a local offset; add base to get
                        // the final absolute address.
                        let local = extract_addr(&words, reloc.lui_word_idx);
                        local.checked_add(base).expect("branch target overflows 16-bit address")
                    }
                    RelocKind::ExternalFunc(name) => {
                        *symbol_table.get(name.as_str()).unwrap_or_else(|| {
                            panic!("linker: undefined function '{}'", name)
                        })
                    }
                };
                patch_addr(&mut words, reloc.lui_word_idx, new_addr);
            }

            cursor += words.len() as u16;
            rom.extend_from_slice(&words);
        }

        rom
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Read the 16-bit address encoded in the LUI+LLI pair at `lui_idx`.
///
/// LUI: `opcode(4) | reg(4) | upper8`  — bits [7:0] are the upper address byte
/// LLI: `opcode(4) | reg(4) | lower8`  — bits [7:0] are the lower address byte
fn extract_addr(words: &[u16], lui_idx: usize) -> u16 {
    let upper = words[lui_idx] & 0x00FF;
    let lower = words[lui_idx + 1] & 0x00FF;
    (upper << 8) | lower
}

/// Overwrite the imm8 fields of the LUI+LLI pair at `lui_idx` with `addr`.
fn patch_addr(words: &mut [u16], lui_idx: usize, addr: u16) {
    words[lui_idx]     = (words[lui_idx]     & 0xFF00) | (addr >> 8);
    words[lui_idx + 1] = (words[lui_idx + 1] & 0xFF00) | (addr & 0xFF);
}
