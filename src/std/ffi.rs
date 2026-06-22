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
//! // High-level: bind caches the type info — no per-call string parsing
//! cos = ffi.bind(lib, "cos", "double", ["double"]);
//! val = cos(0.0);               // callable directly
//!
//! // Struct support
//! Color = ffi.struct_def(["uint8", "uint8", "uint8", "uint8"]);
//! c     = Color(255, 0, 0, 255);    // callable — invokes StructDef.__call__
//! clear_bg = ffi.bind(lib, "ClearBackground", "void", [Color]);
//! clear_bg(c);                      // struct by value
//! ffi.dlclose(lib);
//! ```

use std::collections::HashMap;
use std::ffi::{CString, c_char, c_void};

use crate::{impl_object_instance_data, NativeFunction, ObjectHandle, ShrString, ToShrString};
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};

// ===========================================================================
// CType — unified C scalar type descriptor
// ===========================================================================
//
// CType is the single source of truth for all C scalar type information:
// size, alignment, libffi type mapping, marshalling, and return conversion.
// Instead of scattering match-on-string across six functions, each operation
// is a method on this enum.  Parsed once at bind/struct_def time, cheaply
// copied thereafter.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CType {
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
    /// Valid only as a return type (maps to `void`).
    Void,
}

impl CType {
    /// Parse a C type name string.
    fn from_str(s: &str) -> RuntimeResult<Self> {
        match s {
            "void"    => Ok(CType::Void),
            "int8"    => Ok(CType::I8),
            "int16"   => Ok(CType::I16),
            "int32"   => Ok(CType::I32),
            "int64"   => Ok(CType::I64),
            "uint8"   => Ok(CType::U8),
            "uint16"  => Ok(CType::U16),
            "uint32"  => Ok(CType::U32),
            "uint64"  => Ok(CType::U64),
            "float"   => Ok(CType::F32),
            "double"  => Ok(CType::F64),
            "pointer" => Ok(CType::Pointer),
            "cstring" => Ok(CType::CString),
            "bool"    => Ok(CType::Bool),
            _ => Err(RuntimeErrorKind::FfiError(format!(
                "unknown C type '{s}'. Supported: void int8 int16 int32 int64 \
                 uint8 uint16 uint32 uint64 float double pointer cstring bool"
            ))),
        }
    }

    /// `(size, alignment)` in bytes for struct layout computation.
    fn size_align(self) -> RuntimeResult<(usize, usize)> {
        match self {
            CType::I8 | CType::U8 | CType::Bool      => Ok((1, 1)),
            CType::I16 | CType::U16                   => Ok((2, 2)),
            CType::I32 | CType::U32 | CType::F32      => Ok((4, 4)),
            CType::I64 | CType::U64 | CType::F64
                | CType::Pointer | CType::CString      => Ok((8, 8)),
            CType::Void => Err(RuntimeErrorKind::FfiError(
                "void has no size".into(),
            )),
        }
    }

    /// Map to the corresponding `libffi::middle::Type`.
    fn to_ffi_type(self) -> libffi::middle::Type {
        match self {
            CType::I8      => libffi::middle::Type::i8(),
            CType::I16     => libffi::middle::Type::i16(),
            CType::I32     => libffi::middle::Type::i32(),
            CType::I64     => libffi::middle::Type::i64(),
            CType::U8      => libffi::middle::Type::u8(),
            CType::U16     => libffi::middle::Type::u16(),
            CType::U32     => libffi::middle::Type::u32(),
            CType::U64     => libffi::middle::Type::u64(),
            CType::F32     => libffi::middle::Type::f32(),
            CType::F64     => libffi::middle::Type::f64(),
            CType::Pointer => libffi::middle::Type::pointer(),
            CType::CString => libffi::middle::Type::pointer(),
            CType::Bool    => libffi::middle::Type::u8(),
            CType::Void    => libffi::middle::Type::void(),
        }
    }

