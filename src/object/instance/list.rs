use crate::{
    native_a1,
    NativeFunction, ObjectHandle, ObjectInstanceData,
    vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine},
};
use super::ObjectHeap;

// ========================================================================== //
//  ObjectListIterator (iterator state)
// ========================================================================== //

/// Iterator state for a list.
pub struct ObjectListIterator {
    pub list_handle: ObjectHandle,
    pub index: usize,
}

// ========================================================================== //
//  ObjectList
// ========================================================================== //

/// Represents the `List` built-in type.
pub struct ObjectList;

// A free function that returns the receiver unchanged — used for iterator
// `__iter__` implementations that just return `self`.
fn identity_iter(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    Ok(receiver)
}

impl ObjectList {
    native_a1!(__not__, items: &Vec<ObjectHandle>, { items.is_empty() });

    pub fn __add__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_items = vm.get_list_instance(lhs)?.clone();
        if let Ok(rhs_items) = vm.get_list_instance(rhs) {
            let mut new_items = lhs_items;
            new_items.extend_from_slice(rhs_items);
            return Ok(vm.obj_heap.alloc_list_instance(new_items));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("add", "list", vm.value_type_name(rhs)))
    }

    pub fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let items = vm.get_list_instance(receiver)?.clone();
        let mut result = String::from("[");
        for (i, &item) in items.iter().enumerate() {
            if i > 0 { result.push_str(", "); }
            result.push_str(&vm.__str__(item)?);
        }
        result.push(']');
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    native_a1!(__bool__, items: &Vec<ObjectHandle>, { !items.is_empty() });

    native_a1!(__len__, items: &Vec<ObjectHandle>, { items.len() as i64 });

    pub fn len(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> { Self::__len__(vm, receiver) }

    pub fn __getitem__(vm: &mut VirtualMachine, receiver: ObjectHandle, idx_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let items = vm.get_list_instance(receiver).cloned()?;
        let idx_val = *vm.get_integer_instance(idx_handle)?;
        let len = items.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(RuntimeErrorKind::IndexOutOfRange(idx, len));
        }
        Ok(items[idx as usize])
    }

    pub fn __setitem__(vm: &mut VirtualMachine, receiver: ObjectHandle, idx_handle: ObjectHandle, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let idx_val = *vm.get_integer_instance(idx_handle)?;
        let items = vm.get_list_instance_mut(receiver)?;
        let len = items.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(RuntimeErrorKind::IndexOutOfRange(idx, len));
        }
        items[idx as usize] = value;
        Ok(value)
    }

    pub fn __eq__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if let Ok(rhs_items) = vm.get_list_instance(rhs) {
            let lhs_items = vm.get_list_instance(lhs)?.clone();
            let rhs_items = rhs_items.clone();
            if lhs_items.len() != rhs_items.len() {
                return Ok(vm.obj_heap.alloc_bool_instance(false));
            }
            for (&a, &b) in lhs_items.iter().zip(rhs_items.iter()) {
                let eq_result = vm.__eq__(a, b)?;
                if !vm.__bool__(eq_result)? {
                    return Ok(vm.obj_heap.alloc_bool_instance(false));
                }
            }
            return Ok(vm.obj_heap.alloc_bool_instance(true));
        }
        Ok(vm.obj_heap.alloc_bool_instance(false))
    }

    pub fn __ne__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let eq = Self::__eq__(vm, lhs, rhs)?;
        let b = *vm.get_bool_instance(eq)?;
        Ok(vm.obj_heap.alloc_bool_instance(!b))
    }

    /// `list.append(value)` — add an item to the end of the list.
    pub fn append(vm: &mut VirtualMachine, receiver: ObjectHandle, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let items = vm.get_list_instance_mut(receiver)?;
        items.push(value);
        Ok(value)
    }

    /// `list.pop()` — remove and return the last item.
    pub fn pop(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let items = vm.get_list_instance_mut(receiver)?;
        items.pop().ok_or(RuntimeErrorKind::EmptyPop)
    }

    /// `list.extend(other)` — extend this list with all items from another list.
    pub fn extend(vm: &mut VirtualMachine, receiver: ObjectHandle, other: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let other_items = vm.get_list_instance(other)?.clone();
        let items = vm.get_list_instance_mut(receiver)?;
        items.extend(other_items);
        Ok(ObjectHandle::NIL)
    }

    // ---- iteration protocol ----

    pub fn __iter__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let iter = ObjectListIterator { list_handle: receiver, index: 0 };
        Ok(vm.obj_heap.alloc_instance(
            vm.obj_heap.list_iter_class,
            ObjectInstanceData::ListIter(iter),
        ))
    }

    pub fn iter_next(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let (list_handle, idx) = {
            let iter = vm.get_list_iter(receiver)?;
            (iter.list_handle, iter.index)
        };
        let items = vm.get_list_instance(list_handle)?;
        if idx >= items.len() {
            return Ok(ObjectHandle::ITER_END);
        }
        let value = items[idx];
        // NLL drops `items` reference here; the &mut self borrow below is
        // now exclusive, allowing the index update.
        let iter = vm.get_list_iter_mut(receiver)?;
        iter.index = idx + 1;
        Ok(value)
    }
}

// ========================================================================== //
//  Registration
// ========================================================================== //

/// Register all `List` magic methods directly on the class during heap init.
pub fn register_list_builtins(heap: &mut ObjectHeap) {
    let lc = heap.list_class;
    heap.register_native_method(lc, "__not__",     NativeFunction::a1(ObjectList::__not__));
    heap.register_native_method(lc, "__add__",     NativeFunction::a2(ObjectList::__add__));
    heap.register_native_method(lc, "__eq__",      NativeFunction::a2(ObjectList::__eq__));
    heap.register_native_method(lc, "__ne__",      NativeFunction::a2(ObjectList::__ne__));
    heap.register_native_method(lc, "__str__",     NativeFunction::a1(ObjectList::__str__));
    heap.register_native_method(lc, "__bool__",    NativeFunction::a1(ObjectList::__bool__));
    heap.register_native_method(lc, "__len__",     NativeFunction::a1(ObjectList::__len__));
    heap.register_native_method(lc, "__getitem__", NativeFunction::a2(ObjectList::__getitem__));
    heap.register_native_method(lc, "__setitem__", NativeFunction::a3(ObjectList::__setitem__));
    heap.register_native_method(lc, "append",      NativeFunction::a2(ObjectList::append));
    heap.register_native_method(lc, "pop",         NativeFunction::a1(ObjectList::pop));
    heap.register_native_method(lc, "len",         NativeFunction::a1(ObjectList::len));
    heap.register_native_method(lc, "extend",      NativeFunction::a2(ObjectList::extend));
    heap.register_native_method(lc, "__iter__",    NativeFunction::a1(ObjectList::__iter__));

    let lic = heap.list_iter_class;
    heap.register_native_method(lic, "__iter__", NativeFunction::a1(identity_iter));
    heap.register_native_method(lic, "__next__", NativeFunction::a1(ObjectList::iter_next));
}
