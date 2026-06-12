use crate::backend::isel::machine::{MachineBlock, MachineFunc, MachineInst, VReg, phys};

/// Scratch register used in the prologue to load the frame size into a register
/// before subtracting it from SP. r12 is caller-saved and free at function entry.
const SCRATCH: VReg = phys::R12;

/// Stack frame layout (FP-relative offsets after prologue):
///
///   [FP +  0]  saved RA         (return address)
///   [FP + -1]  saved FP         (caller's frame pointer)
///   [FP + -2]  spill slot 0
///   [FP + -3]  spill slot 1
///   ...
///   [FP - (1 + num_spill_slots)]  spill slot (num_spill_slots - 1)
///
///   SP = FP - frame_size   where frame_size = 2 + num_spill_slots
///
/// This pass runs after register allocation. It:
///   1. Replaces SpillLoad/SpillStore pseudos with real FP-relative Lw/Sw.
///   2. Prepends the prologue to the entry block.
///   3. Inserts epilogue instructions before every Ret.
pub fn run(mfunc: &mut MachineFunc, num_spill_slots: u32) {
    let frame_size = 2 + num_spill_slots;
    replace_spill_pseudos(mfunc);
    insert_prologue(&mut mfunc.blocks[0], frame_size);
    insert_epilogue(mfunc);
}

// ── Spill pseudo replacement ──────────────────────────────────────────────────

fn replace_spill_pseudos(mfunc: &mut MachineFunc) {
    for block in &mut mfunc.blocks {
        let old = std::mem::take(&mut block.insts);
        block.insts = old
            .into_iter()
            .map(|inst| match inst {
                MachineInst::SpillLoad { dst, slot } => MachineInst::Lw {
                    dst,
                    base: phys::FP,
                    offset: -(2 + slot as i8),
                },
                MachineInst::SpillStore { slot, src } => MachineInst::Sw {
                    base: phys::FP,
                    src,
                    offset: -(2 + slot as i8),
                },
                other => other,
            })
            .collect();
    }
}

// ── Prologue ──────────────────────────────────────────────────────────────────

fn insert_prologue(entry: &mut MachineBlock, frame_size: u32) {
    let mut prologue = vec![
        // Save RA and caller FP relative to current SP (= entry SP = future FP).
        MachineInst::Sw { base: phys::SP, src: phys::RA, offset: 0 },
        MachineInst::Sw { base: phys::SP, src: phys::FP, offset: -1 },
        // FP = entry SP.
        MachineInst::Copy { dst: phys::FP, src: phys::SP },
        // SP -= frame_size  (allocates RA slot + FP slot + spill area).
        MachineInst::LoadImm { dst: SCRATCH, imm: frame_size as u16 },
        MachineInst::Sub { dst: phys::SP, lhs: phys::SP, rhs: SCRATCH },
    ];
    prologue.append(&mut entry.insts);
    entry.insts = prologue;
}

// ── Epilogue ──────────────────────────────────────────────────────────────────

fn insert_epilogue(mfunc: &mut MachineFunc) {
    for block in &mut mfunc.blocks {
        let old = std::mem::take(&mut block.insts);
        let mut new_insts = Vec::with_capacity(old.len() + 3);
        for inst in old {
            if matches!(inst, MachineInst::Ret) {
                // Restore SP to entry SP, then restore RA and FP.
                // The RA load uses FP before FP is overwritten, so order matters.
                new_insts.push(MachineInst::Copy { dst: phys::SP, src: phys::FP });
                new_insts.push(MachineInst::Lw { dst: phys::RA, base: phys::FP, offset: 0 });
                new_insts.push(MachineInst::Lw { dst: phys::FP, base: phys::FP, offset: -1 });
            }
            new_insts.push(inst);
        }
        block.insts = new_insts;
    }
}
