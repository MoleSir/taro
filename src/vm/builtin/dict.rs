use crate::{ObjectInstanceData, ObjectHandle};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};
use super::utils::top_args;

impl VirtualMachine {
    pub fn dict_not(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let entries = self.get_dict_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(entries.is_empty()))
    }

    pub fn dict_str(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let entries = self.get_dict_instance(args[0])?.clone();
        let mut result = String::from("{");
        let mut first = true;
        for &(k, v) in &entries {
            if !first { result.push_str(", "); }
            first = false;
            result.push_str(&self.__str__(k)?);
            result.push_str(": ");
            result.push_str(&self.__str__(v)?);
        }
        result.push('}');
        Ok(self.obj_heap.alloc_string_instance(result.into()))
    }

    pub fn dict_bool(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let entries = self.get_dict_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(!entries.is_empty()))
    }

    pub fn dict_len(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let entries = self.get_dict_instance(args[0])?;
        Ok(self.obj_heap.alloc_integer_instance(entries.len() as i64))
    }

    pub fn dict_getitem(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let entries = self.get_dict_instance(args[0]).cloned()?;
        let key = args[1];
        for &(k, v) in &entries {
            let eq = self.__eq__(k, key)?;
            if self.__bool__(eq)? {
                return Ok(v);
            }
        }
        Err(ExecuteError::KeyNotFound)
    }

    pub fn dict_setitem(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 3 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 2, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let key = args[1];
        let value = args[2];

        // Clone entries, remove existing key if present, push new.
        let entries = self.get_dict_instance(receiver).cloned()?;
        let mut new_entries = entries;
        let mut found_pos = None;
        for (i, &(k, _)) in new_entries.iter().enumerate() {
            let eq = self.__eq__(k, key)?;
            if self.__bool__(eq)? {
                found_pos = Some(i);
                break;
            }
        }
        if let Some(pos) = found_pos {
            new_entries.remove(pos);
        }
        new_entries.push((key, value));

        let inst_mut = self.get_instance_mut(receiver)?;
        if let ObjectInstanceData::Dict(entries) = &mut inst_mut.data {
            *entries = new_entries;
        }
        Ok(value)
    }
    /// `dict.get(key)` — get a value by key, returning nil if not found.
    pub fn dict_get(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let key = args[1];
        let entries = self.get_dict_instance(receiver).cloned()?;
        for &(k, v) in &entries {
            let eq_result = self.__eq__(k, key)?;
            if self.__bool__(eq_result)? {
                return Ok(v);
            }
        }
        Ok(ObjectHandle::NIL)
    }

    /// `dict.keys()` — return a list of all keys.
    pub fn dict_keys(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let keys = self.get_dict_instance(receiver)?.iter().map(|&(k, _)| k).collect();
        Ok(self.obj_heap.alloc_list_instance(keys))
    }

    /// `dict.values()` — return a list of all values.
    pub fn dict_values(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let values = self.get_dict_instance(receiver)?.iter().map(|&(_, v)| v).collect();

        Ok(self.obj_heap.alloc_list_instance(values))
    }

    /// `dict.pop(key)` — remove a key and return its value.
    pub fn dict_pop(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let key = args[1];

        // Clone entries, find key, remove, write back.
        let entries = self.get_dict_instance(receiver).cloned()?;

        let mut pos = None;
        for (i, &(k, _)) in entries.iter().enumerate() {
            let eq = self.__eq__(k, key)?;
            if self.__bool__(eq)? {
                pos = Some(i);
                break;
            }
        }
        match pos {
            Some(idx) => {
                let removed = entries[idx].1;
                let bi_mut = self.get_instance_mut(receiver)?;
                match &mut bi_mut.data {
                    ObjectInstanceData::Dict(e) => {
                        e.remove(idx);
                    }
                    _ => unreachable!(),
                }
                Ok(removed)
            }
            None => Err(ExecuteError::KeyNotFound),
        }
    }

    pub fn register_dict_builtins(&mut self) {
        let dc = self.obj_heap.dict_class;
        self.reg_native_method(dc, "__not__", VirtualMachine::dict_not);
        self.reg_native_method(dc, "__str__", VirtualMachine::dict_str);
        self.reg_native_method(dc, "__bool__", VirtualMachine::dict_bool);
        self.reg_native_method(dc, "__len__", VirtualMachine::dict_len);
        self.reg_native_method(dc, "__getitem__", VirtualMachine::dict_getitem);
        self.reg_native_method(dc, "__setitem__", VirtualMachine::dict_setitem);
        self.reg_native_method(dc, "get", VirtualMachine::dict_get);
        self.reg_native_method(dc, "keys", VirtualMachine::dict_keys);
        self.reg_native_method(dc, "values", VirtualMachine::dict_values);
        self.reg_native_method(dc, "pop", VirtualMachine::dict_pop);
    }
}
