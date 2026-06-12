use std::collections::HashMap;

use crate::{
    cfg::mir::{Block, ConstValue, Func, InstValue, Place, Terminator, Ty, Value},
    ctx::ctx::GlobalCtx,
};

use super::machine::{phys, MachineBlock, MachineBlockId, MachineFunc, MachineInst, VReg, FIRST_VIRTUAL};

/// Translates all IR functions in `ctx` into abstract machine functions.
///
/// Returns one [`MachineFunc`] per IR function, in the same order. The caller
/// is responsible for pushing them into `GlobalCtx::machine_funcs`.
pub struct InstructionSelector<'ctx> {
    ctx: &'ctx GlobalCtx,
}

impl<'ctx> InstructionSelector<'ctx> {
    pub fn new(ctx: &'ctx GlobalCtx) -> Self {
        Self { ctx }
    }

    pub fn run(&self) -> Vec<MachineFunc> {
        self.ctx
            .get_funcs()
            .iter()
            .enumerate()
            .map(|(idx, _)| FuncSelector::new(self.ctx).run(Func::new(idx)))
            .collect()
    }
}

// ── Per-function state ────────────────────────────────────────────────────────

struct FuncSelector<'ctx> {
    ctx: &'ctx GlobalCtx,
    mfunc: MachineFunc,
    next_vreg: u32,
    value_map: HashMap<Value, VReg>,
    block_map: HashMap<Block, MachineBlockId>,
}

impl<'ctx> FuncSelector<'ctx> {
    fn new(ctx: &'ctx GlobalCtx) -> Self {
        Self {
            ctx,
            mfunc: MachineFunc::new(String::new(), Func::new(0)),
            next_vreg: FIRST_VIRTUAL,
            value_map: HashMap::new(),
            block_map: HashMap::new(),
        }
    }

    fn alloc_vreg(&mut self) -> VReg {
        let v = VReg(self.next_vreg);
        self.next_vreg += 1;
        v
    }

    fn get_vreg(&self, value: Value) -> VReg {
        *self.value_map.get(&value).expect("value has no assigned vreg")
    }

    fn get_block_id(&self, block: Block) -> MachineBlockId {
        *self.block_map.get(&block).expect("block has no machine block id")
    }

    fn run(mut self, func_id: Func) -> MachineFunc {
        let func_name = self.ctx.get_func(func_id).get_name().to_string();
        let num_params = self.ctx.get_func(func_id).param_count();
        let ir_blocks: Vec<Block> = self.ctx.get_func(func_id).get_blocks().to_vec();

        self.mfunc = MachineFunc::new(func_name, func_id);

        // Pre-allocate machine blocks so that forward branch targets resolve
        // correctly when we emit branch instructions later.
        for (midx, &ir_block) in ir_blocks.iter().enumerate() {
            let mbid = MachineBlockId(midx as u32);
            self.block_map.insert(ir_block, mbid);
            self.mfunc.blocks.push(MachineBlock { id: mbid, insts: Vec::new() });
        }

        // Calling convention parameter setup.
        // The first four params arrive in r1-r4 (pre-colored vregs 1-4).
        // Additional params are pushed by the caller above the frame; we load
        // them from FP+offset in the entry block.
        let mut entry_insts = Vec::new();
        for i in 0..num_params {
            let param_val = self.ctx.get_nth_param(func_id, i);
            if i < 4 {
                self.value_map.insert(param_val, VReg((i + 1) as u32));
            } else {
                let vreg = self.alloc_vreg();
                entry_insts.push(MachineInst::Lw {
                    dst: vreg,
                    base: phys::FP,
                    offset: (i - 4) as i8,
                });
                self.value_map.insert(param_val, vreg);
            }
        }

        // Lower each IR block.
        for (midx, &ir_block) in ir_blocks.iter().enumerate() {
            // Clone out the data we need so the immutable borrow of ctx ends
            // before we mutably call self.select_inst / select_terminator.
            let (ir_insts, terminator) = {
                let bv = self.ctx.get_block(ir_block);
                (bv.inst.clone(), bv.terminator.clone())
            };

            let mut block_insts = if midx == 0 {
                std::mem::take(&mut entry_insts)
            } else {
                Vec::new()
            };

            for &val in &ir_insts {
                self.select_inst(val, &mut block_insts);
            }
            self.select_terminator(&terminator, &mut block_insts);

            self.mfunc.blocks[midx].insts = block_insts;
        }

        self.mfunc.num_vregs = self.next_vreg;
        self.mfunc
    }

