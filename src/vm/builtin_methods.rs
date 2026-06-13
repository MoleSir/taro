use crate::{ObjectInstanceData, ObjectHandle};
use super::{ExecuteError, ExecuteResult, VirtualMachine};

// ========================================================================== //
//                    Helpers
// ========================================================================== //

/// Return a slice of the top `arg_count` stack entries.
fn top_args(vm: &VirtualMachine, arg_count: usize) -> &[ObjectHandle] {
    &vm.stack[vm.stack.len() - arg_count..]
}

// ========================================================================== //
//                    List methods
// ========================================================================== //
//
// When called, the stack layout is:
//   [..., receiver, arg1, arg2, ...]
// arg_count includes the receiver (self).

impl VirtualMachine {
    /// `list.append(value)` — add an item to the end of the list.
    pub fn list_append(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        // self (receiver) + 1 explicit arg
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let value = args[1];
        let bi = self.obj_heap.get_instance_mut(receiver)?;
        match &mut bi.data {
            ObjectInstanceData::List(items) => {
                items.push(value);
                Ok(value)
            }
            _ => Err(ExecuteError::UnexpectType("list", self.value_type_name(receiver))),
        }
    }

    /// `list.pop()` — remove and return the last item.
    pub fn list_pop(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        // self only, no explicit args
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let bi = self.obj_heap.get_instance_mut(receiver)?;
        match &mut bi.data {
            ObjectInstanceData::List(items) => {
                items.pop().ok_or(ExecuteError::EmptyPop)
            }
            _ => Err(ExecuteError::UnexpectType("list", self.value_type_name(receiver))),
        }
    }

    /// `list.extend(other)` — extend this list with all items from another list.
    pub fn list_extend(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let other = args[1];
        let other_items = {
            let other_bi = self.obj_heap.get_instance_mut(other)?;
            match &other_bi.data {
                ObjectInstanceData::List(items) => items.clone(),
                _ => return Err(ExecuteError::UnexpectType("list", self.value_type_name(other))),
            }
        };
        let bi = self.obj_heap.get_instance_mut(receiver)?;
        match &mut bi.data {
            ObjectInstanceData::List(items) => {
                items.extend(other_items);
                Ok(ObjectHandle::NIL)
            }
            _ => Err(ExecuteError::UnexpectType("list", self.value_type_name(receiver))),
        }
    }
}

// ========================================================================== //
//                    Dict methods
// ========================================================================== //

impl VirtualMachine {
    /// `dict.get(key)` — get a value by key, returning nil if not found.
    pub fn dict_get(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let key = args[1];
        let entries = {
            let bi = self.obj_heap.get_instance_mut(receiver)?;
            match &bi.data {
                ObjectInstanceData::Dict(entries) => entries.clone(),
                _ => return Err(ExecuteError::UnexpectType("dict", self.value_type_name(receiver))),
            }
        };
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
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let keys: Vec<ObjectHandle> = {
            let bi = self.obj_heap.get_instance_mut(receiver)?;
            match &bi.data {
                ObjectInstanceData::Dict(entries) => entries.iter().map(|&(k, _)| k).collect(),
                _ => return Err(ExecuteError::UnexpectType("dict", self.value_type_name(receiver))),
            }
        };
        Ok(self.obj_heap.alloc_list(keys))
    }

    /// `dict.values()` — return a list of all values.
    pub fn dict_values(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let values: Vec<ObjectHandle> = {
            let bi = self.obj_heap.get_instance_mut(receiver)?;
            match &bi.data {
                ObjectInstanceData::Dict(entries) => entries.iter().map(|&(_, v)| v).collect(),
                _ => return Err(ExecuteError::UnexpectType("dict", self.value_type_name(receiver))),
            }
        };
        Ok(self.obj_heap.alloc_list(values))
    }

    /// `dict.pop(key)` — remove a key and return its value.
    pub fn dict_pop(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let key = args[1];

        // Clone entries, find key, remove, write back.
        let entries = {
            let bi = self.obj_heap.get_instance_mut(receiver)?;
            match &bi.data {
                ObjectInstanceData::Dict(entries) => entries.clone(),
                _ => return Err(ExecuteError::UnexpectType("dict", self.value_type_name(receiver))),
            }
        };
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
                let bi_mut = self.obj_heap.get_instance_mut(receiver)?;
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
}
