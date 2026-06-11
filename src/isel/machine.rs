use crate::cfg::mir::Func;

/// A virtual register. VReg(0..=15) are pre-colored to physical registers
/// r0..=r15 and must not be allocated as fresh virtuals. Fresh virtual
/// registers start from [`FIRST_VIRTUAL`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VReg(pub u32);

/// Pre-colored virtual registers corresponding to each physical RiSC-P register.
///
/// Calling convention:
///   r1-r4   argument registers (r1 also carries the return value)
///   r5-r12  caller-saved general-purpose temporaries
///   r13     frame pointer  (FP)
///   r14     stack pointer  (SP)
///   r15     return address (RA)
#[allow(dead_code)]
pub mod phys {
    use super::VReg;
    pub const R0:  VReg = VReg(0);   // always zero
    pub const R1:  VReg = VReg(1);   // arg0 / return value
    pub const R2:  VReg = VReg(2);   // arg1
    pub const R3:  VReg = VReg(3);   // arg2
    pub const R4:  VReg = VReg(4);   // arg3
    pub const R5:  VReg = VReg(5);
    pub const R6:  VReg = VReg(6);
    pub const R7:  VReg = VReg(7);
    pub const R8:  VReg = VReg(8);
    pub const R9:  VReg = VReg(9);
    pub const R10: VReg = VReg(10);
    pub const R11: VReg = VReg(11);
    pub const R12: VReg = VReg(12);
    pub const FP:  VReg = VReg(13);
    pub const SP:  VReg = VReg(14);
    pub const RA:  VReg = VReg(15);
}

/// Virtual registers below this index are pre-colored to physical registers.
/// The vreg allocator in each function starts here.
pub const FIRST_VIRTUAL: u32 = 16;

/// Index of a machine basic block within a [`MachineFunc`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MachineBlockId(pub u32);

/// Index of a [`MachineFunc`] within `GlobalCtx::machine_funcs`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MachineFuncId(pub u32);

/// Abstract machine instructions for RiSC-P, using virtual registers.
///
/// Concrete instructions map 1-to-1 to RiSC-P opcodes. Pseudo-instructions
/// are expanded by the code-emission phase that follows register allocation.
pub enum MachineInst {
    // ── Concrete RiSC-P instructions ─────────────────────────────────────────

    // RRR format
    Add  { dst: VReg, lhs: VReg, rhs: VReg },
    Sub  { dst: VReg, lhs: VReg, rhs: VReg },
    Mul  { dst: VReg, lhs: VReg, rhs: VReg },
    Xor  { dst: VReg, lhs: VReg, rhs: VReg },
    Nand { dst: VReg, lhs: VReg, rhs: VReg },

    // RRI format (imm is 4-bit for shifts, 4-bit signed for memory)
    Shl  { dst: VReg, src: VReg, imm: u8 },
    Shr  { dst: VReg, src: VReg, imm: u8 },
    Lw   { dst: VReg, base: VReg, offset: i8 },
    Sw   { base: VReg, src: VReg, offset: i8 },
    Jalr { dst: VReg, base: VReg, offset: i8 },

    // RI format
    Lui { dst: VReg, imm: u8 },
    Lli { dst: VReg, imm: u8 },

    // Conditional branches. The branch target is a block ID resolved during
    // code emission: the emitter loads the target address into a temp register
    // and uses the three-register BEQ/BNE/BLT encoding.
    Beq { lhs: VReg, rhs: VReg, target: MachineBlockId },
    Bne { lhs: VReg, rhs: VReg, target: MachineBlockId },
    Blt { lhs: VReg, rhs: VReg, target: MachineBlockId },

    // ── Pseudo-instructions ──────────────────────────────────────────────────

    /// Load a 16-bit constant. Expands to LUI+LLI, or just LLI when the
    /// value fits in the lower 8 bits with zero-extension.
    LoadImm { dst: VReg, imm: u16 },

    /// Register copy. Expands to ADD dst, src, r0.
    Copy { dst: VReg, src: VReg },

    /// Unconditional branch. Expands to LUI+LLI (target address into a temp)
    /// followed by JALR r0, tmp, 0.
    Jump { target: MachineBlockId },

    /// Return from function. Expands to JALR r0, r15, 0 (jump to return
    /// address; writing to r0 discards the link value).
    Ret,

    /// Load the link-time address of a named function into a register.
    /// Resolved to a LUI+LLI pair by the linker/emitter.
    LoadFuncAddr { dst: VReg, func_name: String },

    /// Full call sequence. Expands during code emission to: move each arg into
    /// its calling-convention register (r1-r4) or push excess args onto the
    /// stack, then JALR r15, callee, 0. The return value is live in r1 after
    /// the call; the instruction selector copies it out immediately.
    Call { callee: VReg, args: Vec<VReg> },

    // ── SetCC pseudo-instructions ─────────────────────────────────────────────

    /// Materialize a comparison result as 0 (false) or 1 (true) in `dst`.
    ///
    /// Expanded by the code emitter to a branch-based sequence:
    ///   dst = 0
    ///   B<cc> lhs, rhs, true_label
    ///   jump  end_label
    /// true_label:
    ///   dst = 1
    /// end_label:
    ///
    /// When a SetCC result feeds directly into a Bne-against-zero (the common
    /// case from Branch terminators), the emitter can fuse the pair into a
    /// single compare-and-branch without materializing 0/1.
    SetEq  { dst: VReg, lhs: VReg, rhs: VReg },
    SetNe  { dst: VReg, lhs: VReg, rhs: VReg },
    SetSlt { dst: VReg, lhs: VReg, rhs: VReg },
    SetUlt { dst: VReg, lhs: VReg, rhs: VReg },
    SetSgt { dst: VReg, lhs: VReg, rhs: VReg },
    SetUgt { dst: VReg, lhs: VReg, rhs: VReg },
}

pub struct MachineBlock {
    pub id: MachineBlockId,
    pub insts: Vec<MachineInst>,
}

pub struct MachineFunc {
    pub name: String,
    pub ir_func: Func,
    pub blocks: Vec<MachineBlock>,
    /// Total vreg count after instruction selection (always >= FIRST_VIRTUAL).
    /// The register allocator uses this to size its data structures.
    pub num_vregs: u32,
}

impl MachineFunc {
    pub fn new(name: String, ir_func: Func) -> Self {
        Self {
            name,
            ir_func,
            blocks: Vec::new(),
            num_vregs: FIRST_VIRTUAL,
        }
    }
}
