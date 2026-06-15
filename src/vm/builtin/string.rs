use crate::{NativeFunc, ObjectHandle};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

macro_rules! string_cmp_op {
    ($name:ident, $op:expr, $op_name:literal) => {
        pub fn $name(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
            let lhs_s = self.get_string_instance(lhs)?.clone();
            if let Ok(rhs_s) = self.get_string_instance(rhs) {
                return Ok(self.obj_heap.alloc_bool_instance($op(lhs_s.as_str(), rhs_s.as_str())));
            }
            Err(ExecuteError::BinaryOpTypeMismatch($op_name, "string", self.value_type_name(rhs)))
        }
    };
}

impl VirtualMachine {
    pub fn string_add(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_s = self.get_string_instance(lhs)?.clone();
        if let Ok(rhs_s) = self.get_string_instance(rhs) {
            let result = format!("{}{}", lhs_s.as_str(), rhs_s.as_str());
            return Ok(self.obj_heap.alloc_string_instance(result.into()));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("add", "string", self.value_type_name(rhs)))
    }

    string_cmp_op!(string_eq, |a, b| a == b, "eq");
    string_cmp_op!(string_ne, |a, b| a != b, "ne");
    string_cmp_op!(string_gt, |a, b| a > b, "gt");
    string_cmp_op!(string_ge, |a, b| a >= b, "ge");
    string_cmp_op!(string_lt, |a, b| a < b, "lt");
    string_cmp_op!(string_le, |a, b| a <= b, "le");

    pub fn string_not(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = self.get_string_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(s.is_empty()))
    }

    pub fn string_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Return self.
        Ok(receiver)
    }

    pub fn string_bool(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = self.get_string_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(!s.is_empty()))
    }

    pub fn string_int(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = self.get_string_instance(receiver)?;
        let val: i64 = s.as_str().parse().map_err(|_| {
            ExecuteError::BadIntResult("string")
        })?;
        Ok(self.obj_heap.alloc_integer_instance(val))
    }

    pub fn string_float(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = self.get_string_instance(receiver)?;
        let val: f64 = s.as_str().parse().map_err(|_| {
            ExecuteError::BadFloatResult("string")
        })?;
        Ok(self.obj_heap.alloc_float_instance(val))
    }

    pub fn string_len(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = self.get_string_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(s.len() as i64))
    }

    pub fn string_getitem(&mut self, receiver: ObjectHandle, idx_handle: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = self.get_string_instance(receiver)?.clone();
        let idx_val = *self.get_integer_instance(idx_handle)?;
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
        self.register_native_method(sc, "__not__",      NativeFunc::a1(VirtualMachine::string_not));
        self.register_native_method(sc, "__add__",      NativeFunc::a2(VirtualMachine::string_add));
        self.register_native_method(sc, "__eq__",       NativeFunc::a2(VirtualMachine::string_eq));
        self.register_native_method(sc, "__ne__",       NativeFunc::a2(VirtualMachine::string_ne));
        self.register_native_method(sc, "__gt__",       NativeFunc::a2(VirtualMachine::string_gt));
        self.register_native_method(sc, "__ge__",       NativeFunc::a2(VirtualMachine::string_ge));
        self.register_native_method(sc, "__lt__",       NativeFunc::a2(VirtualMachine::string_lt));
        self.register_native_method(sc, "__le__",       NativeFunc::a2(VirtualMachine::string_le));
        self.register_native_method(sc, "__str__",      NativeFunc::a1(VirtualMachine::string_str));
        self.register_native_method(sc, "__bool__",     NativeFunc::a1(VirtualMachine::string_bool));
        self.register_native_method(sc, "__int__",      NativeFunc::a1(VirtualMachine::string_int));
        self.register_native_method(sc, "__float__",    NativeFunc::a1(VirtualMachine::string_float));
        self.register_native_method(sc, "__len__",      NativeFunc::a1(VirtualMachine::string_len));
        self.register_native_method(sc, "__getitem__",  NativeFunc::a2(VirtualMachine::string_getitem));
    }
}
