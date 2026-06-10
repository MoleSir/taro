use crate::ChunkError;

#[thiserrorctx::context_error]
pub enum ExecuteError {
    #[error(transparent)]
    Chunk(#[from] ChunkError),

    #[error("Divide by zero")]
    DivideByZero,

    #[error("Unexpect empty stack")]
    StackEmpty,

    #[error("Stack index {0} out of range")]
    StackIndexOutOfRange(usize),

    #[error("Type mismatch in unary op {0} for {1}")]
    UnaryOpTypeMismatch(&'static str, &'static str),

    #[error("Type mismatch in binary op {0} with {1} and {2}")]
    BinaryOpTypeMismatch(&'static str, &'static str, &'static str),

    #[error("expect type {0}, not got {1}")]
    UnexpectType(&'static str, &'static str),

    #[error("Variable not found")]
    VariableNotFound(String),
}