    // ── Instruction lowering ─────────────────────────────────────────────────

    fn select_inst(&mut self, val: Value, insts: &mut Vec<MachineInst>) {
        // Clone to release the borrow on ctx before calling other self methods.
        let inst = self.ctx.get_inst(val).clone();

        match inst {
            InstValue::Param { .. } => {
                // Params are mapped during calling-convention setup; they are
                // never present in a block's instruction list.
                unreachable!("Param instruction found inside a block body")
            }

            InstValue::Const { value } => {
                let imm: u16 = match value {
                    ConstValue::I16(v) => v as u16,
                    ConstValue::U16(v) => v,
                    ConstValue::Bool(b) => b as u16,
                };
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::LoadImm { dst, imm });
            }

            InstValue::Add { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Add { dst, lhs: l, rhs: r });
            }

            InstValue::Sub { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Sub { dst, lhs: l, rhs: r });
            }

            InstValue::Mul { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Mul { dst, lhs: l, rhs: r });
            }

            InstValue::SDiv { lhs: _, rhs: _ } => {
                // TODO: SDiv has no hardware support on RiSC-P.
                //
                // Planned lowering: emit a call to a __divsi3-style software
                // helper that implements 16-bit signed division via a
                // shift-and-subtract loop.
                //
                // Implementation notes:
                //   - Extract sign bits of both operands.
                //   - Perform unsigned division on absolute values.
                //   - Negate the quotient iff operand signs differ.
                //   - Remainder takes the sign of the dividend (C99 / Rust
                //     truncating-toward-zero semantics).
                //   - Special-case: i16::MIN / -1 overflows; clamp or trap.
                todo!("SDiv: software division not yet implemented")
            }

            InstValue::UDiv { lhs: _, rhs: _ } => {
                // TODO: UDiv has no hardware support on RiSC-P.
                //
                // Planned lowering: emit a call to a __udivsi3-style software
                // helper. Unsigned 16-bit division is simpler than signed: a
                // restoring or non-restoring shift-and-subtract loop with no
                // sign fixup required.
                todo!("UDiv: software division not yet implemented")
            }

            // Comparisons materialise the boolean result as 0 or 1 via SetCC
            // pseudos. The code emitter expands each SetCC to a small
            // branch-based sequence. When a SetCC feeds directly into a
            // Bne-against-zero (the common case from Branch terminators), the
            // emitter can fuse the pair into a single compare-and-branch.
            InstValue::Equal { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::SetEq { dst, lhs: l, rhs: r });
            }

            InstValue::NotEqual { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::SetNe { dst, lhs: l, rhs: r });
            }

            InstValue::SLessThan { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::SetSlt { dst, lhs: l, rhs: r });
            }

            InstValue::ULessThan { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::SetUlt { dst, lhs: l, rhs: r });
            }

            InstValue::SGreaterThan { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::SetSgt { dst, lhs: l, rhs: r });
            }

            InstValue::UGreaterThan { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::SetUgt { dst, lhs: l, rhs: r });
            }

            // Logical ops operate on Bool (0/1) values. RiSC-P has no OR/AND
            // opcodes; synthesise them from NAND using De Morgan identities.
            InstValue::LogicalAnd { lhs, rhs } => {
                // AND(a, b) = NAND(NAND(a, b), NAND(a, b))
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let tmp = self.alloc_vreg();
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Nand { dst: tmp, lhs: l, rhs: r });
                insts.push(MachineInst::Nand { dst, lhs: tmp, rhs: tmp });
            }

            InstValue::LogicalOr { lhs, rhs } => {
                // OR(a, b) = NAND(NAND(a, a), NAND(b, b))
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let not_l = self.alloc_vreg();
                let not_r = self.alloc_vreg();
                let dst   = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Nand { dst: not_l, lhs: l, rhs: l });
                insts.push(MachineInst::Nand { dst: not_r, lhs: r, rhs: r });
                insts.push(MachineInst::Nand { dst, lhs: not_l, rhs: not_r });
            }

            InstValue::BitwiseAnd { lhs, rhs } => {
                // AND(a, b) = NAND(NAND(a, b), NAND(a, b))
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let tmp = self.alloc_vreg();
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Nand { dst: tmp, lhs: l, rhs: r });
                insts.push(MachineInst::Nand { dst, lhs: tmp, rhs: tmp });
            }

            InstValue::BitwiseOr { lhs, rhs } => {
                // OR(a, b) = NAND(NAND(a, a), NAND(b, b))
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let not_l = self.alloc_vreg();
                let not_r = self.alloc_vreg();
                let dst   = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Nand { dst: not_l, lhs: l, rhs: l });
                insts.push(MachineInst::Nand { dst: not_r, lhs: r, rhs: r });
                insts.push(MachineInst::Nand { dst, lhs: not_l, rhs: not_r });
            }

            InstValue::BitwiseXor { lhs, rhs } => {
                let (l, r) = (self.get_vreg(lhs), self.get_vreg(rhs));
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Xor { dst, lhs: l, rhs: r });
            }

            InstValue::BitwiseNot { value: src_val } => {
                // NOT(a) = NAND(a, a)
                let src = self.get_vreg(src_val);
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                insts.push(MachineInst::Nand { dst, lhs: src, rhs: src });
            }

            InstValue::Load { place } => {
                // TODO: frame layout — FP offsets for locals are not yet
                // tracked. All local loads use FP+0 as a placeholder until
                // the stack-frame layout pass is implemented.
                // Global loads require resolving the global's absolute address
                // (a link-time constant) into a temp register first.
                let dst = self.alloc_vreg();
                self.value_map.insert(val, dst);
                match place {
                    Place::Local(_local) => {
                        insts.push(MachineInst::Lw { dst, base: phys::FP, offset: 0 });
                    }
                    Place::Global(_global) => {
                        todo!("global variable loads not yet implemented")
                    }
                }
            }

            InstValue::Store { place, value: src_val } => {
                // Store has unit type; do not insert into value_map.
                // TODO: FP offsets for locals are placeholders (see Load above).
                let src = self.get_vreg(src_val);
                match place {
                    Place::Local(_local) => {
                        insts.push(MachineInst::Sw { base: phys::FP, src, offset: 0 });
                    }
                    Place::Global(_global) => {
                        todo!("global variable stores not yet implemented")
                    }
                }
            }

            InstValue::Call { func, args } => {
                let func_name = self.ctx.get_func(func).get_name().to_string();
                let return_ty = self.ctx.get_func_return_ty(func);
                let arg_vregs: Vec<VReg> = args.iter().map(|&a| self.get_vreg(a)).collect();

                let callee = self.alloc_vreg();
                insts.push(MachineInst::LoadFuncAddr { dst: callee, func_name });
                insts.push(MachineInst::Call { callee, args: arg_vregs });

                // The return value is in r1 after the call. Copy it into a
                // fresh vreg immediately so a subsequent call cannot clobber it.
                if !matches!(return_ty, Ty::Unit) {
                    let ret = self.alloc_vreg();
                    insts.push(MachineInst::Copy { dst: ret, src: phys::R1 });
                    self.value_map.insert(val, ret);
                }
            }
        }
    }

    // ── Terminator lowering ──────────────────────────────────────────────────

    fn select_terminator(&mut self, term: &Terminator, insts: &mut Vec<MachineInst>) {
        match term {
            Terminator::Return { value } => {
                let src = self.get_vreg(*value);
                if src != phys::R1 {
                    insts.push(MachineInst::Copy { dst: phys::R1, src });
                }
                insts.push(MachineInst::Ret);
            }

            Terminator::ReturnNone => {
                insts.push(MachineInst::Ret);
            }

            Terminator::Jump { target } => {
                insts.push(MachineInst::Jump { target: self.get_block_id(*target) });
            }

            Terminator::Branch { cond, true_target, false_target } => {
                let cond_vreg = self.get_vreg(*cond);
                let true_mid  = self.get_block_id(*true_target);
                let false_mid = self.get_block_id(*false_target);
                // Condition is 0 or 1. Branch to true block when cond != 0.
                insts.push(MachineInst::Bne { lhs: cond_vreg, rhs: phys::R0, target: true_mid });
                insts.push(MachineInst::Jump { target: false_mid });
            }

            Terminator::None => {
                // Unterminated block — nothing to emit.
            }
        }
    }
}
