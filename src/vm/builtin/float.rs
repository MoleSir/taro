use crate::ObjectHandle;
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

/// Return a slice of the top `arg_count` stack entries.
fn top_args(vm: &VirtualMachine, arg_count: usize) -> &[ObjectHandle] {
    &vm.stack[vm.stack.len() - arg_count..]
}

macro_rules! float_binary_arith {
    ($name:ident, $float_op:expr, $op_name:literal) => {
        pub fn $name(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
            if arg_count != 2 {
                Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
            }
            let args = top_args(self, arg_count);
            let lhs_val = *self.get_float_instance(args[0])?;
            let other = args[1];
            if let Ok(rhs) = self.get_float_instance(other) {
                return Ok(self.obj_heap.alloc_float_instance($float_op(lhs_val, *rhs)));
            }
            if let Ok(rhs) = self.get_integer_instance(other) {
                return Ok(self.obj_heap.alloc_float_instance($float_op(lhs_val, *rhs as f64)));
            }
            Err(ExecuteError::BinaryOpTypeMismatch($op_name, "float", self.value_type_name(other)))
        }
    };
}

macro_rules! float_cmp_op {
    ($name:ident, $float_cmp:expr, $op_name:literal) => {
        pub fn $name(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
            if arg_count != 2 {
                Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
            }
            let args = top_args(self, arg_count);
            let lhs_val = *self.get_float_instance(args[0])?;
            let other = args[1];
            let result = if let Ok(rhs) = self.get_float_instance(other) {
                $float_cmp(lhs_val, *rhs)
            } else if let Ok(rhs) = self.get_integer_instance(other) {
                $float_cmp(lhs_val, *rhs as f64)
            } else {
                return Err(ExecuteError::BinaryOpTypeMismatch($op_name, "float", self.value_type_name(other)));
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

    pub fn float_div(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_val = *self.get_float_instance(args[0])?;
        let other = args[1];
        if let Ok(rhs) = self.get_float_instance(other) {
            if *rhs == 0.0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / *rhs));
        }
        if let Ok(rhs) = self.get_integer_instance(other) {
            if *rhs == 0 { return Err(ExecuteError::DivideByZero); }
            return Ok(self.obj_heap.alloc_float_instance(lhs_val / *rhs as f64));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("div", "float", self.value_type_name(other)))
    }

    pub fn float_neg(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_float_instance(args[0])?;
        Ok(self.obj_heap.alloc_float_instance(-val))
    }

    pub fn float_not(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_float_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(val == 0.0))
    }

    pub fn float_str(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_float_instance(args[0])?;
        Ok(self.obj_heap.alloc_string_instance(crate::format_shr!("{}", val)))
    }

    pub fn float_bool(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_float_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(val != 0.0))
    }

    pub fn float_int(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let val = *self.get_float_instance(args[0])?;
        Ok(self.obj_heap.alloc_integer_instance(val as i64))
    }

    pub fn float_float(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        // Return self.
        Ok(top_args(self, arg_count)[0])
    }
}