    fn taro_to_cvalue(self, vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<CValue> {
        match self {
            CType::I8  => int_to_cvalue(vm, handle, |v| CValue::I8(v as i8)),
            CType::I16 => int_to_cvalue(vm, handle, |v| CValue::I16(v as i16)),
            CType::I32 => int_to_cvalue(vm, handle, |v| CValue::I32(v as i32)),
            CType::I64 => int_to_cvalue(vm, handle, CValue::I64),
            CType::U8  => int_to_cvalue(vm, handle, |v| CValue::U8(v as u8)),
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
                let v = vm.get_integer_instance(handle).copied()?;
                Ok(CValue::Pointer(v as *const c_void))
            }
            CType::CString => {
                let s = vm.get_string_instance(handle)?;
                let cs = CString::new(s.as_str())
                    .map_err(|e| RuntimeErrorKind::FfiError(format!("CString error: {e}")))?;
                let ptr: *const c_char = cs.as_ptr();
                Ok(CValue::CString { _cstring: cs, ptr })
            }
            CType::Bool => {
                let v = vm.get_bool_instance(handle).copied()?;
                Ok(CValue::Bool(if v { 1u8 } else { 0u8 }))
            }
            CType::Void => Err(RuntimeErrorKind::FfiError(
                "cannot marshal void as argument".into(),
            )),
        }
    }

    fn write_to_buffer(self, vm: &VirtualMachine, handle: ObjectHandle, buf: &mut [u8]) -> RuntimeResult<()> {
        match self {
            CType::I8 | CType::U8 | CType::Bool => {
                let v = vm.get_integer_instance(handle).copied()? as i8;
                buf[0] = v.to_ne_bytes()[0];
            }
            CType::I16 | CType::U16 => {
                let v = vm.get_integer_instance(handle).copied()? as i16;
                buf[..2].copy_from_slice(&v.to_ne_bytes());
            }
            CType::I32 | CType::U32 => {
                let v = vm.get_integer_instance(handle).copied()? as i32;
                buf[..4].copy_from_slice(&v.to_ne_bytes());
            }
            CType::I64 | CType::U64 => {
                let v = vm.get_integer_instance(handle).copied()?;
                buf[..8].copy_from_slice(&v.to_ne_bytes());
            }
            CType::F32 => {
                let v = as_f64(vm, handle)? as f32;
                buf[..4].copy_from_slice(&v.to_ne_bytes());
            }
            CType::F64 => {
                let v = as_f64(vm, handle)?;
                buf[..8].copy_from_slice(&v.to_ne_bytes());
            }
            CType::Pointer | CType::CString => {
                let v = vm.get_integer_instance(handle).copied()?;
                buf[..8].copy_from_slice(&v.to_ne_bytes());
            }
            CType::Void => return Err(RuntimeErrorKind::FfiError(
                "void is not a valid struct field type".into(),
            )),
        }
        Ok(())
    }

    fn call_ffi(
        self, vm: &mut VirtualMachine, cif: &libffi::middle::Cif, code_ptr: libffi::middle::CodePtr, args: &[libffi::middle::Arg]
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
                let v: *const c_void = unsafe {
                    cif.call::<*const c_void>(code_ptr, args)
                };
                Ok(vm.obj_heap.alloc_integer_instance(v as usize as i64))
            }
            CType::Bool => {
                let v: u8 = unsafe { cif.call::<u8>(code_ptr, args) };
                Ok(vm.obj_heap.alloc_bool_instance(v != 0))
            }
            CType::CString => {
                let v: *const c_char = unsafe {
                    cif.call::<*const c_char>(code_ptr, args)
                };
                if v.is_null() {
                    Ok(ObjectHandle::NIL)
                } else {
                    let bytes = unsafe { std::ffi::CStr::from_ptr(v) };
                    let s = bytes.to_string_lossy().into_owned().to_shrstring();
                    Ok(vm.obj_heap.alloc_string_instance(s))
                }
            }
        }
    }
}

// ===========================================================================
// ArgType — argument type descriptor (scalar or struct)
// ===========================================================================
//
// Unifies what were previously three separate types (ArgTypeInfo,
// ArgTypeDescriptor, StructDefRef).  Pre-parsed at bind/call time so that
// per-call marshalling does zero string comparisons.

#[derive(Clone)]
enum ArgType {
    Scalar(CType),
    Struct(Vec<CType>),   // field types of the struct
}

impl ArgType {
    fn to_ffi_type(&self) -> RuntimeResult<libffi::middle::Type> {
        match self {
            ArgType::Scalar(ct) => Ok(ct.to_ffi_type()),
            ArgType::Struct(fields) => {
                let tys: Vec<libffi::middle::Type> = fields
                    .iter()
                    .map(|ct| ct.to_ffi_type())
                    .collect();
                Ok(libffi::middle::Type::structure(tys))
            }
        }
    }

