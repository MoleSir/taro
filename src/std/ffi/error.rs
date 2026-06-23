use crate::vm::RuntimeErrorKind;

#[derive(Debug, thiserror::Error)]
pub(super) enum FfiError {
    // ---- type resolution ----
    #[error("unknown C type '{0}'")]
    UnknownCType(String),
    #[error("void has no size or alignment")]
    VoidNoSize,
    #[error("cannot marshal void as argument")]
    VoidAsArgument,
    #[error("void is not a valid struct field type")]
    VoidAsField,
    #[error("struct return type not yet supported")]
    StructReturnUnsupported,
    #[error("expected a type string or CType object, got {0}")]
    ExpectedType(String),

    // ---- struct definition ----
    #[error("define_struct: expected a list of field descriptors")]
    StructDefExpectedList,
    #[error("define_struct: expected a list of types or [name, type] pairs")]
    StructDefInvalidFormat,
    #[error("define_struct: field list must not be empty")]
    StructDefEmptyList,
    #[error("define_struct: expected [name, type] pair at position {0}")]
    StructDefExpectedPair(usize),
    #[error("define_struct: each pair must be [name, type], got {0} elements at position {1}")]
    StructDefPairLen(usize, usize),
    #[error("define_struct: field name at position {0} must be a string")]
    StructDefNameNotString(usize),
    #[error("define_struct: invalid type at position {pos}: {reason}")]
    StructDefInvalidType { pos: usize, reason: String },

    // ---- struct layout ----
    #[error("internal layout error: {0}")]
    Layout(String),

    // ---- struct instance ----
    #[error("struct expects {expected} value(s), got {got}")]
    StructArgCount { expected: usize, got: usize },
    #[error("struct field '{0}' not found")]
    StructFieldNotFound(String),
    #[error("struct field '{name}': {error}")]
    StructFieldError { name: String, error: String },
    #[error("struct has no field '{0}'")]
    StructNoField(String),
    #[error("Struct cannot be constructed directly; use ffi.define_struct() to create a struct type")]
    StructDirectConstruction,

    #[error("CSymbol cannot be constructed directly")]
    CSymbolDirectConstruction,

    #[error("CType cannot be constructed directly")]
    CTypeDirectConstruction,

    // ---- struct accessors ----
    #[error("__getattr__ requires 2 arguments (self, name)")]
    GetAttrArgCount,
    #[error("__getattr__: not a struct instance")]
    GetAttrNotStruct,
    #[error("__setattr__ requires 3 arguments (self, name, value)")]
    SetAttrArgCount,
    #[error("__setattr__: not a struct instance")]
    SetAttrNotStruct,

    // ---- marshal ----
    #[error("expected a struct instance")]
    ExpectedStruct,
    #[error("expected a struct instance for nested field")]
    ExpectedNestedStruct,
    #[error("nested struct back-link is not a struct type")]
    NestedNotStruct,
    #[error("CString conversion error: {0}")]
    CString(String),
    #[error("expected a number, got {0}")]
    ExpectedNumber(String),
    #[error("argument {idx}: {reason}")]
    MarshalArg { idx: usize, reason: String },

    // ---- CType.__call__ ----
    #[error("CType.__call__: missing self")]
    CTypeCallMissingSelf,
    #[error("CType.__call__: self is not a CType")]
    CTypeCallNotCType,
    #[error("scalar CType expects exactly 1 value, got {0}")]
    ScalarArgCount(usize),
    #[error("CType class not found in ffi module")]
    CTypeClassNotFound,
    #[error("Struct class not found in ffi module")]
    StructClassNotFound,

    // ---- BoundFn ----
    #[error("bound function call: missing self")]
    BoundFnMissingSelf,
    #[error("bound function call: self is not a BoundFn")]
    BoundFnNotBoundFn,
    #[error("bound function expects {expected} argument(s), got {got}")]
    BoundFnArgCount { expected: usize, got: usize },
    #[error("BoundFn cannot be constructed directly; use ffi.bind()")]
    BoundFnDirectConstruction,

    // ---- bind ----
    #[error("bind: not a library handle")]
    BindNotLibrary,
    #[error("bind: argument types must be a list")]
    BindArgTypesNotList,

    // ---- ffi.call ----
    #[error("ffi.call requires at least 3 arguments (func_ptr, ret_type, arg_types[, args])")]
    CallTooFewArgs,

    // ---- library ----
    #[error("dlopen: {0}")]
    DlOpen(String),
    #[error("dlsym: not a library handle")]
    DlSymNotLibrary,
    #[error("dlsym('{name}'): {error}")]
    DlSym { name: String, error: String },
}

impl From<FfiError> for RuntimeErrorKind {
    fn from(e: FfiError) -> Self {
        RuntimeErrorKind::FfiError(e.to_string())
    }
}
