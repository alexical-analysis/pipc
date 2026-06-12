use crate::{
    backend::isel::machine::{MachineFunc, MachineInst, VReg},
    ctx::ctx::GlobalCtx,
};

/// r12 is the dedicated address-scratch register for branch expansion.
/// It must not appear as a live virtual register at emission time.
const SCRATCH: u8 = 12;

// ── Public output types ───────────────────────────────────────────────────────

pub struct CompiledFunc {
    pub name: String,
    /// Encoded u16 instruction words (may contain unpatched 0 placeholders).
    pub words: Vec<u16>,
    /// Addresses inside `words` that the linker must patch.
    pub relocations: Vec<Relocation>,
}

pub struct Relocation {
    /// Index of the LUI word in `CompiledFunc::words`. The LLI word is always
    /// at `lui_word_idx + 1`.
    pub lui_word_idx: usize,
    pub kind: RelocKind,
}

pub enum RelocKind {
    /// Add the owning function's final ROM base address to this LUI+LLI pair.
    /// Used for all intra-function branch targets (Jump, Beq/Bne/Blt, SetCC).
    FuncBase,
    /// Replace this LUI+LLI pair with the named function's ROM address.
    ExternalFunc(String),
}

// ── Emitter ───────────────────────────────────────────────────────────────────

pub struct Emitter<'ctx> {
    #[allow(dead_code)]
    ctx: &'ctx GlobalCtx,
}

impl<'ctx> Emitter<'ctx> {
    pub fn new(ctx: &'ctx GlobalCtx) -> Self {
        Self { ctx }
    }

    /// Encode one machine function into a word stream.
    ///
    /// The output assumes the function is placed at ROM address 0. The linker
    /// patches all `FuncBase` relocations by adding the actual base address,
    /// and fills `ExternalFunc` relocations with the callee's address.
    pub fn emit_func(&self, mfunc: &MachineFunc) -> CompiledFunc {
        // Pass 1 — layout: compute the word offset of each block's first
        // instruction from the start of the function.
        let block_offsets = layout(mfunc);

        // Pass 2 — emission: encode each instruction.
        let mut words = Vec::new();
        let mut relocations = Vec::new();
        for block in &mfunc.blocks {
            for inst in &block.insts {
                emit_inst(inst, &block_offsets, &mut words, &mut relocations);
            }
        }

        CompiledFunc { name: mfunc.name.clone(), words, relocations }
    }
}

// ── Layout pass ───────────────────────────────────────────────────────────────

fn layout(mfunc: &MachineFunc) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(mfunc.blocks.len());
    let mut cursor = 0usize;
    for block in &mfunc.blocks {
        offsets.push(cursor);
        for inst in &block.insts {
            cursor += inst_words(inst);
        }
    }
    offsets
}

/// Number of u16 words this instruction expands to.
fn inst_words(inst: &MachineInst) -> usize {
    match inst {
        // ── Concrete single-word instructions ─────────────────────────────────
        MachineInst::Add  { .. }
        | MachineInst::Sub  { .. }
        | MachineInst::Mul  { .. }
        | MachineInst::Xor  { .. }
        | MachineInst::Nand { .. }
        | MachineInst::Shl  { .. }
        | MachineInst::Shr  { .. }
        | MachineInst::Lw   { .. }
        | MachineInst::Sw   { .. }
        | MachineInst::Jalr { .. }
        | MachineInst::Lui  { .. }
        | MachineInst::Lli  { .. } => 1,

        // ── Pseudos ───────────────────────────────────────────────────────────
        MachineInst::Copy        { .. } => 1,  // ADD dst, src, R0
        MachineInst::Ret                => 1,  // JALR R0, R15, 0
        MachineInst::LoadImm     { .. } => 2,  // LUI + LLI
        MachineInst::LoadFuncAddr { .. } => 2, // LUI + LLI (linker patches)

        // Branches with block targets → LUI + LLI (addr into R12) + B<cc>
        MachineInst::Beq { .. } | MachineInst::Bne { .. } | MachineInst::Blt { .. } => 3,

        // Jump → LUI + LLI + JALR R0, R12, 0
        MachineInst::Jump { .. } => 3,

        // Call → one ADD per arg (up to 4) + JALR R15, callee, 0
        MachineInst::Call { args, .. } => {
            assert!(args.len() <= 4, "Call: stack argument passing for >4 args not yet implemented");
            args.len() + 1
        }

        // Standalone SetCC → LoadImm(2) + addr+B<cc>(3) + Jump(3) + LoadImm(2)
        MachineInst::SetEq  { .. }
        | MachineInst::SetNe  { .. }
        | MachineInst::SetSlt { .. }
        | MachineInst::SetUlt { .. }
        | MachineInst::SetSgt { .. }
        | MachineInst::SetUgt { .. } => 10,

        MachineInst::SpillLoad  { .. }
        | MachineInst::SpillStore { .. } =>
            panic!("SpillLoad/SpillStore reached emitter — prologue pass must run first"),
    }
}

