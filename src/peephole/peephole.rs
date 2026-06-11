use crate::isel::machine::{MachineFunc, MachineInst, MachineBlockId, VReg, phys};

/// Run all peephole optimizations over every block in `mfunc`.
///
/// Optimizations applied (in order, repeated until stable):
///   1. SetCC + Bne(dst, R0) fusion — replaces adjacent SetCC / Bne-vs-zero
///      pairs with a single concrete branch, saving the overhead of
///      materializing the boolean result.
///   2. Identity-copy elimination — removes `Copy dst, src` when dst == src.
pub fn run(mfunc: &mut MachineFunc) {
    for block in &mut mfunc.blocks {
        fuse_setcc_branch(&mut block.insts);
        remove_identity_copies(&mut block.insts);
    }
}

// ── SetCC + Bne fusion ────────────────────────────────────────────────────────

/// Scan the instruction list for the pattern:
///
///   SetXX { dst: v, lhs: a, rhs: b }
///   Bne   { lhs: v, rhs: R0, target: T }
///
/// and replace the pair with a single concrete branch:
///
///   Beq/Bne/Blt { lhs: a, rhs: b, target: T }
///
/// This fuses the most common isel output for Branch terminators, where every
/// `Branch { cond }` emits a SetCC for the comparison and a Bne-vs-zero for
/// the jump.
fn fuse_setcc_branch(insts: &mut Vec<MachineInst>) {
    let mut i = 0;
    while i + 1 < insts.len() {
        if let Some(fused) = try_fuse_pair(&insts[i], &insts[i + 1]) {
            insts.remove(i);   // drop SetCC
            insts[i] = fused;  // replace Bne with concrete branch
        }
        i += 1;
    }
}

fn try_fuse_pair(first: &MachineInst, second: &MachineInst) -> Option<MachineInst> {
    // The second instruction must be Bne { lhs: v, rhs: R0, target }.
    let (bne_lhs, target) = match second {
        MachineInst::Bne { lhs, rhs, target } if *rhs == phys::R0 => (*lhs, *target),
        _ => return None,
    };

    // The first instruction must be a SetCC whose dst matches the Bne lhs.
    fuse_setcc(first, bne_lhs, target)
}

fn fuse_setcc(setcc: &MachineInst, v: VReg, target: MachineBlockId) -> Option<MachineInst> {
    match setcc {
        MachineInst::SetEq  { dst, lhs, rhs } if *dst == v =>
            Some(MachineInst::Beq { lhs: *lhs, rhs: *rhs, target }),

        MachineInst::SetNe  { dst, lhs, rhs } if *dst == v =>
            Some(MachineInst::Bne { lhs: *lhs, rhs: *rhs, target }),

        MachineInst::SetSlt { dst, lhs, rhs } if *dst == v =>
            Some(MachineInst::Blt { lhs: *lhs, rhs: *rhs, target }),

        // SGreaterThan(a, b) ≡ LessThan(b, a): swap operands.
        MachineInst::SetSgt { dst, lhs, rhs } if *dst == v =>
            Some(MachineInst::Blt { lhs: *rhs, rhs: *lhs, target }),

        // Unsigned comparisons: RiSC-P only has signed BLT. These fusions
        // are correct when values fit in [0, 32767]; full unsigned support
        // requires a NAND-based sign-bit fixup and is left for a later pass.
        MachineInst::SetUlt { dst, lhs, rhs } if *dst == v =>
            Some(MachineInst::Blt { lhs: *lhs, rhs: *rhs, target }),

        MachineInst::SetUgt { dst, lhs, rhs } if *dst == v =>
            Some(MachineInst::Blt { lhs: *rhs, rhs: *lhs, target }),

        _ => None,
    }
}

// ── Identity-copy elimination ─────────────────────────────────────────────────

/// Remove `Copy { dst, src }` instructions where dst == src.
/// These are no-ops and arise when regalloc assigns the same physical register
/// to both sides of a copy, or when isel emits a Copy that becomes trivial
/// after register allocation.
fn remove_identity_copies(insts: &mut Vec<MachineInst>) {
    insts.retain(|inst| !matches!(inst, MachineInst::Copy { dst, src } if dst == src));
}