    fn taro_to_cvalue(
        &self,
        vm: &VirtualMachine,
        handle: ObjectHandle,
    ) -> RuntimeResult<CValue> {
        match self {
            ArgType::Scalar(ct) => ct.taro_to_cvalue(vm, handle),
            ArgType::Struct(_) => {
                let sv = vm.obj_heap
                    .get_native::<StructValue>(handle)
                    .ok_or_else(|| RuntimeErrorKind::FfiError("expected struct value".into()))?;
                Ok(CValue::Struct { data: sv.data.clone() })
            }
        }
    }

    /// Parse an argument type from a Taro handle.
    ///
    /// The handle is either a type-name string (e.g. `"double"`) or a struct
    /// definition object (from `ffi.struct_def`).
    fn from_handle(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        if let Some(def) = vm.obj_heap.get_native::<StructDef>(handle) {
            Ok(ArgType::Struct(def.field_types.clone()))
        } else if let Ok(s) = vm.get_string_instance(handle) {
            let ct = CType::from_str(s.as_str())?;
            Ok(ArgType::Scalar(ct))
        } else {
            Err(RuntimeErrorKind::FfiError(format!(
                "expected type string or struct def, got {}",
                vm.value_type_name(handle)
            )))
        }
    }
}

// ===========================================================================
// CValue — marshalled C value storage
// ===========================================================================

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
        /// Owned CString — kept alive until the FFI call completes.
        _cstring: CString,
        ptr: *const c_char,
    },
    /// Owned struct bytes — stored inline so the data is alive for the
    /// duration of the FFI call.  Using `Vec<u8>` rather than a raw pointer
    /// avoids a dependency on GC-heap liveness.
    Struct {
        data: Vec<u8>,
    },
}

impl CValue {
    fn as_arg(&self) -> libffi::middle::Arg {
        match self {
            CValue::I8(v)      => libffi::middle::arg(v),
            CValue::I16(v)     => libffi::middle::arg(v),
            CValue::I32(v)     => libffi::middle::arg(v),
            CValue::I64(v)     => libffi::middle::arg(v),
            CValue::U8(v)      => libffi::middle::arg(v),
            CValue::U16(v)     => libffi::middle::arg(v),
            CValue::U32(v)     => libffi::middle::arg(v),
            CValue::U64(v)     => libffi::middle::arg(v),
            CValue::F32(v)     => libffi::middle::arg(v),
            CValue::F64(v)     => libffi::middle::arg(v),
            CValue::Bool(v)    => libffi::middle::arg(v),
            CValue::Pointer(v) => libffi::middle::arg(v),
            CValue::CString { ptr, .. } => libffi::middle::arg(ptr),
            // Important: we need an Arg that points to the struct *data*,
            // not to a pointer-to-the-data.  arg(&byte_ref) achieves this:
            // Arg stores &byte_ref, which IS the address of the struct's
            // first byte.  (Contrast with arg(raw_ptr) which would point to
            // the raw-pointer variable itself, producing garbage.)
            CValue::Struct { data } => {
                let byte_ref: &u8 = if data.is_empty() {
                    // Zero-size struct: point to a static; libffi reads 0 bytes.
                    static ZERO: u8 = 0;
                    &ZERO
                } else {
                    // SAFETY: data is a non-empty Vec<u8>; as_ptr() points to
                    // at least one valid allocated byte.
                    unsafe { &*data.as_ptr() }
                };
                libffi::middle::arg(byte_ref)
            }
        }
    }
}

// ===========================================================================
// LibraryHandle — stores a loaded dynamic library on the GC heap
// ===========================================================================

struct LibraryHandle {
    lib: libloading::Library,
}

impl LibraryHandle {
    fn new(lib: libloading::Library) -> Self {
        Self { lib }
    }
}

impl_object_instance_data!(LibraryHandle, "LibraryHandle");

// ===========================================================================
// StructDef — C struct layout descriptor
// ===========================================================================

struct StructDef {
    /// Field types parsed once at definition time.
    field_types: Vec<CType>,
    /// Byte offset of each field from the start of the struct.
    offsets: Vec<usize>,
    /// Total size of the struct (including tail padding).
    size: usize,
    #[allow(dead_code)]
    alignment: usize,
}

