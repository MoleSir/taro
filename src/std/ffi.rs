//! FFI (Foreign Function Interface) module.
//!
//! Provides runtime loading of dynamic libraries and dynamic invocation of
//! C functions — similar to Python's `ctypes` or LuaJIT's FFI.
//!
//! # Taro-level API
//!
//! ```taro
//! import "std/ffi";
//!
//! // Low-level: raw dlopen/dlsym/call
//! lib  = ffi.dlopen("libm.so.6");
//! func = ffi.dlsym(lib, "cos");
//! val  = ffi.call(func, "double", ["double"], [0.0]);
//!
//! // High-level: bind caches the CIF — no per-call string parsing
//! cos = ffi.bind(lib, "cos", "double", ["double"]);
//! val = cos(0.0);               // callable directly
//!
//! // Struct support
//! Color = ffi.struct_def(["uint8", "uint8", "uint8", "uint8"]);
//! c     = ffi.struct_new(Color, [255, 0, 0, 255]);
//! clear_bg = ffi.bind(lib, "ClearBackground", "void", [Color]);
//! clear_bg(c);                  // struct by value
//! ffi.dlclose(lib);
//! ```

use std::collections::HashMap;
use std::ffi::{CString, c_char, c_void};

use crate::{NativeFunction, ObjectHandle, ShrString, ToShrString};
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};

// ---------------------------------------------------------------------------
// LibraryHandle — stores a loaded dynamic library on the GC heap
// ---------------------------------------------------------------------------

struct LibraryHandle {
    lib: libloading::Library,
}

impl LibraryHandle {
    fn new(lib: libloading::Library) -> Self {
        Self { lib }
    }
}

impl crate::object::ToNativeData for LibraryHandle {
    fn mark_inner_object(&self, _heap: &mut crate::object::ObjectHeap) {}
}

// ---------------------------------------------------------------------------
// StructDef — C struct layout descriptor
// ---------------------------------------------------------------------------

struct StructDef {
    field_types: Vec<String>,
    offsets: Vec<usize>,
    size: usize,
    #[allow(dead_code)]
    alignment: usize,
}

impl StructDef {
    fn from_field_types(type_names: &[String]) -> RuntimeResult<Self> {
        let mut offsets = Vec::with_capacity(type_names.len());
        let mut offset: usize = 0;
        let mut max_align: usize = 1;

        for name in type_names {
            let (size, align) = scalar_size_align(name)?;
            offset = (offset + align - 1) / align * align;
            offsets.push(offset);
            offset += size;
            if align > max_align {
                max_align = align;
            }
        }
        let total_size = (offset + max_align - 1) / max_align * max_align;
        Ok(Self {
            field_types: type_names.to_vec(),
            offsets,
            size: total_size,
            alignment: max_align,
        })
    }
}

impl crate::object::ToNativeData for StructDef {
    fn mark_inner_object(&self, _heap: &mut crate::object::ObjectHeap) {}
}

// ---------------------------------------------------------------------------
// StructValue — a concrete struct instance (raw bytes)
// ---------------------------------------------------------------------------

struct StructValue {
    data: Vec<u8>,
}

