use crate::{ObjectHandle, ShrString};
use num_enum::TryFromPrimitive;

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
    Mod,
    FloorDiv,
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
    CallKw,

    Closure,

    GetUpvalue,
    SetUpvalue,
    CloseUpvalue,

    Class,
    SetProperty,
    GetProperty,
    Method,
    Inherit,
    SuperInvoke,

    BuildList,
    BuildDict,
    BuildSet,
    IndexGet,
    IndexSet,
    Import,

    IterEnd,
    ForInIter,
    ForInNext,
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
    Mod,
    FloorDiv,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    Constant(ObjectHandle),

    DefineGlobal(ShrString),
    GetGlobal(ShrString),
    SetGlobal(ShrString),

    GetLocal(usize),
    SetLocal(usize),

    JumpIfFalse(usize),
    Jump(usize),
    Loop(usize),

    Call(usize),
    /// Function call with keyword arguments.
    /// `pos_count` positional args followed by `kw_count` keyword args.
    /// `kw_names` lists the parameter names for each keyword argument, in order.
    CallKw {
        pos_count: usize,
        kw_count: usize,
        kw_names: Vec<ShrString>,
    },

    Closure {
        function: ObjectHandle,
        upvalues: Vec<UpvalueDesc>,
    },

    GetUpvalue(usize),
    SetUpvalue(usize),
    CloseUpvalue,

    Class(ShrString),
    SetProperty(ShrString),
    GetProperty(ShrString),
    Method(ShrString),
    SuperInvoke(ShrString, usize),
    Inherit,

    BuildList(usize),
    BuildDict(usize),
    BuildSet(usize),
    IndexGet,
    IndexSet,

    Import(ShrString),

    IterEnd,
    ForInIter,
    ForInNext(usize),
}