impl StructDef {
    fn from_field_types(type_names: &[String]) -> RuntimeResult<Self> {
        let mut field_types = Vec::with_capacity(type_names.len());
        let mut offsets = Vec::with_capacity(type_names.len());
        let mut offset: usize = 0;
        let mut max_align: usize = 1;

        for name in type_names {
            let ct = CType::from_str(name)?;
            let (size, align) = ct.size_align()?;
            offset = (offset + align - 1) / align * align;
            offsets.push(offset);
            offset += size;
            if align > max_align {
                max_align = align;
            }
            field_types.push(ct);
        }

        let total_size = (offset + max_align - 1) / max_align * max_align;
        Ok(Self {
            field_types,
            offsets,
            size: total_size,
            alignment: max_align,
        })
    }
}

impl_object_instance_data!(StructDef, "StructDef");

// ===========================================================================
// StructValue — a concrete struct instance (raw bytes)
// ===========================================================================

struct StructValue {
    data: Vec<u8>,
}

impl StructValue {
    fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl_object_instance_data!(StructValue, "StructValue");

// ===========================================================================
// BoundFunction — a callable C function with pre-parsed type info
// ===========================================================================
//
// Stored on the GC heap as `Native` data.  At call time we rebuild the
// `libffi::Cif` from the pre-parsed `arg_types` — the rebuild is cheap
// (no string parsing), and it sidesteps borrow-checker issues with caching
// the CIF itself.

struct BoundFunction {
    func_ptr: *const c_void,
    arg_types: Vec<ArgType>,
    ret_type: CType,
}

unsafe impl Send for BoundFunction {}
unsafe impl Sync for BoundFunction {}

impl_object_instance_data!(BoundFunction, "BoundFunction");

// ===========================================================================
// Helpers
// ===========================================================================

/// Extract an integer from the VM and map it through `f`.
fn int_to_cvalue<F>(vm: &VirtualMachine, handle: ObjectHandle, f: F) -> RuntimeResult<CValue>
where
    F: FnOnce(i64) -> CValue,
{
    let v = vm.get_integer_instance(handle).copied()?;
    Ok(f(v))
}

/// Convert a Taro value to `f64`, accepting both integer and float instances.
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

// ===========================================================================
// Core dispatch
// ===========================================================================

/// Common tail for both `ffi.call` and bound-function invocation: build the
/// `Arg` slice, construct the `CodePtr`, call through `Cif`, and convert the
/// return value back to a Taro object.
fn dispatch_call(
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
// Low-level ffi.call implementation
// ===========================================================================

fn ffi_call_impl(
    vm: &mut VirtualMachine,
    func_ptr_raw: i64,
    ret_type_str: &str,
    arg_type_handles: &[ObjectHandle],
    arg_handles: &[ObjectHandle],
) -> RuntimeResult<ObjectHandle> {
    // Parse return type.
    let ret_type = CType::from_str(ret_type_str)?;

    // Parse argument types (handles both strings and struct defs).
    let arg_types: Vec<ArgType> = arg_type_handles
        .iter()
        .map(|&h| ArgType::from_handle(vm, h))
        .collect::<RuntimeResult<_>>()?;

    if arg_handles.len() != arg_types.len() {
        return Err(RuntimeErrorKind::FfiError(format!(
            "argument count mismatch: {} value(s) but {} type(s)",
            arg_handles.len(),
            arg_types.len()
        )));
    }

    // Build CIF.
    let ffi_arg_types: Vec<libffi::middle::Type> = arg_types
        .iter()
        .map(|at| at.to_ffi_type())
        .collect::<RuntimeResult<_>>()?;
    let cif = libffi::middle::Cif::new(ffi_arg_types, ret_type.to_ffi_type());

    // Marshal arguments.
    let mut c_values: Vec<CValue> = Vec::with_capacity(arg_handles.len());
    for (i, (at, &handle)) in arg_types.iter().zip(arg_handles).enumerate() {
        let cv = at.taro_to_cvalue(vm, handle)
            .map_err(|e| RuntimeErrorKind::FfiError(format!("argument {i}: {e}")))?;
        c_values.push(cv);
    }

    dispatch_call(vm, &cif, ret_type, func_ptr_raw as *mut c_void, &c_values)
}

// ===========================================================================
// Bound function call — invoked via __call__ on the BoundFn class
// ===========================================================================

/// Native method behind `BoundFn.__call__`.  Receives `(self, args...)`,
/// extracts the [`BoundFunction`], marshals the remaining arguments and
/// invokes the cached CIF.
fn bound_fn_call(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(RuntimeErrorKind::FfiError(
            "bound function call: missing self".into(),
        ));
    }

    let self_handle = args[0];
    let user_args = &args[1..];

    // Snapshot everything we need from the BoundFunction so we can release
    // the immutable borrow on vm.obj_heap before we need &mut vm for dispatch.
    let (func_ptr, expected, arg_types, ret_type) = {
        let bound = vm.obj_heap
            .get_native::<BoundFunction>(self_handle)
            .ok_or_else(|| RuntimeErrorKind::FfiError(
                "bound function call: self is not a BoundFunction".into(),
            ))?;
        (
            bound.func_ptr,
            bound.arg_types.len(),
            bound.arg_types.clone(),
            bound.ret_type,
        )
    };

    if user_args.len() != expected {
        return Err(RuntimeErrorKind::FfiError(format!(
            "bound function expects {expected} argument(s), got {}",
            user_args.len()
        )));
    }

    // Marshal arguments using the cached type info.
    let mut c_values: Vec<CValue> = Vec::with_capacity(expected);
    for (i, (at, &handle)) in arg_types.iter().zip(user_args).enumerate() {
        let cv = at.taro_to_cvalue(vm, handle)
            .map_err(|e| RuntimeErrorKind::FfiError(format!("argument {i}: {e}")))?;
        c_values.push(cv);
    }

    // Rebuild the CIF from the type info — the CIF is cheap to build and this
    // avoids storing a non-Send+Sync object on the GC heap.
    let ffi_arg_types: Vec<libffi::middle::Type> = arg_types
        .iter()
        .map(|at| at.to_ffi_type())
        .collect::<RuntimeResult<_>>()?;
    let cif = libffi::middle::Cif::new(ffi_arg_types, ret_type.to_ffi_type());

    dispatch_call(vm, &cif, ret_type, func_ptr as *mut c_void, &c_values)
}

// ===========================================================================
// Native-function implementations (exported to Taro)
// ===========================================================================

// -- dlopen / dlsym / dlclose ------------------------------------------------

fn dlopen(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let path_str = vm.get_string_instance(path)?;
    let lib = unsafe { libloading::Library::new(path_str.as_str()) }
        .map_err(|e| RuntimeErrorKind::FfiError(format!("dlopen: {e}")))?;

    let lib_handle = LibraryHandle::new(lib);
    let obj = vm.obj_heap.alloc_instance(vm.obj_heap.module_class, lib_handle);
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
    let arg_values: Vec<ObjectHandle> = if args.len() > 3 {
        vm.get_list_instance(args[3])?.clone()
    } else {
        vec![]
    };

    ffi_call_impl(vm, func_ptr, &ret_type, &arg_types_list, &arg_values)
}

fn struct_def(vm: &mut VirtualMachine, field_types_list: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let handles = vm.get_list_instance(field_types_list)?;
    let mut type_names = Vec::with_capacity(handles.len());
    for &h in handles {
        let s = vm.get_string_instance(h)?.as_str().to_string();
        type_names.push(s);
    }

    let def = StructDef::from_field_types(&type_names)?;
    let obj = vm.obj_heap.alloc_instance(vm.obj_heap.struct_def_class, def);
    Ok(obj)
}

fn struct_new(vm: &mut VirtualMachine, def_handle: ObjectHandle, values_list: ObjectHandle) -> RuntimeResult<ObjectHandle> {
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
    for (i, (&value_handle, &field_type)) in
        value_handles.iter().zip(&def.field_types).enumerate()
    {
        let offset = def.offsets[i];
        field_type
            .write_to_buffer(vm, value_handle, &mut data[offset..])
            .map_err(|e| RuntimeErrorKind::FfiError(format!(
                "struct_new field {i}: {e}"
            )))?;
    }

    let sv = StructValue::new(data);
    let obj = vm.obj_heap.alloc_instance(vm.obj_heap.struct_instance_class, sv);
    Ok(obj)
}

// -- struct_def_call (StructDef.__call__) ------------------------------------

/// Native method behind `StructDef.__call__`.  Receives `(self, field_values...)`,
/// extracts the [`StructDef`] from self, and creates a [`StructValue`] instance.
///
/// This enables the idiom:
/// ```taro
/// Color = ffi.struct_def(["uint8", "uint8", "uint8", "uint8"]);
/// c = Color(255, 0, 0, 255);  // calls StructDef.__call__
/// ```
fn struct_def_call(
    vm: &mut VirtualMachine,
    args: &[ObjectHandle],
) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(RuntimeErrorKind::FfiError(
            "struct call: missing self".into(),
        ));
    }

    let self_handle = args[0];
    let field_values = &args[1..];

    // Snapshot everything we need from the StructDef so we can release
    // the immutable borrow on vm.obj_heap before we need &mut vm for alloc.
    let (field_types, offsets, size) = {
        let def = vm.obj_heap
            .get_native::<StructDef>(self_handle)
            .ok_or_else(|| RuntimeErrorKind::FfiError(
                "struct call: self is not a StructDef".into(),
            ))?;
        (def.field_types.clone(), def.offsets.clone(), def.size)
    };

    if field_values.len() != field_types.len() {
        return Err(RuntimeErrorKind::FfiError(format!(
            "struct expects {} value(s), got {}",
            field_types.len(),
            field_values.len()
        )));
    }

    let mut data = vec![0u8; size];
    for (i, (&value_handle, &field_type)) in
        field_values.iter().zip(&field_types).enumerate()
    {
        let offset = offsets[i];
        field_type
            .write_to_buffer(vm, value_handle, &mut data[offset..])
            .map_err(|e| RuntimeErrorKind::FfiError(format!(
                "struct field {i}: {e}"
            )))?;
    }

    let sv = StructValue::new(data);
    let obj = vm.obj_heap.alloc_instance(vm.obj_heap.struct_instance_class, sv);
    Ok(obj)
}

