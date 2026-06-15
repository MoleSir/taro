use crate::{NativeFunction, ObjectHandle, ObjectInstanceData};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

impl VirtualMachine {
    pub fn dict_not(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_dict_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(entries.is_empty()))
    }

    pub fn dict_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_dict_instance(receiver)?.clone();
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

    pub fn dict_bool(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_dict_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(!entries.is_empty()))
    }

    pub fn dict_len(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_dict_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(entries.len() as i64))
    }

    pub fn dict_getitem(&mut self, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_dict_instance(receiver).cloned()?;
        for &(k, v) in &entries {
            let eq = self.__eq__(k, key)?;
            if self.__bool__(eq)? {
                return Ok(v);
            }
        }
        Err(ExecuteError::KeyNotFound)
    }

    pub fn dict_setitem(&mut self, receiver: ObjectHandle, key: ObjectHandle, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
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
    pub fn dict_get(&mut self, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
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
    pub fn dict_keys(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let keys = self.get_dict_instance(receiver)?.iter().map(|&(k, _)| k).collect();
        Ok(self.obj_heap.alloc_list_instance(keys))
    }

    /// `dict.values()` — return a list of all values.
    pub fn dict_values(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let values = self.get_dict_instance(receiver)?.iter().map(|&(_, v)| v).collect();
        Ok(self.obj_heap.alloc_list_instance(values))
    }

    /// `dict.pop(key)` — remove a key and return its value.
    pub fn dict_pop(&mut self, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
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
        self.register_native_method(dc, "__not__",     NativeFunction::a1(VirtualMachine::dict_not));
        self.register_native_method(dc, "__str__",     NativeFunction::a1(VirtualMachine::dict_str));
        self.register_native_method(dc, "__bool__",    NativeFunction::a1(VirtualMachine::dict_bool));
        self.register_native_method(dc, "__len__",     NativeFunction::a1(VirtualMachine::dict_len));
        self.register_native_method(dc, "__getitem__", NativeFunction::a2(VirtualMachine::dict_getitem));
        self.register_native_method(dc, "__setitem__", NativeFunction::a3(VirtualMachine::dict_setitem));
        self.register_native_method(dc, "get",         NativeFunction::a2(VirtualMachine::dict_get));
        self.register_native_method(dc, "keys",        NativeFunction::a1(VirtualMachine::dict_keys));
        self.register_native_method(dc, "values",      NativeFunction::a1(VirtualMachine::dict_values));
        self.register_native_method(dc, "pop",         NativeFunction::a2(VirtualMachine::dict_pop));
    }
}
