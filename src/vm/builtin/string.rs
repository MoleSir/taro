use crate::ObjectHandle;
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};
use super::utils::top_args;

macro_rules! string_cmp_op {
    ($name:ident, $op:expr, $op_name:literal) => {
        pub fn $name(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
            if arg_count != 2 {
                Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
            }
            let args = top_args(self, arg_count);
            let lhs_s = self.get_string_instance(args[0])?.clone();
            let other = args[1];
            if let Ok(rhs_s) = self.get_string_instance(other) {
                return Ok(self.obj_heap.alloc_bool_instance($op(lhs_s.as_str(), rhs_s.as_str())));
            }
            Err(ExecuteError::BinaryOpTypeMismatch($op_name, "string", self.value_type_name(other)))
        }
    };
}

impl VirtualMachine {
    pub fn string_add(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_s = self.get_string_instance(args[0])?.clone();
        let other = args[1];
        if let Ok(rhs_s) = self.get_string_instance(other) {
            let result = format!("{}{}", lhs_s.as_str(), rhs_s.as_str());
            return Ok(self.obj_heap.alloc_string_instance(result.into()));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("add", "string", self.value_type_name(other)))
    }

    string_cmp_op!(string_eq, |a, b| a == b, "eq");
    string_cmp_op!(string_ne, |a, b| a != b, "ne");
    string_cmp_op!(string_gt, |a, b| a > b, "gt");
    string_cmp_op!(string_ge, |a, b| a >= b, "ge");
    string_cmp_op!(string_lt, |a, b| a < b, "lt");
    string_cmp_op!(string_le, |a, b| a <= b, "le");

    pub fn string_not(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let s = self.get_string_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(s.is_empty()))
    }

    pub fn string_str(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        // Return self.
        Ok(top_args(self, arg_count)[0])
    }

    pub fn string_bool(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let s = self.get_string_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(!s.is_empty()))
    }

    pub fn string_int(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let s = self.get_string_instance(args[0])?;
        let val: i64 = s.as_str().parse().map_err(|_| {
            ExecuteError::BadIntResult("string")
        })?;
        Ok(self.obj_heap.alloc_integer_instance(val))
    }

    pub fn string_float(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let s = self.get_string_instance(args[0])?;
        let val: f64 = s.as_str().parse().map_err(|_| {
            ExecuteError::BadFloatResult("string")
        })?;
        Ok(self.obj_heap.alloc_float_instance(val))
    }

    pub fn string_len(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let s = self.get_string_instance(args[0])?;
        Ok(self.obj_heap.alloc_integer_instance(s.len() as i64))
    }

    pub fn string_getitem(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let s = self.get_string_instance(args[0])?.clone();
        let idx_val = *self.get_integer_instance(args[1])?;
        let len = s.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(ExecuteError::IndexOutOfRange(idx, len));
        }
        let ch = s.as_str()[idx as usize..idx as usize + 1].to_string();
        Ok(self.obj_heap.alloc_string_instance(ch.into()))
    }

    pub fn register_string_builtins(&mut self) {
        let sc = self.obj_heap.string_class;
        self.reg_native_method(sc, "__not__", VirtualMachine::string_not);
        self.reg_native_method(sc, "__add__", VirtualMachine::string_add);
        self.reg_native_method(sc, "__eq__", VirtualMachine::string_eq);
        self.reg_native_method(sc, "__ne__", VirtualMachine::string_ne);
        self.reg_native_method(sc, "__gt__", VirtualMachine::string_gt);
        self.reg_native_method(sc, "__ge__", VirtualMachine::string_ge);
        self.reg_native_method(sc, "__lt__", VirtualMachine::string_lt);
        self.reg_native_method(sc, "__le__", VirtualMachine::string_le);
        self.reg_native_method(sc, "__str__", VirtualMachine::string_str);
        self.reg_native_method(sc, "__bool__", VirtualMachine::string_bool);
        self.reg_native_method(sc, "__int__", VirtualMachine::string_int);
        self.reg_native_method(sc, "__float__", VirtualMachine::string_float);
        self.reg_native_method(sc, "__len__", VirtualMachine::string_len);
        self.reg_native_method(sc, "__getitem__", VirtualMachine::string_getitem);
    }
}