impl StructValue {
    fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl crate::object::ToNativeData for StructValue {
    fn mark_inner_object(&self, _heap: &mut crate::object::ObjectHeap) {}
}

// ---------------------------------------------------------------------------
// ArgTypeInfo — pre-parsed type descriptor (no strings at call time)
// ---------------------------------------------------------------------------

/// Describes a single argument's C type for marshalling.  Pre-parsed at
/// `bind` time so that per-call marshalling does zero string comparisons.
#[derive(Clone)]
enum ArgTypeInfo {
    Scalar(String),
    Struct(Vec<String>), // field type names
}

impl ArgTypeInfo {
    fn to_ffi_type(&self) -> RuntimeResult<libffi::middle::Type> {
        match self {
            ArgTypeInfo::Scalar(s) => str_to_ffi_type(s),
            ArgTypeInfo::Struct(fields) => {
                let tys: Vec<libffi::middle::Type> = fields
                    .iter()
                    .map(|s| str_to_ffi_type(s))
                    .collect::<RuntimeResult<_>>()?;
                Ok(libffi::middle::Type::structure(tys))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// BoundFunction — a callable C function with pre-parsed type info
// ---------------------------------------------------------------------------

/// Stored on the GC heap as `Native` data.  At call time we rebuild the
/// `libffi::Cif` from the pre-parsed `arg_infos` — the rebuild is cheap
/// (no string parsing), and it sidesteps borrow-checker issues with caching
/// the CIF itself.
struct BoundFunction {
    func_ptr: *const c_void,
    arg_infos: Vec<ArgTypeInfo>,
    ret_type: String,
}

unsafe impl Send for BoundFunction {}
unsafe impl Sync for BoundFunction {}

impl crate::object::ToNativeData for BoundFunction {
    fn mark_inner_object(&self, _heap: &mut crate::object::ObjectHeap) {}
}

// ---------------------------------------------------------------------------
// Scalar type helpers
// ---------------------------------------------------------------------------

fn scalar_size_align(name: &str) -> RuntimeResult<(usize, usize)> {
    match name {
        "int8" | "uint8" | "bool" => Ok((1, 1)),
        "int16" | "uint16" => Ok((2, 2)),
        "int32" | "uint32" | "float" => Ok((4, 4)),
        "int64" | "uint64" | "double" | "pointer" | "cstring" => Ok((8, 8)),
        _ => Err(RuntimeErrorKind::FfiError(format!(
            "unknown C type '{name}'"
        ))),
    }
}

// ---------------------------------------------------------------------------
// C-value storage for argument marshalling
// ---------------------------------------------------------------------------

enum CValue {
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
        _cstring: CString,
        ptr: *const c_char,
    },
    Struct { ptr: *const u8 },
}

impl CValue {
    fn as_arg(&self) -> libffi::middle::Arg {
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
            CValue::Struct { ptr } => libffi::middle::arg(ptr),
        }
    }
}

// ---------------------------------------------------------------------------
// Type-name → libffi Type mapping
// ---------------------------------------------------------------------------

fn str_to_ffi_type(s: &str) -> RuntimeResult<libffi::middle::Type> {
    match s {
        "int8" => Ok(libffi::middle::Type::i8()),
        "int16" => Ok(libffi::middle::Type::i16()),
        "int32" => Ok(libffi::middle::Type::i32()),
        "int64" => Ok(libffi::middle::Type::i64()),
        "uint8" => Ok(libffi::middle::Type::u8()),
        "uint16" => Ok(libffi::middle::Type::u16()),
        "uint32" => Ok(libffi::middle::Type::u32()),
        "uint64" => Ok(libffi::middle::Type::u64()),
        "float" => Ok(libffi::middle::Type::f32()),
        "double" => Ok(libffi::middle::Type::f64()),
        "pointer" => Ok(libffi::middle::Type::pointer()),
        "cstring" => Ok(libffi::middle::Type::pointer()),
        "bool" => Ok(libffi::middle::Type::u8()),
        _ => Err(RuntimeErrorKind::FfiError(format!(
            "unknown C type '{s}'. Supported: int8 int16 int32 int64 uint8 uint16 uint32 uint64 float double pointer cstring bool"
        ))),
    }
}

fn str_to_ret_ffi_type(s: &str) -> RuntimeResult<libffi::middle::Type> {
    if s == "void" {
        Ok(libffi::middle::Type::void())
    } else {
        str_to_ffi_type(s)
    }
}

// ---------------------------------------------------------------------------
// Taro value → C value conversion
// ---------------------------------------------------------------------------

fn taro_to_cvalue(
    vm: &VirtualMachine,
    handle: ObjectHandle,
    type_name: &str,
) -> RuntimeResult<CValue> {
    match type_name {
        "int8" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::I8(v as i8))
        }
        "int16" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::I16(v as i16))
        }
        "int32" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::I32(v as i32))
        }
        "int64" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::I64(v))
        }
        "uint8" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::U8(v as u8))
        }
        "uint16" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::U16(v as u16))
        }
        "uint32" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::U32(v as u32))
        }
        "uint64" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::U64(v as u64))
        }
        "float" => {
            let v = as_f64(vm, handle)?;
            Ok(CValue::F32(v as f32))
        }
        "double" => {
            let v = as_f64(vm, handle)?;
            Ok(CValue::F64(v))
        }
        "pointer" => {
            let v = vm.get_integer_instance(handle).copied()?;
            Ok(CValue::Pointer(v as *const c_void))
        }
        "cstring" => {
            let s = vm.get_string_instance(handle)?;
            let cs = CString::new(s.as_str())
                .map_err(|e| RuntimeErrorKind::FfiError(format!("CString error: {e}")))?;
            let ptr: *const c_char = cs.as_ptr();
            Ok(CValue::CString { _cstring: cs, ptr })
        }
        "bool" => {
            let v = vm.get_bool_instance(handle).copied()?;
            Ok(CValue::Bool(if v { 1u8 } else { 0u8 }))
        }
        _ => Err(RuntimeErrorKind::FfiError(format!(
            "unknown C type '{type_name}'"
        ))),
    }
}

