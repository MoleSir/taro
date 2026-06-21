use crate::{compile::CompileError, ChunkError};

#[derive(Debug)]
pub enum InterpretError {
    Compile(CompileError),
    Runtime(RuntimeError),
}

impl std::fmt::Display for InterpretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpretError::Compile(e) => write!(f, "{e:?}"),
            InterpretError::Runtime(e) => write!(f, "{e}"),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(col)) => write!(f, "[line {line}:{col}] {}", self.reason),
            (Some(line), None) => write!(f, "[line {line}] {}", self.reason),
            (None, _) => write!(f, "{}", self.reason),
        }
    }
}

pub type RuntimeResult<T> = Result<T, RuntimeErrorKind>;

#[derive(Debug)]
pub struct RuntimeError {
    pub line: Option<usize>,
    /// 1-based column within the source line (if available).
    pub column: Option<usize>,
    pub reason: RuntimeErrorKind,
}

// impl From<ChunkError> for RuntimeError {
//     fn from(e: ChunkError) -> Self {
//         RuntimeError { line: None, column: None, reason: RuntimeErrorKind::Chunk(e) }
//     }
// }

// impl From<RuntimeErrorKind> for RuntimeError {
//     fn from(reason: RuntimeErrorKind) -> Self {
//         RuntimeError { line: None, column: None, reason }
//     }
// }

#[derive(Debug, thiserror::Error)]
pub enum RuntimeErrorKind {
    #[error(transparent)]
    Chunk(#[from] ChunkError),

    #[error("Divide by zero")]
    DivideByZero,

    #[error("Unexpected empty stack")]
    StackEmpty,

    #[error("Unexpected empty frame")]
    CallFrameEmpty,

    #[error("Stack index {0} out of range")]
    StackIndexOutOfRange(usize),

    #[error("bad operand type for unary {0}: '{1}'")]
    UnaryOpTypeMismatch(&'static str, &'static str),

    #[error("unsupported operand type(s) for {0}: '{1}' and '{2}'")]
    BinaryOpTypeMismatch(&'static str, &'static str, &'static str),

    #[error("'{1}' is not {0}")]
    UnexpectedType(&'static str, &'static str),

    #[error("Variable '{0}' not found")]
    VariableNotFound(String),

    #[error("Can't call {0}")]
    CanNotCall(&'static str),

    #[error("Expected {expected} arguments but got {got}")]
    ArgumentCountMismatch { expected: usize, got: usize },

    #[error("Expected {min}..{max} arguments but got {got}")]
    ArgumentCountRange { min: usize, max: usize, got: usize },

    #[error("unknown keyword argument '{0}'")]
    UnknownKeywordArg(String),

    #[error("got multiple values for argument '{0}'")]
    DuplicateKeywordArg(String),

    #[error("missing required argument '{0}'")]
    MissingArgument(String),

    #[error("no superclass to call super method on")]
    NoSuperclass,

    #[error("Undefined property {0}")]
    UndefinedProperty(String),

    #[error("Cannot set property on {0}")]
    CannotSetProperty(&'static str),

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

    #[error("index {0} out of range (len = {1})")]
    IndexOutOfRange(i64, usize),

    #[error("key not found in dict")]
    KeyNotFound,

    #[error("cannot pop from empty list")]
    EmptyPop,

    #[error("expected {expected}, got {found}")]
    TypeMismatch { expected: &'static str, found: &'static str },

    #[error("class '{0}' not implement '{1}' method")]
    NoImplementMethod(String, &'static str),

    #[error("unsupport method call '{0}' for {1}")]
    UnsupportedMethodCall(&'static str, &'static str),

    #[error("import error: {0}")]
    ImportError(String),

    #[error("random error: {0}")]
    RandomError(String),

    #[error("net error: {0}")]
    NetError(String),

    #[error("time error: {0}")]
    TimeError(String),

    #[error("os error: {0}")]
    OsError(String),

    #[error("json error: {0}")]
    JosnError(String),

    #[error("FFI error: {0}")]
    FfiError(String),
}
