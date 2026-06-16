use crate::{ToNativeData, NativeFunction, ObjectHandle};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

/// Native state for a list iterator.
struct ListIterator {
    list_handle: ObjectHandle,
    index: usize,
}

impl ToNativeData for ListIterator {
    fn mark_inner_object(&self, heap: &mut crate::ObjectHeap) {
        heap.mark_object(self.list_handle);
    }
}

impl VirtualMachine {
    pub fn list_not(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let items = self.get_list_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(items.is_empty()))
    }

    pub fn list_add(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_items = self.get_list_instance(lhs)?.clone();
        if let Ok(rhs_items) = self.get_list_instance(rhs) {
            let mut new_items = lhs_items;
            new_items.extend_from_slice(rhs_items);
            return Ok(self.obj_heap.alloc_list_instance(new_items));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("add", "list", self.value_type_name(rhs)))
    }

    pub fn list_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let items = self.get_list_instance(receiver)?.clone();
        let mut result = String::from("[");
        for (i, &item) in items.iter().enumerate() {
            if i > 0 { result.push_str(", "); }
            result.push_str(&self.__str__(item)?);
        }
        result.push(']');
        Ok(self.obj_heap.alloc_string_instance(result.into()))
    }

    pub fn list_bool(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let items = self.get_list_instance(receiver)?;
        Ok(self.obj_heap.alloc_bool_instance(!items.is_empty()))
    }

    pub fn list_len(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let items = self.get_list_instance(receiver)?;
        Ok(self.obj_heap.alloc_integer_instance(items.len() as i64))
    }

    pub fn list_getitem(&mut self, receiver: ObjectHandle, idx_handle: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let items = self.get_list_instance(receiver).cloned()?;
        let idx_val = *self.get_integer_instance(idx_handle)?;
        let len = items.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(ExecuteError::IndexOutOfRange(idx, len));
        }
        Ok(items[idx as usize])
    }

    pub fn list_setitem(&mut self, receiver: ObjectHandle, idx_handle: ObjectHandle, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let idx_val = *self.get_integer_instance(idx_handle)?;
        let items = self.get_list_instance_mut(receiver)?;
        let len = items.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(ExecuteError::IndexOutOfRange(idx, len));
        }
        items[idx as usize] = value;
        Ok(value)
    }

    pub fn list_eq(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if let Ok(rhs_items) = self.get_list_instance(rhs) {
            let lhs_items = self.get_list_instance(lhs)?.clone();
            let rhs_items = rhs_items.clone();
            if lhs_items.len() != rhs_items.len() {
                return Ok(self.obj_heap.alloc_bool_instance(false));
            }
            for (&a, &b) in lhs_items.iter().zip(rhs_items.iter()) {
                let eq_result = self.__eq__(a, b)?;
                if !self.__bool__(eq_result)? {
                    return Ok(self.obj_heap.alloc_bool_instance(false));
                }
            }
            return Ok(self.obj_heap.alloc_bool_instance(true));
        }
        // Different types are not equal.
        Ok(self.obj_heap.alloc_bool_instance(false))
    }

    pub fn list_ne(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let eq = self.list_eq(lhs, rhs)?;
        let b = *self.get_bool_instance(eq)?;
        Ok(self.obj_heap.alloc_bool_instance(!b))
    }

    /// `list.append(value)` — add an item to the end of the list.
    pub fn list_append(&mut self, receiver: ObjectHandle, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let items = self.get_list_instance_mut(receiver)?;
        items.push(value);
        Ok(value)
    }

    /// `list.pop()` — remove and return the last item.
    pub fn list_pop(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let items = self.get_list_instance_mut(receiver)?;
        items.pop().ok_or(ExecuteError::EmptyPop)
    }

    /// `list.extend(other)` — extend this list with all items from another list.
    pub fn list_extend(&mut self, receiver: ObjectHandle, other: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let other_items = self.get_list_instance(other)?.clone();
        let items = self.get_list_instance_mut(receiver)?;
        items.extend(other_items);
        Ok(ObjectHandle::NIL)
    }

    // ---- iteration protocol ----

    pub fn list_iter(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let iter = ListIterator { list_handle: receiver, index: 0 };
        Ok(self.obj_heap.alloc_instance(
            self.obj_heap.list_iter_class,
            crate::ObjectInstanceData::Native(crate::NativeData::new(iter)),
        ))
    }

    pub fn list_iter_next(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let list_handle = {
            let iter = self.get_native::<ListIterator>(receiver)?;
            iter.list_handle
        };
        // Read the items first, then update the index.
        let items = self.get_list_instance(list_handle)?.clone();
        let iter = self.get_native_mut::<ListIterator>(receiver)?;
        if iter.index < items.len() {
            let value = items[iter.index];
            iter.index += 1;
            Ok(value)
        } else {
            Ok(ObjectHandle::ITER_END)
        }
    }

    pub fn register_list_builtins(&mut self) {
        let lc = self.obj_heap.list_class;
        self.register_native_method(lc, "__not__",     NativeFunction::a1(VirtualMachine::list_not));
        self.register_native_method(lc, "__add__",     NativeFunction::a2(VirtualMachine::list_add));
        self.register_native_method(lc, "__eq__",      NativeFunction::a2(VirtualMachine::list_eq));
        self.register_native_method(lc, "__ne__",      NativeFunction::a2(VirtualMachine::list_ne));
        self.register_native_method(lc, "__str__",     NativeFunction::a1(VirtualMachine::list_str));
        self.register_native_method(lc, "__bool__",    NativeFunction::a1(VirtualMachine::list_bool));
        self.register_native_method(lc, "__len__",     NativeFunction::a1(VirtualMachine::list_len));
        self.register_native_method(lc, "__getitem__", NativeFunction::a2(VirtualMachine::list_getitem));
        self.register_native_method(lc, "__setitem__", NativeFunction::a3(VirtualMachine::list_setitem));
        self.register_native_method(lc, "append",      NativeFunction::a2(VirtualMachine::list_append));
        self.register_native_method(lc, "pop",         NativeFunction::a1(VirtualMachine::list_pop));
        self.register_native_method(lc, "len",         NativeFunction::a1(VirtualMachine::list_len));
        self.register_native_method(lc, "extend",      NativeFunction::a2(VirtualMachine::list_extend));

        self.register_native_method(lc, "__iter__", NativeFunction::a1(VirtualMachine::list_iter));

        let lic = self.obj_heap.list_iter_class;
        self.register_native_method(lic, "__iter__", NativeFunction::a1(|_vm, receiver| Ok(receiver)));
        self.register_native_method(lic, "__next__", NativeFunction::a1(VirtualMachine::list_iter_next));
    }
}
