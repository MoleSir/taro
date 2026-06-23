use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use crate::vm::{RuntimeResult, VirtualMachine};
use crate::{impl_object_instance_data, ObjectHandle, ShrString, ToShrString};
use super::error::FfiError;
use super::library::CSymbol;
use super::types::{CStruct, CType, CValue};

pub(super) struct CFunction {
    pub(super) ptr: libffi::middle::CodePtr,
    pub(super) ret_type: CType,
    pub(super) param_types: Vec<CType>,
    cif: libffi::middle::Cif,
    /// When `ret_type` is `Struct(layout)`, this holds the CType instance
    /// handle that describes the struct.  Used when constructing the result
    /// [`CStruct`](super::types::CStruct).
    struct_type_handle: Option<ObjectHandle>,
}

unsafe impl Send for CFunction {}
unsafe impl Sync for CFunction {}

impl_object_instance_data!(CFunction, "CFunction");

impl CFunction {
    pub(super) fn new(symbol: CSymbol, ret_type: CType, param_types: Vec<CType>, struct_type_handle: Option<ObjectHandle>) -> Self {
        let ffi_arg_types: Vec<libffi::middle::Type> = param_types.iter().map(|ct| ct.to_ffi_type()).collect();
        let cif = libffi::middle::Cif::new(ffi_arg_types, ret_type.to_ffi_type());
        Self { ptr: libffi::middle::CodePtr(symbol.raw), ret_type, param_types, cif, struct_type_handle }
    }

    pub(crate) fn from_handle(vm: &mut VirtualMachine, symbol: CSymbol, ret_type_handle: ObjectHandle, arg_types: ObjectHandle) -> RuntimeResult<Self> {
        // Parse return type — supports both string names ("int32", …)
        // and CType struct instances (for struct return types).
        let ret_type = CType::from_handle(vm, ret_type_handle)?;
        let struct_type_handle = if matches!(ret_type, CType::Struct(_)) {
            Some(ret_type_handle)
        } else {
            None
        };

        // Parse argument types
        let arg_type_handles = vm.obj_heap.get_list_instance(arg_types).ok_or(FfiError::BindArgTypesNotList)?;
        let arg_types: Vec<CType> = arg_type_handles.iter().map(|&h| CType::from_handle(vm, h)).collect::<RuntimeResult<_>>()?;

        Ok(Self::new(symbol, ret_type, arg_types, struct_type_handle))
    }

    fn call_cif<R>(&self, args: &[libffi::middle::Arg]) -> R {
        unsafe { self.cif.call(self.ptr, args) }
    }
}

