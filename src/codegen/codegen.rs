use std::collections::HashMap;

use crate::{
    cfg::mir::{BlockValue, ConstValue, FuncValue, InstValue, Place, Value},
    ctx::ctx::GlobalCtx,
};

enum Addr {
    ROData(u16),
}

pub struct Gen<'ctx> {
    ctx: &'ctx GlobalCtx,
    read_only_section: Vec<u16>,
    // addr map maps a value to it's final location in memory
    addr_map: HashMap<Value, Addr>,
}

impl<'ctx> Gen<'ctx> {
    pub fn new(ctx: &'ctx GlobalCtx) -> Self {
        Self {
            ctx,
            read_only_section: Vec::new(),
            addr_map: HashMap::new(),
        }
    }

    pub fn generate(&mut self) -> Vec<u16> {
        let exec = Vec::new();

        for func in self.ctx.get_funcs() {
            self.generate_func(func);
        }

        exec
    }

    fn generate_func(&mut self, func: &FuncValue) -> Vec<u16> {
        todo!("generate_func")
    }

    fn generate_block(&mut self, block: &BlockValue) -> Vec<u16> {
        let mut code_block = Vec::with_capacity(block.inst.len());

        for &value in &block.inst {
            let inst = self.ctx.get_inst(value);
            match inst {
                InstValue::Param { index } => todo!("what do I do with these?"),
                InstValue::Const { value: v } => self.generate_const(value, v),
                InstValue::Load { place } => self.generate_load(value, *place),
                InstValue::Store { place, value } => todo!("store place"),
            };
        }

        code_block
    }

    fn generate_const(&mut self, value: Value, const_value: &ConstValue) {
        let fill = match const_value {
            ConstValue::Bool(v) => match v {
                &true => 0x00_01,
                &false => 0x00_00,
            },
            ConstValue::I16(v) => *v as u16,
            ConstValue::U16(v) => *v,
        };

        let idx = self.read_only_section.len();
        self.read_only_section.push(fill);

        self.addr_map.insert(value, Addr::ROData(idx as u16));
    }

    fn generate_load(&mut self, value: Value, place: Place) {
        match place {
            Place::Local(p) => todo!("load local"),
            Place::Global(p) => todo!("load global"),
        }

        todo!("generate_load")
    }
}
