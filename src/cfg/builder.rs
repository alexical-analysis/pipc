use crate::{
    cfg::mir::{
        Block, ConstValue, Func, FuncValue, Global, InstValue, Local, LocalValue, Place,
        Terminator, Ty, Value,
    },
    ctx::ctx::GlobalCtx,
};

pub struct Builder<'ctx> {
    ctx: &'ctx mut GlobalCtx,
    position: Block,
}

impl<'ctx> Builder<'ctx> {
    pub fn new(ctx: &'ctx mut GlobalCtx) -> Self {
        Self {
            ctx,
            position: Block::new(0),
        }
    }

    pub fn add_func(&mut self, name: String, params: Vec<Ty>, return_ty: Ty) -> Func {
        let mut param_values = Vec::with_capacity(params.len());

        for (i, &ty) in params.iter().enumerate() {
            let value = self.ctx.add_param(i as u32, ty);
            param_values.push(value);
        }

        let func_value = FuncValue::new(name, param_values, return_ty);
        self.ctx.add_func(func_value)
    }

    pub fn append_block(&mut self, func: Func) -> Block {
        self.ctx.append_block(func)
    }

    pub fn get_nth_param(&self, func: Func, n: usize) -> Value {
        self.ctx.get_nth_param(func, n)
    }

    pub fn position_at_end(&mut self, block: Block) {
        self.position = block;
    }

    pub fn build_const(&mut self, value: ConstValue) -> Value {
        let ty = match value {
            ConstValue::Bool(_) => Ty::Bool,
            ConstValue::I16(_) => Ty::I32,
            ConstValue::U16(_) => Ty::U32,
        };

        let inst = InstValue::Const { value };
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_load_global(&mut self, global: Global) -> Value {
        let inst = InstValue::Load {
            place: Place::Global(global),
        };

        let ty = self.ctx.get_global_ty(global);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_load_local(&mut self, local: Local) -> Value {
        let inst = InstValue::Load {
            place: Place::Local(local),
        };

        let ty = self.ctx.get_local_ty(local);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_store_global(&mut self, global: Global, value: Value) {
        let inst = InstValue::Store {
            place: Place::Global(global),
            value,
        };

        self.ctx.add_inst(self.position, inst, Ty::Unit);
    }

    pub fn build_store_local(&mut self, local: Local, value: Value) {
        let inst = InstValue::Store {
            place: Place::Local(local),
            value,
        };
        self.ctx.add_inst(self.position, inst, Ty::Unit);
    }

    pub fn build_call(&mut self, func: Func, args: Vec<Value>) -> Value {
        let return_ty = self.ctx.get_func_return_ty(func);

        let inst = InstValue::Call { func, args };
        self.ctx.add_inst(self.position, inst, return_ty)
    }

    pub fn build_add(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::Add { lhs, rhs };

        let ty = self.ctx.get_value_ty(lhs);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_sub(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::Sub { lhs, rhs };

        let ty = self.ctx.get_value_ty(lhs);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_mul(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::Mul { lhs, rhs };

        let ty = self.ctx.get_value_ty(lhs);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_sdiv(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::SDiv { lhs, rhs };

        let ty = self.ctx.get_value_ty(lhs);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_udiv(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::UDiv { lhs, rhs };

        let ty = self.ctx.get_value_ty(lhs);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_equal(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::Equal { lhs, rhs };
        self.ctx.add_inst(self.position, inst, Ty::Bool)
    }

    pub fn build_not_equal(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::NotEqual { lhs, rhs };
        self.ctx.add_inst(self.position, inst, Ty::Bool)
    }

    pub fn build_sless_than(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::SLessThan { lhs, rhs };
        self.ctx.add_inst(self.position, inst, Ty::Bool)
    }

    pub fn build_uless_than(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::ULessThan { lhs, rhs };
        self.ctx.add_inst(self.position, inst, Ty::Bool)
    }

    pub fn build_sgreater_than(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::SGreaterThan { lhs, rhs };
        self.ctx.add_inst(self.position, inst, Ty::Bool)
    }

    pub fn build_ugreater_than(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::UGreaterThan { lhs, rhs };
        self.ctx.add_inst(self.position, inst, Ty::Bool)
    }

    pub fn build_logical_or(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::LogicalOr { lhs, rhs };
        self.ctx.add_inst(self.position, inst, Ty::Bool)
    }

    pub fn build_logical_and(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::LogicalAnd { lhs, rhs };
        self.ctx.add_inst(self.position, inst, Ty::Bool)
    }

    pub fn build_bitwise_or(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::BitwiseOr { lhs, rhs };

        let ty = self.ctx.get_value_ty(lhs);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_bitwise_and(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::BitwiseAnd { lhs, rhs };

        let ty = self.ctx.get_value_ty(lhs);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_bitwise_xor(&mut self, lhs: Value, rhs: Value) -> Value {
        let inst = InstValue::BitwiseXor { lhs, rhs };

        let ty = self.ctx.get_value_ty(lhs);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_bitwise_not(&mut self, value: Value) -> Value {
        let inst = InstValue::BitwiseNot { value };

        let ty = self.ctx.get_value_ty(value);
        self.ctx.add_inst(self.position, inst, ty)
    }

    pub fn build_return_value(&mut self, value: Value) {
        self.ctx
            .terminate_block(self.position, Terminator::Return { value })
    }

    pub fn build_return(&mut self) {
        self.ctx
            .terminate_block(self.position, Terminator::ReturnNone)
    }

    pub fn build_jump(&mut self, target: Block) {
        self.ctx
            .terminate_block(self.position, Terminator::Jump { target })
    }

    pub fn build_branch(&mut self, cond: Value, true_target: Block, false_target: Block) {
        self.ctx.terminate_block(
            self.position,
            Terminator::Branch {
                cond,
                true_target,
                false_target,
            },
        )
    }
}
