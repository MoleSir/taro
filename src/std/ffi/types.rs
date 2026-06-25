//! C type descriptors, value storage, and marshalling.
//!
//! `CType` is the single source of truth for all C type information:
//! size, alignment, libffi type mapping, and value conversion.
//! Parsed once at bind/define_struct time, cheaply cloned thereafter.
//!
//! # Macro strategy
//!
//! The [`impl_scalar_methods!`] macro takes the full list of 13 scalar
//! variants and generates three methods (`from_str`, `size_align`,
//! `to_ffi_type`) in one shot.  The remaining methods (`taro_to_cvalue`,
//! `write_to_buffer`, `call_ffi`) have enough per-category variation that
//! keeping them explicit is clearer than forcing them into a table.

use std::alloc::Layout;
use std::collections::HashMap;
use std::ffi::{CString, c_char, c_void};

use crate::object::{ObjectHeap, ObjectInstanceData};
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, ShrString};
use std::any::Any;

use super::error::FfiError;

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
                _ => Err(FfiError::UnknownCType(s.into()).into()),
            }
        }

        /// `(size, alignment)` in bytes.
        pub(super) fn size_align(&self) -> RuntimeResult<(usize, usize)> {
            match self {
                $(CType::$variant => Ok(($size, $align)),)*
                CType::Void => Err(FfiError::VoidNoSize.into()),
                CType::Struct(layout) => Ok((layout.size, layout.alignment)),
            }
        }

        /// Map to the corresponding `libffi::middle::Type`.
        pub(super) fn to_ffi_type(&self) -> libffi::middle::Type {
            match self {
                $(CType::$variant => libffi::middle::Type::$ffi_ctor(),)*
                CType::Void => libffi::middle::Type::void(),
                CType::Struct(layout) => {
                    let tys: Vec<libffi::middle::Type> = layout
                        .field_types
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
// StructLayout — C struct layout metadata
// ===========================================================================

/// Pre-computed layout information for a C struct type.
///
/// Created by [`struct_layout_from_descriptors`] when the user calls
/// `ffi.define_struct(...)`, then embedded in a `CType::Struct` variant
/// stored on a `CType` instance.
#[derive(Debug, Clone)]
pub(super) struct StructLayout {
    /// Field types in layout order.
    pub(super) field_types: Vec<CType>,
    /// Field names in layout order.
    pub(super) field_names: Vec<String>,
    /// Byte offset of each field from the start of the struct.
    pub(super) offsets: Vec<usize>,
    /// Total size of the struct (including tail padding).
    pub(super) size: usize,
    /// Alignment of the struct.
    pub(super) alignment: usize,
}

/// Compute the `StructLayout` from field `(name, CType)` descriptors.
pub(super) fn struct_layout_from_descriptors(descriptors: &[(String, CType)]) -> RuntimeResult<StructLayout> {
    let mut field_types = Vec::with_capacity(descriptors.len());
    let mut field_names = Vec::with_capacity(descriptors.len());
    let mut offsets = Vec::with_capacity(descriptors.len());

    let mut layout = Layout::from_size_align(0, 1).map_err(|_| FfiError::Layout("invalid initial alignment".into()))?;

    for (name, ct) in descriptors {
        let (size, align) = ct.size_align()?;
        let field_layout = Layout::from_size_align(size, align)
            .map_err(|_| FfiError::Layout(format!("invalid layout for field '{name}' (size={size}, align={align})")))?;
        let (new_layout, offset) =
            layout.extend(field_layout).map_err(|_| FfiError::Layout("struct exceeds maximum supported size".into()))?;
        layout = new_layout;
        offsets.push(offset);
        field_types.push(ct.clone());
        field_names.push(name.clone());
    }

    let total_layout = layout.pad_to_align();
    Ok(StructLayout { field_types, field_names, offsets, size: total_layout.size(), alignment: total_layout.align() })
}

// ===========================================================================
// CType — unified C type descriptor
// ===========================================================================

#[derive(Debug, Clone)]
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
    Struct(StructLayout),
}

// ===========================================================================
// ObjectInstanceData impl — CType is stored directly on the heap
// ===========================================================================

impl ObjectInstanceData for CType {
    fn mark_references(&self, _heap: &mut ObjectHeap) {
        // CType contains no ObjectHandles — scalar variants are zero-size,
        // StructLayout owns only String/Vec/CType, all non-GC types.
    }
    fn type_name(&self) -> &'static str {
        "CType"
    }
    fn as_any_ref(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
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
    // from_handle — resolve a type handle to a CType
    // ------------------------------------------------------------------

    pub(super) fn from_handle(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        if let Some(ct) = vm.obj_heap.get_instance_data::<CType>(handle) {
            return Ok(ct.clone());
        }
        if let Some(s) = vm.obj_heap.get_string_instance(handle) {
            return CType::from_str(s.as_str());
        }
        Err(FfiError::ExpectedType(vm.obj_heap.type_of(handle).into()).into())
    }

    pub(super) fn taro_to_cvalue(&self, vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<CValue> {
        if let Some(cv) = try_unwrap_scalar_to_cvalue(vm, handle) {
            return Ok(cv);
        }
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
                let v = vm.obj_heap.expect_integer(handle).copied()?;
                Ok(CValue::Pointer(v as *const c_void))
            }
            CType::CString => {
                let s = vm.obj_heap.expect_string(handle)?;
                let cs = CString::new(s.as_str()).map_err(|e| FfiError::CString(e.to_string()))?;
                let ptr: *const c_char = cs.as_ptr();
                Ok(CValue::CString { _cstring: cs, ptr })
            }
            CType::Bool => {
                let v = vm.obj_heap.expect_bool(handle).copied()?;
                Ok(CValue::Bool(if v { 1u8 } else { 0u8 }))
            }
            CType::Void => Err(FfiError::VoidAsArgument.into()),
            CType::Struct(layout) => {
                // Rebuild raw byte buffer from the Struct instance's named fields.
                let struct_data = vm.obj_heap.get_instance_data::<CStruct>(handle).ok_or(FfiError::ExpectedStruct)?;

                let mut data = vec![0u8; layout.size];
                for (i, (name, fct)) in layout.field_names.iter().zip(&layout.field_types).enumerate() {
                    let value_handle =
                        struct_data.fields.get(name.as_str()).copied().ok_or_else(|| FfiError::StructFieldNotFound(name.clone()))?;
                    fct.write_to_buffer(vm, value_handle, &mut data[layout.offsets[i]..])
                        .map_err(|e| FfiError::StructFieldError { name: name.clone(), error: e.to_string() })?;
                }
                Ok(CValue::Struct { data })
            }
        }
    }

    pub(super) fn write_to_buffer(&self, vm: &VirtualMachine, handle: ObjectHandle, buf: &mut [u8]) -> RuntimeResult<()> {
        // If the value is a typed scalar wrapper, write it directly.
        if let Some(result) = try_unwrap_scalar_to_buffer(vm, handle, buf) {
            return result;
        }
        match self {
            CType::I8 | CType::U8 | CType::Bool => {
                let v = vm.obj_heap.expect_integer(handle).copied()? as i8;
                buf[0] = v.to_ne_bytes()[0];
                Ok(())
            }
            CType::I16 | CType::U16 => {
                let v = vm.obj_heap.expect_integer(handle).copied()? as i16;
                buf[..2].copy_from_slice(&v.to_ne_bytes());
                Ok(())
            }
            CType::I32 | CType::U32 | CType::F32 => {
                if matches!(self, CType::F32) {
                    let v = as_f64(vm, handle)? as f32;
                    buf[..4].copy_from_slice(&v.to_ne_bytes());
                } else {
                    let v = vm.obj_heap.expect_integer(handle).copied()? as i32;
                    buf[..4].copy_from_slice(&v.to_ne_bytes());
                }
                Ok(())
            }
            CType::I64 | CType::U64 | CType::F64 | CType::Pointer | CType::CString => {
                if matches!(self, CType::F64) {
                    let v = as_f64(vm, handle)?;
                    buf[..8].copy_from_slice(&v.to_ne_bytes());
                } else {
                    let v = vm.obj_heap.expect_integer(handle).copied()?;
                    buf[..8].copy_from_slice(&v.to_ne_bytes());
                }
                Ok(())
            }
            CType::Void => Err(FfiError::VoidAsField.into()),
            CType::Struct(_) => {
                // Recursively serialize a nested struct instance into the buffer.
                let struct_data = vm.obj_heap.get_instance_data::<CStruct>(handle).ok_or(FfiError::ExpectedNestedStruct)?;

                // Get the nested struct's CType from its back-link to extract the layout.
                let ctype = vm
                    .obj_heap
                    .get_instance_data::<CType>(struct_data.ctype)
                    .ok_or(FfiError::Layout("struct type not found for nested field".into()))?;

                let layout = match ctype {
                    CType::Struct(layout) => layout,
                    _ => return Err(FfiError::NestedNotStruct.into()),
                };

                for (i, (name, fct)) in layout.field_names.iter().zip(&layout.field_types).enumerate() {
                    let value_handle =
                        struct_data.fields.get(name.as_str()).copied().ok_or_else(|| FfiError::StructFieldNotFound(name.clone()))?;
                    fct.write_to_buffer(vm, value_handle, &mut buf[layout.offsets[i]..])?;
                }
                Ok(())
            }
        }
    }

    /// `self_handle` is the heap handle of the CType instance that owns this
    /// type descriptor — used to walk back to the owning module for class lookups.
    pub(super) fn read_from_buffer(&self, vm: &mut VirtualMachine, buf: &[u8], self_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        // Helper: allocate a scalar wrapper instance by looking up its class.
        macro_rules! alloc_scalar_from_buf {
            ($wrapper:ident, $value:expr) => {{
                let class = vm
                    .lookup_module_export(self_handle, stringify!($wrapper))
                    .ok_or_else(|| FfiError::ScalarClassNotFound(stringify!($wrapper).into()))?;
                vm.obj_heap.alloc_instance(class, $wrapper { value: $value })
            }};
        }

        match self {
            CType::I8 => {
                let v = buf[0] as i8;
                Ok(alloc_scalar_from_buf!(CI8, v))
            }
            CType::I16 => {
                let v = i16::from_ne_bytes(buf[..2].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CI16, v))
            }
            CType::I32 => {
                let v = i32::from_ne_bytes(buf[..4].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CI32, v))
            }
            CType::I64 => {
                let v = i64::from_ne_bytes(buf[..8].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CI64, v))
            }
            CType::U8 => Ok(alloc_scalar_from_buf!(CUint8, buf[0])),
            CType::U16 => {
                let v = u16::from_ne_bytes(buf[..2].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CUint16, v))
            }
            CType::U32 => {
                let v = u32::from_ne_bytes(buf[..4].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CUint32, v))
            }
            CType::U64 => {
                let v = u64::from_ne_bytes(buf[..8].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CUint64, v))
            }
            CType::F32 => {
                let v = f32::from_ne_bytes(buf[..4].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CFloat, v))
            }
            CType::F64 => {
                let v = f64::from_ne_bytes(buf[..8].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CDouble, v))
            }
            CType::Bool => Ok(alloc_scalar_from_buf!(CBool, if buf[0] != 0 { 1u8 } else { 0u8 })),
            CType::Pointer => {
                let v = usize::from_ne_bytes(buf[..8].try_into().unwrap());
                Ok(alloc_scalar_from_buf!(CPointer, v))
            }
            CType::CString => {
                let v = usize::from_ne_bytes(buf[..8].try_into().unwrap());
                let ptr = v as *const c_char;
                if ptr.is_null() {
                    Ok(ObjectHandle::NIL)
                } else {
                    let bytes = unsafe { std::ffi::CStr::from_ptr(ptr) };
                    let s = bytes.to_string_lossy().into_owned();
                    Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&s)))
                }
            }
            CType::Void => Err(FfiError::VoidAsField.into()),
            CType::Struct(inner_layout) => {
                // Allocate a temporary CType instance on the heap as the
                // ctype back-link for the nested result struct.
                let ctype_class = vm.lookup_module_export(self_handle, "CType").ok_or(FfiError::CTypeClassNotFound)?;
                let inner_ctype_handle = vm.obj_heap.alloc_instance(ctype_class, self.clone());

                let mut fields = HashMap::with_capacity(inner_layout.field_names.len());
                for (i, name) in inner_layout.field_names.iter().enumerate() {
                    let field_val =
                        inner_layout.field_types[i].read_from_buffer(vm, &buf[inner_layout.offsets[i]..], inner_ctype_handle)?;
                    fields.insert(ShrString::new_string(name), field_val);
                }

                let struct_class = vm.lookup_module_export(inner_ctype_handle, "CStruct").ok_or(FfiError::StructClassNotFound)?;
                let instance = CStruct { ctype: inner_ctype_handle, fields };
                Ok(vm.obj_heap.alloc_instance(struct_class, instance))
            }
        }
    }

    // ------------------------------------------------------------------
    // __call__ — create a Struct instance (struct) or convert a value
    // ------------------------------------------------------------------

    pub(super) fn __call__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.is_empty() {
            return Err(FfiError::CTypeCallMissingSelf.into());
        }

        let self_handle = args[0];
        let field_values = &args[1..];

        let ctype = vm.obj_heap.get_instance_data::<CType>(self_handle).ok_or(FfiError::CTypeCallNotCType)?;

        // Helper: create a scalar wrapper instance.
        // Looks up the class from the ffi module and allocates on the heap.
        macro_rules! alloc_scalar {
            ($wrapper:ident, $value:expr) => {{
                let class = vm
                    .lookup_module_export(self_handle, stringify!($wrapper))
                    .ok_or_else(|| FfiError::ScalarClassNotFound(stringify!($wrapper).into()))?;
                vm.obj_heap.alloc_instance(class, $wrapper { value: $value })
            }};
        }

        match ctype {
            CType::Struct(layout) => {
                // Struct construction — same logic as the old StructDef.__call__.
                if field_values.len() != layout.field_types.len() {
                    return Err(FfiError::StructArgCount { expected: layout.field_types.len(), got: field_values.len() }.into());
                }

                let mut fields = HashMap::with_capacity(layout.field_names.len());
                for (i, name) in layout.field_names.iter().enumerate() {
                    fields.insert(ShrString::new_string(name.as_str()), field_values[i]);
                }

                let instance_data = CStruct { ctype: self_handle, fields };

                let class = vm.lookup_module_export(self_handle, "CStruct").ok_or(FfiError::StructClassNotFound)?;
                Ok(vm.obj_heap.alloc_instance(class, instance_data))
            }
            CType::I8 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CI8, v as i8))
            }
            CType::U8 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CUint8, v as u8))
            }
            CType::I16 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CI16, v as i16))
            }
            CType::U16 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CUint16, v as u16))
            }
            CType::I32 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CI32, v as i32))
            }
            CType::U32 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CUint32, v as u32))
            }
            CType::I64 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CI64, v))
            }
            CType::U64 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CUint64, v as u64))
            }
            CType::F32 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = as_f64(vm, field_values[0])?;
                Ok(alloc_scalar!(CFloat, v as f32))
            }
            CType::F64 => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = as_f64(vm, field_values[0])?;
                Ok(alloc_scalar!(CDouble, v))
            }
            CType::Bool => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_bool(field_values[0]).copied()?;
                Ok(alloc_scalar!(CBool, if v { 1u8 } else { 0u8 }))
            }
            CType::Pointer => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                let v = vm.obj_heap.expect_integer(field_values[0]).copied()?;
                Ok(alloc_scalar!(CPointer, v as usize))
            }
            CType::CString => {
                if field_values.len() != 1 {
                    return Err(FfiError::ScalarArgCount(field_values.len()).into());
                }
                // CString is special: no Copy wrapper; validate and return raw string.
                ctype.taro_to_cvalue(vm, field_values[0])?;
                Ok(field_values[0])
            }
            CType::Void => Err(FfiError::VoidAsArgument.into()),
        }
    }

    pub(super) fn __new__(_vm: &mut VirtualMachine, _args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        Err(FfiError::CTypeDirectConstruction)?
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
    /// Convert to a libffi `Arg`.
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
// Macro: define all scalar wrapper types
// ===========================================================================
//
// Each scalar wrapper stores a native C value directly — no GC overhead,
// no ObjectHandle indirection.  The wrapper's Rust type IS its C type
// (e.g. `CUint8` always means `CValue::U8`).
//
// Generates:
//   1. Struct definition + ObjectInstanceData + to_cvalue()
//   2. Shared scalar_getattr / scalar_setattr (downcast-chain dispatch)
//   3. try_unwrap_scalar_to_cvalue / try_unwrap_scalar_to_buffer

macro_rules! define_scalar_wrappers {
    (
        $(
            $name:ident: $ty:ty => $cvalue:ident, $kind:ident
        );* $(;)?
    ) => {
        // ---- Per-type struct definitions ----
        $(
            #[derive(Debug, Clone, Copy)]
            pub(super) struct $name {
                pub(super) value: $ty,
            }

            impl ObjectInstanceData for $name {
                fn mark_references(&self, _heap: &mut ObjectHeap) {}
                fn type_name(&self) -> &'static str { stringify!($name) }
                fn as_any_ref(&self) -> &dyn Any { self }
                fn as_any_mut(&mut self) -> &mut dyn Any { self }
            }

            impl $name {
                pub(super) fn to_cvalue(self) -> CValue {
                    define_scalar_wrappers!(@to_cvalue self $cvalue $kind)
                }
            }
        )*

        // ---- Shared __getattr__: dispatch via downcast chain ----
        pub(super) fn scalar_getattr(
            vm: &mut VirtualMachine,
            args: &[ObjectHandle],
        ) -> RuntimeResult<ObjectHandle> {
            if args.len() < 2 {
                return Err(FfiError::GetAttrArgCount.into());
            }
            let name = vm
                .obj_heap.expect_string(args[1])?
                .as_str()
                .to_string();
            if name != "value" {
                return Err(FfiError::ScalarNoAttr(name).into());
            }
            let handle = args[0];
            $(
                if let Some(data) = vm.obj_heap.get_instance_data::<$name>(handle) {
                    return Ok(define_scalar_wrappers!(@to_taro vm data $kind));
                }
            )*
            Err(FfiError::GetAttrNotScalar.into())
        }

        // ---- Shared __setattr__: dispatch via downcast chain ----
        pub(super) fn scalar_setattr(
            vm: &mut VirtualMachine,
            args: &[ObjectHandle],
        ) -> RuntimeResult<ObjectHandle> {
            if args.len() < 3 {
                return Err(FfiError::SetAttrArgCount.into());
            }
            let name = vm
                .obj_heap.expect_string(args[1])?
                .as_str()
                .to_string();
            if name != "value" {
                return Err(FfiError::ScalarNoAttr(name).into());
            }
            let handle = args[0];
            let value_handle = args[2];
            // Two-phase: check type (immutable borrow), then convert + update (mutable borrow).
            $(
                if vm.obj_heap.get_instance_data::<$name>(handle).is_some() {
                    let new_val = define_scalar_wrappers!(@from_taro vm value_handle $kind $ty)?;
                    vm.obj_heap
                        .get_instance_data_mut::<$name>(handle)
                        .unwrap()
                        .value = new_val;
                    return Ok(ObjectHandle::NIL);
                }
            )*
            Err(FfiError::SetAttrNotScalar.into())
        }

        // ---- Try to extract CValue from any scalar wrapper ----
        pub(super) fn try_unwrap_scalar_to_cvalue(
            vm: &VirtualMachine,
            handle: ObjectHandle,
        ) -> Option<CValue> {
            $(
                if let Some(data) = vm.obj_heap.get_instance_data::<$name>(handle) {
                    return Some(data.to_cvalue());
                }
            )*
            None
        }

        // ---- Try to write a scalar wrapper to a byte buffer ----
        pub(super) fn try_unwrap_scalar_to_buffer(
            vm: &VirtualMachine,
            handle: ObjectHandle,
            buf: &mut [u8],
        ) -> Option<RuntimeResult<()>> {
            $(
                if let Some(data) = vm.obj_heap.get_instance_data::<$name>(handle) {
                    let bytes = data.value.to_ne_bytes();
                    let len = bytes.len();
                    if buf.len() < len {
                        return Some(Err(FfiError::Layout(
                            format!(
                                "buffer too small for {}: need {}, got {}",
                                stringify!($name),
                                len,
                                buf.len()
                            ),
                        )
                        .into()));
                    }
                    buf[..len].copy_from_slice(&bytes);
                    return Some(Ok(()));
                }
            )*
            None
        }

        // ---- Try to unwrap a scalar wrapper to a Taro value (for struct field access) ----
        pub(super) fn try_unwrap_scalar_to_taro(
            vm: &mut VirtualMachine,
            handle: ObjectHandle,
        ) -> Option<ObjectHandle> {
            $(
                if let Some(data) = vm.obj_heap.get_instance_data::<$name>(handle) {
                    return Some(define_scalar_wrappers!(@to_taro vm data $kind));
                }
            )*
            None
        }

        // ---- Error helper for scalar __new__ ----
        pub(super) fn scalar_new_error(
            _vm: &mut VirtualMachine,
            _args: &[ObjectHandle],
        ) -> RuntimeResult<ObjectHandle> {
            Err(FfiError::ScalarDirectConstruction.into())
        }
    };

    // ========================================================================
    // Internal helpers: scalar value ↔ Taro ObjectHandle / CValue conversion
    // ========================================================================

    // scalar value → CValue
    (@to_cvalue $self:ident Pointer pointer) => {
        CValue::Pointer($self.value as *const std::ffi::c_void)
    };
    (@to_cvalue $self:ident $cvalue:ident $kind:ident) => {
        CValue::$cvalue($self.value)
    };

    // scalar value → Taro ObjectHandle (for .value getter)
    (@to_taro $vm:ident $data:ident int) => {
        $vm.obj_heap.alloc_integer_instance($data.value as i64)
    };
    (@to_taro $vm:ident $data:ident float) => {
        $vm.obj_heap.alloc_float_instance($data.value as f64)
    };
    (@to_taro $vm:ident $data:ident bool) => {
        $vm.obj_heap.alloc_bool_instance($data.value != 0)
    };
    (@to_taro $vm:ident $data:ident pointer) => {
        $vm.obj_heap.alloc_integer_instance($data.value as usize as i64)
    };

    // Taro ObjectHandle → scalar value (for .value setter)
    (@from_taro $vm:ident $handle:ident int $ty:ty) => {{
        let v = $vm
            .obj_heap.expect_integer($handle)
            .copied()?;
        Ok::<$ty, RuntimeErrorKind>(v as $ty)
    }};
    (@from_taro $vm:ident $handle:ident float $ty:ty) => {{
        let v = as_f64($vm, $handle)?;
        Ok::<$ty, RuntimeErrorKind>(v as $ty)
    }};
    (@from_taro $vm:ident $handle:ident bool $ty:ty) => {{
        let v = $vm
            .obj_heap.expect_bool($handle)
            .copied()?;
        Ok::<$ty, RuntimeErrorKind>((if v { 1u8 } else { 0u8 }) as $ty)
    }};
    (@from_taro $vm:ident $handle:ident pointer $ty:ty) => {{
        let v = $vm
            .obj_heap.expect_integer($handle)
            .copied()?;
        Ok::<$ty, RuntimeErrorKind>(v as $ty)
    }};
}

// ---- Invoke the macro to generate everything ----
define_scalar_wrappers! {
    CI8:      i8   => I8,      int;
    CUint8:   u8   => U8,      int;
    CI16:     i16  => I16,     int;
    CUint16:  u16  => U16,     int;
    CI32:     i32  => I32,     int;
    CUint32:  u32  => U32,     int;
    CI64:     i64  => I64,     int;
    CUint64:  u64  => U64,     int;
    CFloat:   f32  => F32,     float;
    CDouble:  f64  => F64,     float;
    CBool:    u8   => Bool,    bool;
    CPointer: usize => Pointer, pointer;
}

// ===========================================================================
// CStruct — concrete struct instance (named fields + type back-link)
// ===========================================================================

pub(super) struct CStruct {
    /// Back-link to the `CType` instance that describes this struct's layout.
    pub(super) ctype: ObjectHandle,
    /// Field values keyed by field name.
    pub(super) fields: HashMap<ShrString, ObjectHandle>,
}

impl ObjectInstanceData for CStruct {
    fn mark_references(&self, heap: &mut ObjectHeap) {
        heap.mark_object(self.ctype);
        for &value in self.fields.values() {
            heap.mark_object(value);
        }
    }

    fn type_name(&self) -> &'static str {
        "StructInstance"
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl CStruct {
    pub(super) fn __new__(_vm: &mut VirtualMachine, _args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        Err(FfiError::StructDirectConstruction.into())
    }

    pub(super) fn __getattr__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.len() < 2 {
            return Err(FfiError::GetAttrArgCount.into());
        }
        let self_handle = args[0];
        let field_name = vm.obj_heap.expect_string(args[1])?.as_str().to_string();

        let data = vm.obj_heap.get_instance_data::<CStruct>(self_handle).ok_or(FfiError::GetAttrNotStruct)?;

        let field_handle = match data.fields.get(field_name.as_str()) {
            Some(&h) => h,
            None => return Err(FfiError::StructNoField(field_name).into()),
        };

        // Auto-unwrap scalar wrappers so struct field access returns plain values
        // (matching Python ctypes: p.x returns int, not c_int).
        if let Some(taro_val) = try_unwrap_scalar_to_taro(vm, field_handle) {
            return Ok(taro_val);
        }
        Ok(field_handle)
    }

    pub(super) fn __setattr__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.len() < 3 {
            return Err(FfiError::SetAttrArgCount.into());
        }
        let self_handle = args[0];
        let field_name = vm.obj_heap.expect_string(args[1])?.as_str().to_string();
        let value = args[2];

        let data = vm.obj_heap.get_instance_data_mut::<CStruct>(self_handle).ok_or(FfiError::SetAttrNotStruct)?;

        data.fields.insert(ShrString::new_string(field_name.as_str()), value);
        Ok(ObjectHandle::NIL)
    }
}

