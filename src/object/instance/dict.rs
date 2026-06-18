use crate::{
    NativeFunction, NativeData, ObjectHandle, ObjectInstanceData, ToNativeData,
    vm::{ExecuteError, ExecuteResult, VirtualMachine},
};
use super::ObjectHeap;

// ========================================================================== //
//  DictKeyIterator (native state)
// ========================================================================== //

/// Native state for a dict-key iterator.
///
/// Keys are collected eagerly at iterator-creation time so that each
/// `__next__` call is O(1) — no per-step re-collection or cloning.
struct DictKeyIterator {
    keys: Vec<ObjectHandle>,
    index: usize,
}

impl ToNativeData for DictKeyIterator {
    fn mark_inner_object(&self, heap: &mut ObjectHeap) {
        for &key in &self.keys {
            heap.mark_object(key);
        }
    }
}

// ========================================================================== //
//  ObjectDict
// ========================================================================== //

/// Represents the `Dict` built-in type.
pub struct ObjectDict;

impl ObjectDict {
    pub fn __not__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = vm.get_dict_instance(receiver)?;
        let is_empty = entries.values().all(|b| b.is_empty());
        Ok(vm.obj_heap.alloc_bool_instance(is_empty))
    }

    pub fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let all_entries: Vec<(ObjectHandle, ObjectHandle)> = vm.get_dict_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().copied())
            .collect();

        let mut result = String::from("{");
        let mut first = true;
        for (k, v) in all_entries {
            if !first { result.push_str(", "); }
            first = false;
            result.push_str(&vm.__str__(k)?);
            result.push_str(": ");
            result.push_str(&vm.__str__(v)?);
        }
        result.push('}');
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    pub fn __bool__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = vm.get_dict_instance(receiver)?;
        let has_any = entries.values().any(|b| !b.is_empty());
        Ok(vm.obj_heap.alloc_bool_instance(has_any))
    }

    pub fn __len__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let entries = vm.get_dict_instance(receiver)?;
        let total: usize = entries.values().map(|b| b.len()).sum();
        Ok(vm.obj_heap.alloc_integer_instance(total as i64))
    }

    pub fn len(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        Self::__len__(vm, receiver)
    }

    pub fn __getitem__(vm: &mut VirtualMachine, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = vm.__hash__(key)?;

        let bucket = {
            let entries = vm.get_dict_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        for &(k, v) in &bucket {
            let eq = vm.__eq__(k, key)?;
            if vm.__bool__(eq)? {
                return Ok(v);
            }
        }
        Err(ExecuteError::KeyNotFound)
    }

    pub fn __setitem__(vm: &mut VirtualMachine, receiver: ObjectHandle, key: ObjectHandle, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = vm.__hash__(key)?;

        let mut bucket = {
            let entries = vm.get_dict_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        let mut found_idx = None;
        for (i, &(k, _)) in bucket.iter().enumerate() {
            let eq = vm.__eq__(k, key)?;
            if vm.__bool__(eq)? {
                found_idx = Some(i);
                break;
            }
        }
        if let Some(i) = found_idx {
            bucket[i].1 = value;
        } else {
            bucket.push((key, value));
        }

        let inst = vm.get_instance_mut(receiver)?;
        if let ObjectInstanceData::Dict(entries) = &mut inst.data {
            entries.insert(hash, bucket);
        }
        Ok(value)
    }

    /// `dict.get(key)` — get a value by key, returning nil if not found.
    pub fn get(vm: &mut VirtualMachine, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = vm.__hash__(key)?;

        let bucket = {
            let entries = vm.get_dict_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        for &(k, v) in &bucket {
            let eq_result = vm.__eq__(k, key)?;
            if vm.__bool__(eq_result)? {
                return Ok(v);
            }
        }
        Ok(ObjectHandle::NIL)
    }

    /// `dict.keys()` — return a list of all keys.
    pub fn keys(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let keys: Vec<ObjectHandle> = vm.get_dict_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().map(|&(k, _)| k))
            .collect();
        Ok(vm.obj_heap.alloc_list_instance(keys))
    }

    /// `dict.values()` — return a list of all values.
    pub fn values(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let values: Vec<ObjectHandle> = vm.get_dict_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().map(|&(_, v)| v))
            .collect();
        Ok(vm.obj_heap.alloc_list_instance(values))
    }

    pub fn contains(vm: &mut VirtualMachine, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = vm.__hash__(key)?;
        let entries = vm.get_dict_instance(receiver)?;
        if let Some(bucket) = entries.get(&hash).cloned() {
            for &(k, _) in bucket.iter() {
                let eq = vm.__eq__(k, key)?;
                if vm.__bool__(eq)? {
                    return Ok(vm.obj_heap.alloc_bool_instance(true))
                }
            }
        }

        Ok(vm.obj_heap.alloc_bool_instance(false))
    }

    /// `dict.pop(key)` — remove a key and return its value.
    pub fn pop(vm: &mut VirtualMachine, receiver: ObjectHandle, key: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let hash = vm.__hash__(key)?;

        let mut bucket = {
            let entries = vm.get_dict_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        let mut found_idx = None;
        for (i, &(k, _)) in bucket.iter().enumerate() {
            let eq = vm.__eq__(k, key)?;
            if vm.__bool__(eq)? {
                found_idx = Some(i);
                break;
            }
        }

        match found_idx {
            Some(idx) => {
                let removed = bucket.remove(idx);
                let inst = vm.get_instance_mut(receiver)?;
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

    pub fn __iter__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let keys: Vec<ObjectHandle> = vm.get_dict_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().map(|&(k, _)| k))
            .collect();
        Ok(vm.obj_heap.alloc_instance(
            vm.obj_heap.dict_iter_class,
            ObjectInstanceData::Native(NativeData::new(DictKeyIterator { keys, index: 0 })),
        ))
    }

    pub fn iter_next(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let iter = vm.get_native_mut::<DictKeyIterator>(receiver)?;
        if iter.index < iter.keys.len() {
            let key = iter.keys[iter.index];
            iter.index += 1;
            Ok(key)
        } else {
            Ok(ObjectHandle::ITER_END)
        }
    }
}

// A free function that returns the receiver unchanged — used for iterator
// `__iter__` implementations that just return `self`.
fn identity_iter(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    Ok(receiver)
}

// ========================================================================== //
//  Registration
// ========================================================================== //

/// Register all `Dict` magic methods directly on the class during heap init.
pub fn register_dict_builtins(heap: &mut ObjectHeap) {
    let dc = heap.dict_class;
    heap.register_native_method(dc, "__not__",     NativeFunction::a1(ObjectDict::__not__));
    heap.register_native_method(dc, "__str__",     NativeFunction::a1(ObjectDict::__str__));
    heap.register_native_method(dc, "__bool__",    NativeFunction::a1(ObjectDict::__bool__));
    heap.register_native_method(dc, "__len__",     NativeFunction::a1(ObjectDict::__len__));
    heap.register_native_method(dc, "__getitem__", NativeFunction::a2(ObjectDict::__getitem__));
    heap.register_native_method(dc, "__setitem__", NativeFunction::a3(ObjectDict::__setitem__));
    heap.register_native_method(dc, "get",         NativeFunction::a2(ObjectDict::get));
    heap.register_native_method(dc, "keys",        NativeFunction::a1(ObjectDict::keys));
    heap.register_native_method(dc, "values",      NativeFunction::a1(ObjectDict::values));
    heap.register_native_method(dc, "pop",         NativeFunction::a2(ObjectDict::pop));
    heap.register_native_method(dc, "contains",    NativeFunction::a2(ObjectDict::contains));
    heap.register_native_method(dc, "len",         NativeFunction::a1(ObjectDict::len));
    heap.register_native_method(dc, "__iter__",    NativeFunction::a1(ObjectDict::__iter__));

    let dic = heap.dict_iter_class;
    heap.register_native_method(dic, "__iter__", NativeFunction::a1(identity_iter));
    heap.register_native_method(dic, "__next__", NativeFunction::a1(ObjectDict::iter_next));
}