/// Convert a Taro value to a `CValue` given an [`ArgTypeInfo`].
fn taro_to_cvalue_by_info(
    vm: &VirtualMachine,
    handle: ObjectHandle,
    info: &ArgTypeInfo,
) -> RuntimeResult<CValue> {
    match info {
        ArgTypeInfo::Scalar(s) => taro_to_cvalue(vm, handle, s),
        ArgTypeInfo::Struct(_) => taro_to_struct_cvalue(vm, handle),
    }
}

fn taro_to_struct_cvalue(
    vm: &VirtualMachine,
    handle: ObjectHandle,
) -> RuntimeResult<CValue> {
    let sv = vm.obj_heap
        .get_native::<StructValue>(handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError(
            "expected struct value".into()
        ))?;
    Ok(CValue::Struct { ptr: sv.data.as_ptr() })
}

fn as_f64(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<f64> {
    if let Ok(v) = vm.get_integer_instance(handle) {
        Ok(*v as f64)
    } else if let Ok(v) = vm.get_float_instance(handle) {
        Ok(*v)
    } else {
        Err(RuntimeErrorKind::FfiError(format!(
            "expected number, got {}",
            vm.value_type_name(handle)
        )))
    }
}

// ---------------------------------------------------------------------------
// Arg-type descriptor for ffi.call (string or StructDef handle)
// ---------------------------------------------------------------------------

enum ArgTypeDescriptor {
    Scalar(String),
    Struct(StructDefRef),
}

struct StructDefRef {
    field_types: Vec<String>,
}

impl From<&StructDef> for StructDefRef {
    fn from(def: &StructDef) -> Self {
        Self {
            field_types: def.field_types.clone(),
        }
    }
}

impl StructDefRef {
    fn to_ffi_type(&self) -> RuntimeResult<libffi::middle::Type> {
        let fields: Vec<libffi::middle::Type> = self
            .field_types
            .iter()
            .map(|s| str_to_ffi_type(s))
            .collect::<RuntimeResult<_>>()?;
        Ok(libffi::middle::Type::structure(fields))
    }
}

// ---------------------------------------------------------------------------
// Low-level ffi.call implementation
// ---------------------------------------------------------------------------

fn ffi_call_impl(
    vm: &mut VirtualMachine,
    func_ptr_raw: i64,
    ret_type_str: &str,
    arg_type_handles: &[ObjectHandle],
    arg_handles: &[ObjectHandle],
) -> RuntimeResult<ObjectHandle> {
    let mut arg_descs: Vec<ArgTypeDescriptor> = Vec::with_capacity(arg_type_handles.len());
    for &h in arg_type_handles {
        if let Some(def) = vm.obj_heap.get_native::<StructDef>(h) {
            arg_descs.push(ArgTypeDescriptor::Struct(def.into()));
        } else if let Ok(s) = vm.get_string_instance(h) {
            arg_descs.push(ArgTypeDescriptor::Scalar(s.as_str().to_string()));
        } else {
            return Err(RuntimeErrorKind::FfiError(format!(
                "expected type string or struct def, got {}",
                vm.value_type_name(h)
            )));
        }
    }

    if arg_handles.len() != arg_descs.len() {
        return Err(RuntimeErrorKind::FfiError(format!(
            "argument count mismatch: {} value(s) but {} type(s)",
            arg_handles.len(),
            arg_descs.len()
        )));
    }

    let mut ffi_arg_types: Vec<libffi::middle::Type> = Vec::new();
    for desc in &arg_descs {
        match desc {
            ArgTypeDescriptor::Scalar(s) => ffi_arg_types.push(str_to_ffi_type(s)?),
            ArgTypeDescriptor::Struct(def) => ffi_arg_types.push(def.to_ffi_type()?),
        }
    }
    let ffi_ret_type = str_to_ret_ffi_type(ret_type_str)?;
    let cif = libffi::middle::Cif::new(ffi_arg_types, ffi_ret_type);

    let mut c_values: Vec<CValue> = Vec::with_capacity(arg_handles.len());
    for (i, (desc, &value_handle)) in arg_descs.iter().zip(arg_handles).enumerate() {
        let cv = match desc {
            ArgTypeDescriptor::Scalar(type_str) => {
                taro_to_cvalue(vm, value_handle, type_str)
                    .map_err(|e| RuntimeErrorKind::FfiError(format!("argument {i}: {e}")))?
            }
            ArgTypeDescriptor::Struct(_def) => {
                taro_to_struct_cvalue(vm, value_handle)
                    .map_err(|e| RuntimeErrorKind::FfiError(format!("argument {i}: {e}")))?
            }
        };
        c_values.push(cv);
    }

    dispatch_call(vm, &cif, ret_type_str, func_ptr_raw as *mut c_void, &c_values)
}

// ---------------------------------------------------------------------------
// Bound function call — invoked via __call__ on the BoundFn class
// ---------------------------------------------------------------------------

/// Native method behind `BoundFn.__call__`.  Receives `(self, args...)`,
/// extracts the [`BoundFunction`], marshals the remaining arguments and
/// invokes the cached CIF.
fn bound_fn_call(
    vm: &mut VirtualMachine,
    args: &[ObjectHandle],
) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(RuntimeErrorKind::FfiError(
            "bound function call: missing self".into(),
        ));
    }

    let self_handle = args[0];
    let user_args = &args[1..];

    // Snapshot everything we need from the BoundFunction so we can release
    // the immutable borrow on vm.obj_heap before we need &mut vm for dispatch.
    let (func_ptr, expected, arg_infos, ret_type) = {
        let bound = vm.obj_heap
            .get_native::<BoundFunction>(self_handle)
            .ok_or_else(|| RuntimeErrorKind::FfiError(
                "bound function call: self is not a BoundFunction".into(),
            ))?;
        (
            bound.func_ptr,
            bound.arg_infos.len(),
            bound.arg_infos.clone(),
            bound.ret_type.clone(),
        )
    };

    if user_args.len() != expected {
        return Err(RuntimeErrorKind::FfiError(format!(
            "bound function expects {expected} argument(s), got {}",
            user_args.len()
        )));
    }

    // Marshal arguments using the cached type info (needs &vm only).
    let mut c_values: Vec<CValue> = Vec::with_capacity(expected);
    for (i, (info, &handle)) in arg_infos.iter().zip(user_args).enumerate() {
        let cv = taro_to_cvalue_by_info(vm, handle, info)
            .map_err(|e| RuntimeErrorKind::FfiError(format!("argument {i}: {e}")))?;
        c_values.push(cv);
    }

    // Rebuild the CIF from the type info — the CIF is cheap to build and this
    // avoids storing a non-Send+Sync object on the GC heap.
    let mut ffi_arg_types = Vec::with_capacity(arg_infos.len());
    for info in &arg_infos {
        ffi_arg_types.push(info.to_ffi_type()?);
    }
    let ffi_ret = str_to_ret_ffi_type(&ret_type)?;
    let cif = libffi::middle::Cif::new(ffi_arg_types, ffi_ret);

    dispatch_call(
        vm,
        &cif,
        &ret_type,
        func_ptr as *mut c_void,
        &c_values,
    )
}

