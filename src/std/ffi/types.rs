//! C type descriptors, value storage, and marshalling.
//!
//! `CType` is the single source of truth for all C type information:
//! size, alignment, libffi type mapping, and value conversion.
//! Parsed once at bind/struct_def time, cheaply cloned thereafter.
//!
//! # Macro strategy
//!
//! The [`impl_scalar_methods!`] macro takes the full list of 13 scalar
//! variants and generates three methods (`from_str`, `size_align`,
//! `to_ffi_type`) in one shot.  The remaining methods (`taro_to_cvalue`,
//! `write_to_buffer`, `call_ffi`) have enough per-category variation that
//! keeping them explicit is clearer than forcing them into a table.

use std::ffi::{CString, c_char, c_void};

use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, ShrString, ToShrString};

// ===========================================================================
// Macro: generate from_str + size_align + to_ffi_type
// ===========================================================================

macro_rules! impl_scalar_methods {
    (
        $(
            $variant:ident, $name:literal, $size:literal, $align:literal, $ffi_ctor:ident
        );* $(;)?
    ) => {
        /// Parse a C type name string.
        pub(super) fn from_str(s: &str) -> RuntimeResult<CType> {
            match s {
                $($name => Ok(CType::$variant),)*
                "void" => Ok(CType::Void),
                _ => Err(RuntimeErrorKind::FfiError(format!(
                    "unknown C type '{s}'. Supported: void int8 int16 int32 int64 \
                     uint8 uint16 uint32 uint64 float double pointer cstring bool"
                ))),
            }
        }

        /// `(size, alignment)` in bytes.
        pub(super) fn size_align(&self) -> RuntimeResult<(usize, usize)> {
            match self {
                $(CType::$variant => Ok(($size, $align)),)*
                CType::Void => Err(RuntimeErrorKind::FfiError("void has no size".into())),
                CType::Struct(_) => Err(RuntimeErrorKind::FfiError(
                    "nested structs not supported".into(),
                )),
            }
        }

        /// Map to the corresponding `libffi::middle::Type`.
        pub(super) fn to_ffi_type(&self) -> libffi::middle::Type {
            match self {
                $(CType::$variant => libffi::middle::Type::$ffi_ctor(),)*
                CType::Void => libffi::middle::Type::void(),
                CType::Struct(fields) => {
                    let tys: Vec<libffi::middle::Type> = fields
                        .iter()
                        .map(|ct| ct.to_ffi_type())
                        .collect();
                    libffi::middle::Type::structure(tys)
                }
            }
        }
    };
}

// ===========================================================================
// CType — unified C type descriptor
// ===========================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Bool,
    Pointer,
    CString,
    Void,
    Struct(Vec<CType>),
}

impl CType {
    impl_scalar_methods! {
        I8,      "int8",     1, 1, i8;
        I16,     "int16",    2, 2, i16;
        I32,     "int32",    4, 4, i32;
        I64,     "int64",    8, 8, i64;
        U8,      "uint8",    1, 1, u8;
        U16,     "uint16",   2, 2, u16;
        U32,     "uint32",   4, 4, u32;
        U64,     "uint64",   8, 8, u64;
        F32,     "float",    4, 4, f32;
        F64,     "double",   8, 8, f64;
        Bool,    "bool",     1, 1, u8;
        Pointer, "pointer",  8, 8, pointer;
        CString, "cstring",  8, 8, pointer
    }

    // ------------------------------------------------------------------
    // from_handle — detect type string or StructDef object
    // ------------------------------------------------------------------

    pub(super) fn from_handle(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        if let Some(def) = vm.obj_heap.get_native::<super::structs::StructDef>(handle) {
            Ok(CType::Struct(def.field_types.clone()))
        } else if let Some(s) = vm.obj_heap.get_string_instance(handle) {
            CType::from_str(s.as_str())
        } else {
            Err(RuntimeErrorKind::FfiError(format!("expected type string or struct def, got {}", vm.value_type_name(handle))))
        }
    }

    // ------------------------------------------------------------------
    // taro_to_cvalue — convert a Taro value to a CValue for FFI args
    // ------------------------------------------------------------------