impl CFunction {
    pub(super) fn __call__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.is_empty() {
            return Err(FfiError::BoundFnMissingSelf.into());
        }
        Self::__call__impl(vm, args[0], &args[1..])
    }

    #[allow(non_snake_case)]
    fn __call__impl(vm: &mut VirtualMachine, self_handle: ObjectHandle, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let function = vm.obj_heap.get_native::<Self>(self_handle).ok_or(FfiError::BoundFnNotBoundFn)?;
        if args.len() != function.param_types.len() {
            return Err(FfiError::BoundFnArgCount { expected: function.param_types.len(), got: args.len() }.into());
        }

        // Marshal Taro values to CValues, keeping them alive for the FFI call.
        // CString/Struct Arg values borrow from the CValue, so they must not
        // be dropped before the call completes.
        let mut c_values: Vec<CValue> = Vec::with_capacity(function.param_types.len());
        for (i, (ct, &handle)) in function.param_types.iter().zip(args).enumerate() {
            let cv = ct
                .taro_to_cvalue(vm, handle)
                .map_err(|e| FfiError::MarshalArg { idx: i, reason: e.to_string() })?;
            c_values.push(cv);
        }
        let ffi_args: Vec<libffi::middle::Arg> = c_values.iter().map(|cv| cv.as_arg()).collect();

        match &function.ret_type {
            CType::Void => {
                function.call_cif::<()>(&ffi_args);
                Ok(ObjectHandle::NIL)
            }
            CType::I8 => {
                let v: i8 = function.call_cif::<i8>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::I16 => {
                let v: i16 = function.call_cif::<i16>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::I32 => {
                let v: i32 = function.call_cif::<i32>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::I64 => {
                let v: i64 = function.call_cif::<i64>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v))
            }
            CType::U8 => {
                let v: u8 = function.call_cif::<u8>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::U16 => {
                let v: u16 = function.call_cif::<u16>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::U32 => {
                let v: u32 = function.call_cif::<u32>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::U64 => {
                let v: u64 = function.call_cif::<u64>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::F32 => {
                let v: f32 = function.call_cif::<f32>(&ffi_args);
                Ok(vm.obj_heap.alloc_float_instance(v as f64))
            }
            CType::F64 => {
                let v: f64 = function.call_cif::<f64>(&ffi_args);
                Ok(vm.obj_heap.alloc_float_instance(v))
            }
            CType::Pointer => {
                let v: *const c_void = function.call_cif::<*const c_void>(&ffi_args);
                Ok(vm.obj_heap.alloc_integer_instance(v as usize as i64))
            }
            CType::Bool => {
                let v: u8 = function.call_cif::<u8>(&ffi_args);
                Ok(vm.obj_heap.alloc_bool_instance(v != 0))
            }
            CType::CString => {
                let v: *const c_char = function.call_cif::<*const c_char>(&ffi_args);
                if v.is_null() {
                    Ok(ObjectHandle::NIL)
                } else {
                    let bytes = unsafe { std::ffi::CStr::from_ptr(v) };
                    let s = bytes.to_string_lossy().into_owned().to_shrstring();
                    Ok(vm.obj_heap.alloc_string_instance(s))
                }
            }
            CType::Struct(layout) => {
                // Struct return — we must use raw::ffi_call because the
                // struct size isn't known at compile time, so we can't
                // use the generic middle::Cif::call::<R>().
                let cif_raw = function.cif.as_raw_ptr();
                let code_ptr = function.ptr;
                let struct_type_handle = function
                    .struct_type_handle
                    .ok_or(FfiError::StructReturnUnsupported)?;
                // Clone layout to detach from `function`'s borrow of
                // obj_heap before mutating vm below.
                let layout = layout.clone();

                let mut buffer = vec![0u8; layout.size];
                unsafe {
                    libffi::raw::ffi_call(
                        cif_raw,
                        Some(*code_ptr.as_safe_fun()),
                        buffer.as_mut_ptr().cast::<c_void>(),
                        ffi_args.as_ptr() as *mut *mut c_void,
                    );
                }
                // `function` borrow ends here — NLL allows mutable vm
                // accesses below.

                // Decode each field from the raw buffer.
                let mut fields = HashMap::with_capacity(layout.field_names.len());
                for (i, name) in layout.field_names.iter().enumerate() {
                    let field_val = layout.field_types[i]
                        .read_from_buffer(vm, &buffer[layout.offsets[i]..])?;
                    fields.insert(ShrString::new_string(name), field_val);
                }

                let struct_class = vm
                    .lookup_module_export(struct_type_handle, &ShrString::new_str("CStruct"))
                    .ok_or(FfiError::StructClassNotFound)?;
                let instance = CStruct { ctype: struct_type_handle, fields };
                Ok(vm.obj_heap.alloc_instance(struct_class, instance))
            }
        }
    }

    pub(super) fn __new__(_vm: &mut VirtualMachine, _args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        Err(FfiError::BoundFnDirectConstruction.into())
    }
}

pub(super) fn call(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.len() < 3 {
        return Err(FfiError::CallTooFewArgs)?;
    }
    let symbol = vm.expect_type(vm.obj_heap.get_instance_data::<CSymbol>(args[0]), args[0], "CSymbol")?.clone();
    let function = CFunction::from_handle(vm, symbol, args[1], args[2])?;
    let function_class = vm.lookup_module_export(args[0], &"CFunction".to_shrstring()).expect("must has cfunction");
    let function = vm.obj_heap.alloc_instance(function_class, function);
    let arg_values: Vec<ObjectHandle> = if args.len() > 3 { 
        vm.expect_type(vm.obj_heap.get_list_instance(args[3]), args[3], "list")?.clone() 
    } else { 
        vec![] 
    };
    CFunction::__call__impl(vm, function, &arg_values)
}