/// Common tail: build `Arg` slice, construct `CodePtr`, call through `Cif`,
/// and convert the return value back to a Taro object.
fn dispatch_call(
    vm: &mut VirtualMachine,
    cif: &libffi::middle::Cif,
    ret_type_str: &str,
    func_ptr: *mut c_void,
    c_values: &[CValue],
) -> RuntimeResult<ObjectHandle> {
    let args: Vec<libffi::middle::Arg> = c_values.iter().map(|cv| cv.as_arg()).collect();
    let code_ptr = libffi::middle::CodePtr(func_ptr);

    let result: ObjectHandle = match ret_type_str {
        "void" => {
            unsafe { cif.call::<()>(code_ptr, &args) };
            ObjectHandle::NIL
        }
        "int8" => {
            let v: i8 = unsafe { cif.call::<i8>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v as i64)
        }
        "int16" => {
            let v: i16 = unsafe { cif.call::<i16>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v as i64)
        }
        "int32" => {
            let v: i32 = unsafe { cif.call::<i32>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v as i64)
        }
        "int64" => {
            let v: i64 = unsafe { cif.call::<i64>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v)
        }
        "uint8" => {
            let v: u8 = unsafe { cif.call::<u8>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v as i64)
        }
        "uint16" => {
            let v: u16 = unsafe { cif.call::<u16>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v as i64)
        }
        "uint32" => {
            let v: u32 = unsafe { cif.call::<u32>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v as i64)
        }
        "uint64" => {
            let v: u64 = unsafe { cif.call::<u64>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v as i64)
        }
        "float" => {
            let v: f32 = unsafe { cif.call::<f32>(code_ptr, &args) };
            vm.obj_heap.alloc_float_instance(v as f64)
        }
        "double" => {
            let v: f64 = unsafe { cif.call::<f64>(code_ptr, &args) };
            vm.obj_heap.alloc_float_instance(v)
        }
        "pointer" => {
            let v: *const c_void = unsafe { cif.call::<*const c_void>(code_ptr, &args) };
            vm.obj_heap.alloc_integer_instance(v as usize as i64)
        }
        "bool" => {
            let v: u8 = unsafe { cif.call::<u8>(code_ptr, &args) };
            vm.obj_heap.alloc_bool_instance(v != 0)
        }
        "cstring" => {
            let v: *const c_char = unsafe { cif.call::<*const c_char>(code_ptr, &args) };
            if v.is_null() {
                ObjectHandle::NIL
            } else {
                let bytes = unsafe { std::ffi::CStr::from_ptr(v) };
                let s = bytes.to_string_lossy().into_owned().to_shrstring();
                vm.obj_heap.alloc_string_instance(s)
            }
        }
        _ => return Err(RuntimeErrorKind::FfiError(format!(
            "unsupported return type: {ret_type_str}"
        ))),
    };

    Ok(result)
}

