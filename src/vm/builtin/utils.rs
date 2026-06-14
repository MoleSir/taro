use crate::ObjectHandle;
use crate::vm::VirtualMachine;

/// Return a slice of the top `arg_count` stack entries.
pub fn top_args(vm: &VirtualMachine, arg_count: usize) -> &[ObjectHandle] {
    &vm.stack[vm.stack.len() - arg_count..]
}
