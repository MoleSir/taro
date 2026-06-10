mod string;
pub use string::*;
mod function;
pub use function::*;
use std::fmt;

use crate::execute::{ExecuteError, ExecuteResult};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Float(f64),
    Integer(i64),
    Bool(bool),
    Nil,
    String(ShrString),
}

impl Value {
    pub fn as_string(&self) -> ExecuteResult<ShrString> {
        if let Value::String(s) = self {
            Ok(s.clone())
        } else {
            Err(ExecuteError::UnexpectType("string", self.type_name()))
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
        }
    }
}