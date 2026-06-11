use crate::{execute::ExecuteResult, Chunk, ShrString, Value};
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

pub type BuiltinFn = fn (&[Value]) -> ExecuteResult<Value>;

pub struct ObjectBuiltinFn {
    pub function: BuiltinFn,
}

impl ObjectBuiltinFn {
    pub fn new(function: BuiltinFn) -> Self {
        Self { function }
    }
}

pub struct ObjectUpvalue {

}

pub struct ObjectClass {

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

pub struct ObjectInstance {
    pub klass: ObjectHandle,
}

pub struct ObjectBoundMethod {
    pub receiver: Value,
    pub method: ObjectHandle,
}
