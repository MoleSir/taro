use crate::{NativeFunction, ObjectHandle};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

impl VirtualMachine {
    pub fn bool_neg(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_bool_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(if val { -1 } else { 0 }))
    }

    pub fn bool_not(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_bool_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(!val))
    }

    /// Treat bool as 0 or 1 for arithmetic.
    fn bool_as_int(&self, handle: ObjectHandle) -> ExecuteResult<i64> {
        let val = *self.get_bool_instance(handle)?;
        Ok(if val { 1 } else { 0 })
    }

    pub fn bool_add(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = self.bool_as_int(lhs)?;

        if let Ok(rhs) = self.get_integer_instance(rhs) {
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_add(*rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 + *rhs));
        }
        if self.get_bool_instance(rhs).is_ok() {
            let rhs_val = self.bool_as_int(rhs)?;
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_add(rhs_val)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("add", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_sub(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = self.bool_as_int(lhs)?;

        if let Ok(rhs) = self.get_integer_instance(rhs) {
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_sub(*rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 - *rhs));
        }
        if self.get_bool_instance(rhs).is_ok() {
            let rhs_val = self.bool_as_int(rhs)?;
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_sub(rhs_val)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("sub", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_mul(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = self.bool_as_int(lhs)?;

        if let Ok(rhs) = self.get_integer_instance(rhs) {
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_mul(*rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 * *rhs));
        }
        if self.get_bool_instance(rhs).is_ok() {
            let rhs_val = self.bool_as_int(rhs)?;
            return Ok(self.obj_heap.alloc_integer_instance(lhs_val.wrapping_mul(rhs_val)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("mul", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_div(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = self.bool_as_int(lhs)? as f64;

        if let Ok(rhs) = self.get_integer_instance(rhs) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / *rhs as f64));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / *rhs));
        }
        if self.get_bool_instance(rhs).is_ok() {
            let rhs_val = self.bool_as_int(rhs)?;
            if rhs_val == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / rhs_val as f64));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("div", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_floordiv(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = self.bool_as_int(lhs)?;
        if let Ok(rhs) = self.get_integer_instance(rhs) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_integer_instance(i64::wrapping_div_euclid(lhs_val, *rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance((lhs_val as f64 / *rhs).floor()));
        }
        if self.get_bool_instance(rhs).is_ok() {
            let rhs_val = self.bool_as_int(rhs)?;
            if rhs_val == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_integer_instance(i64::wrapping_div_euclid(lhs_val, rhs_val)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("floordiv", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_mod(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = self.bool_as_int(lhs)?;
        if let Ok(rhs) = self.get_integer_instance(rhs) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_integer_instance(i64::wrapping_rem_euclid(lhs_val, *rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance((lhs_val as f64).rem_euclid(*rhs)));
        }
        if self.get_bool_instance(rhs).is_ok() {
            let rhs_val = self.bool_as_int(rhs)?;
            if rhs_val == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_integer_instance(i64::wrapping_rem_euclid(lhs_val, rhs_val)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("mod", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_eq(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = *self.get_bool_instance(lhs)?;

        if let Ok(rhs) = self.get_bool_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_val == *rhs));
        }
        // Treat bool as 1/0 for numeric comparison.
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Ok(rhs) = self.get_integer_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int == *rhs));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int as f64 == *rhs));
        }
        Ok(self.obj_heap.alloc_bool_instance(false))
    }

    pub fn bool_ne(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let eq = self.bool_eq(lhs, rhs)?;
        let b = self.get_bool_instance_mut(eq).expect("must return bool");
        *b = !*b;
        Ok(eq)
    }

    pub fn bool_gt(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_int = self.bool_as_int(lhs)?;

        if let Ok(rhs) = self.get_bool_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int > (if *rhs { 1 } else { 0 })));
        }
        if let Ok(rhs) = self.get_integer_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int > *rhs));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int as f64 > *rhs));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("gt", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_ge(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = *self.get_bool_instance(lhs)?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Ok(rhs_val) = self.get_bool_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int >= (if *rhs_val { 1 } else { 0 })));
        }
        if let Ok(rhs_val) = self.get_integer_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int >= *rhs_val));
        }
        if let Ok(rhs_val) = self.get_float_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int as f64 >= *rhs_val));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("ge", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_lt(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = *self.get_bool_instance(lhs)?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Ok(rhs_val) = self.get_bool_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int < (if *rhs_val { 1 } else { 0 })));
        }
        if let Ok(rhs_val) = self.get_integer_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int < *rhs_val));
        }
        if let Ok(rhs_val) = self.get_float_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance((lhs_int as f64) < *rhs_val));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("lt", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_le(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = *self.get_bool_instance(lhs)?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Ok(rhs_val) = self.get_bool_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int <= (if *rhs_val { 1 } else { 0 })));
        }
        if let Ok(rhs_val) = self.get_integer_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int <= *rhs_val));
        }
        if let Ok(rhs_val) = self.get_float_instance(rhs) {
            return Ok(self.obj_heap.alloc_bool_instance(lhs_int as f64 <= *rhs_val));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("le", "bool", self.value_type_name(rhs)))
    }

    pub fn bool_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_bool_instance(receiver)?;
        Ok(self.obj_heap.alloc_string_instance(
            if val { crate::ShrString::from("true") } else { crate::ShrString::from("false") }
        ))
    }

    pub fn bool_bool(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Return self.
        Ok(receiver)
    }

    pub fn bool_hash(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_bool_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(if val { 1 } else { 0 }))
    }

    pub fn bool_int(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_bool_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(if val { 1 } else { 0 }))
    }

    pub fn bool_float(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_bool_instance(receiver)?;
        Ok(self.obj_heap.alloc_float_instance(if val { 1.0 } else { 0.0 }))
    }

    pub fn register_bool_builtins(&mut self) {
        let bc = self.obj_heap.bool_class;
        self.register_native_method(bc, "__neg__",   NativeFunction::a1(VirtualMachine::bool_neg));
        self.register_native_method(bc, "__not__",   NativeFunction::a1(VirtualMachine::bool_not));
        self.register_native_method(bc, "__add__",   NativeFunction::a2(VirtualMachine::bool_add));
        self.register_native_method(bc, "__sub__",   NativeFunction::a2(VirtualMachine::bool_sub));
        self.register_native_method(bc, "__mul__",   NativeFunction::a2(VirtualMachine::bool_mul));
        self.register_native_method(bc, "__div__",      NativeFunction::a2(VirtualMachine::bool_div));
        self.register_native_method(bc, "__floordiv__", NativeFunction::a2(VirtualMachine::bool_floordiv));
        self.register_native_method(bc, "__mod__",      NativeFunction::a2(VirtualMachine::bool_mod));
        self.register_native_method(bc, "__eq__",    NativeFunction::a2(VirtualMachine::bool_eq));
        self.register_native_method(bc, "__ne__",    NativeFunction::a2(VirtualMachine::bool_ne));
        self.register_native_method(bc, "__gt__",    NativeFunction::a2(VirtualMachine::bool_gt));
        self.register_native_method(bc, "__ge__",    NativeFunction::a2(VirtualMachine::bool_ge));
        self.register_native_method(bc, "__lt__",    NativeFunction::a2(VirtualMachine::bool_lt));
        self.register_native_method(bc, "__le__",    NativeFunction::a2(VirtualMachine::bool_le));
        self.register_native_method(bc, "__str__",   NativeFunction::a1(VirtualMachine::bool_str));
        self.register_native_method(bc, "__bool__",  NativeFunction::a1(VirtualMachine::bool_bool));
        self.register_native_method(bc, "__hash__",  NativeFunction::a1(VirtualMachine::bool_hash));
        self.register_native_method(bc, "__int__",   NativeFunction::a1(VirtualMachine::bool_int));
        self.register_native_method(bc, "__float__", NativeFunction::a1(VirtualMachine::bool_float));
    }
}