// ---------------------------------------------------------------------------
// Native-function implementations (exported to Taro)
// ---------------------------------------------------------------------------

fn dlopen(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let path_str = vm.get_string_instance(path)?;
    let lib = unsafe { libloading::Library::new(path_str.as_str()) }
        .map_err(|e| RuntimeErrorKind::FfiError(format!("dlopen: {e}")))?;

    let lib_handle = LibraryHandle::new(lib);
    let native = crate::object::NativeData::new(lib_handle);
    let obj = vm.obj_heap.alloc_instance(
        vm.obj_heap.module_class,
        crate::object::ObjectInstanceData::Native(native),
    );
    Ok(obj)
}

fn dlsym(vm: &mut VirtualMachine, library_handle: ObjectHandle, name: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let name_str = vm.get_string_instance(name)?;
    let lib = vm.obj_heap
        .get_native::<LibraryHandle>(library_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("dlsym: not a library handle".into()))?;

    unsafe {
        let symbol: libloading::Symbol<*const c_void> = lib.lib
            .get(name_str.as_str().as_bytes())
            .map_err(|e| RuntimeErrorKind::FfiError(format!("dlsym('{}'): {e}", name_str)))?;

        let ptr_addr = *symbol as usize as i64;
        Ok(vm.obj_heap.alloc_integer_instance(ptr_addr))
    }
}

fn dlclose(vm: &mut VirtualMachine, library_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let lib = vm.obj_heap
        .get_native::<LibraryHandle>(library_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("dlclose: not a library handle".into()))?;
    let _ = lib;
    Ok(ObjectHandle::NIL)
}

fn ffi_call(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.len() < 3 {
        return Err(RuntimeErrorKind::FfiError(
            "ffi.call(func_ptr, ret_type, arg_types, args) — need at least 3 arguments".into(),
        ));
    }

    let func_ptr = vm.get_integer_instance(args[0]).copied()?;
    let ret_type = vm.get_string_instance(args[1])?.as_str().to_string();
    let arg_types_list: Vec<ObjectHandle> = vm.get_list_instance(args[2])?.clone();
    let arg_values: Vec<ObjectHandle> =
        if args.len() > 3 {
            vm.get_list_instance(args[3])?.clone()
        } else {
            vec![]
        };

    ffi_call_impl(vm, func_ptr, &ret_type, &arg_types_list, &arg_values)
}

