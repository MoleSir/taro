use crate::ObjectHandle;
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};
use super::utils::top_args;

impl VirtualMachine {
    pub fn list_not(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let items = self.get_list_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(items.is_empty()))
    }

    pub fn list_add(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let lhs_items = self.get_list_instance(args[0])?.clone();
        if let Ok(rhs_items) = self.get_list_instance(args[1]) {
            let mut new_items = lhs_items;
            new_items.extend_from_slice(rhs_items);
            return Ok(self.obj_heap.alloc_list_instance(new_items));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("add", "list", self.value_type_name(args[1])))
    }

    pub fn list_str(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let items = self.get_list_instance(args[0])?.clone();
        let mut result = String::from("[");
        for (i, &item) in items.iter().enumerate() {
            if i > 0 { result.push_str(", "); }
            result.push_str(&self.__str__(item)?);
        }
        result.push(']');
        Ok(self.obj_heap.alloc_string_instance(result.into()))
    }

    pub fn list_bool(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let items = self.get_list_instance(args[0])?;
        Ok(self.obj_heap.alloc_bool_instance(!items.is_empty()))
    }

    pub fn list_len(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let items = self.get_list_instance(args[0])?;
        Ok(self.obj_heap.alloc_integer_instance(items.len() as i64))
    }

    pub fn list_getitem(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let items = self.get_list_instance(args[0]).cloned()?;
        let idx_val = *self.get_integer_instance(args[1])?;
        let len = items.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(ExecuteError::IndexOutOfRange(idx, len));
        }
        Ok(items[idx as usize])
    }

    pub fn list_setitem(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 3 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 2, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let idx_val = *self.get_integer_instance(args[1])?;
        let value = args[2];
        let items = self.get_list_instance_mut(receiver)?;
        let len = items.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(ExecuteError::IndexOutOfRange(idx, len));
        }
        items[idx as usize] = value;
        Ok(value)
    }

    pub fn list_eq(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        if let Ok(rhs_items) = self.get_list_instance(args[1]) {
            let lhs_items = self.get_list_instance(args[0])?.clone();
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

    pub fn list_ne(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let eq = self.list_eq(arg_count)?;
        let b = *self.get_bool_instance(eq)?;
        Ok(self.obj_heap.alloc_bool_instance(!b))
    }
    
    /// `list.append(value)` — add an item to the end of the list.
    pub fn list_append(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        // self (receiver) + 1 explicit arg
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let value = args[1];
        let items = self.get_list_instance_mut(receiver)?;
        items.push(value);
        Ok(value)
    }

    /// `list.pop()` — remove and return the last item.
    pub fn list_pop(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        // self only, no explicit args
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let items = self.get_list_instance_mut(receiver)?;
        items.pop().ok_or(ExecuteError::EmptyPop)
    }

    /// `list.extend(other)` — extend this list with all items from another list.
    pub fn list_extend(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        let receiver = args[0];
        let other = args[1];
        let other_items = self.get_list_instance(other)?.clone();
        let items = self.get_list_instance_mut(receiver)?;
        items.extend(other_items);
        Ok(ObjectHandle::NIL)
    }
}
