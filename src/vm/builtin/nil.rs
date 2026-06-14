use crate::ObjectHandle;
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

/// Return a slice of the top `arg_count` stack entries.
fn top_args(vm: &VirtualMachine, arg_count: usize) -> &[ObjectHandle] {
    &vm.stack[vm.stack.len() - arg_count..]
}

impl VirtualMachine {
    pub fn nil_not(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        Ok(self.obj_heap.alloc_bool_instance(true))
    }

    pub fn nil_eq(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        // Only nil equals nil.
        Ok(self.obj_heap.alloc_bool_instance(args[1].is_nil()))
    }

    pub fn nil_ne(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: arg_count.saturating_sub(1) })?;
        }
        let args = top_args(self, arg_count);
        Ok(self.obj_heap.alloc_bool_instance(!args[1].is_nil()))
    }

    pub fn nil_str(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        Ok(self.obj_heap.alloc_string_instance("nil".into()))
    }

    pub fn nil_bool(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count.saturating_sub(1) })?;
        }
        Ok(self.obj_heap.alloc_bool_instance(false))
    }
}