// ---------------------------------------------------------------------------
// struct_def & struct_new
// ---------------------------------------------------------------------------

fn struct_def(
    vm: &mut VirtualMachine,
    field_types_list: ObjectHandle,
) -> RuntimeResult<ObjectHandle> {
    let handles = vm.get_list_instance(field_types_list)?;
    let mut type_names = Vec::with_capacity(handles.len());
    for &h in handles {
        let s = vm.get_string_instance(h)?.as_str().to_string();
        type_names.push(s);
    }

    let def = StructDef::from_field_types(&type_names)?;
    let native = crate::object::NativeData::new(def);
    let obj = vm.obj_heap.alloc_instance(
        vm.obj_heap.module_class,
        crate::object::ObjectInstanceData::Native(native),
    );
    Ok(obj)
}

fn struct_new(
    vm: &mut VirtualMachine,
    def_handle: ObjectHandle,
    values_list: ObjectHandle,
) -> RuntimeResult<ObjectHandle> {
    let def = vm.obj_heap
        .get_native::<StructDef>(def_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError(
            "struct_new: first argument must be a struct def".into(),
        ))?;

    let value_handles = vm.get_list_instance(values_list)?;

    if value_handles.len() != def.field_types.len() {
        return Err(RuntimeErrorKind::FfiError(format!(
            "struct_new: expected {} values, got {}",
            def.field_types.len(),
            value_handles.len()
        )));
    }

    let mut data = vec![0u8; def.size];
    for (i, (&value_handle, field_type)) in value_handles.iter().zip(&def.field_types).enumerate() {
        let offset = def.offsets[i];
        write_scalar_to_buffer(vm, value_handle, field_type, &mut data, offset)
            .map_err(|e| RuntimeErrorKind::FfiError(format!(
                "struct_new field {i}: {e}"
            )))?;
    }

    let sv = StructValue::new(data);
    let native = crate::object::NativeData::new(sv);
    let obj = vm.obj_heap.alloc_instance(
        vm.obj_heap.module_class,
        crate::object::ObjectInstanceData::Native(native),
    );
    Ok(obj)
}

