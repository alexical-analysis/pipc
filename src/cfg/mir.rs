#[derive(Clone, Copy)]
pub enum Ty {
    I32,
    U32,
    Bool,
    Unit,
}

#[derive(Clone, Copy)]
pub struct Local(u32);

impl Local {
    pub fn new(value: usize) -> Self {
        Self(value as u32)
    }

    pub fn idx(&self) -> usize {
        self.0 as usize
    }
}

pub struct LocalValue {
    pub name: String,
    pub ty: Ty,
}

impl LocalValue {
    pub fn new(name: String, ty: Ty) -> Self {
        Self { name, ty }
    }
}

#[derive(Clone, Copy)]
pub struct Global(u32);

impl Global {
    pub fn new(value: usize) -> Self {
        Self(value as u32)
    }

    pub fn idx(&self) -> usize {
        self.0 as usize
    }
}

pub struct GlobalValue {
    pub name: String,
    pub ty: Ty,
}

impl GlobalValue {
    pub fn new(name: String, ty: Ty) -> Self {
        Self { name, ty }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Value(u32);

impl Value {
    pub fn new(value: usize) -> Self {
        Self(value as u32)
    }

    pub fn idx(&self) -> usize {
        self.0 as usize
    }
}

pub enum ConstValue {
    I16(i16),
    U16(u16),
    Bool(bool),
}

#[derive(Clone, Copy)]
pub enum Place {
    Local(Local),
    Global(Global),
}

pub enum InstValue {
    Param { index: u32 },
    Const { value: ConstValue },
    Load { place: Place },
    Store { place: Place, value: Value },
    Call { func: Func, args: Vec<Value> },
    Add { lhs: Value, rhs: Value },
    Sub { lhs: Value, rhs: Value },
    Mul { lhs: Value, rhs: Value },
    SDiv { lhs: Value, rhs: Value },
    UDiv { lhs: Value, rhs: Value },
    Equal { lhs: Value, rhs: Value },
    NotEqual { lhs: Value, rhs: Value },
    SLessThan { lhs: Value, rhs: Value },
    ULessThan { lhs: Value, rhs: Value },
    SGreaterThan { lhs: Value, rhs: Value },
    UGreaterThan { lhs: Value, rhs: Value },
    LogicalOr { lhs: Value, rhs: Value },
    LogicalAnd { lhs: Value, rhs: Value },
    BitwiseOr { lhs: Value, rhs: Value },
    BitwiseAnd { lhs: Value, rhs: Value },
    BitwiseXor { lhs: Value, rhs: Value },
    BitwiseNot { value: Value },
}

#[derive(Clone, Copy)]
pub struct Func(u32);

impl Func {
    pub fn new(value: usize) -> Self {
        Self(value as u32)
    }

    pub fn idx(&self) -> usize {
        self.0 as usize
    }
}

pub struct FuncValue {
    name: String,
    params: Vec<Value>,
    return_ty: Ty,
    locals: Vec<Local>,
    blocks: Vec<Block>,
}

impl FuncValue {
    pub fn new(name: String, params: Vec<Value>, return_ty: Ty) -> Self {
        Self {
            name,
            params,
            return_ty,
            locals: Vec::new(),
            blocks: Vec::new(),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_return_ty(&self) -> Ty {
        self.return_ty
    }

    pub fn add_local(&mut self, local: Local) {
        self.locals.push(local)
    }

    pub fn get_nth_param(&self, n: usize) -> Value {
        *self.params.get(n).expect("failed to get nth param")
    }

    pub fn append_block(&mut self, block: Block) {
        self.blocks.push(block)
    }
}

pub enum Terminator {
    Return {
        value: Value,
    },
    Jump {
        target: Block,
    },
    Branch {
        cond: Value,
        true_target: Block,
        false_target: Block,
    },
    ReturnNone,
    None,
}

#[derive(Clone, Copy)]
pub struct Block(u32);

impl Block {
    pub fn new(value: usize) -> Self {
        Self(value as u32)
    }

    pub fn idx(&self) -> usize {
        self.0 as usize
    }
}

pub struct BlockValue {
    pub inst: Vec<Value>,
    pub terminator: Terminator,
}

impl BlockValue {
    pub fn new() -> Self {
        Self {
            inst: Vec::new(),
            terminator: Terminator::None,
        }
    }
}
