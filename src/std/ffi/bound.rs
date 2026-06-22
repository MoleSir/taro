//! Bound C function — pre-parsed type info with a cached libffi CIF.
//!
//! The CIF is built once at `bind()` time and reused on every call, avoiding
//! per-call allocations.  Because `libffi::middle::Cif` is `!Send + !Sync`,
//! we wrap it in [`CachedCif`] with an unsafe Send+Sync impl — justified by
//! the VM being single-threaded and the CIF being read-only after construction.

use std::ffi::c_void;

use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, ShrString, impl_object_instance_data};

use super::call::dispatch_call;
use super::types::CType;

struct CachedCif(libffi::middle::Cif);

// SAFETY: see struct documentation above.
unsafe impl Send for CachedCif {}
unsafe impl Sync for CachedCif {}

// ===========================================================================
// BoundFn — a callable C function with cached type info
// ===========================================================================

pub(super) struct BoundFn {
    pub(super) func_ptr: *const c_void,
    pub(super) arg_types: Vec<CType>,
    pub(super) ret_type: CType,
    cached_cif: CachedCif,
}

unsafe impl Send for BoundFn {}
unsafe impl Sync for BoundFn {}

impl_object_instance_data!(BoundFn, "BoundFn");

impl BoundFn {
    pub(super) fn __call__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.is_empty() {
            return Err(RuntimeErrorKind::FfiError("bound function call: missing self".into()));
        }

        let self_handle = args[0];
        let user_args = &args[1..];

        // Snapshot everything from the BoundFn so we can release the
        // immutable borrow on vm.obj_heap before dispatch_call needs &mut vm.
        let (func_ptr, expected, arg_types, ret_type, cached_cif) = {
            let bound = vm
                .obj_heap
                .get_native::<BoundFn>(self_handle)
                .ok_or_else(|| RuntimeErrorKind::FfiError("bound function call: self is not a BoundFn".into()))?;
            (
                bound.func_ptr,
                bound.arg_types.len(),
                bound.arg_types.clone(),
                bound.ret_type.clone(),
                bound.cached_cif.0.clone(), // Cif::clone() is a shallow copy — cheap
            )
        };

        if user_args.len() != expected {
            return Err(RuntimeErrorKind::FfiError(format!("bound function expects {expected} argument(s), got {}", user_args.len())));
        }

        // Marshal arguments using the cached type info.
        let mut c_values: Vec<super::types::CValue> = Vec::with_capacity(expected);
        for (i, (ct, &handle)) in arg_types.iter().zip(user_args).enumerate() {
            let cv = ct.taro_to_cvalue(vm, handle).map_err(|e| RuntimeErrorKind::FfiError(format!("argument {i}: {e}")))?;
            c_values.push(cv);
        }

        dispatch_call(vm, &cached_cif, ret_type, func_ptr as *mut c_void, &c_values)
    }

    pub(super) fn __new__(_vm: &mut VirtualMachine, _args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        Err(RuntimeErrorKind::FfiError("BoundFn cannot be constructed directly; use ffi.bind()".into()))
    }
}

// ===========================================================================
// bind — resolve symbol, parse types, build cached CIF
// ===========================================================================

pub(super) fn bind(
    vm: &mut VirtualMachine, library_handle: ObjectHandle, name: ObjectHandle, ret_type_handle: ObjectHandle, arg_types_list: ObjectHandle,
) -> RuntimeResult<ObjectHandle> {
    // --- Resolve function pointer ---
    let name_str = vm.get_string_instance(name)?;
    let lib = vm
        .obj_heap
        .get_native::<super::library::LibraryHandle>(library_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("bind: not a library handle".into()))?;

    let func_ptr: *const c_void = unsafe {
        let symbol: libloading::Symbol<*const c_void> = lib
            .lib
            .get(name_str.as_str().as_bytes())
            .map_err(|e| RuntimeErrorKind::FfiError(format!("bind('{}'): {e}", name_str)))?;
        *symbol
    };

    // --- Parse return type ---
    let ret_type_str = vm.get_string_instance(ret_type_handle)?.as_str().to_string();
    let ret_type = CType::from_str(&ret_type_str)?;

    // --- Parse argument types ---
    let arg_type_handles = vm
        .get_list_instance(arg_types_list)
        .map_err(|_| RuntimeErrorKind::FfiError("bind: argument types must be a list".into()))?;
    let arg_types: Vec<CType> = arg_type_handles.iter().map(|&h| CType::from_handle(vm, h)).collect::<RuntimeResult<_>>()?;

    // --- Build CIF once ---
    let ffi_arg_types: Vec<libffi::middle::Type> = arg_types.iter().map(|ct| ct.to_ffi_type()).collect();
    let cif = libffi::middle::Cif::new(ffi_arg_types, ret_type.to_ffi_type());

    let bound = BoundFn { func_ptr, arg_types, ret_type, cached_cif: CachedCif(cif) };

    let class = vm
        .lookup_loaded_module_export("std/ffi", &ShrString::new_str("BoundFn"))
        .ok_or_else(|| RuntimeErrorKind::FfiError("BoundFn class not found in ffi module".into()))?;
    Ok(vm.obj_heap.alloc_instance(class, bound))
}

