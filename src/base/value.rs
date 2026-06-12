use std::{fmt, hash::{Hash, Hasher}};
use crate::vm::{ExecuteError, ExecuteResult};
use super::{ObjectHandle, ShrString};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Float(f64),
    Integer(i64),
    Bool(bool),
    String(ShrString),
    Object(ObjectHandle),
}

impl Value {
    pub fn as_string(&self) -> ExecuteResult<ShrString> {
        if let Value::String(s) = self {
            Ok(s.clone())
        } else {
            Err(ExecuteError::UnexpectType("string", self.type_name()))
        }
    }

    pub fn as_object(&self) -> ExecuteResult<ObjectHandle> {
        if let Value::Object(h) = self {
            Ok(*h)
        } else {
            Err(ExecuteError::UnexpectType("object", self.type_name()))
        }
    }
}

#[macro_export]
macro_rules! vf {
    ($e:expr) => { crate::Value::Float($e) };
}

// ========================================================================== //
//  From conversions
// ========================================================================== //

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<()> for Value {
    fn from(_: ()) -> Self {
        Value::Nil
    }
}

impl From<ShrString> for Value {
    fn from(s: ShrString) -> Self {
        Value::String(s)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s.into())
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string().into())
    }
}

impl From<ObjectHandle> for Value {
    fn from(h: ObjectHandle) -> Self {
        Value::Object(h)
    }
}

// ========================================================================== //
//  Utility methods
// ========================================================================== //

impl Value {
    /// Return a human-readable type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Float(_) => "Float",
            Value::Integer(_) => "Integer",
            Value::Bool(_) => "Boolean",
            Value::Nil => "Nil",
            Value::String(_) => "String",
            Value::Object(_) => "Object",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Float(v) => write!(f, "{}", v),
            Value::Integer(v) => write!(f, "{}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Nil => write!(f, "nil"),
            Value::String(s) => write!(f, "{}", s.as_str()),
            Value::Object(h) => writeln!(f, "Object({})", h.0),
        }
    }
}

// 手动实现 Hash
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Nil => 0u8.hash(state),
            Value::Float(f) => {
                1u8.hash(state);
                let bits = if *f == 0.0 { 0.0f64.to_bits() } else { f.to_bits() };
                bits.hash(state);
            }
            Value::Integer(i) => {
                2u8.hash(state);
                i.hash(state);
            }
            Value::Bool(b) => {
                3u8.hash(state);
                b.hash(state);
            }
            Value::String(s) => {
                4u8.hash(state);
                s.as_str().hash(state);
            }
            Value::Object(h) => {
                5u8.hash(state);
                h.hash(state);
            }
        }
    }
}

impl Eq for Value {}