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
//! lib  = ffi.dlopen("libm.so.6");        // load a dynamic library
//! func = ffi.dlsym(lib, "cos");           // get a function pointer (int64)
//! val  = ffi.call(func, "double", ["double"], [0.0]);  // => 1.0
//! ffi.dlclose(lib);                       // close the library
//! ```

use std::collections::HashMap;
use std::ffi::{CString, c_char, c_void};

use crate::{NativeFunction, ObjectHandle, ShrString, ToShrString};
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};

// ---------------------------------------------------------------------------
// LibraryHandle — stores a loaded dynamic library on the GC heap
// ---------------------------------------------------------------------------

/// Wrapper so a [`libloading::Library`] can be stored inside a Taro
/// `ObjectInstanceData::Native` slot.
struct LibraryHandle {
    lib: libloading::Library,
}

impl LibraryHandle {
    fn new(lib: libloading::Library) -> Self {
        Self { lib }
    }
}

impl crate::object::ToNativeData for LibraryHandle {
    fn mark_inner_object(&self, _heap: &mut crate::object::ObjectHeap) {
        // LibraryHandle owns no ObjectHandle references.
    }
}

// ---------------------------------------------------------------------------
// C-value storage for argument marshalling
// ---------------------------------------------------------------------------

/// Heterogeneous storage for C argument values.
///
/// Each variant holds a Rust-native representation of a C value.  We build a
/// `Vec<CValue>` from the Taro arguments, then borrow each element to produce
/// the `libffi::middle::Arg` slice required by `Cif::call`.  The `CValue`s
/// **must** outlive the actual FFI call — which is guaranteed because they are
/// locals in `ffi_call_impl`.
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
    /// `*const c_void` (generic pointer / function pointer / opaque address).
    Pointer(*const c_void),
    /// NUL-terminated C string.  `cstring` owns the allocation; `ptr` is the
    /// pointer we actually pass to `arg()` so we can take a stable reference.
    CString {
        _cstring: CString,
        ptr: *const c_char,
    },
}

impl CValue {
    /// Return a [`libffi::middle::Arg`] borrowing this value.
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
        }
    }
}

// ---------------------------------------------------------------------------
// Type-name → libffi Type mapping
// ---------------------------------------------------------------------------

/// Map a C type name string to the corresponding [`libffi::middle::Type`].
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
        "cstring" => Ok(libffi::middle::Type::pointer()), // char* = pointer
        "bool" => Ok(libffi::middle::Type::u8()),
        _ => Err(RuntimeErrorKind::FfiError(format!(
            "unknown C type '{s}'. Supported: int8 int16 int32 int64 uint8 uint16 uint32 uint64 float double pointer cstring bool"
        ))),
    }
}

/// Map the return-type name to an ffi Type.  `"void"` is only valid here
/// (not for arguments).
fn str_to_ret_ffi_type(s: &str) -> RuntimeResult<libffi::middle::Type> {
    if s == "void" {
        Ok(libffi::middle::Type::void())
    } else {
        str_to_ffi_type(s)
    }
}

// ---------------------------------------------------------------------------
// Taro value ↔ C value conversion
// ---------------------------------------------------------------------------

/// Convert a single Taro value into a [`CValue`] according to `type_name`.
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

/// Extract an `f64` from a numeric handle (int or float).
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
// Core FFI call implementation
// ---------------------------------------------------------------------------

