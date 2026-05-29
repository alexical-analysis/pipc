use crate::cfg::builder::Builder;
use crate::cfg::mir::{
    Block, BlockValue, Func, FuncValue, Global, GlobalValue, InstValue, Local, LocalValue,
    Terminator, Ty, Value,
};

pub struct GlobalCtx {
    locals: Vec<LocalValue>,
    globals: Vec<GlobalValue>,
    insts: Vec<InstValue>,
    value_tys: Vec<Ty>,
    funcs: Vec<FuncValue>,
    blocks: Vec<BlockValue>,
}

impl GlobalCtx {
    pub fn new() -> Self {
        Self {
            locals: Vec::new(),
            globals: Vec::new(),
            insts: Vec::new(),
            value_tys: Vec::new(),
            funcs: Vec::new(),
            blocks: Vec::new(),
        }
    }

    pub fn create_builder<'ctx>(&'ctx mut self) -> Builder<'ctx> {
        Builder::new(self)
    }

    pub fn add_param(&mut self, index: u32, ty: Ty) -> Value {
        let idx = self.insts.len();
        self.insts.push(InstValue::Param { index });
        self.value_tys.push(ty);

        Value::new(idx)
    }

    pub fn add_inst(&mut self, block: Block, inst: InstValue, ty: Ty) -> Value {
        let block_value = self
            .blocks
            .get_mut(block.idx())
            .expect("failed to find block");

        let idx = self.insts.len();
        self.insts.push(inst);
        self.value_tys.push(ty);
        let value = Value::new(idx);

        block_value.inst.push(value);
        value
    }

    pub fn get_inst(&self, inst: Value) -> &InstValue {
        self.insts
            .get(inst.idx())
            .expect("failed to find instruction value")
    }

    pub fn get_value_ty(&self, value: Value) -> Ty {
        *self
            .value_tys
            .get(value.idx())
            .expect("failed to get value type")
    }

    pub fn terminate_block(&mut self, block: Block, terminator: Terminator) {
        let block_value = self
            .blocks
            .get_mut(block.idx())
            .expect("failed to find block");

        block_value.terminator = terminator
    }

    pub fn add_global(&mut self, global: GlobalValue) -> Global {
        let idx = self.globals.len();
        self.globals.push(global);

        Global::new(idx)
    }

    pub fn get_global_ty(&self, global: Global) -> Ty {
        let global = self
            .globals
            .get(global.idx())
            .expect("failed to find global value");

        global.ty
    }

    pub fn add_local(&mut self, local: LocalValue) -> Local {
        let idx = self.locals.len();
        self.locals.push(local);

        Local::new(idx)
    }

    pub fn get_local_ty(&self, local: Local) -> Ty {
        let local = self
            .locals
            .get(local.idx())
            .expect("failed to find local value");

        local.ty
    }

    pub fn add_func(&mut self, func: FuncValue) -> Func {
        let idx = self.funcs.len();
        self.funcs.push(func);

        Func::new(idx)
    }

    pub fn get_func_return_ty(&self, func: Func) -> Ty {
        let func_value = self.funcs.get(func.idx()).expect("failed to find function");
        func_value.get_return_ty()
    }

    pub fn append_block(&mut self, func: Func) -> Block {
        let idx = self.blocks.len();
        self.blocks.push(BlockValue::new());
        let block = Block::new(idx);

        let func_value = self
            .funcs
            .get_mut(func.idx())
            .expect("failed to find function for append_block");

        func_value.append_block(block);

        block
    }

    pub fn get_nth_param(&self, func: Func, n: usize) -> Value {
        let func_value = self
            .funcs
            .get(func.idx())
            .expect("failed to get function for get_nth_param");

        func_value.get_nth_param(n)
    }

    pub fn get_funcs(&self) -> &[FuncValue] {
        &self.funcs
    }
}
