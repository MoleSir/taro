//! Dynamic library loading: `dlopen`, `dlsym`, `dlclose`.

use std::ffi::c_void;

use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, impl_object_instance_data};

// ===========================================================================
// LibraryHandle — stores a loaded dynamic library on the GC heap
// ===========================================================================

pub(super) struct LibraryHandle {
    pub(super) lib: libloading::Library,
}

impl LibraryHandle {
    pub(super) fn new(lib: libloading::Library) -> Self {
        Self { lib }
    }
}

impl_object_instance_data!(LibraryHandle, "LibraryHandle");

// ===========================================================================
// Native-function implementations
// ===========================================================================

pub(super) fn dlopen(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let path_str = vm.get_string_instance(path)?;
    let lib = unsafe { libloading::Library::new(path_str.as_str()) }.map_err(|e| RuntimeErrorKind::FfiError(format!("dlopen: {e}")))?;

    let lib_handle = LibraryHandle::new(lib);
    let obj = vm.obj_heap.alloc_instance(vm.obj_heap.module_class, lib_handle);
    Ok(obj)
}

pub(super) fn dlsym(vm: &mut VirtualMachine, library_handle: ObjectHandle, name: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let name_str = vm.get_string_instance(name)?;
    let lib = vm
        .obj_heap
        .get_native::<LibraryHandle>(library_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("dlsym: not a library handle".into()))?;

    unsafe {
        let symbol: libloading::Symbol<*const c_void> = lib
            .lib
            .get(name_str.as_str().as_bytes())
            .map_err(|e| RuntimeErrorKind::FfiError(format!("dlsym('{}'): {e}", name_str)))?;

        let ptr_addr = *symbol as usize as i64;
        Ok(vm.obj_heap.alloc_integer_instance(ptr_addr))
    }
}

pub(super) fn dlclose(vm: &mut VirtualMachine, library_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let lib = vm
        .obj_heap
        .get_native::<LibraryHandle>(library_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("dlclose: not a library handle".into()))?;
    let _ = lib;
    Ok(ObjectHandle::NIL)
}
