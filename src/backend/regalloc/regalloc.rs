use std::collections::HashMap;

use crate::backend::isel::machine::{MachineFunc, MachineInst, VReg, phys, FIRST_VIRTUAL};

use super::liveness::{LiveInterval, compute_intervals, inst_defs, inst_uses};

// ── Register budget ───────────────────────────────────────────────────────────

/// Registers the allocator may assign to virtual registers.
/// r11 and r12 are excluded: r11 and r12 serve as spill temporaries.
const ALLOCATABLE: &[VReg] = &[
    phys::R1, phys::R2, phys::R3,  phys::R4,  phys::R5,
    phys::R6, phys::R7, phys::R8,  phys::R9,  phys::R10,
];

/// First spill temporary. Used to reload a spilled USE operand.
const SPILL_TEMP_0: VReg = phys::R11;
/// Second spill temporary. Used when an instruction has two spilled operands,
/// or when a spilled DEF needs a temporary distinct from SPILL_TEMP_0.
const SPILL_TEMP_1: VReg = phys::R12;

// ── Public entry point ────────────────────────────────────────────────────────

/// Assign physical registers to every virtual register in `mfunc`, inserting
/// SpillLoad / SpillStore pseudo-instructions for values that do not fit in
/// registers. Returns the number of spill slots allocated (used by the
/// prologue/epilogue pass to size the stack frame).
pub fn run(mfunc: &mut MachineFunc) -> u32 {
    let mut intervals = compute_intervals(mfunc);
    let (reg_map, spill_map, num_slots) = linear_scan(&mut intervals);
    apply_allocation(mfunc, &reg_map, &spill_map);
    num_slots
}

// ── Linear Scan ───────────────────────────────────────────────────────────────

/// Returns (reg_map, spill_map, num_spill_slots).
///
/// reg_map:   virtual VReg → physical VReg (for non-spilled virtuals)
/// spill_map: virtual VReg → spill slot index (for spilled virtuals)
fn linear_scan(
    intervals: &mut Vec<LiveInterval>,
) -> (HashMap<VReg, VReg>, HashMap<VReg, u32>, u32) {
    // Sort by start; break ties by placing pre-colored intervals first so their
    // physical registers are blocked before virtuals are allocated at the same point.
    intervals.sort_by(|a, b| {
        a.start.cmp(&b.start).then_with(|| {
            let a_pre = a.vreg.0 < FIRST_VIRTUAL;
            let b_pre = b.vreg.0 < FIRST_VIRTUAL;
            b_pre.cmp(&a_pre)
        })
    });

    let mut free_regs: Vec<VReg> = ALLOCATABLE.to_vec();
    // active is kept sorted by end point (shortest end first) for O(n) eviction.
    let mut active: Vec<LiveInterval> = Vec::new();
    let mut reg_map: HashMap<VReg, VReg> = HashMap::new();
    let mut spill_map: HashMap<VReg, u32> = HashMap::new();
    let mut next_spill = 0u32;

    for interval in intervals.iter() {
        expire_old(&mut active, &mut free_regs, &reg_map, interval.start);

        let is_precolored = interval.vreg.0 < FIRST_VIRTUAL;

        if is_precolored {
            // Pre-colored intervals are already assigned to a physical register.
            // Block that register in the free pool for the duration of the interval.
            free_regs.retain(|&r| r != interval.vreg);
            insert_by_end(&mut active, interval.clone());
        } else if free_regs.is_empty() {
            // No register available — spill the virtual interval with the
            // furthest end point (classic linear-scan heuristic).
            let furthest = active
                .iter()
                .enumerate()
                .filter(|(_, i)| i.vreg.0 >= FIRST_VIRTUAL)
                .max_by_key(|(_, i)| i.end);

            match furthest {
                Some((fi, _)) if active[fi].end > interval.end => {
                    // Evict the furthest active interval; give its register to current.
                    let evicted = active.remove(fi);
                    let reg = *reg_map.get(&evicted.vreg).expect("active interval has no reg");
                    reg_map.remove(&evicted.vreg);
                    spill_map.insert(evicted.vreg, next_spill);
                    next_spill += 1;
                    reg_map.insert(interval.vreg, reg);
                    insert_by_end(&mut active, interval.clone());
                }
                _ => {
                    // Current interval has the furthest end (or all active are
                    // pre-colored): spill current.
                    spill_map.insert(interval.vreg, next_spill);
                    next_spill += 1;
                }
            }
        } else {
            let reg = free_regs.remove(0);
            reg_map.insert(interval.vreg, reg);
            insert_by_end(&mut active, interval.clone());
        }
    }

    (reg_map, spill_map, next_spill)
}

fn expire_old(
    active: &mut Vec<LiveInterval>,
    free_regs: &mut Vec<VReg>,
    reg_map: &HashMap<VReg, VReg>,
    current_start: usize,
) {
    let mut freed = Vec::new();
    active.retain(|interval| {
        if interval.end < current_start {
            if interval.vreg.0 < FIRST_VIRTUAL {
                // Pre-colored: release its physical register back to the pool
                // only if it is in the allocatable set.
                if ALLOCATABLE.contains(&interval.vreg) {
                    freed.push(interval.vreg);
                }
            } else if let Some(&reg) = reg_map.get(&interval.vreg) {
                freed.push(reg);
            }
            false
        } else {
            true
        }
    });
    free_regs.extend(freed);
}

fn insert_by_end(active: &mut Vec<LiveInterval>, interval: LiveInterval) {
    let pos = active.partition_point(|i| i.end <= interval.end);
    active.insert(pos, interval);
}

// ── Allocation application ────────────────────────────────────────────────────

