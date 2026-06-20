use std::collections::HashMap;

use crate::{
    NativeFunction, ObjectHandle, ObjectInstanceData,
    vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine},
};
use super::ObjectHeap;

// ========================================================================== //
//  ObjectSetIterator (iterator state)
// ========================================================================== //

/// Iterator state for a set-key iterator.
///
/// Items are collected eagerly at iterator-creation time so that each
/// `__next__` call is O(1) — no per-step re-collection or cloning.
pub struct ObjectSetIterator {
    pub items: Vec<ObjectHandle>,
    pub index: usize,
}

// ========================================================================== //
//  ObjectSet
// ========================================================================== //

/// Represents the `Set` built-in type.
pub struct ObjectSet;

impl ObjectSet {
    // ---- global constructor ----

    /// `set(args...)` — create a new set from the given arguments.
    pub fn constructor(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let set_handle = vm.obj_heap.alloc_set_instance(HashMap::new());
        for &item in args {
            Self::add(vm, set_handle, item)?;
        }
        Ok(set_handle)
    }

    // ---- core operations ----

    /// `s.add(item)` — add an item to the set.
    pub fn add(vm: &mut VirtualMachine, receiver: ObjectHandle, item: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let hash = vm.__hash__(item)?;

        let mut bucket = {
            let entries = vm.get_set_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        for &existing in &bucket {
            let eq = vm.__eq__(existing, item)?;
            if vm.__bool__(eq)? {
                return Ok(item); // already present — no-op
            }
        }
        bucket.push(item);

        let inst = vm.get_instance_mut(receiver)?;
        if let ObjectInstanceData::Set(entries) = &mut inst.data {
            entries.insert(hash, bucket);
        }
        Ok(item)
    }

    /// `s.remove(item)` — remove an item from the set.
    pub fn remove(vm: &mut VirtualMachine, receiver: ObjectHandle, item: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let hash = vm.__hash__(item)?;

        let mut bucket = {
            let entries = vm.get_set_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        let mut found_idx = None;
        for (i, &existing) in bucket.iter().enumerate() {
            let eq = vm.__eq__(existing, item)?;
            if vm.__bool__(eq)? {
                found_idx = Some(i);
                break;
            }
        }

        match found_idx {
            Some(idx) => {
                let removed = bucket.remove(idx);
                let inst = vm.get_instance_mut(receiver)?;
                if let ObjectInstanceData::Set(entries) = &mut inst.data {
                    if bucket.is_empty() {
                        entries.remove(&hash);
                    } else {
                        entries.insert(hash, bucket);
                    }
                }
                Ok(removed)
            }
            None => Err(RuntimeErrorKind::KeyNotFound),
        }
    }

    /// `s.contains(item)` — check if item is in the set.
    pub fn contains(vm: &mut VirtualMachine, receiver: ObjectHandle, item: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let hash = vm.__hash__(item)?;

        let bucket = {
            let entries = vm.get_set_instance(receiver)?;
            entries.get(&hash).cloned().unwrap_or_default()
        };

        for &existing in &bucket {
            let eq = vm.__eq__(existing, item)?;
            if vm.__bool__(eq)? {
                return Ok(vm.obj_heap.alloc_bool_instance(true));
            }
        }
        Ok(vm.obj_heap.alloc_bool_instance(false))
    }

    // ---- magic methods ----

    pub fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let all_items: Vec<ObjectHandle> = vm.get_set_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().copied())
            .collect();

        let mut result = String::from("{");
        let mut first = true;
        for item in all_items {
            if !first { result.push_str(", "); }
            first = false;
            result.push_str(&vm.__str__(item)?);
        }
        result.push('}');
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    pub fn __bool__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let entries = vm.get_set_instance(receiver)?;
        let has_any = entries.values().any(|b| !b.is_empty());
        Ok(vm.obj_heap.alloc_bool_instance(has_any))
    }

    pub fn __not__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let entries = vm.get_set_instance(receiver)?;
        let is_empty = entries.values().all(|b| b.is_empty());
        Ok(vm.obj_heap.alloc_bool_instance(is_empty))
    }

    pub fn __len__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let entries = vm.get_set_instance(receiver)?;
        let total: usize = entries.values().map(|b| b.len()).sum();
        Ok(vm.obj_heap.alloc_integer_instance(total as i64))
    }

    // ---- iteration protocol ----

    pub fn __iter__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let items: Vec<ObjectHandle> = vm.get_set_instance(receiver)?
            .values()
            .flat_map(|b| b.iter().copied())
            .collect();
        Ok(vm.obj_heap.alloc_instance(
            vm.obj_heap.set_iter_class,
            ObjectInstanceData::SetIter(ObjectSetIterator { items, index: 0 }),
        ))
    }

    pub fn iter_next(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let iter = vm.get_set_iter_mut(receiver)?;
        if iter.index < iter.items.len() {
            let item = iter.items[iter.index];
            iter.index += 1;
            Ok(item)
        } else {
            Ok(ObjectHandle::ITER_END)
        }
    }
}

// A free function that returns the receiver unchanged — used for iterator
// `__iter__` implementations that just return `self`.
fn identity_iter(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    Ok(receiver)
}

// ========================================================================== //
//  Registration
// ========================================================================== //

/// Register all `Set` magic methods directly on the class during heap init.
pub fn register_set_builtins(heap: &mut ObjectHeap) {
    let sc = heap.set_class;
    heap.register_native_method(sc, "add",      NativeFunction::a2(ObjectSet::add));
    heap.register_native_method(sc, "remove",   NativeFunction::a2(ObjectSet::remove));
    heap.register_native_method(sc, "contains", NativeFunction::a2(ObjectSet::contains));
    heap.register_native_method(sc, "__str__",  NativeFunction::a1(ObjectSet::__str__));
    heap.register_native_method(sc, "__bool__", NativeFunction::a1(ObjectSet::__bool__));
    heap.register_native_method(sc, "__not__",  NativeFunction::a1(ObjectSet::__not__));
    heap.register_native_method(sc, "__len__",  NativeFunction::a1(ObjectSet::__len__));
    heap.register_native_method(sc, "__iter__", NativeFunction::a1(ObjectSet::__iter__));

    let sic = heap.set_iter_class;
    heap.register_native_method(sic, "__iter__", NativeFunction::a1(identity_iter));
    heap.register_native_method(sic, "__next__", NativeFunction::a1(ObjectSet::iter_next));
}