// ── Emission pass ─────────────────────────────────────────────────────────────

fn emit_inst(
    inst: &MachineInst,
    block_offsets: &[usize],
    words: &mut Vec<u16>,
    relocs: &mut Vec<Relocation>,
) {
    let here = words.len(); // word offset of this instruction from function start

    match inst {
        // ── Concrete RRR (opcode | rA | rB | rC) ──────────────────────────────
        MachineInst::Add  { dst, lhs, rhs } => rrr(0x0, r(dst), r(lhs), r(rhs), words),
        MachineInst::Sub  { dst, lhs, rhs } => rrr(0x1, r(dst), r(lhs), r(rhs), words),
        MachineInst::Mul  { dst, lhs, rhs } => rrr(0x2, r(dst), r(lhs), r(rhs), words),
        MachineInst::Xor  { dst, lhs, rhs } => rrr(0x3, r(dst), r(lhs), r(rhs), words),
        MachineInst::Nand { dst, lhs, rhs } => rrr(0x4, r(dst), r(lhs), r(rhs), words),

        // ── Concrete RRI (opcode | rA | rB | imm4) ───────────────────────────
        // imm4: pass raw bits so signed and unsigned immediates encode identically.
        MachineInst::Shl { dst, src, imm } => rri(0x5, r(dst), r(src), *imm,        words),
        MachineInst::Shr { dst, src, imm } => rri(0x6, r(dst), r(src), *imm,        words),
        MachineInst::Lw  { dst, base, offset } => rri(0xA, r(dst), r(base), *offset as u8, words),
        // SW: Mem[R[B]+imm] = R[A]  →  rA=src, rB=base
        MachineInst::Sw  { base, src, offset } => rri(0x9, r(src), r(base), *offset as u8, words),
        MachineInst::Jalr { dst, base, offset } => rri(0xB, r(dst), r(base), *offset as u8, words),

        // ── Concrete RI (opcode | rA | imm8) ─────────────────────────────────
        MachineInst::Lui { dst, imm } => ri(0x7, r(dst), *imm, words),
        MachineInst::Lli { dst, imm } => ri(0x8, r(dst), *imm, words),

        // ── Concrete branches with block targets ──────────────────────────────
        // Load target address into R12 via LUI+LLI, then branch.
        // BEQ/BNE/BLT encoding (RRR): rA=target_reg, rB=lhs, rC=rhs.
        MachineInst::Beq { lhs, rhs, target } => {
            load_func_local(SCRATCH, block_offsets[target.0 as usize], words, relocs);
            rrr(0xC, SCRATCH, r(lhs), r(rhs), words);
        }
        MachineInst::Bne { lhs, rhs, target } => {
            load_func_local(SCRATCH, block_offsets[target.0 as usize], words, relocs);
            rrr(0xD, SCRATCH, r(lhs), r(rhs), words);
        }
        MachineInst::Blt { lhs, rhs, target } => {
            load_func_local(SCRATCH, block_offsets[target.0 as usize], words, relocs);
            rrr(0xE, SCRATCH, r(lhs), r(rhs), words);
        }

        // ── Pseudos ───────────────────────────────────────────────────────────

        // Copy dst, src  →  ADD dst, src, R0
        MachineInst::Copy { dst, src } => rrr(0x0, r(dst), r(src), 0, words),

        // Ret  →  JALR R0, R15, 0
        MachineInst::Ret => rri(0xB, 0, 15, 0, words),

        // LoadImm dst, imm  →  LUI dst, imm[15:8]  +  LLI dst, imm[7:0]
        MachineInst::LoadImm { dst, imm } => {
            ri(0x7, r(dst), (*imm >> 8) as u8, words);
            ri(0x8, r(dst), (*imm & 0xFF) as u8, words);
        }

        // Jump target  →  LUI R12, offset[15:8]  +  LLI R12, offset[7:0]  +  JALR R0, R12, 0
        MachineInst::Jump { target } => {
            load_func_local(SCRATCH, block_offsets[target.0 as usize], words, relocs);
            rri(0xB, 0, SCRATCH, 0, words);
        }

        // LoadFuncAddr dst, name  →  LUI dst, 0  +  LLI dst, 0  (linker patches)
        MachineInst::LoadFuncAddr { dst, func_name } => {
            relocs.push(Relocation {
                lui_word_idx: words.len(),
                kind: RelocKind::ExternalFunc(func_name.clone()),
            });
            ri(0x7, r(dst), 0, words);
            ri(0x8, r(dst), 0, words);
        }

        // Call { callee, args }
        //   →  (ADD R(i+1), args[i], R0) for i in 0..args.len()
        //   +  JALR R15, callee, 0
        MachineInst::Call { callee, args } => {
            assert!(args.len() <= 4, "Call: stack argument passing for >4 args not yet implemented");
            for (i, arg) in args.iter().enumerate() {
                rrr(0x0, (i + 1) as u8, r(arg), 0, words);
            }
            rri(0xB, 15, r(callee), 0, words);
        }

        // ── Standalone SetCC ──────────────────────────────────────────────────
        // Expands to a branch-over sequence; see `setcc` below.
        MachineInst::SetEq  { dst, lhs, rhs } => setcc(0xC, *dst, *lhs, *rhs, here, words),
        MachineInst::SetNe  { dst, lhs, rhs } => setcc(0xD, *dst, *lhs, *rhs, here, words),
        MachineInst::SetSlt { dst, lhs, rhs } => setcc(0xE, *dst, *lhs, *rhs, here, words),
        // SGreaterThan: swap operands so BLT fires when lhs > rhs.
        MachineInst::SetSgt { dst, lhs, rhs } => setcc(0xE, *dst, *rhs, *lhs, here, words),
        // Unsigned: reuse signed BLT (correct for values in [0, 0x7FFF]).
        MachineInst::SetUlt { dst, lhs, rhs } => setcc(0xE, *dst, *lhs, *rhs, here, words),
        MachineInst::SetUgt { dst, lhs, rhs } => setcc(0xE, *dst, *rhs, *lhs, here, words),

        MachineInst::SpillLoad  { .. }
        | MachineInst::SpillStore { .. } =>
            panic!("SpillLoad/SpillStore reached emitter — prologue pass must run first"),
    }
}

