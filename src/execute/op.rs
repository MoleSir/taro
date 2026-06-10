use crate::execute::{ExecuteError, ExecuteResult};

use super::Value;

impl Value {
    // ------------------------------------------------------------------------
    //  Unary ops
    // ------------------------------------------------------------------------

    /// Arithmetic negation.  Works on Float and Integer only.
    pub fn neg(&self) -> ExecuteResult<Self> {
        match self {
            Value::Float(v) => Ok(Value::Float(-*v)),
            Value::Integer(v) => Ok(Value::Integer(v.wrapping_neg())),
            Value::Bool(v) => Ok(Value::Bool(!*v)),
            other => Err(ExecuteError::UnaryOpTypeMismatch("neg", other.type_name())),
        }
    }

    /// Logical not
    pub fn not(&self) -> ExecuteResult<Self> {
        match self {
            Value::Float(v) => Ok(Value::Float(-*v)),
            Value::Integer(v) => Ok(Value::Integer(v.wrapping_neg())),
            Value::Bool(v) => Ok(Value::Bool(!*v)),
            other => Err(ExecuteError::UnaryOpTypeMismatch("not", other.type_name())),
        }
    }

    // ------------------------------------------------------------------------
    //  Arithmetic binary ops
    // ------------------------------------------------------------------------

    /// Addition: numbers add numerically; strings concatenate.
    /// String + non-string coerces the right operand to its display form.
    pub fn add(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
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

    pub fn sub(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l.wrapping_sub(*r))),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l - r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 - r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l - *r as f64)),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("sub", lhs.type_name(), rhs.type_name()))
        }
    }

    pub fn mul(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l.wrapping_mul(*r))),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l * r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 * r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l * *r as f64)),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("mul", lhs.type_name(), rhs.type_name()))
        }
    }

    pub fn div(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
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

    // ------------------------------------------------------------------------
    //  Comparison / equality binary ops
    // ------------------------------------------------------------------------

    /// Equality.  Any two values can be compared.  Different numeric types
    /// (Integer vs Float) are compared by value after promotion to f64.
    pub fn eq(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
        Ok(Value::Bool(values_equal(lhs, rhs)))
    }

    /// Inequality.  Inverse of `eq`.
    pub fn ne(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
        Ok(Value::Bool(!values_equal(lhs, rhs)))
    }

    pub fn gt(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l > r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l > r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 > *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l > *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() > r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("gt", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn ge(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l >= r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l >= r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 >= *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l >= *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() >= r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("ge", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn lt(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l < r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l < r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool((*l as f64) < *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l < *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() < r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("lt", lhs.type_name(), rhs.type_name())),
        }
    }

    pub fn le(lhs: &Self, rhs: &Self) -> ExecuteResult<Self> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l <= r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l <= r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 <= *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l <= *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() <= r.as_str())),
            (lhs, rhs) => Err(ExecuteError::BinaryOpTypeMismatch("le", lhs.type_name(), rhs.type_name())),
        }
    }
}

// ------------------------------------------------------------------------
//  Helper: structural + cross-numeric equality
// ------------------------------------------------------------------------

fn values_equal(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        // Cross-type numeric equality: compare by value after promoting to f64
        (Value::Integer(l), Value::Float(r)) => *l as f64 == *r,
        (Value::Float(l), Value::Integer(r)) => *l == *r as f64,
        // Same variant → use derived PartialEq
        (Value::Float(l), Value::Float(r)) => l == r,
        (Value::Integer(l), Value::Integer(r)) => l == r,
        (Value::Bool(l), Value::Bool(r)) => l == r,
        (Value::Nil, Value::Nil) => true,
        (Value::String(l), Value::String(r)) => l == r,
        // Different variants → not equal
        _ => false,
    }
}

// ========================================================================== //
//  Truthiness
// ========================================================================== //

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil => false,
            Value::Bool(false) => false,
            _ => true,
        }
    }
}