    pub(super) fn taro_to_cvalue(&self, vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<CValue> {
        match self {
            CType::I8 => int_to_cvalue(vm, handle, |v| CValue::I8(v as i8)),
            CType::I16 => int_to_cvalue(vm, handle, |v| CValue::I16(v as i16)),
            CType::I32 => int_to_cvalue(vm, handle, |v| CValue::I32(v as i32)),
            CType::I64 => int_to_cvalue(vm, handle, CValue::I64),
            CType::U8 => int_to_cvalue(vm, handle, |v| CValue::U8(v as u8)),
            CType::U16 => int_to_cvalue(vm, handle, |v| CValue::U16(v as u16)),
            CType::U32 => int_to_cvalue(vm, handle, |v| CValue::U32(v as u32)),
            CType::U64 => int_to_cvalue(vm, handle, |v| CValue::U64(v as u64)),
            CType::F32 => {
                let v = as_f64(vm, handle)?;
                Ok(CValue::F32(v as f32))
            }
            CType::F64 => {
                let v = as_f64(vm, handle)?;
                Ok(CValue::F64(v))
            }
            CType::Pointer => {
                let v = vm.expect_type(vm.obj_heap.get_integer_instance(handle), handle, "int").copied()?;
                Ok(CValue::Pointer(v as *const c_void))
            }
            CType::CString => {
                let s = vm.expect_type(vm.obj_heap.get_string_instance(handle), handle, "string")?;
                let cs = CString::new(s.as_str()).map_err(|e| RuntimeErrorKind::FfiError(format!("CString error: {e}")))?;
                let ptr: *const c_char = cs.as_ptr();
                Ok(CValue::CString { _cstring: cs, ptr })
            }
            CType::Bool => {
                let v = vm.expect_type(vm.obj_heap.get_bool_instance(handle), handle, "bool").copied()?;
                Ok(CValue::Bool(if v { 1u8 } else { 0u8 }))
            }
            CType::Void => Err(RuntimeErrorKind::FfiError("cannot marshal void as argument".into())),
            CType::Struct(_) => {
                // Rebuild raw byte buffer from the Struct instance's named fields.
                let struct_data = vm
                    .obj_heap
                    .get_native::<super::structs::Struct>(handle)
                    .ok_or_else(|| RuntimeErrorKind::FfiError("expected struct instance".into()))?;

                let def = vm
                    .obj_heap
                    .get_native::<super::structs::StructDef>(struct_data.struct_def)
                    .ok_or_else(|| RuntimeErrorKind::FfiError("struct def not found".into()))?;

                let mut data = vec![0u8; def.size];
                for (i, (name, ctype)) in def.field_names.iter().zip(&def.field_types).enumerate() {
                    let field_key = ShrString::new_string(name.as_str());
                    let value_handle = struct_data
                        .fields
                        .get(&field_key)
                        .copied()
                        .ok_or_else(|| RuntimeErrorKind::FfiError(format!("struct field '{name}' not found")))?;
                    ctype
                        .write_to_buffer(vm, value_handle, &mut data[def.offsets[i]..])
                        .map_err(|e| RuntimeErrorKind::FfiError(format!("struct field '{name}': {e}")))?;
                }
                Ok(CValue::Struct { data })
            }
        }
    }

    // ------------------------------------------------------------------
    // write_to_buffer — write a Taro value into a raw byte slice
    // ------------------------------------------------------------------

    pub(super) fn write_to_buffer(&self, vm: &VirtualMachine, handle: ObjectHandle, buf: &mut [u8]) -> RuntimeResult<()> {
        match self {
            CType::I8 | CType::U8 | CType::Bool => {
                let v = vm.expect_type(vm.obj_heap.get_integer_instance(handle), handle, "int").copied()? as i8;
                buf[0] = v.to_ne_bytes()[0];
                Ok(())
            }
            CType::I16 | CType::U16 => {
                let v = vm.expect_type(vm.obj_heap.get_integer_instance(handle), handle, "int").copied()? as i16;
                buf[..2].copy_from_slice(&v.to_ne_bytes());
                Ok(())
            }
            CType::I32 | CType::U32 | CType::F32 => {
                if matches!(self, CType::F32) {
                    let v = as_f64(vm, handle)? as f32;
                    buf[..4].copy_from_slice(&v.to_ne_bytes());
                } else {
                    let v = vm.expect_type(vm.obj_heap.get_integer_instance(handle), handle, "int").copied()? as i32;
                    buf[..4].copy_from_slice(&v.to_ne_bytes());
                }
                Ok(())
            }
            CType::I64 | CType::U64 | CType::F64 | CType::Pointer | CType::CString => {
                if matches!(self, CType::F64) {
                    let v = as_f64(vm, handle)?;
                    buf[..8].copy_from_slice(&v.to_ne_bytes());
                } else {
                    let v = vm.expect_type(vm.obj_heap.get_integer_instance(handle), handle, "int").copied()?;
                    buf[..8].copy_from_slice(&v.to_ne_bytes());
                }
                Ok(())
            }
            CType::Void => Err(RuntimeErrorKind::FfiError("void is not a valid struct field type".into())),
            CType::Struct(_) => Err(RuntimeErrorKind::FfiError("nested struct definitions are not supported".into())),
        }
    }

    // ------------------------------------------------------------------
    // call_ffi — invoke the CIF and convert the return value
    // ------------------------------------------------------------------

    pub(super) fn call_ffi(
        self,
        vm: &mut VirtualMachine,
        cif: &libffi::middle::Cif,
        code_ptr: libffi::middle::CodePtr,
        args: &[libffi::middle::Arg],
    ) -> RuntimeResult<ObjectHandle> {
        match self {
            CType::Void => {
                unsafe { cif.call::<()>(code_ptr, args) };
                Ok(ObjectHandle::NIL)
            }
            CType::I8 => {
                let v: i8 = unsafe { cif.call::<i8>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::I16 => {
                let v: i16 = unsafe { cif.call::<i16>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::I32 => {
                let v: i32 = unsafe { cif.call::<i32>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::I64 => {
                let v: i64 = unsafe { cif.call::<i64>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v))
            }
            CType::U8 => {
                let v: u8 = unsafe { cif.call::<u8>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::U16 => {
                let v: u16 = unsafe { cif.call::<u16>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::U32 => {
                let v: u32 = unsafe { cif.call::<u32>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::U64 => {
                let v: u64 = unsafe { cif.call::<u64>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v as i64))
            }
            CType::F32 => {
                let v: f32 = unsafe { cif.call::<f32>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_float_instance(v as f64))
            }
            CType::F64 => {
                let v: f64 = unsafe { cif.call::<f64>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_float_instance(v))
            }
            CType::Pointer => {
                let v: *const c_void = unsafe { cif.call::<*const c_void>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_integer_instance(v as usize as i64))
            }
            CType::Bool => {
                let v: u8 = unsafe { cif.call::<u8>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_bool_instance(v != 0))
            }
            CType::CString => {
                let v: *const c_char = unsafe { cif.call::<*const c_char>(code_ptr, args) };
                if v.is_null() {
                    Ok(ObjectHandle::NIL)
                } else {
                    let bytes = unsafe { std::ffi::CStr::from_ptr(v) };
                    let s = bytes.to_string_lossy().into_owned().to_shrstring();
                    Ok(vm.obj_heap.alloc_string_instance(s))
                }
            }
            CType::Struct(_) => Err(RuntimeErrorKind::FfiError("struct return type not supported".into())),
        }
    }
}

// ===========================================================================
// CValue — marshalled C value storage
// ===========================================================================

pub(super) enum CValue {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Bool(u8),
    Pointer(*const c_void),
    CString {
        /// Owned CString — kept alive until the FFI call completes.
        _cstring: CString,
        ptr: *const c_char,
    },
    /// Owned struct bytes — rebuilt from ObjectFields for the duration
    /// of the FFI call.
    Struct {
        data: Vec<u8>,
    },
}

impl CValue {
    /// Convert to a libffi `Arg`.  Generated via macro for all scalar variants;
    /// struct variants handled explicitly.
    pub(super) fn as_arg(&self) -> libffi::middle::Arg {
        match self {
            CValue::I8(v) => libffi::middle::arg(v),
            CValue::I16(v) => libffi::middle::arg(v),
            CValue::I32(v) => libffi::middle::arg(v),
            CValue::I64(v) => libffi::middle::arg(v),
            CValue::U8(v) => libffi::middle::arg(v),
            CValue::U16(v) => libffi::middle::arg(v),
            CValue::U32(v) => libffi::middle::arg(v),
            CValue::U64(v) => libffi::middle::arg(v),
            CValue::F32(v) => libffi::middle::arg(v),
            CValue::F64(v) => libffi::middle::arg(v),
            CValue::Bool(v) => libffi::middle::arg(v),
            CValue::Pointer(v) => libffi::middle::arg(v),
            CValue::CString { ptr, .. } => libffi::middle::arg(ptr),
            CValue::Struct { data } => {
                let byte_ref: &u8 = if data.is_empty() {
                    static ZERO: u8 = 0;
                    &ZERO
                } else {
                    unsafe { &*data.as_ptr() }
                };
                libffi::middle::arg(byte_ref)
            }
        }
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Extract an integer from the VM and map it through `f`.
fn int_to_cvalue<F>(vm: &VirtualMachine, handle: ObjectHandle, f: F) -> RuntimeResult<CValue>
where
    F: FnOnce(i64) -> CValue,
{
    let v = vm.expect_type(vm.obj_heap.get_integer_instance(handle), handle, "int").copied()?;
    Ok(f(v))
}

/// Convert a Taro value to `f64`, accepting both integer and float instances.
pub(super) fn as_f64(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<f64> {
    if let Some(v) = vm.obj_heap.get_integer_instance(handle) {
        Ok(*v as f64)
    } else if let Some(v) = vm.obj_heap.get_float_instance(handle) {
        Ok(*v)
    } else {
        Err(RuntimeErrorKind::FfiError(format!("expected number, got {}", vm.value_type_name(handle))))
    }
}