/// Expand a standalone SetCC to a 10-word branch-over sequence.
///
/// ```text
/// here+0,+1 : LUI+LLI dst, 0            — dst = false
/// here+2,+3 : LUI+LLI R12, (here+8)     — address of true_label
/// here+4    : B<cc> R12, lhs, rhs        — jump if condition holds
/// here+5,+6 : LUI+LLI R12, (here+10)    — address of end_label
/// here+7    : JALR R0, R12, 0            — fall-through jump
/// here+8,+9 : LUI+LLI dst, 1            — true_label: dst = true
///             (here+10 = end_label)
/// ```
///
/// The `here+8` and `here+10` addresses are local offsets from function start.
/// The linker adds the function's base address when patching branches.
fn setcc(
    branch_opcode: u8,
    dst: VReg, lhs: VReg, rhs: VReg,
    here: usize,
    words: &mut Vec<u16>,
) {
    let true_addr = (here + 8) as u16;
    let end_addr  = (here + 10) as u16;

    ri(0x7, r(&dst), 0, words);              // LUI dst, 0
    ri(0x8, r(&dst), 0, words);              // LLI dst, 0  → dst = 0

    ri(0x7, SCRATCH, (true_addr >> 8) as u8, words);   // LUI R12, true[15:8]
    ri(0x8, SCRATCH, (true_addr & 0xFF) as u8, words); // LLI R12, true[7:0]
    rrr(branch_opcode, SCRATCH, r(&lhs), r(&rhs), words); // B<cc> R12, lhs, rhs

    ri(0x7, SCRATCH, (end_addr >> 8) as u8, words);    // LUI R12, end[15:8]
    ri(0x8, SCRATCH, (end_addr & 0xFF) as u8, words);  // LLI R12, end[7:0]
    rri(0xB, 0, SCRATCH, 0, words);          // JALR R0, R12, 0

    ri(0x7, r(&dst), 0, words);              // true_label: LUI dst, 0
    ri(0x8, r(&dst), 1, words);              // LLI dst, 1  → dst = 1
    // end_label = here + 10 (next instruction)
}

