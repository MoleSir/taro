use std::collections::HashMap;

use crate::{execute::{ExecuteResult, VirtualMachine}, Chunk, ShrString, Value};
use super::ObjectHandle;

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

pub type BuiltinFn = fn (&mut VirtualMachine, arg_count: usize) -> ExecuteResult<Value>;

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
    pub closed: Value,
    /// Intrusive linked list: the next open upvalue that refers to the same
    /// stack slot (or to a slot below this one).  Used by the VM to find all
    /// upvalues that need to be closed when a local goes out of scope.
    pub next: Option<ObjectHandle>,
}

pub struct ObjectClass {
    pub name: ShrString,
    pub methods: HashMap<ShrString, ObjectHandle>,
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
    pub fields: HashMap<ShrString, Value>,
}

impl ObjectInstance {
    pub fn new(class: ObjectHandle) -> Self {
        Self {
            class,
            fields: HashMap::new(),
        }
    }
}

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

pub struct ObjectBoundMethod {
    pub receiver: Value,
    pub method: ObjectHandle,
}

impl ObjectBoundMethod {
    pub fn new(receiver: Value, method: ObjectHandle) -> Self {
        Self { receiver, method }
    }
}