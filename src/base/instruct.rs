use num_enum::TryFromPrimitive;

use super::{ShrString, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum ByteCode {
    Return = 0,
    Pop,
    Nil,
    True,
    False,
    Negate,
    Not,
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    Constant,
    DefineGlobal,
    GetGlobal,
    SetGlobal,
    GetLocal,
    SetLocal,

    JumpIfFalse,
    Jump,
    Loop,

    Call,

    Closure,

    GetUpvalue,
    SetUpvalue,
    CloseUpvalue,

    Class,
    SetProperty,
    GetProperty,   
    Method,
}

/// Descriptor for a single upvalue captured by a closure.
#[derive(Debug, Clone)]
pub struct UpvalueDesc {
    /// `true` → references a stack slot of the enclosing function directly.
    /// `false` → references an upvalue of the enclosing closure.
    pub is_local: bool,
    /// Slot index in the enclosing function, or upvalue index if `!is_local`.
    pub index: usize,
}

/// High-level instruction with resolved parameters.
///
/// The VM and compiler operate on [`Instruction`] directly.
/// [`ByteCode`] is only used internally inside [`Chunk`](super::Chunk)
/// to encode / decode the compact byte representation.
#[derive(Debug, Clone)]
pub enum Instruction {
    Return,
    Pop,
    Nil,
    True,
    False,
    Negate,
    Not,
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    Constant(Value),

    DefineGlobal(ShrString),
    GetGlobal(ShrString),
    SetGlobal(ShrString),

    GetLocal(usize),
    SetLocal(usize),

    JumpIfFalse(usize),
    Jump(usize),
    Loop(usize),

    Call(usize),

    Closure {
        function: Value,
        upvalues: Vec<UpvalueDesc>,
    },

    GetUpvalue(usize),
    SetUpvalue(usize),
    CloseUpvalue,

    Class(ShrString),
    SetProperty(ShrString),
    GetProperty(ShrString),  
    Method(ShrString),
}