// ── Encoding helpers ──────────────────────────────────────────────────────────

/// Emit a LUI+LLI pair that loads a function-local word offset into `reg`.
/// Records a FuncBase relocation so the linker can add the function's ROM base.
fn load_func_local(
    reg: u8,
    local_offset: usize,
    words: &mut Vec<u16>,
    relocs: &mut Vec<Relocation>,
) {
    relocs.push(Relocation { lui_word_idx: words.len(), kind: RelocKind::FuncBase });
    let addr = local_offset as u16;
    ri(0x7, reg, (addr >> 8) as u8, words);
    ri(0x8, reg, (addr & 0xFF) as u8, words);
}

/// RRR format: `opcode[15:12] | rA[11:8] | rB[7:4] | rC[3:0]`
#[inline]
fn rrr(opcode: u8, ra: u8, rb: u8, rc: u8, words: &mut Vec<u16>) {
    words.push(
        ((opcode as u16) << 12)
            | ((ra as u16) << 8)
            | ((rb as u16) << 4)
            | (rc as u16 & 0xF),
    );
}

/// RRI format: `opcode[15:12] | rA[11:8] | rB[7:4] | imm4[3:0]`
///
/// `imm4` is passed as raw bits: signed offsets should be cast via `offset as u8`
/// before calling so the lower 4 bits carry the two's-complement representation.
#[inline]
fn rri(opcode: u8, ra: u8, rb: u8, imm4: u8, words: &mut Vec<u16>) {
    words.push(
        ((opcode as u16) << 12)
            | ((ra as u16) << 8)
            | ((rb as u16) << 4)
            | (imm4 as u16 & 0xF),
    );
}

/// RI format: `opcode[15:12] | rA[11:8] | imm8[7:0]`
#[inline]
fn ri(opcode: u8, ra: u8, imm8: u8, words: &mut Vec<u16>) {
    words.push(((opcode as u16) << 12) | ((ra as u16) << 8) | imm8 as u16);
}

/// Extract the physical register number from a (post-allocation) VReg.
#[inline(always)]
fn r(vreg: &VReg) -> u8 {
    debug_assert!(vreg.0 < 16, "VReg({}) is still virtual at emission time", vreg.0);
    vreg.0 as u8
}
