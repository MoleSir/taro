use crate::{NativeFunction, ObjectHandle};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

macro_rules! float_binary_arith {
    ($name:ident, $float_op:expr, $op_name:literal) => {
        pub fn $name(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
            let lhs_val = *self.get_float_instance(lhs)?;
            if let Ok(rhs) = self.get_float_instance(rhs) {
                return Ok(self.obj_heap.alloc_float_instance($float_op(lhs_val, *rhs)));
            }
            if let Ok(rhs) = self.get_integer_instance(rhs) {
                return Ok(self.obj_heap.alloc_float_instance($float_op(lhs_val, *rhs as f64)));
            }
            Err(ExecuteError::BinaryOpTypeMismatch($op_name, "float", self.value_type_name(rhs)))
        }
    };
}

macro_rules! float_cmp_op {
    ($name:ident, $float_cmp:expr, $op_name:literal) => {
        pub fn $name(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
            let lhs_val = *self.get_float_instance(lhs)?;
            let result = if let Ok(rhs) = self.get_float_instance(rhs) {
                $float_cmp(lhs_val, *rhs)
            } else if let Ok(rhs) = self.get_integer_instance(rhs) {
                $float_cmp(lhs_val, *rhs as f64)
            } else {
                return Err(ExecuteError::BinaryOpTypeMismatch($op_name, "float", self.value_type_name(rhs)));
            };
            Ok(self.obj_heap.alloc_bool_instance(result))
        }
    };
}

impl VirtualMachine {
    float_binary_arith!(float_add, |a, b| a + b, "add");
    float_binary_arith!(float_sub, |a, b| a - b, "sub");
    float_binary_arith!(float_mul, |a, b| a * b, "mul");

    float_cmp_op!(float_eq, |a, b| a == b, "eq");
    float_cmp_op!(float_ne, |a, b| a != b, "ne");
    float_cmp_op!(float_gt, |a, b| a > b, "gt");
    float_cmp_op!(float_ge, |a, b| a >= b, "ge");
    float_cmp_op!(float_lt, |a, b| a < b, "lt");
    float_cmp_op!(float_le, |a, b| a <= b, "le");

    pub fn float_div(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_val = *self.get_float_instance(lhs)?;
        if let Ok(rhs) = self.get_float_instance(rhs) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / *rhs));
        }
        if let Ok(rhs) = self.get_integer_instance(rhs) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / *rhs as f64));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("div", "float", self.value_type_name(rhs)))
    }

    pub fn float_neg(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_float_instance(receiver)?;
        Ok(self.obj_heap.alloc_float_instance(-val))
    }

    pub fn float_not(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_float_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(val == 0.0))
    }

    pub fn float_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_float_instance(receiver)?;
        Ok(self.obj_heap.alloc_string_instance(crate::format_shr!("{}", val)))
    }

    pub fn float_bool(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_float_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(val != 0.0))
    }

    pub fn float_int(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let val = *self.get_float_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(val as i64))
    }

    pub fn float_float(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Return self.
        Ok(receiver)
    }

    pub fn register_float_builtins(&mut self) {
        let fc = self.obj_heap.float_class;
        self.register_native_method(fc, "__neg__",   NativeFunction::a1(VirtualMachine::float_neg));
        self.register_native_method(fc, "__not__",   NativeFunction::a1(VirtualMachine::float_not));
        self.register_native_method(fc, "__add__",   NativeFunction::a2(VirtualMachine::float_add));
        self.register_native_method(fc, "__sub__",   NativeFunction::a2(VirtualMachine::float_sub));
        self.register_native_method(fc, "__mul__",   NativeFunction::a2(VirtualMachine::float_mul));
        self.register_native_method(fc, "__div__",   NativeFunction::a2(VirtualMachine::float_div));
        self.register_native_method(fc, "__eq__",    NativeFunction::a2(VirtualMachine::float_eq));
        self.register_native_method(fc, "__ne__",    NativeFunction::a2(VirtualMachine::float_ne));
        self.register_native_method(fc, "__gt__",    NativeFunction::a2(VirtualMachine::float_gt));
        self.register_native_method(fc, "__ge__",    NativeFunction::a2(VirtualMachine::float_ge));
        self.register_native_method(fc, "__lt__",    NativeFunction::a2(VirtualMachine::float_lt));
        self.register_native_method(fc, "__le__",    NativeFunction::a2(VirtualMachine::float_le));
        self.register_native_method(fc, "__str__",   NativeFunction::a1(VirtualMachine::float_str));
        self.register_native_method(fc, "__bool__",  NativeFunction::a1(VirtualMachine::float_bool));
        self.register_native_method(fc, "__int__",   NativeFunction::a1(VirtualMachine::float_int));
        self.register_native_method(fc, "__float__", NativeFunction::a1(VirtualMachine::float_float));
    }
}
