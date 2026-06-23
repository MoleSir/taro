use std::ffi::c_void;
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{impl_object_instance_data, ObjectHandle, ToShrString};
use super::error::FfiError;
use super::function::CFunction;

#[derive(Clone)]
pub(super) struct CSymbol {
    pub(super) raw: *mut c_void
}

unsafe impl Send for CSymbol {}
unsafe impl Sync for CSymbol {}

impl CSymbol {
    pub fn new(raw: *mut c_void) -> Self {
        Self { raw }
    }
}

impl_object_instance_data!(CSymbol, "CSymbol");

impl CSymbol {
    pub(super) fn __new__(_vm: &mut VirtualMachine, _args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        Err(FfiError::CSymbolDirectConstruction)?
    }
}

pub(super) struct CDynLib {
    pub(super) lib: libloading::Library,
}
impl_object_instance_data!(CDynLib, "CDynLib");

impl CDynLib {
    pub(super) fn symbol_impl(&self, name: &str) -> RuntimeResult<CSymbol> {
        unsafe {
            let symbol: libloading::Symbol<*const c_void> = self.lib
                .get(name.as_bytes())
                .map_err(|e| FfiError::DlSym { name: name.to_string(), error: e.to_string() })?;
    
            let ptr_addr = *symbol as *mut c_void;
            Ok(CSymbol::new(ptr_addr))
        }
    }
}

impl CDynLib {
    pub(super) fn __new__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.len() != 2 {
            Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 2, got: args.len() })?;
        }
        let class = args[0];
        let path = args[1];
        let path_str = vm.expect_type(vm.obj_heap.get_string_instance(path), path, "string")?;
        let lib = unsafe { libloading::Library::new(path_str.as_str()) }.map_err(|e| FfiError::DlOpen(e.to_string()))?;
        let lib_handle = Self { lib };
        let obj = vm.obj_heap.alloc_instance(class, lib_handle);
        Ok(obj)
    }

    pub(super) fn symbol(vm: &mut VirtualMachine, library: ObjectHandle, name: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let name_str = vm.expect_type(vm.obj_heap.get_string_instance(name), name, "string")?;
        let lib = vm.obj_heap.get_native::<Self>(library).ok_or(FfiError::DlSymNotLibrary)?;
        let symbol = lib.symbol_impl(name_str.as_str())?;
        let symbol_class = vm.lookup_module_export(library, &"CSymbol".to_shrstring()).expect("must symbol");
        Ok(vm.obj_heap.alloc_instance(symbol_class, symbol))
    }

    pub(super) fn bind(
        vm: &mut VirtualMachine, library: ObjectHandle, name: ObjectHandle, ret_type: ObjectHandle, param_types: ObjectHandle,
    ) -> RuntimeResult<ObjectHandle> {
        let name_str = vm.expect_type(vm.obj_heap.get_string_instance(name), name, "string")?;
        let lib = vm.obj_heap.get_native::<Self>(library).ok_or(FfiError::BindNotLibrary)?;
        let symbol = lib.symbol_impl(name_str.as_str())?;

        let function = CFunction::from_handle(vm, symbol, ret_type, param_types)?;
        let function_class = vm.lookup_module_export(library, &"CFunction".to_shrstring()).expect("must function");

        Ok(vm.obj_heap.alloc_instance(function_class, function))
    }
}
