use std::collections::HashMap;

use crate::{vm::{ExecuteResult, VirtualMachine}, ShrString};
use super::ObjectHandle;

// ========================================================================== //
//                    Method (unified user + builtin)
// ========================================================================== //

/// A callable method — either a user-defined closure or a Rust builtin.
#[derive(Copy, Clone)]
pub enum Method {
    /// User-defined method (closure handle, compiled from Taro source).
    User(ObjectHandle),
    /// Builtin method (Rust function pointer).
    Builtin(BuiltinFn),
}

// ========================================================================== //
//                    BuiltinInstance
// ========================================================================== //

/// Inline data for all builtin / primitive types.
#[derive(Debug, Clone)]
pub enum BuiltinInstance {
    Nil,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(ShrString),
    List(Vec<ObjectHandle>),
    /// Dict uses linear search (identity handles differ for equal values).
    Dict(Vec<(ObjectHandle, ObjectHandle)>),
}

pub struct ObjectBuiltinInstance {
    pub data: BuiltinInstance,
}

impl ObjectBuiltinInstance {
    pub fn new(data: BuiltinInstance) -> Self {
        Self { data }
    }
}

// ========================================================================== //
//                    Function, BuiltinFn, Upvalue
// ========================================================================== //

pub struct ObjectFunction {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: ShrString,
}

impl ObjectFunction {
    pub fn new(name: impl Into<ShrString>, arity: usize, chunk: Chunk) -> Self {
        Self { arity, name: name.into(), chunk }
    }
}

pub type BuiltinFn = fn (&mut VirtualMachine, arg_count: usize) -> ExecuteResult<ObjectHandle>;

pub struct ObjectBuiltinFn {
    pub name: &'static str,
    pub function: BuiltinFn,
}

impl ObjectBuiltinFn {
    pub fn new(name: &'static str, function: BuiltinFn) -> Self {
        Self { name, function }
    }
}

pub struct ObjectUpvalue {
    /// Stack slot index when the upvalue is still "open" (the local variable
    /// is alive on the stack).  Set to `None` once the variable goes out of
    /// scope and the upvalue is "closed" — the value has been moved into
    /// `closed`.
    pub location: Option<usize>,
    pub closed: ObjectHandle,
    /// Intrusive linked list: the next open upvalue that refers to the same
    /// stack slot (or to a slot below this one).  Used by the VM to find all
    /// upvalues that need to be closed when a local goes out of scope.
    pub next: Option<ObjectHandle>,
}

// ========================================================================== //
//                    Class, Instance
// ========================================================================== //

pub struct ObjectClass {
    pub name: ShrString,
    pub methods: HashMap<ShrString, Method>,
    pub superclass: Option<ObjectHandle>,
}

impl ObjectClass {
    pub fn new(name: impl Into<ShrString>) -> Self {
        Self {
            name: name.into(),
            methods: HashMap::new(),
            superclass: None,
        }
    }
}

pub struct ObjectInstance {
    pub class: ObjectHandle,
    pub fields: HashMap<ShrString, ObjectHandle>,
}

impl ObjectInstance {
    pub fn new(class: ObjectHandle) -> Self {
        Self {
            class,
            fields: HashMap::new(),
        }
    }
}

// ========================================================================== //
//                    Closure
// ========================================================================== //

pub struct ObjectClosure {
    pub function: ObjectHandle,
    pub upvalues: Vec<ObjectHandle>,
}

impl ObjectClosure {
    pub fn new(function: ObjectHandle) -> Self {
        Self {
            function,
            upvalues: vec![],
        }
    }
}

// ========================================================================== //
//                    BoundMethod
// ========================================================================== //

pub struct ObjectBoundMethod {
    pub receiver: ObjectHandle,
    pub method: Method,
}

impl ObjectBoundMethod {
    pub fn new(receiver: ObjectHandle, method: Method) -> Self {
        Self { receiver, method }
    }
}

// Re-export Chunk for use in ObjectFunction.
use crate::Chunk;
