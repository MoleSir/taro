//! FFI call dispatch: build CIF, marshal arguments, invoke, convert result.

use std::ffi::c_void;

use crate::ObjectHandle;
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};

use super::types::{CType, CValue};

// ===========================================================================
// dispatch_call — common tail for both ffi.call and bound-function calls
// ===========================================================================

pub(super) fn dispatch_call(
    vm: &mut VirtualMachine,
    cif: &libffi::middle::Cif,
    ret_type: CType,
    func_ptr: *mut c_void,
    c_values: &[CValue],
) -> RuntimeResult<ObjectHandle> {
    let args: Vec<libffi::middle::Arg> = c_values.iter().map(|cv| cv.as_arg()).collect();
    let code_ptr = libffi::middle::CodePtr(func_ptr);
    ret_type.call_ffi(vm, cif, code_ptr, &args)
}

// ===========================================================================
// ffi_call_impl — low-level ffi.call implementation
// ===========================================================================

pub(super) fn ffi_call_impl(
    vm: &mut VirtualMachine,
    func_ptr_raw: i64,
    ret_type_str: &str,
    arg_type_handles: &[ObjectHandle],
    arg_handles: &[ObjectHandle],
) -> RuntimeResult<ObjectHandle> {
    // Parse return type.
    let ret_type = CType::from_str(ret_type_str)?;

    // Parse argument types (handles both strings and struct defs).
    let arg_types: Vec<CType> = arg_type_handles.iter().map(|&h| CType::from_handle(vm, h)).collect::<RuntimeResult<_>>()?;

    if arg_handles.len() != arg_types.len() {
        return Err(RuntimeErrorKind::FfiError(format!(
            "argument count mismatch: {} value(s) but {} type(s)",
            arg_handles.len(),
            arg_types.len()
        )));
    }

    // Build CIF (ad-hoc call — no caching, same as before).
    let ffi_arg_types: Vec<libffi::middle::Type> = arg_types.iter().map(|ct| ct.to_ffi_type()).collect();
    let cif = libffi::middle::Cif::new(ffi_arg_types, ret_type.to_ffi_type());

    // Marshal arguments.
    let mut c_values: Vec<CValue> = Vec::with_capacity(arg_handles.len());
    for (i, (ct, &handle)) in arg_types.iter().zip(arg_handles).enumerate() {
        let cv = ct.taro_to_cvalue(vm, handle).map_err(|e| RuntimeErrorKind::FfiError(format!("argument {i}: {e}")))?;
        c_values.push(cv);
    }

    dispatch_call(vm, &cif, ret_type, func_ptr_raw as *mut c_void, &c_values)
}

// ===========================================================================
// ffi.call — Taro-level FFI call entry point
// ===========================================================================

pub(super) fn ffi_call(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.len() < 3 {
        return Err(RuntimeErrorKind::FfiError("ffi.call(func_ptr, ret_type, arg_types, args) — need at least 3 arguments".into()));
    }

    let func_ptr = vm.expect_type(vm.obj_heap.get_integer_instance(args[0]), args[0], "int").copied()?;
    let ret_type = vm.expect_type(vm.obj_heap.get_string_instance(args[1]), args[1], "string")?.as_str().to_string();
    let arg_types_list: Vec<ObjectHandle> = vm.expect_type(vm.obj_heap.get_list_instance(args[2]), args[2], "list")?.clone();
    let arg_values: Vec<ObjectHandle> = if args.len() > 3 { vm.expect_type(vm.obj_heap.get_list_instance(args[3]), args[3], "list")?.clone() } else { vec![] };

    ffi_call_impl(vm, func_ptr, &ret_type, &arg_types_list, &arg_values)
}
