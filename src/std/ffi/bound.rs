//! Bound C function — pre-parsed type info with a cached libffi CIF.
//!
//! The CIF is built once at `bind()` time and reused on every call, avoiding
//! per-call allocations.  Because `libffi::middle::Cif` is `!Send + !Sync`,
//! we wrap it in [`CachedCif`] with an unsafe Send+Sync impl — justified by
//! the VM being single-threaded and the CIF being read-only after construction.

use std::ffi::c_void;

use crate::vm::{RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, ShrString, impl_object_instance_data};

use super::call::dispatch_call;
use super::error::FfiError;
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
            return Err(FfiError::BoundFnMissingSelf.into());
        }

        let self_handle = args[0];
        let user_args = &args[1..];

        // Snapshot everything from the BoundFn so we can release the
        // immutable borrow on vm.obj_heap before dispatch_call needs &mut vm.
        let (func_ptr, expected, arg_types, ret_type, cached_cif) = {
            let bound = vm.obj_heap.get_native::<BoundFn>(self_handle).ok_or(FfiError::BoundFnNotBoundFn)?;
            (bound.func_ptr, bound.arg_types.len(), bound.arg_types.clone(), bound.ret_type.clone(), bound.cached_cif.0.clone())
        };

        if user_args.len() != expected {
            return Err(FfiError::BoundFnArgCount { expected, got: user_args.len() }.into());
        }

        // Marshal arguments using the cached type info.
        let mut c_values: Vec<super::types::CValue> = Vec::with_capacity(expected);
        for (i, (ct, &handle)) in arg_types.iter().zip(user_args).enumerate() {
            let cv = ct.taro_to_cvalue(vm, handle).map_err(|e| FfiError::MarshalArg { idx: i, reason: e.to_string() })?;
            c_values.push(cv);
        }

        dispatch_call(vm, &cached_cif, ret_type, func_ptr as *mut c_void, &c_values)
    }

    pub(super) fn __new__(_vm: &mut VirtualMachine, _args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        Err(FfiError::BoundFnDirectConstruction.into())
    }
}

// ===========================================================================
// bind — resolve symbol, parse types, build cached CIF
// ===========================================================================

pub(super) fn bind(
    vm: &mut VirtualMachine,
    library_handle: ObjectHandle,
    name: ObjectHandle,
    ret_type_handle: ObjectHandle,
    arg_types_list: ObjectHandle,
) -> RuntimeResult<ObjectHandle> {
    // --- Resolve function pointer ---
    let name_str = vm.expect_type(vm.obj_heap.get_string_instance(name), name, "string")?;
    let lib = vm.obj_heap.get_native::<super::library::LibraryHandle>(library_handle).ok_or(FfiError::BindNotLibrary)?;

    let func_ptr: *const c_void = unsafe {
        let symbol: libloading::Symbol<*const c_void> = lib
            .lib
            .get(name_str.as_str().as_bytes())
            .map_err(|e| FfiError::BindSymbol { name: name_str.as_str().to_string(), error: e.to_string() })?;
        *symbol
    };

    // --- Parse return type ---
    let ret_type_str = vm.expect_type(vm.obj_heap.get_string_instance(ret_type_handle), ret_type_handle, "string")?.as_str().to_string();
    let ret_type = CType::from_str(&ret_type_str)?;

    // --- Parse argument types ---
    let arg_type_handles = vm.obj_heap.get_list_instance(arg_types_list).ok_or(FfiError::BindArgTypesNotList)?;
    let arg_types: Vec<CType> = arg_type_handles.iter().map(|&h| CType::from_handle(vm, h)).collect::<RuntimeResult<_>>()?;

    // --- Build CIF once ---
    let ffi_arg_types: Vec<libffi::middle::Type> = arg_types.iter().map(|ct| ct.to_ffi_type()).collect();
    let cif = libffi::middle::Cif::new(ffi_arg_types, ret_type.to_ffi_type());

    let bound = BoundFn { func_ptr, arg_types, ret_type, cached_cif: CachedCif(cif) };

    let class = vm.lookup_loaded_module_export("std/ffi", &ShrString::new_str("BoundFn")).ok_or(FfiError::BoundFnClassNotFound)?;
    Ok(vm.obj_heap.alloc_instance(class, bound))
}