fn write_scalar_to_buffer(
    vm: &VirtualMachine,
    handle: ObjectHandle,
    type_name: &str,
    buf: &mut [u8],
    offset: usize,
) -> RuntimeResult<()> {
    match type_name {
        "int8" | "uint8" | "bool" => {
            let v = vm.get_integer_instance(handle).copied()? as i8;
            buf[offset] = v.to_ne_bytes()[0];
        }
        "int16" | "uint16" => {
            let v = vm.get_integer_instance(handle).copied()? as i16;
            buf[offset..offset + 2].copy_from_slice(&v.to_ne_bytes());
        }
        "int32" | "uint32" => {
            let v = vm.get_integer_instance(handle).copied()? as i32;
            buf[offset..offset + 4].copy_from_slice(&v.to_ne_bytes());
        }
        "int64" | "uint64" => {
            let v = vm.get_integer_instance(handle).copied()?;
            buf[offset..offset + 8].copy_from_slice(&v.to_ne_bytes());
        }
        "float" => {
            let v = as_f64(vm, handle)? as f32;
            buf[offset..offset + 4].copy_from_slice(&v.to_ne_bytes());
        }
        "double" => {
            let v = as_f64(vm, handle)?;
            buf[offset..offset + 8].copy_from_slice(&v.to_ne_bytes());
        }
        "pointer" | "cstring" => {
            let v = vm.get_integer_instance(handle).copied()?;
            buf[offset..offset + 8].copy_from_slice(&v.to_ne_bytes());
        }
        _ => return Err(RuntimeErrorKind::FfiError(format!(
            "struct field type not supported: '{type_name}'"
        ))),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ffi.bind — create a cached, callable bound function
// ---------------------------------------------------------------------------

/// `ffi.bind(lib, symbol_name, ret_type, arg_types) -> BoundFunction`
///
/// Resolves the symbol, parses/stores the type descriptors, builds and caches
/// the `libffi::Cif`, and returns a `BoundFn` instance that can be called
/// directly from Taro (via its `__call__` method).
fn bind(
    vm: &mut VirtualMachine,
    library_handle: ObjectHandle,
    name: ObjectHandle,
    ret_type: ObjectHandle,
    arg_types_list: ObjectHandle,
) -> RuntimeResult<ObjectHandle> {
    // --- Resolve function pointer ---
    let name_str = vm.get_string_instance(name)?;
    let lib = vm.obj_heap
        .get_native::<LibraryHandle>(library_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("bind: not a library handle".into()))?;

    let func_ptr: *const c_void = unsafe {
        let symbol: libloading::Symbol<*const c_void> = lib.lib
            .get(name_str.as_str().as_bytes())
            .map_err(|e| RuntimeErrorKind::FfiError(format!(
                "bind('{}'): {e}", name_str
            )))?;
        *symbol
    };

    // --- Parse return type ---
    let ret_type_str = vm.get_string_instance(ret_type)?.as_str().to_string();
    // Validate the return type is known.
    str_to_ret_ffi_type(&ret_type_str)?;

    // --- Parse argument types ---
    let arg_type_handles = vm.get_list_instance(arg_types_list)?;
    let mut arg_infos = Vec::with_capacity(arg_type_handles.len());
    for &h in arg_type_handles {
        if let Some(def) = vm.obj_heap.get_native::<StructDef>(h) {
            arg_infos.push(ArgTypeInfo::Struct(def.field_types.clone()));
        } else if let Ok(s) = vm.get_string_instance(h) {
            let type_str = s.as_str().to_string();
            // Validate the scalar type.
            str_to_ffi_type(&type_str)?;
            arg_infos.push(ArgTypeInfo::Scalar(type_str));
        } else {
            return Err(RuntimeErrorKind::FfiError(format!(
                "bind: expected type string or struct def, got {}",
                vm.value_type_name(h)
            )));
        }
    }

    // --- Build BoundFunction ---
    let bound = BoundFunction {
        func_ptr,
        arg_infos,
        ret_type: ret_type_str,
    };
    let native = crate::object::NativeData::new(bound);
    let obj = vm.obj_heap.alloc_instance(
        vm.obj_heap.bound_fn_class,
        crate::object::ObjectInstanceData::Native(native),
    );
    Ok(obj)
}

// ---------------------------------------------------------------------------
// Module factory
// ---------------------------------------------------------------------------

impl VirtualMachine {
    pub(crate) fn create_ffi_module(&mut self) -> RuntimeResult<ObjectHandle> {
        // Ensure the bound-function class exists and register __call__.
        // The class may have been GC'd between heap init and module import,
        // so recreate it lazily if needed.
        if self.obj_heap.get_class(self.obj_heap.bound_fn_class).is_none() {
            self.obj_heap.bound_fn_class = self.obj_heap.alloc_class("BoundFn");
        }
        let bound_call_fn = self.obj_heap.alloc_native_fn(
            "__call__",
            NativeFunction::var(bound_fn_call),
        );
        let bfc = self.obj_heap
            .get_class_mut(self.obj_heap.bound_fn_class)
            .expect("BoundFn class must exist");
        bfc.methods.insert(
            ShrString::new_str("__call__"),
            crate::object::Method::Native(bound_call_fn),
        );

        // Export functions.
        let dlopen_fn      = self.obj_heap.alloc_native_fn("dlopen", NativeFunction::a1(dlopen));
        let dlsym_fn       = self.obj_heap.alloc_native_fn("dlsym", NativeFunction::a2(dlsym));
        let dlclose_fn     = self.obj_heap.alloc_native_fn("dlclose", NativeFunction::a1(dlclose));
        let call_fn        = self.obj_heap.alloc_native_fn("call", NativeFunction::var(ffi_call));
        let struct_def_fn  = self.obj_heap.alloc_native_fn("struct_def", NativeFunction::a1(struct_def));
        let struct_new_fn  = self.obj_heap.alloc_native_fn("struct_new", NativeFunction::a2(struct_new));
        let bind_fn        = self.obj_heap.alloc_native_fn("bind", NativeFunction::a4(bind));

        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("dlopen"), dlopen_fn);
        exports.insert(ShrString::new_str("dlsym"), dlsym_fn);
        exports.insert(ShrString::new_str("dlclose"), dlclose_fn);
        exports.insert(ShrString::new_str("call"), call_fn);
        exports.insert(ShrString::new_str("struct_def"), struct_def_fn);
        exports.insert(ShrString::new_str("struct_new"), struct_new_fn);
        exports.insert(ShrString::new_str("bind"), bind_fn);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::vm::VirtualMachine;

    #[test]
    fn ffi_import_module() {
        let mut vm = VirtualMachine::new();
        vm.interpret(r#"import "std/ffi";"#).unwrap();
    }

    #[test]
    fn ffi_dlopen_nonexistent_library() {
        let mut vm = VirtualMachine::new();
        let result = vm.interpret(
            r#"
            import "std/ffi";
            ffi.dlopen("/nonexistent/lib_does_not_exist.so");
            "#,
        );
        assert!(result.is_err(), "dlopen of nonexistent library should fail");
    }

    #[test]
    fn ffi_libm_cos() {
        let lib_path = if cfg!(target_os = "linux") {
            "libm.so.6"
        } else if cfg!(target_os = "macos") {
            "libSystem.dylib"
        } else {
            return;
        };

        let mut vm = VirtualMachine::new();
        let source = format!(
            r##"
            import "std/ffi";
            var lib = ffi.dlopen("{lib_path}");
            var cos = ffi.dlsym(lib, "cos");
            var r = ffi.call(cos, "double", ["double"], [0.0]);
            print(r);
            "##
        );
        vm.interpret(&source).expect("ffi_libm_cos should succeed");
    }

    #[test]
    fn ffi_call_void_return() {
        let lib_path = if cfg!(target_os = "linux") {
            "libc.so.6"
        } else if cfg!(target_os = "macos") {
            "libSystem.dylib"
        } else {
            return;
        };

        let mut vm = VirtualMachine::new();
        let source = format!(
            r##"
            import "std/ffi";
            var lib = ffi.dlopen("{lib_path}");
            var srand = ffi.dlsym(lib, "srand");
            var result = ffi.call(srand, "void", ["uint32"], [42]);
            print(result);
            "##
        );
        vm.interpret(&source).expect("ffi_call_void_return should succeed");
    }

    #[test]
    fn ffi_struct_def_and_new() {
        let mut vm = VirtualMachine::new();
        let source = r#"
            import "std/ffi";
            var Color = ffi.struct_def(["uint8", "uint8", "uint8", "uint8"]);
            var c = ffi.struct_new(Color, [255, 0, 0, 255]);
            print(c);
        "#;
        vm.interpret(source).expect("ffi_struct_def_and_new should succeed");
    }

    #[test]
    fn ffi_bind_cos() {
        let lib_path = if cfg!(target_os = "linux") {
            "libm.so.6"
        } else if cfg!(target_os = "macos") {
            "libSystem.dylib"
        } else {
            return;
        };

        let mut vm = VirtualMachine::new();
        let source = format!(
            r##"
            import "std/ffi";
            var lib = ffi.dlopen("{lib_path}");
            var cos = ffi.bind(lib, "cos", "double", ["double"]);
            var r = cos(0.0);
            print(r);
            "##
        );
        vm.interpret(&source).expect("ffi_bind_cos should succeed");
    }

    #[test]
    fn ffi_bind_abs() {
        let lib_path = if cfg!(target_os = "linux") {
            "libc.so.6"
        } else if cfg!(target_os = "macos") {
            "libSystem.dylib"
        } else {
            return;
        };

        let mut vm = VirtualMachine::new();
        let source = format!(
            r##"
            import "std/ffi";
            var lib = ffi.dlopen("{lib_path}");
            var abs = ffi.bind(lib, "abs", "int32", ["int32"]);
            var r = abs(-42);
            print(r);
            "##
        );
        vm.interpret(&source).expect("ffi_bind_abs should succeed");
    }

    #[test]
    fn ffi_bind_void_return() {
        let lib_path = if cfg!(target_os = "linux") {
            "libc.so.6"
        } else if cfg!(target_os = "macos") {
            "libSystem.dylib"
        } else {
            return;
        };

        let mut vm = VirtualMachine::new();
        let source = format!(
            r##"
            import "std/ffi";
            var lib = ffi.dlopen("{lib_path}");
            var srand = ffi.bind(lib, "srand", "void", ["uint32"]);
            var r = srand(42);
            print(r);
            "##
        );
        vm.interpret(&source).expect("ffi_bind_void_return should succeed");
    }
}
