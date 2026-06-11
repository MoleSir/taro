use crate::execute::{ExecuteError, ExecuteResult};
use super::{Value, VirtualMachine};

impl VirtualMachine {
    pub fn neg(&mut self, value: &Value) -> ExecuteResult<Value> {
        match value {
            Value::Float(v) => Ok(Value::Float(-*v)),
            Value::Integer(v) => Ok(Value::Integer(v.wrapping_neg())),
            Value::Bool(v) => Ok(Value::Bool(!*v)),
            other => Err(ExecuteError::UnaryOpTypeMismatch("neg", other.type_name())),
        }
    }

    pub fn not(&mut self, value: &Value) -> ExecuteResult<Value> {
        match value {
            Value::Nil => Ok(Value::Bool(true)),
            Value::Float(v) => Ok(Value::Float(-*v)),
            Value::Integer(v) => Ok(Value::Integer(v.wrapping_neg())),
            Value::Bool(v) => Ok(Value::Bool(!*v)),
            other => Err(ExecuteError::UnaryOpTypeMismatch("not", other.type_name())),
        }
    }

    pub fn add(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            // Same numeric types
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l.wrapping_add(*r))),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l + r)),
            // Cross-type numbers → promote to Float
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 + r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l + *r as f64)),
            // String concatenation
            (Value::String(l), Value::String(r)) => {
                let result = format!("{}{}", l.as_str(), r.as_str());
                Ok(Value::String(result.into()))
            }
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("add", lhs.type_name(), rhs.type_name()))
        }
    }

    pub fn sub(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l.wrapping_sub(*r))),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l - r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 - r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l - *r as f64)),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("sub", lhs.type_name(), rhs.type_name()))
        }
    }

    pub fn mul(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l.wrapping_mul(*r))),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l * r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 * r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l * *r as f64)),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("mul", lhs.type_name(), rhs.type_name()))
        }
    }

    pub fn div(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(..), Value::Integer(0)) | (Value::Float(..), Value::Float(0.0)) => {
                Err(ExecuteError::DivideByZero)
            }
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Float(*l as f64 / *r as f64)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l / r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 / r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l / *r as f64)),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("div", lhs.type_name(), rhs.type_name()))
        }
    }

    pub fn eq(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Nil, Value::Nil) => Ok(Value::Bool(true)),
            (Value::Bool(l), Value::Bool(r)) => Ok(Value::Bool(l == r)),
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l == r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l == r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 == *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l == *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() == r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("eq", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn ne(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Nil, Value::Nil) => Ok(Value::Bool(false)),
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l != r)),
            (Value::Bool(l), Value::Bool(r)) => Ok(Value::Bool(l != r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l != r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 != *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l != *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() != r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("ne", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn gt(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l > r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l > r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 > *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l > *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() > r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("gt", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn ge(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l >= r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l >= r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 >= *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l >= *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() >= r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("ge", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn lt(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l < r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l < r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool((*l as f64) < *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l < *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() < r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("lt", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn le(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l <= r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l <= r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 <= *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l <= *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() <= r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("le", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn is_truthy(value: &Value) -> bool {
        match value {
            Value::Nil => false,
            Value::Bool(false) => false,
            Value::Float(v) => *v != 0.0,
            Value::Integer(v) => *v != 0,
            _ => true,
        }
    }
}
