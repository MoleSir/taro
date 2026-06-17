use crate::{ToNativeData, NativeFunction, ObjectHandle, ObjectInstanceData};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

/// Native state for a dict-key iterator.
struct DictKeyIterator {
    dict_handle: ObjectHandle,
    index: usize,
}

impl ToNativeData for DictKeyIterator {
    fn mark_inner_object(&self, heap: &mut crate::ObjectHeap) {
        heap.mark_object(self.dict_handle);
    }
}

impl VirtualMachine {
    pub fn dict_not(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_dict_instance(receiver)?;
        let is_empty = entries.values().all(|b| b.is_empty());
        Ok(self.obj_heap.alloc_bool_instance(is_empty))
    }

    pub fn dict_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Collect all entries eagerly to release the dict borrow before
        // calling `__str__` on each key/value.
        let all_entries: Vec<(ObjectHandle, ObjectHandle)> = self.get_dict_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().copied())
            .collect();

        let mut result = String::from("{");
        let mut first = true;
        for (k, v) in all_entries {
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
        let has_any = entries.values().any(|b| !b.is_empty());
        Ok(self.obj_heap.alloc_bool_instance(has_any))
    }

    pub fn dict_len(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_dict_instance(receiver)?;
        let total: usize = entries.values().map(|b| b.len()).sum();
        Ok(self.obj_heap.alloc_integer_instance(total as i64))
    }

    pub fn dict_getitem(&mut self, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = self.__hash__(key)?;

        // Clone the bucket (small) to release the dict borrow before calling __eq__.
        let bucket = {
            let entries = self.get_dict_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        for &(k, v) in &bucket {
            let eq = self.__eq__(k, key)?;
            if self.__bool__(eq)? {
                return Ok(v);
            }
        }
        Err(ExecuteError::KeyNotFound)
    }

    pub fn dict_setitem(&mut self, receiver: ObjectHandle, key: ObjectHandle, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = self.__hash__(key)?;

        // Clone the bucket to release the dict borrow.
        let mut bucket = {
            let entries = self.get_dict_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        // Linear scan with __eq__ inside the bucket.
        let mut found_idx = None;
        for (i, &(k, _)) in bucket.iter().enumerate() {
            let eq = self.__eq__(k, key)?;
            if self.__bool__(eq)? {
                found_idx = Some(i);
                break;
            }
        }
        if let Some(i) = found_idx {
            bucket[i].1 = value;
        } else {
            bucket.push((key, value));
        }

        // Write back.
        let inst = self.get_instance_mut(receiver)?;
        if let ObjectInstanceData::Dict(entries) = &mut inst.data {
            entries.insert(hash, bucket);
        }
        Ok(value)
    }

    /// `dict.get(key)` — get a value by key, returning nil if not found.
    pub fn dict_get(&mut self, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = self.__hash__(key)?;

        let bucket = {
            let entries = self.get_dict_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        for &(k, v) in &bucket {
            let eq_result = self.__eq__(k, key)?;
            if self.__bool__(eq_result)? {
                return Ok(v);
            }
        }
        Ok(ObjectHandle::NIL)
    }

    /// `dict.keys()` — return a list of all keys.
    pub fn dict_keys(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let keys: Vec<ObjectHandle> = self.get_dict_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().map(|&(k, _)| k))
            .collect();
        Ok(self.obj_heap.alloc_list_instance(keys))
    }

    /// `dict.values()` — return a list of all values.
    pub fn dict_values(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let values: Vec<ObjectHandle> = self.get_dict_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().map(|&(_, v)| v))
            .collect();
        Ok(self.obj_heap.alloc_list_instance(values))
    }

    /// `dict.pop(key)` — remove a key and return its value.
    pub fn dict_pop(&mut self, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = self.__hash__(key)?;

        // Clone the bucket to release the dict borrow.
        let mut bucket = {
            let entries = self.get_dict_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        let mut found_idx = None;
        for (i, &(k, _)) in bucket.iter().enumerate() {
            let eq = self.__eq__(k, key)?;
            if self.__bool__(eq)? {
                found_idx = Some(i);
                break;
            }
        }

        match found_idx {
            Some(idx) => {
                let removed = bucket.remove(idx);
                // Write back the updated bucket.
                let inst = self.get_instance_mut(receiver)?;
                if let ObjectInstanceData::Dict(entries) = &mut inst.data {
                    if bucket.is_empty() {
                        entries.remove(&hash);
                    } else {
                        entries.insert(hash, bucket);
                    }
                }
                Ok(removed.1)
            }
            None => Err(ExecuteError::KeyNotFound),
        }
    }

    // ---- iteration protocol ----

    pub fn dict_iter(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let iter = DictKeyIterator { dict_handle: receiver, index: 0 };
        Ok(self.obj_heap.alloc_instance(
            self.obj_heap.dict_iter_class,
            crate::ObjectInstanceData::Native(crate::NativeData::new(iter)),
        ))
    }

    pub fn dict_iter_next(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let dict_handle = {
            let iter = self.get_native::<DictKeyIterator>(receiver)?;
            iter.dict_handle
        };

        // Collect all keys eagerly to release the dict borrow before
        // mutably borrowing the iterator's native state.
        let keys: Vec<ObjectHandle> = self.get_dict_instance(dict_handle)?
            .values()
            .flat_map(|b| b.iter().map(|&(k, _)| k))
            .collect();

        let iter = self.get_native_mut::<DictKeyIterator>(receiver)?;
        if iter.index < keys.len() {
            let key = keys[iter.index];
            iter.index += 1;
            Ok(key)
        } else {
            Ok(ObjectHandle::ITER_END)
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
        self.register_native_method(dc, "len",         NativeFunction::a1(VirtualMachine::dict_len));
        self.register_native_method(dc, "__iter__", NativeFunction::a1(VirtualMachine::dict_iter));

        let dic = self.obj_heap.dict_iter_class;
        self.register_native_method(dic, "__iter__", NativeFunction::a1(|_vm, receiver| Ok(receiver)));
        self.register_native_method(dic, "__next__", NativeFunction::a1(VirtualMachine::dict_iter_next));
    }
}
