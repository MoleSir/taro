use crate::{execute::ExecuteResult, Chunk, ShrString, Value};

use super::ObjectHandle;


pub struct ObjectFunction {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: ShrString,
}

pub type BuiltinFn = fn (&[Value]) -> ExecuteResult<Value>;

pub struct ObjectBuiltinFn {
    pub function: BuiltinFn,
}

pub struct ObjectUpvalue {

}

pub struct ObjectClass {

}

pub struct ObjectClosure {
    pub function: ObjectHandle,
    pub upvalues: Vec<ObjectHandle>, 
}

pub struct ObjectInstance {
    pub klass: ObjectHandle,
}

pub struct ObjectBoundMethod {
    pub receiver: Value,
    pub method: ObjectHandle,
}
