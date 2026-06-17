use crate::{NativeFunction, ObjectHandle};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

macro_rules! int_binary_arith {
    ($name:ident, $int_op:expr, $float_op:expr, $op_name:literal) => {
        pub fn $name(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
            let lhs_val = *self.get_integer_instance(lhs)?;
            if let Ok(rhs) = self.get_integer_instance(rhs) {
                return Ok(self.obj_heap.alloc_integer_instance($int_op(lhs_val, *rhs)));
            }
            if let Ok(rhs) = self.get_float_instance(rhs) {
                return Ok(self.obj_heap.alloc_float_instance($float_op(lhs_val as f64, *rhs)));
            }
            Err(ExecuteError::BinaryOpTypeMismatch($op_name, "integer", self.value_type_name(rhs)))
        }
    };
}

macro_rules! int_cmp_op {
    ($name:ident, $int_cmp:expr, $float_cmp:expr, $op_name:literal) => {
        pub fn $name(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
            let lhs_val = *self.get_integer_instance(lhs)?;
            let result = if let Ok(rhs) = self.get_integer_instance(rhs) {
                $int_cmp(lhs_val, *rhs)
            } else if let Ok(rhs) = self.get_float_instance(rhs) {
                $float_cmp(lhs_val as f64, *rhs)
            } else {
                return Err(ExecuteError::BinaryOpTypeMismatch($op_name, "integer", self.value_type_name(rhs)));
            };
            Ok(self.obj_heap.alloc_bool_instance(result))
        }
    };
}

impl VirtualMachine {
    int_binary_arith!(int_add, |a, b| i64::wrapping_add(a, b), |a, b| a + b, "add");
    int_binary_arith!(int_sub, |a, b| i64::wrapping_sub(a, b), |a, b| a - b, "sub");
    int_binary_arith!(int_mul, |a, b| i64::wrapping_mul(a, b), |a, b| a * b, "mul");

    int_cmp_op!(int_eq, |a, b| a == b, |a, b| a == b, "eq");
    int_cmp_op!(int_ne, |a, b| a != b, |a, b| a != b, "ne");
    int_cmp_op!(int_gt, |a, b| a > b, |a, b| a > b, "gt");
    int_cmp_op!(int_ge, |a, b| a >= b, |a, b| a >= b, "ge");
    int_cmp_op!(int_lt, |a, b| a < b, |a, b| a < b, "lt");
    int_cmp_op!(int_le, |a, b| a <= b, |a, b| a <= b, "le");

    pub fn int_div(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = *self.get_integer_instance(lhs)?;
        if let Ok(rhs) = self.get_integer_instance(rhs) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 / *rhs as f64));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 / *rhs));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("div", "integer", self.value_type_name(rhs)))
    }

    pub fn int_floordiv(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = *self.get_integer_instance(lhs)?;
        if let Ok(rhs) = self.get_integer_instance(rhs) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_integer_instance(i64::wrapping_div_euclid(lhs_val, *rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance((lhs_val as f64 / *rhs).floor()));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("floordiv", "integer", self.value_type_name(rhs)))
    }

    pub fn int_mod(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = *self.get_integer_instance(lhs)?;
        if let Ok(rhs) = self.get_integer_instance(rhs) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_integer_instance(i64::wrapping_rem_euclid(lhs_val, *rhs)));
        }
        if let Ok(rhs) = self.get_float_instance(rhs) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance((lhs_val as f64).rem_euclid(*rhs)));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("mod", "integer", self.value_type_name(rhs)))
    }

    pub fn int_neg(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_integer_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(val.wrapping_neg()))
    }

    pub fn int_not(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_integer_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(val == 0))
    }

    pub fn int_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_integer_instance(receiver)?;
        Ok(self.obj_heap.alloc_string_instance(crate::format_shr!("{}", val)))
    }

    pub fn int_bool(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_integer_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(val != 0))
    }

    pub fn int_hash(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_integer_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(val))
    }

    pub fn int_int(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Return self — already an int instance.
        Ok(receiver)
    }

    pub fn int_float(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_integer_instance(receiver)?;
        Ok(self.obj_heap.alloc_float_instance(val as f64))
    }

    pub fn register_int_builtins(&mut self) {
        let ic = self.obj_heap.int_class;
        self.register_native_method(ic, "__neg__",   NativeFunction::a1(VirtualMachine::int_neg));
        self.register_native_method(ic, "__not__",   NativeFunction::a1(VirtualMachine::int_not));
        self.register_native_method(ic, "__add__",   NativeFunction::a2(VirtualMachine::int_add));
        self.register_native_method(ic, "__sub__",   NativeFunction::a2(VirtualMachine::int_sub));
        self.register_native_method(ic, "__mul__",   NativeFunction::a2(VirtualMachine::int_mul));
        self.register_native_method(ic, "__div__",      NativeFunction::a2(VirtualMachine::int_div));
        self.register_native_method(ic, "__floordiv__", NativeFunction::a2(VirtualMachine::int_floordiv));
        self.register_native_method(ic, "__mod__",      NativeFunction::a2(VirtualMachine::int_mod));
        self.register_native_method(ic, "__eq__",    NativeFunction::a2(VirtualMachine::int_eq));
        self.register_native_method(ic, "__ne__",    NativeFunction::a2(VirtualMachine::int_ne));
        self.register_native_method(ic, "__gt__",    NativeFunction::a2(VirtualMachine::int_gt));
        self.register_native_method(ic, "__ge__",    NativeFunction::a2(VirtualMachine::int_ge));
        self.register_native_method(ic, "__lt__",    NativeFunction::a2(VirtualMachine::int_lt));
        self.register_native_method(ic, "__le__",    NativeFunction::a2(VirtualMachine::int_le));
        self.register_native_method(ic, "__str__",   NativeFunction::a1(VirtualMachine::int_str));
        self.register_native_method(ic, "__bool__",  NativeFunction::a1(VirtualMachine::int_bool));
        self.register_native_method(ic, "__hash__",  NativeFunction::a1(VirtualMachine::int_hash));
        self.register_native_method(ic, "__int__",   NativeFunction::a1(VirtualMachine::int_int));
        self.register_native_method(ic, "__float__", NativeFunction::a1(VirtualMachine::int_float));
    }
}