// ===========================================================================
// define_struct — define a C struct type, return a CType instance
// ===========================================================================

pub(super) fn define_struct(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(FfiError::StructDefExpectedList.into());
    }
    let field_descriptors = args[0];
    let descriptors = parse_struct_descriptors(vm, field_descriptors)?;
    let layout = struct_layout_from_descriptors(&descriptors)?;
    let ctype_instance = CType::Struct(layout);

    let class = vm.lookup_loaded_module_export("std/ffi", "CType").ok_or(FfiError::CTypeClassNotFound)?;
    Ok(vm.obj_heap.alloc_instance(class, ctype_instance))
}

// ===========================================================================
// parse_struct_descriptors — parse field descriptor list
// ===========================================================================

fn parse_struct_descriptors(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Vec<(String, CType)>> {
    let items = vm.obj_heap.get_list_instance(handle).ok_or(FfiError::StructDefInvalidFormat)?;

    if items.is_empty() {
        return Err(FfiError::StructDefEmptyList.into());
    }

    let is_named = vm.obj_heap.get_list_instance(items[0]).is_some();

    let mut descriptors = Vec::with_capacity(items.len());

    if is_named {
        for (i, &item) in items.iter().enumerate() {
            let pair = vm.obj_heap.get_list_instance(item).ok_or(FfiError::StructDefExpectedPair(i))?;
            if pair.len() != 2 {
                return Err(FfiError::StructDefPairLen(pair.len(), i).into());
            }
            let name = vm.obj_heap.get_string_instance(pair[0]).ok_or(FfiError::StructDefNameNotString(i))?;
            let ct = CType::from_handle(vm, pair[1]).map_err(|e| FfiError::StructDefInvalidType { pos: i, reason: e.to_string() })?;
            descriptors.push((name.as_str().to_string(), ct));
        }
    } else {
        for (i, &item) in items.iter().enumerate() {
            let ct = CType::from_handle(vm, item).map_err(|e| FfiError::StructDefInvalidType { pos: i, reason: e.to_string() })?;
            descriptors.push((i.to_string(), ct));
        }
    }

    Ok(descriptors)
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Extract an integer from the VM and map it through `f`.
fn int_to_cvalue<F>(vm: &VirtualMachine, handle: ObjectHandle, f: F) -> RuntimeResult<CValue>
where
    F: FnOnce(i64) -> CValue,
{
    let v = vm.obj_heap.expect_integer(handle).copied()?;
    Ok(f(v))
}

/// Convert a Taro value to `f64`, accepting both integer and float instances.
pub(super) fn as_f64(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<f64> {
    if let Some(v) = vm.obj_heap.get_integer_instance(handle) {
        Ok(*v as f64)
    } else if let Some(v) = vm.obj_heap.get_float_instance(handle) {
        Ok(*v)
    } else {
        Err(FfiError::ExpectedNumber(vm.obj_heap.type_of(handle).into()).into())
    }
}
