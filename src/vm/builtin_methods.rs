use crate::Value;
use super::{ExecuteError, ExecuteResult, VirtualMachine};

// ========================================================================== //
//                    List methods
// ========================================================================== //
//
// When called, the stack layout is:
//   [..., receiver, arg1, arg2, ...]
// arg_count includes the receiver (self).

impl VirtualMachine {
    /// `list.append(value)` — add an item to the end of the list.
    pub fn list_append(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        // self (receiver) + 1 explicit arg
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = &self.stack[self.stack.len() - arg_count..];
        let receiver = args[0].as_object()?;
        let value = args[1].clone();
        let list = self.obj_heap.get_list_mut(receiver)?;
        list.items.push(value.clone());
        Ok(value)
    }

    /// `list.pop()` — remove and return the last item.
    pub fn list_pop(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        // self only, no explicit args
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = &self.stack[self.stack.len() - arg_count..];
        let receiver = args[0].as_object()?;
        let list = self.obj_heap.get_list_mut(receiver)?;
        list.items.pop().ok_or(ExecuteError::EmptyPop)
    }

    /// `list.extend(other)` — extend this list with all items from another list.
    pub fn list_extend(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = &self.stack[self.stack.len() - arg_count..];
        let receiver = args[0].as_object()?;
        let other_val = args[1].clone();
        let other_handle = other_val.as_object()?;
        let other_items = {
            let other_list = self.obj_heap.get_list(other_handle)?;
            other_list.items.clone()
        };
        let list = self.obj_heap.get_list_mut(receiver)?;
        list.items.extend(other_items);
        Ok(Value::Nil)
    }
}

// ========================================================================== //
//                    Dict methods
// ========================================================================== //

impl VirtualMachine {
    /// `dict.get(key)` — get a value by key, returning nil if not found.
    pub fn dict_get(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = &self.stack[self.stack.len() - arg_count..];
        let receiver = args[0].as_object()?;
        let key = args[1].clone();
        let dict = self.obj_heap.get_dict(receiver)?;
        Ok(dict.items.get(&key).cloned().unwrap_or(Value::Nil))
    }

    /// `dict.keys()` — return a list of all keys.
    pub fn dict_keys(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = &self.stack[self.stack.len() - arg_count..];
        let receiver = args[0].as_object()?;
        let keys: Vec<Value> = {
            let dict = self.obj_heap.get_dict(receiver)?;
            dict.items.keys().cloned().collect()
        };
        Ok(Value::Object(self.obj_heap.alloc_list(self.list_class, keys)))
    }

    /// `dict.values()` — return a list of all values.
    pub fn dict_values(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = &self.stack[self.stack.len() - arg_count..];
        let receiver = args[0].as_object()?;
        let values: Vec<Value> = {
            let dict = self.obj_heap.get_dict(receiver)?;
            dict.items.values().cloned().collect()
        };
        Ok(Value::Object(self.obj_heap.alloc_list(self.list_class, values)))
    }

    /// `dict.pop(key)` — remove a key and return its value.
    pub fn dict_pop(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = &self.stack[self.stack.len() - arg_count..];
        let receiver = args[0].as_object()?;
        let key = args[1].clone();
        let dict = self.obj_heap.get_dict_mut(receiver)?;
        dict.items.remove(&key).ok_or(ExecuteError::KeyNotFound)
    }
}