// -- ffi.bind -----------------------------------------------------------------

/// `ffi.bind(lib, symbol_name, ret_type, arg_types) -> BoundFunction`
///
/// Resolves the symbol, parses/stores the type descriptors, and returns a
/// `BoundFn` instance that can be called directly from Taro (via its
/// `__call__` method).
fn bind(
    vm: &mut VirtualMachine,
    library_handle: ObjectHandle,
    name: ObjectHandle,
    ret_type_handle: ObjectHandle,
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
    let ret_type_str = vm.get_string_instance(ret_type_handle)?.as_str().to_string();
    let ret_type = CType::from_str(&ret_type_str)?;

    // --- Parse argument types ---
    let arg_type_handles = vm.get_list_instance(arg_types_list)?;
    let arg_types: Vec<ArgType> = arg_type_handles
        .iter()
        .map(|&h| ArgType::from_handle(vm, h))
        .collect::<RuntimeResult<_>>()?;

    // --- Build BoundFunction ---
    let bound = BoundFunction {
        func_ptr,
        arg_types,
        ret_type,
    };
    let obj = vm.obj_heap.alloc_instance(vm.obj_heap.bound_fn_class, bound);
    Ok(obj)
}

// ===========================================================================
// Module factory
// ===========================================================================

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

        // Ensure the struct-def class exists and register __call__ so that
        // `ffi.struct_def(...)` results are directly callable.
        if self.obj_heap.get_class(self.obj_heap.struct_def_class).is_none() {
            self.obj_heap.struct_def_class = self.obj_heap.alloc_class("StructDef");
        }
        let struct_call_fn = self.obj_heap.alloc_native_fn(
            "__call__",
            NativeFunction::var(struct_def_call),
        );
        let sdc = self.obj_heap
            .get_class_mut(self.obj_heap.struct_def_class)
            .expect("StructDef class must exist");
        sdc.methods.insert(
            ShrString::new_str("__call__"),
            crate::object::Method::Native(struct_call_fn),
        );

        // Make sure the Struct instance class exists.
        if self.obj_heap.get_class(self.obj_heap.struct_instance_class).is_none() {
            self.obj_heap.struct_instance_class = self.obj_heap.alloc_class("Struct");
        }

        // Export functions.
        let dlopen_fn     = self.obj_heap.alloc_native_fn("dlopen", NativeFunction::a1(dlopen));
        let dlsym_fn      = self.obj_heap.alloc_native_fn("dlsym", NativeFunction::a2(dlsym));
        let dlclose_fn    = self.obj_heap.alloc_native_fn("dlclose", NativeFunction::a1(dlclose));
        let call_fn       = self.obj_heap.alloc_native_fn("call", NativeFunction::var(ffi_call));
        let struct_def_fn = self.obj_heap.alloc_native_fn("struct_def", NativeFunction::a1(struct_def));
        let struct_new_fn = self.obj_heap.alloc_native_fn("struct_new", NativeFunction::a2(struct_new));
        let bind_fn       = self.obj_heap.alloc_native_fn("bind", NativeFunction::a4(bind));

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

// ===========================================================================
// Tests
// ===========================================================================

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
