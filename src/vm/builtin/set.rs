use std::collections::HashMap;

use crate::{ToNativeData, NativeFunction, ObjectHandle, ObjectInstanceData};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

/// Native state for a set-key iterator.
struct SetKeyIterator {
    set_handle: ObjectHandle,
    index: usize,
}

impl ToNativeData for SetKeyIterator {
    fn mark_inner_object(&self, heap: &mut crate::ObjectHeap) {
        heap.mark_object(self.set_handle);
    }
}

impl VirtualMachine {
    // ---- global constructor ----

    /// `set(args...)` — create a new set from the given arguments.
    pub fn set(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        let set_handle = self.obj_heap.alloc_set_instance(HashMap::new());
        for &item in args {
            self.set_add(set_handle, item)?;
        }
        Ok(set_handle)
    }

    // ---- core operations ----

    /// `s.add(item)` — add an item to the set.
    pub fn set_add(&mut self, receiver: ObjectHandle, item: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = self.__hash__(item)?;

        let mut bucket = {
            let entries = self.get_set_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        for &existing in &bucket {
            let eq = self.__eq__(existing, item)?;
            if self.__bool__(eq)? {
                return Ok(item); // already present — no-op
            }
        }
        bucket.push(item);

        let inst = self.get_instance_mut(receiver)?;
        if let ObjectInstanceData::Set(entries) = &mut inst.data {
            entries.insert(hash, bucket);
        }
        Ok(item)
    }

    /// `s.remove(item)` — remove an item from the set.
    pub fn set_remove(&mut self, receiver: ObjectHandle, item: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = self.__hash__(item)?;

        let mut bucket = {
            let entries = self.get_set_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        let mut found_idx = None;
        for (i, &existing) in bucket.iter().enumerate() {
            let eq = self.__eq__(existing, item)?;
            if self.__bool__(eq)? {
                found_idx = Some(i);
                break;
            }
        }

        match found_idx {
            Some(idx) => {
                let removed = bucket.remove(idx);
                let inst = self.get_instance_mut(receiver)?;
                if let ObjectInstanceData::Set(entries) = &mut inst.data {
                    if bucket.is_empty() {
                        entries.remove(&hash);
                    } else {
                        entries.insert(hash, bucket);
                    }
                }
                Ok(removed)
            }
            None => Err(ExecuteError::KeyNotFound),
        }
    }

    /// `s.contains(item)` — check if item is in the set.
    pub fn set_contains(&mut self, receiver: ObjectHandle, item: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = self.__hash__(item)?;

        let bucket = {
            let entries = self.get_set_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        for &existing in &bucket {
            let eq = self.__eq__(existing, item)?;
            if self.__bool__(eq)? {
                return Ok(self.obj_heap.alloc_bool_instance(true));
            }
        }
        Ok(self.obj_heap.alloc_bool_instance(false))
    }

    // ---- magic methods ----

    pub fn set_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let all_items: Vec<ObjectHandle> = self.get_set_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().copied())
            .collect();

        let mut result = String::from("{");
        let mut first = true;
        for item in all_items {
            if !first { result.push_str(", "); }
            first = false;
            result.push_str(&self.__str__(item)?);
        }
        result.push('}');
        Ok(self.obj_heap.alloc_string_instance(result.into()))
    }

    pub fn set_bool(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_set_instance(receiver)?;
        let has_any = entries.values().any(|b| !b.is_empty());
        Ok(self.obj_heap.alloc_bool_instance(has_any))
    }

    pub fn set_not(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_set_instance(receiver)?;
        let is_empty = entries.values().all(|b| b.is_empty());
        Ok(self.obj_heap.alloc_bool_instance(is_empty))
    }

    pub fn set_len(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = self.get_set_instance(receiver)?;
        let total: usize = entries.values().map(|b| b.len()).sum();
        Ok(self.obj_heap.alloc_integer_instance(total as i64))
    }

    // ---- iteration protocol ----

    pub fn set_iter(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let iter = SetKeyIterator { set_handle: receiver, index: 0 };
        Ok(self.obj_heap.alloc_instance(
            self.obj_heap.set_iter_class,
            ObjectInstanceData::Native(crate::NativeData::new(iter)),
        ))
    }

    pub fn set_iter_next(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let set_handle = {
            let iter = self.get_native::<SetKeyIterator>(receiver)?;
            iter.set_handle
        };

        let keys: Vec<ObjectHandle> = self.get_set_instance(set_handle)?
            .values()
            .flat_map(|b| b.iter().copied())
            .collect();

        let iter = self.get_native_mut::<SetKeyIterator>(receiver)?;
        if iter.index < keys.len() {
            let key = keys[iter.index];
            iter.index += 1;
            Ok(key)
        } else {
            Ok(ObjectHandle::ITER_END)
        }
    }

    // ---- registration ----

    pub fn register_set_builtins(&mut self) {
        let sc = self.obj_heap.set_class;
        self.register_native_method(sc, "add",      NativeFunction::a2(VirtualMachine::set_add));
        self.register_native_method(sc, "remove",   NativeFunction::a2(VirtualMachine::set_remove));
        self.register_native_method(sc, "contains", NativeFunction::a2(VirtualMachine::set_contains));
        self.register_native_method(sc, "__str__",  NativeFunction::a1(VirtualMachine::set_str));
        self.register_native_method(sc, "__bool__", NativeFunction::a1(VirtualMachine::set_bool));
        self.register_native_method(sc, "__not__",  NativeFunction::a1(VirtualMachine::set_not));
        self.register_native_method(sc, "__len__",  NativeFunction::a1(VirtualMachine::set_len));
        self.register_native_method(sc, "__iter__", NativeFunction::a1(VirtualMachine::set_iter));

        let sic = self.obj_heap.set_iter_class;
        self.register_native_method(sic, "__iter__", NativeFunction::a1(|_vm, receiver| Ok(receiver)));
        self.register_native_method(sic, "__next__", NativeFunction::a1(VirtualMachine::set_iter_next));
    }
}
