use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{ObjectBytes, ObjectHandle, ObjectSet};
use std::collections::HashMap;

pub fn str(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.__str__(arg)?;
    Ok(vm.obj_heap.alloc_string_instance(s))
}

pub fn bool(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let b = vm.__bool__(arg)?;
    Ok(vm.obj_heap.alloc_bool_instance(b))
}

pub fn int(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let n = vm.__int__(arg)?;
    Ok(vm.obj_heap.alloc_integer_instance(n))
}

pub fn float(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let n = vm.__float__(arg)?;
    Ok(vm.obj_heap.alloc_float_instance(n))
}

/// list
pub fn list(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    Ok(vm.obj_heap.alloc_list_instance(args.to_vec()))
}

/// dict
pub fn dict(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    Ok(vm.obj_heap.alloc_dict_instance(std::collections::HashMap::new()))
}

/// `set(args...)` — create a new set from the given arguments.
pub fn set(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    let set_handle = vm.obj_heap.alloc_set_instance(HashMap::new());
    for &item in args {
        ObjectSet::add(vm, set_handle, item)?;
    }
    Ok(set_handle)
}

/// `bytes(value)` — create bytes from a string or list of ints.
pub fn bytes(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    // Snapshot what we need to decide, then drop the immutable borrow.
    let is_string = vm
        .obj_heap
        .get_instance(arg)
        .map(|inst| inst.data.as_any_ref().downcast_ref::<crate::object::ObjectString>().is_some())
        .unwrap_or(false);
    let is_list = vm
        .obj_heap
        .get_instance(arg)
        .map(|inst| inst.data.as_any_ref().downcast_ref::<crate::object::ObjectList>().is_some())
        .unwrap_or(false);

    if is_string {
        let s = vm.obj_heap.get_string_instance(arg).expect("must string").as_str().to_string();
        ObjectBytes::from_string(vm, s.as_str())
    } else if is_list {
        ObjectBytes::from_list(vm, arg)
    } else {
        Err(RuntimeErrorKind::UnexpectedType("string or list of ints", vm.obj_heap.type_of(arg)))
    }
}