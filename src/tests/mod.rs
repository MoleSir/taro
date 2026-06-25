use crate::vm::VirtualMachine;
use super::vm::module::ModuleKey;
use crate::{Chunk, Instruction, ObjectHandle, ObjectHeap};

/// Build a chunk and run it: creates VM first, then calls `build` with VM's heap.
fn run_chunk(build: impl FnOnce(&mut Chunk, &mut ObjectHeap)) -> VirtualMachine {
    let mut vm = VirtualMachine::new();
    // SAFETY: `vm.obj_heap` and the local `chunk` are separate allocations.
    // `build` only uses the heap for pushing constants; it never reads or
    // modifies vm internals.  The raw pointer is valid for the duration of
    // the call because `vm` is pinned on the stack.
    let heap_ptr = &mut vm.obj_heap as *mut ObjectHeap;
    let mut chunk = Chunk::new();
    unsafe {
        build(&mut chunk, &mut *heap_ptr);
    }
    let function = vm.obj_heap.alloc_function("script", 0, 0, vec![], vec![], chunk);
    vm.interpret_function(function).unwrap();
    vm
}

/// Helper: get integer value from an instance handle.
fn get_int(vm: &VirtualMachine, handle: ObjectHandle) -> i64 {
    *vm.obj_heap.get_integer_instance(handle).unwrap()
}

/// Helper: get float value.
fn get_float(vm: &VirtualMachine, handle: ObjectHandle) -> f64 {
    *vm.obj_heap.get_float_instance(handle).unwrap()
}

/// Helper: get bool value.
fn get_bool(vm: &VirtualMachine, handle: ObjectHandle) -> bool {
    *vm.obj_heap.get_bool_instance(handle).unwrap()
}

/// Helper: check nil.
fn is_nil(handle: ObjectHandle) -> bool {
    handle.is_nil()
}

/// Path to the math test module used by import tests.
pub(super) fn math_module_path() -> String {
    "tests/scripts/lib/math.taro".to_string()
}

/// Helper: return a temporary file path for file/std tests.
pub(super) fn tmp_file_path(name: &str) -> String {
    format!("/tmp/taro_test_{name}")
}

/// Clean up a temporary test file.
pub(super) fn rm_tmp(name: &str) {
    let path = tmp_file_path(name);
    let _ = std::fs::remove_file(&path);
}

mod arith;
mod builtin;
mod call_magic;
mod class;
mod collections;
mod control_flow;
mod functions;
mod import;
mod regression;
mod stdlib;
mod type_error;
