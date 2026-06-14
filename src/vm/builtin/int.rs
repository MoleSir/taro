use crate::ObjectHandle;
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};
use super::utils::top_args;

macro_rules! int_binary_arith {
    ($name:ident, $int_op:expr, $float_op:expr, $op_name:literal) => {
        pub fn $name(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
            if arg_count != 2 {
                Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
            }
            let args = top_args(self, arg_count);
            let lhs_val = *self.get_integer_instance(args[0])?;
            let other = args[1];
            if let Ok(rhs) = self.get_integer_instance(other) {
                return Ok(self.obj_heap.alloc_integer_instance($int_op(lhs_val, *rhs)));
            }
            if let Ok(rhs) = self.get_float_instance(other) {
                return Ok(self.obj_heap.alloc_float_instance($float_op(lhs_val as f64, *rhs)));
            }
            Err(ExecuteError::BinaryOpTypeMismatch($op_name, "integer", self.value_type_name(other)))
        }
    };
}

macro_rules! int_cmp_op {
    ($name:ident, $int_cmp:expr, $float_cmp:expr, $op_name:literal) => {
        pub fn $name(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
            if arg_count != 2 {
                Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
            }
            let args = top_args(self, arg_count);
            let lhs_val = *self.get_integer_instance(args[0])?;
            let other = args[1];
            let result = if let Ok(rhs) = self.get_integer_instance(other) {
                $int_cmp(lhs_val, *rhs)
            } else if let Ok(rhs) = self.get_float_instance(other) {
                $float_cmp(lhs_val as f64, *rhs)
            } else {
                return Err(ExecuteError::BinaryOpTypeMismatch($op_name, "integer", self.value_type_name(other)));
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

    pub fn int_div(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = *self.get_integer_instance(args[0])?;
        let other = args[1];
        if let Ok(rhs) = self.get_integer_instance(other) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 / *rhs as f64));
        }
        if let Ok(rhs) = self.get_float_instance(other) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val as f64 / *rhs));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("div", "integer", self.value_type_name(other)))
    }

    pub fn int_neg(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_integer_instance(args[0])?;
        Ok(self.obj_heap.alloc_integer_instance(val.wrapping_neg()))
    }

    pub fn int_not(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_integer_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(val == 0))
    }

    pub fn int_str(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_integer_instance(args[0])?;
        Ok(self.obj_heap.alloc_string_instance(crate::format_shr!("{}", val)))
    }

    pub fn int_bool(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_integer_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(val != 0))
    }

    pub fn int_int(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        // Return self — already an int instance.
        Ok(top_args(self, arg_count)[0])
    }

    pub fn int_float(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_integer_instance(args[0])?;
        Ok(self.obj_heap.alloc_float_instance(val as f64))
    }

    pub fn register_int_builtins(&mut self) {
        let ic = self.obj_heap.int_class;
        self.reg_native_method(ic, "__neg__", VirtualMachine::int_neg);
        self.reg_native_method(ic, "__not__", VirtualMachine::int_not);
        self.reg_native_method(ic, "__add__", VirtualMachine::int_add);
        self.reg_native_method(ic, "__sub__", VirtualMachine::int_sub);
        self.reg_native_method(ic, "__mul__", VirtualMachine::int_mul);
        self.reg_native_method(ic, "__div__", VirtualMachine::int_div);
        self.reg_native_method(ic, "__eq__", VirtualMachine::int_eq);
        self.reg_native_method(ic, "__ne__", VirtualMachine::int_ne);
        self.reg_native_method(ic, "__gt__", VirtualMachine::int_gt);
        self.reg_native_method(ic, "__ge__", VirtualMachine::int_ge);
        self.reg_native_method(ic, "__lt__", VirtualMachine::int_lt);
        self.reg_native_method(ic, "__le__", VirtualMachine::int_le);
        self.reg_native_method(ic, "__str__", VirtualMachine::int_str);
        self.reg_native_method(ic, "__bool__", VirtualMachine::int_bool);
        self.reg_native_method(ic, "__int__", VirtualMachine::int_int);
        self.reg_native_method(ic, "__float__", VirtualMachine::int_float);
    }
}