fn apply_allocation(
    mfunc: &mut MachineFunc,
    reg_map: &HashMap<VReg, VReg>,
    spill_map: &HashMap<VReg, u32>,
) {
    for block in &mut mfunc.blocks {
        let old_insts = std::mem::take(&mut block.insts);
        let mut new_insts = Vec::with_capacity(old_insts.len() * 2);

        for inst in old_insts {
            // Identify which use and def operands are spilled, and assign spill
            // temporaries (SPILL_TEMP_0 / SPILL_TEMP_1) to them.
            let uses = inst_uses(&inst);
            let defs = inst_defs(&inst);

            let mut temp_idx = 0usize;
            let mut use_temp: HashMap<VReg, VReg> = HashMap::new();
            let mut def_temp: HashMap<VReg, VReg> = HashMap::new();

            // Emit SpillLoad for each unique spilled USE operand.
            for vreg in &uses {
                if spill_map.contains_key(vreg) && !use_temp.contains_key(vreg) {
                    let temp = pick_temp(temp_idx);
                    temp_idx += 1;
                    let slot = spill_map[vreg];
                    new_insts.push(MachineInst::SpillLoad { dst: temp, slot });
                    use_temp.insert(*vreg, temp);
                }
            }

            // Assign spill temporaries for each spilled DEF operand.
            for vreg in &defs {
                if spill_map.contains_key(vreg) && !def_temp.contains_key(vreg) {
                    let temp = pick_temp(temp_idx);
                    temp_idx += 1;
                    def_temp.insert(*vreg, temp);
                }
            }

            // Rewrite the instruction: replace every VReg with its physical
            // assignment, using the spill temporaries where needed.
            let resolve = |v: VReg| -> VReg {
                if v.0 < FIRST_VIRTUAL { return v; }  // pre-colored
                if let Some(&t) = use_temp.get(&v) { return t; }
                if let Some(&t) = def_temp.get(&v) { return t; }
                *reg_map.get(&v).unwrap_or_else(|| panic!("VReg({}) unallocated", v.0))
            };

            new_insts.push(rewrite_inst(inst, &resolve));

            // Emit SpillStore for each spilled DEF operand.
            for (vreg, &temp) in &def_temp {
                let slot = spill_map[vreg];
                new_insts.push(MachineInst::SpillStore { slot, src: temp });
            }
        }

        block.insts = new_insts;
    }
}

fn pick_temp(idx: usize) -> VReg {
    match idx {
        0 => SPILL_TEMP_0,
        1 => SPILL_TEMP_1,
        _ => panic!("instruction requires more than 2 spill temporaries"),
    }
}

fn rewrite_inst(inst: MachineInst, resolve: &impl Fn(VReg) -> VReg) -> MachineInst {
    let r = |v| resolve(v);
    match inst {
        MachineInst::Add  { dst, lhs, rhs } => MachineInst::Add  { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::Sub  { dst, lhs, rhs } => MachineInst::Sub  { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::Mul  { dst, lhs, rhs } => MachineInst::Mul  { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::Xor  { dst, lhs, rhs } => MachineInst::Xor  { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::Nand { dst, lhs, rhs } => MachineInst::Nand { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },

        MachineInst::Shl { dst, src, imm } => MachineInst::Shl { dst: r(dst), src: r(src), imm },
        MachineInst::Shr { dst, src, imm } => MachineInst::Shr { dst: r(dst), src: r(src), imm },

        MachineInst::Lw   { dst, base, offset } => MachineInst::Lw   { dst: r(dst), base: r(base), offset },
        MachineInst::Sw   { base, src, offset } => MachineInst::Sw   { base: r(base), src: r(src), offset },
        MachineInst::Jalr { dst, base, offset } => MachineInst::Jalr { dst: r(dst), base: r(base), offset },

        MachineInst::Lui { dst, imm } => MachineInst::Lui { dst: r(dst), imm },
        MachineInst::Lli { dst, imm } => MachineInst::Lli { dst: r(dst), imm },

        MachineInst::Beq { lhs, rhs, target } => MachineInst::Beq { lhs: r(lhs), rhs: r(rhs), target },
        MachineInst::Bne { lhs, rhs, target } => MachineInst::Bne { lhs: r(lhs), rhs: r(rhs), target },
        MachineInst::Blt { lhs, rhs, target } => MachineInst::Blt { lhs: r(lhs), rhs: r(rhs), target },

        MachineInst::LoadImm     { dst, imm }      => MachineInst::LoadImm     { dst: r(dst), imm },
        MachineInst::Copy        { dst, src }       => MachineInst::Copy        { dst: r(dst), src: r(src) },
        MachineInst::Jump        { target }         => MachineInst::Jump        { target },
        MachineInst::Ret                            => MachineInst::Ret,
        MachineInst::LoadFuncAddr { dst, func_name } => MachineInst::LoadFuncAddr { dst: r(dst), func_name },

        MachineInst::Call { callee, args } =>
            MachineInst::Call { callee: r(callee), args: args.into_iter().map(r).collect() },

        MachineInst::SetEq  { dst, lhs, rhs } => MachineInst::SetEq  { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::SetNe  { dst, lhs, rhs } => MachineInst::SetNe  { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::SetSlt { dst, lhs, rhs } => MachineInst::SetSlt { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::SetUlt { dst, lhs, rhs } => MachineInst::SetUlt { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::SetSgt { dst, lhs, rhs } => MachineInst::SetSgt { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },
        MachineInst::SetUgt { dst, lhs, rhs } => MachineInst::SetUgt { dst: r(dst), lhs: r(lhs), rhs: r(rhs) },

        MachineInst::SpillLoad  { dst, slot } => MachineInst::SpillLoad  { dst: r(dst), slot },
        MachineInst::SpillStore { slot, src } => MachineInst::SpillStore { slot, src: r(src) },
    }
}
