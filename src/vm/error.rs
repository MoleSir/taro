use crate::{compile::CompileError, ChunkError, ObjectError};

#[derive(Debug)]
pub enum InterpretError {
    Compile(CompileError),
    Runtime(ExecuteError),
}

#[thiserrorctx::context_error]
pub enum ExecuteError {
    #[error(transparent)]
    Object(#[from] ObjectError),

    #[error(transparent)]
    Chunk(#[from] ChunkError),

    #[error("Divide by zero")]
    DivideByZero,

    #[error("Unexpect empty stack")]
    StackEmpty,

    #[error("Unexpect empty frame")]
    CallFrameEmpty,

    #[error("Stack index {0} out of range")]
    StackIndexOutOfRange(usize),

    #[error("Type mismatch in unary op {0} for {1}")]
    UnaryOpTypeMismatch(&'static str, &'static str),

    #[error("Type mismatch in binary op {0} with {1} and {2}")]
    BinaryOpTypeMismatch(&'static str, &'static str, &'static str),

    #[error("expect type {0}, not got {1}")]
    UnexpectType(&'static str, &'static str),

    #[error("Variable '{0}' not found")]
    VariableNotFound(String),

    #[error("Can't call {0}")]
    CanNotCall(&'static str),

    #[error("Expected {expcted} arguments but got {got}")]
    ArgmentCountUnmatch { expcted: usize, got: usize },

    #[error("no superclass to call super method on")]
    NoSuperclass,

    #[error("Undefined property {0}")]
    UndefinedProperty(String),

    #[error("__str__ method must return a string, got '{0}'")]
    BadStrResult(&'static str),

    #[error("__bool__ method must return a bool, got '{0}'")]
    BadBoolResult(&'static str),

    #[error("__len__ method must return an integer, got '{0}'")]
    BadLenResult(&'static str),

    #[error("__int__ method must return an integer, got '{0}'")]
    BadIntResult(&'static str),

    #[error("__float__ method must return a float, got '{0}'")]
    BadFloatResult(&'static str),

    #[error("I/O error: {0}")]
    IoError(String),
}