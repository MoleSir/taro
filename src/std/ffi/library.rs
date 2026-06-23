//! Dynamic library loading: `dlopen`, `dlsym`, `dlclose`.

use std::ffi::c_void;

use crate::vm::{RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, impl_object_instance_data};

use super::error::FfiError;

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
    let path_str = vm.expect_type(vm.obj_heap.get_string_instance(path), path, "string")?;
    let lib = unsafe { libloading::Library::new(path_str.as_str()) }.map_err(|e| FfiError::DlOpen(e.to_string()))?;

    let lib_handle = LibraryHandle::new(lib);
    let obj = vm.obj_heap.alloc_instance(vm.obj_heap.module_class, lib_handle);
    Ok(obj)
}

pub(super) fn dlsym(vm: &mut VirtualMachine, library_handle: ObjectHandle, name: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let name_str = vm.expect_type(vm.obj_heap.get_string_instance(name), name, "string")?;
    let lib = vm.obj_heap.get_native::<LibraryHandle>(library_handle).ok_or(FfiError::DlSymNotLibrary)?;

    unsafe {
        let symbol: libloading::Symbol<*const c_void> = lib
            .lib
            .get(name_str.as_str().as_bytes())
            .map_err(|e| FfiError::DlSym { name: name_str.as_str().to_string(), error: e.to_string() })?;

        let ptr_addr = *symbol as usize as i64;
        Ok(vm.obj_heap.alloc_integer_instance(ptr_addr))
    }
}

pub(super) fn dlclose(vm: &mut VirtualMachine, library_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let lib = vm.obj_heap.get_native::<LibraryHandle>(library_handle).ok_or(FfiError::DlCloseNotLibrary)?;
    let _ = lib;
    Ok(ObjectHandle::NIL)
}
