use num_enum::TryFromPrimitive;

use super::{ShrString, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum ByteCode {
    Return = 0,
    Print,
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
}

/// High-level instruction with resolved parameters.
///
/// The VM and compiler operate on [`Instruction`] directly.
/// [`ByteCode`] is only used internally inside [`Chunk`](super::Chunk)
/// to encode / decode the compact byte representation.
#[derive(Debug, Clone)]
pub enum Instruction {
    Return,
    Print,
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
}