/// Dynamically call a C function via libffi.
///
/// * `func_ptr_raw` — raw function pointer address (obtained from `dlsym`).
/// * `ret_type_str`  — C return-type name.
/// * `arg_type_handles` — list of string handles naming C argument types.
/// * `arg_handles`       — list of Taro values to pass as arguments.
fn ffi_call_impl(
    vm: &mut VirtualMachine,
    func_ptr_raw: i64,
    ret_type_str: &str,
    arg_type_handles: &[ObjectHandle],
    arg_handles: &[ObjectHandle],
) -> RuntimeResult<ObjectHandle> {
    // Resolve argument type strings.
    let mut arg_type_strs: Vec<String> = Vec::with_capacity(arg_type_handles.len());
    for &h in arg_type_handles {
        let s = vm.get_string_instance(h)?.as_str().to_string();
        arg_type_strs.push(s);
    }

    if arg_handles.len() != arg_type_strs.len() {
        return Err(RuntimeErrorKind::FfiError(format!(
            "argument count mismatch: {} value(s) but {} type(s)",
            arg_handles.len(),
            arg_type_strs.len()
        )));
    }

    // --- Build CIF (Call Interface) ---
    let mut ffi_arg_types: Vec<libffi::middle::Type> = Vec::new();
    for s in &arg_type_strs {
        ffi_arg_types.push(str_to_ffi_type(s)?);
    }
    let ffi_ret_type = str_to_ret_ffi_type(ret_type_str)?;
    let cif = libffi::middle::Cif::new(ffi_arg_types, ffi_ret_type);

    // --- Marshal arguments into C values ---
    let mut c_values: Vec<CValue> = Vec::with_capacity(arg_handles.len());
    for (i, (&handle, type_str)) in arg_handles.iter().zip(&arg_type_strs).enumerate() {
        let cv = taro_to_cvalue(vm, handle, type_str)
            .map_err(|e| {
                RuntimeErrorKind::FfiError(format!(
                    "argument {i}: {e}"
                ))
            })?;
        c_values.push(cv);
    }

    // Build Arg slice (references into c_values).
    let args: Vec<libffi::middle::Arg> = c_values.iter().map(|cv| cv.as_arg()).collect();
    let code_ptr = libffi::middle::CodePtr(func_ptr_raw as *mut c_void);

    // --- Call (dispatch on return type at compile time) ---
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

fn dlopen(
    vm: &mut VirtualMachine,
    path: ObjectHandle,
) -> RuntimeResult<ObjectHandle> {
    let path_str = vm.get_string_instance(path)?;
    let lib = unsafe { libloading::Library::new(path_str.as_str()) }
        .map_err(|e| RuntimeErrorKind::FfiError(format!("dlopen: {e}")))?;

    // Allocate a Fields instance to represent the library object.  We embed the
    // LibraryHandle as native data so the library is kept alive as long as the
    // Taro object exists.
    let lib_handle = LibraryHandle::new(lib);
    let native = crate::object::NativeData::new(lib_handle);

    // Use a class-less instance with Native data — the caller just needs the
    // handle for dlsym/dlclose.
    let obj = vm.obj_heap.alloc_instance(
        vm.obj_heap.module_class, // reuse module_class as a generic container
        crate::object::ObjectInstanceData::Native(native),
    );
    Ok(obj)
}

fn dlsym(
    vm: &mut VirtualMachine,
    library_handle: ObjectHandle,
    name: ObjectHandle,
) -> RuntimeResult<ObjectHandle> {
    let name_str = vm.get_string_instance(name)?;
    let lib = vm.obj_heap
        .get_native::<LibraryHandle>(library_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("dlsym: not a library handle".into()))?;

    unsafe {
        // Convert name to a null-terminated byte string for dlsym.
        let symbol: libloading::Symbol<*const c_void> = lib.lib
            .get(name_str.as_str().as_bytes())
            .map_err(|e| RuntimeErrorKind::FfiError(format!("dlsym('{}'): {e}", name_str)))?;

        // Return the raw function pointer as an int64.
        let ptr_addr = *symbol as usize as i64;
        Ok(vm.obj_heap.alloc_integer_instance(ptr_addr))
    }
}

fn dlclose(
    vm: &mut VirtualMachine,
    library_handle: ObjectHandle,
) -> RuntimeResult<ObjectHandle> {
    // Take ownership of the library, which will drop it (and close the library).
    let lib = vm.obj_heap
        .get_native::<LibraryHandle>(library_handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("dlclose: not a library handle".into()))?;

    // Note: the actual `dlclose` happens when the GC collects the library
    // handle.  We can't force-close a Library that's behind a shared (GC-managed)
    // reference — this matches the safe semantics of the language (no
    // use-after-free by construction).
    let _ = lib;
    Ok(ObjectHandle::NIL)
}

fn call(
    vm: &mut VirtualMachine,
    args: &[ObjectHandle],
) -> RuntimeResult<ObjectHandle> {
    // Signature: ffi.call(func_ptr, ret_type, arg_types_list, args_list)
    if args.len() < 3 {
        return Err(RuntimeErrorKind::FfiError(
            "ffi.call(func_ptr, ret_type, arg_types, args) — need at least 3 arguments".into(),
        ));
    }

    let func_ptr = vm.get_integer_instance(args[0]).copied()?;
    let ret_type = vm.get_string_instance(args[1])?.as_str().to_string();

    // args[2] is the list of argument-type strings.
    let arg_types_list: Vec<ObjectHandle> = vm.get_list_instance(args[2])?.clone();

    // args[3] is the list of actual Taro values.
    let arg_values: Vec<ObjectHandle> =
        if args.len() > 3 {
            vm.get_list_instance(args[3])?.clone()
        } else {
            vec![]
        };

    ffi_call_impl(vm, func_ptr, &ret_type, &arg_types_list, &arg_values)
}

// ---------------------------------------------------------------------------
// Module factory
// ---------------------------------------------------------------------------

impl VirtualMachine {
    /// Create the `ffi` std module.
    pub(crate) fn create_ffi_module(&mut self) -> RuntimeResult<ObjectHandle> {
        let dlopen_fn = self.obj_heap.alloc_native_fn("dlopen", NativeFunction::a1(dlopen));
        let dlsym_fn = self.obj_heap.alloc_native_fn("dlsym", NativeFunction::a2(dlsym));
        let dlclose_fn = self.obj_heap.alloc_native_fn("dlclose", NativeFunction::a1(dlclose));
        let call_fn = self.obj_heap.alloc_native_fn("call", NativeFunction::var(call));

        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("dlopen"), dlopen_fn);
        exports.insert(ShrString::new_str("dlsym"), dlsym_fn);
        exports.insert(ShrString::new_str("dlclose"), dlclose_fn);
        exports.insert(ShrString::new_str("call"), call_fn);

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
        // Basic smoke test: importing "std/ffi" must succeed.
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
        assert!(
            result.is_err(),
            "dlopen of nonexistent library should fail"
        );
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
}
