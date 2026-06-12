use std::collections::HashMap;

use crate::backend::isel::machine::{MachineFunc, MachineInst, VReg, phys};

#[derive(Clone, Debug)]
pub struct LiveInterval {
    pub vreg: VReg,
    pub start: usize,
    pub end: usize,
}

/// Virtual registers defined by `inst`.
pub fn inst_defs(inst: &MachineInst) -> Vec<VReg> {
    match inst {
        MachineInst::Add  { dst, .. }
        | MachineInst::Sub  { dst, .. }
        | MachineInst::Mul  { dst, .. }
        | MachineInst::Xor  { dst, .. }
        | MachineInst::Nand { dst, .. } => vec![*dst],

        MachineInst::Shl { dst, .. } | MachineInst::Shr { dst, .. } => vec![*dst],

        MachineInst::Lw   { dst, .. } => vec![*dst],
        MachineInst::Sw   { .. }      => vec![],
        MachineInst::Jalr { dst, .. } => vec![*dst],

        MachineInst::Lui { dst, .. } | MachineInst::Lli { dst, .. } => vec![*dst],

        MachineInst::Beq { .. } | MachineInst::Bne { .. } | MachineInst::Blt { .. } => vec![],

        MachineInst::LoadImm     { dst, .. } => vec![*dst],
        MachineInst::Copy        { dst, .. } => vec![*dst],
        MachineInst::Jump        { .. }      => vec![],
        MachineInst::Ret                     => vec![],
        MachineInst::LoadFuncAddr { dst, .. } => vec![*dst],

        // Call implicitly clobbers r1 (return value). All of r1-r10 are
        // caller-saved and technically clobbered, but we only track r1 here;
        // any VReg allocated to r1-r10 that must survive a call will be
        // spilled because its live interval crosses the call site.
        MachineInst::Call { .. } => vec![phys::R1],

        MachineInst::SetEq  { dst, .. }
        | MachineInst::SetNe  { dst, .. }
        | MachineInst::SetSlt { dst, .. }
        | MachineInst::SetUlt { dst, .. }
        | MachineInst::SetSgt { dst, .. }
        | MachineInst::SetUgt { dst, .. } => vec![*dst],

        MachineInst::SpillLoad  { dst, .. } => vec![*dst],
        MachineInst::SpillStore { .. }      => vec![],
    }
}

/// Virtual registers read by `inst`.
pub fn inst_uses(inst: &MachineInst) -> Vec<VReg> {
    match inst {
        MachineInst::Add  { lhs, rhs, .. }
        | MachineInst::Sub  { lhs, rhs, .. }
        | MachineInst::Mul  { lhs, rhs, .. }
        | MachineInst::Xor  { lhs, rhs, .. }
        | MachineInst::Nand { lhs, rhs, .. } => vec![*lhs, *rhs],

        MachineInst::Shl { src, .. } | MachineInst::Shr { src, .. } => vec![*src],

        MachineInst::Lw { base, .. }         => vec![*base],
        MachineInst::Sw { base, src, .. }    => vec![*base, *src],
        MachineInst::Jalr { base, .. }       => vec![*base],

        MachineInst::Lui { .. } | MachineInst::Lli { .. } => vec![],

        MachineInst::Beq { lhs, rhs, .. }
        | MachineInst::Bne { lhs, rhs, .. }
        | MachineInst::Blt { lhs, rhs, .. } => vec![*lhs, *rhs],

        MachineInst::LoadImm     { .. }       => vec![],
        MachineInst::Copy        { src, .. }  => vec![*src],
        MachineInst::Jump        { .. }       => vec![],
        MachineInst::LoadFuncAddr { .. }      => vec![],

        // Ret reads r1 (return value) and r15 (return address).
        MachineInst::Ret => vec![phys::R1, phys::RA],

        MachineInst::Call { callee, args, .. } => {
            let mut v = vec![*callee];
            v.extend_from_slice(args);
            v
        }

        MachineInst::SetEq  { lhs, rhs, .. }
        | MachineInst::SetNe  { lhs, rhs, .. }
        | MachineInst::SetSlt { lhs, rhs, .. }
        | MachineInst::SetUlt { lhs, rhs, .. }
        | MachineInst::SetSgt { lhs, rhs, .. }
        | MachineInst::SetUgt { lhs, rhs, .. } => vec![*lhs, *rhs],

        MachineInst::SpillLoad  { .. }       => vec![],
        MachineInst::SpillStore { src, .. }  => vec![*src],
    }
}

/// Compute one live interval per VReg across the entire function.
///
/// The interval is [start, end] where start is the earliest instruction index
/// at which the VReg appears (as def or use) and end is the latest.
/// Pre-colored VRegs (r0-r15) that appear only as uses — such as param
/// registers at function entry — get start = 0.
pub fn compute_intervals(mfunc: &MachineFunc) -> Vec<LiveInterval> {
    // Track the first instruction index where a VReg appears as a def or use.
    let mut first_occurrence: HashMap<VReg, usize> = HashMap::new();
    let mut last_use: HashMap<VReg, usize> = HashMap::new();

    let mut idx = 0usize;
    for block in &mfunc.blocks {
        for inst in &block.insts {
            for vreg in inst_defs(inst) {
                first_occurrence.entry(vreg).or_insert(idx);
            }
            for vreg in inst_uses(inst) {
                first_occurrence.entry(vreg).or_insert(idx);
                last_use.insert(vreg, idx); // always overwrite → keeps the last
            }
            idx += 1;
        }
    }

    first_occurrence
        .into_iter()
        .map(|(vreg, start)| {
            let end = last_use.get(&vreg).copied().unwrap_or(start);
            LiveInterval { vreg, start, end: end.max(start) }
        })
        .collect()
}
