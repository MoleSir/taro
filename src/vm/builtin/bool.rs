use crate::ObjectHandle;
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};
use super::utils::top_args;

impl VirtualMachine {
    pub fn bool_neg(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_bool_instance(args[0])?;
        Ok(self.obj_heap.alloc_integer_instance(if val { -1 } else { 0 }))
    }

    pub fn bool_not(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_bool_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(!val))
    }

    /// Treat bool as 0 or 1 for arithmetic.
    fn bool_as_int(&self, handle: ObjectHandle) -> ExecuteResult<i64> {
        let val = *self.get_bool_instance(handle)?;
        Ok(if val { 1 } else { 0 })
    }

    pub fn bool_add(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = self.bool_as_int(args[0])?;
        let other = args[1];
        if let Ok(rhs) = self.get_integer_instance(other) {
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_add(*rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 + *rhs));
        }
        if self.get_bool_instance(other).is_ok() {
            let rhs_val = self.bool_as_int(other)?;
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_add(rhs_val)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("add", "bool", self.value_type_name(other)))
    }

    pub fn bool_sub(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = self.bool_as_int(args[0])?;
        let other = args[1];
        if let Ok(rhs) = self.get_integer_instance(other) {
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_sub(*rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 - *rhs));
        }
        if self.get_bool_instance(other).is_ok() {
            let rhs_val = self.bool_as_int(other)?;
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_sub(rhs_val)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("sub", "bool", self.value_type_name(other)))
    }

    pub fn bool_mul(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = self.bool_as_int(args[0])?;
        let other = args[1];
        if let Ok(rhs) = self.get_integer_instance(other) {
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_mul(*rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 * *rhs));
        }
        if self.get_bool_instance(other).is_ok() {
            let rhs_val = self.bool_as_int(other)?;
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_mul(rhs_val)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("mul", "bool", self.value_type_name(other)))
    }

    pub fn bool_div(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = self.bool_as_int(args[0])? as f64;
        let other = args[1];
        if let Ok(rhs) = self.get_integer_instance(other) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / *rhs as f64));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / *rhs));
        }
        if self.get_bool_instance(other).is_ok() {
            let rhs_val = self.bool_as_int(other)?;
            if rhs_val == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / rhs_val as f64));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("div", "bool", self.value_type_name(other)))
    }

    pub fn bool_eq(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = *self.get_bool_instance(args[0])?;
        let other = args[1];
        if let Ok(rhs) = self.get_bool_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_val == *rhs));
        }
        // Treat bool as 1/0 for numeric comparison.
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Ok(rhs) = self.get_integer_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int == *rhs));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int as f64 == *rhs));
        }
        Ok(self.obj_heap.alloc_bool_instance(false))
    }

    pub fn bool_ne(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let eq = self.bool_eq(arg_count)?;
        let b = *self.get_bool_instance(eq)?;
        Ok(self.obj_heap.alloc_bool_instance(!b))
    }

    pub fn bool_gt(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = *self.get_bool_instance(args[0])?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        let other = args[1];
        if let Ok(rhs) = self.get_bool_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int > (if *rhs { 1 } else { 0 })));
        }
        if let Ok(rhs) = self.get_integer_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int > *rhs));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int as f64 > *rhs));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("gt", "bool", self.value_type_name(other)))
    }

    pub fn bool_ge(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = *self.get_bool_instance(args[0])?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        let other = args[1];
        if let Ok(rhs) = self.get_bool_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int >= (if *rhs { 1 } else { 0 })));
        }
        if let Ok(rhs) = self.get_integer_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int >= *rhs));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int as f64 >= *rhs));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("ge", "bool", self.value_type_name(other)))
    }

    pub fn bool_lt(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = *self.get_bool_instance(args[0])?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        let other = args[1];
        if let Ok(rhs) = self.get_bool_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int < (if *rhs { 1 } else { 0 })));
        }
        if let Ok(rhs) = self.get_integer_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int < *rhs));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance((lhs_int as f64) < *rhs));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("lt", "bool", self.value_type_name(other)))
    }

    pub fn bool_le(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = *self.get_bool_instance(args[0])?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        let other = args[1];
        if let Ok(rhs) = self.get_bool_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int <= (if *rhs { 1 } else { 0 })));
        }
        if let Ok(rhs) = self.get_integer_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int <= *rhs));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int as f64 <= *rhs));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("le", "bool", self.value_type_name(other)))
    }

    pub fn bool_str(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_bool_instance(args[0])?;
        Ok(self.obj_heap.alloc_string_instance(
            if val { crate::ShrString::from("true") } else { crate::ShrString::from("false") }
        ))
    }

    pub fn bool_bool(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        // Return self.
        Ok(top_args(self, arg_count)[0])
    }

    pub fn bool_int(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_bool_instance(args[0])?;
        Ok(self.obj_heap.alloc_integer_instance(if val { 1 } else { 0 }))
    }

    pub fn bool_float(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_bool_instance(args[0])?;
        Ok(self.obj_heap.alloc_float_instance(if val { 1.0 } else { 0.0 }))
    }
}