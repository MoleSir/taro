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
//! val = cos(0.0);
//!
//! // Struct support — positional
//! Color = ffi.define_struct(["uint8", "uint8", "uint8", "uint8"]);
//! c     = Color(255, 0, 0, 255);
//!
//! // Struct support — named fields
//! Vec3 = ffi.define_struct([["x", "float"], ["y", "float"], ["z", "float"]]);
//! v    = Vec3(1.0, 2.0, 3.0);
//! print(v.x);  // 1.0  — named field access via dot
//!
//! // Nested structs — CType objects used as field types
//! Vector3 = ffi.define_struct([["x", ffi.c_float], ["y", ffi.c_float], ["z", ffi.c_float]]);
//! Camera3D = ffi.define_struct([
//!     ["position", Vector3],
//!     ["target", Vector3],
//!     ["up", Vector3],
//!     ["fovy", ffi.c_float],
//!     ["projection", ffi.c_int32]
//! ]);
//!
//! ffi.dlclose(lib);
//! ```
mod error;
mod function;
mod library;
mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::vm::{RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle, ShrString};

use types::CType;

// ===========================================================================
// Module factory
// ===========================================================================

impl VirtualMachine {
    pub(crate) fn create_ffi_module(&mut self) -> RuntimeResult<ObjectHandle> {
        let library_class = self.obj_heap.alloc_class("CDynLib");
        self.register_native_method(library_class, "__new__", NativeFunction::var(library::CDynLib::__new__));
        self.register_native_method(library_class, "symbol", NativeFunction::a2(library::CDynLib::symbol));
        self.register_native_method(library_class, "bind", NativeFunction::a4(library::CDynLib::bind));

        let csymbol_class = self.obj_heap.alloc_class("CSymbol");
        self.register_native_method(csymbol_class, "__new__", NativeFunction::var(library::CSymbol::__new__));

        let cfunction_class = self.obj_heap.alloc_class("CFunction");
        self.register_native_method(cfunction_class, "__new__", NativeFunction::var(function::CFunction::__new__));
        self.register_native_method(cfunction_class, "__call__", NativeFunction::var(function::CFunction::__call__));

        let ctype_class = self.obj_heap.alloc_class("CType");
        self.register_native_method(ctype_class, "__new__", NativeFunction::var(types::CType::__new__));
        self.register_native_method(ctype_class, "__call__", NativeFunction::var(types::CType::__call__));

        let struct_instance_class = self.obj_heap.alloc_class("CStruct");
        self.register_native_method(struct_instance_class, "__new__", NativeFunction::var(types::CStruct::__new__));
        self.register_native_method(struct_instance_class, "__getattr__", NativeFunction::var(types::CStruct::__getattr__));
        self.register_native_method(struct_instance_class, "__setattr__", NativeFunction::var(types::CStruct::__setattr__));

        // Register a scalar wrapper class (e.g. CI8, CUint8, …).
        // Each scalar class has the same method signatures — the per-type
        // dispatch happens inside scalar_getattr / scalar_setattr.
        macro_rules! register_scalar_class {
            ($name:ident) => {{
                let class = self.obj_heap.alloc_class(stringify!($name));
                self.register_native_method(class, "__new__", NativeFunction::var(types::scalar_new_error));
                self.register_native_method(class, "__getattr__", NativeFunction::var(types::scalar_getattr));
                self.register_native_method(class, "__setattr__", NativeFunction::var(types::scalar_setattr));
                class
            }};
        }

        let ci8_class = register_scalar_class!(CI8);
        let cuint8_class = register_scalar_class!(CUint8);
        let ci16_class = register_scalar_class!(CI16);
        let cuint16_class = register_scalar_class!(CUint16);
        let ci32_class = register_scalar_class!(CI32);
        let cuint32_class = register_scalar_class!(CUint32);
        let ci64_class = register_scalar_class!(CI64);
        let cuint64_class = register_scalar_class!(CUint64);
        let cfloat_class = register_scalar_class!(CFloat);
        let cdouble_class = register_scalar_class!(CDouble);
        let cbool_class = register_scalar_class!(CBool);
        let cpointer_class = register_scalar_class!(CPointer);

        macro_rules! ctype_singleton {
            ($variant:ident) => {
                self.obj_heap.alloc_instance(ctype_class, CType::$variant)
            };
        }

        // ---- export functions ----
        let call_fn = self.obj_heap.alloc_native_fn("call", NativeFunction::var(function::call));
        let define_struct_fn = self.obj_heap.alloc_native_fn("define_struct", NativeFunction::var(types::define_struct));

        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("call"), call_fn);
        exports.insert(ShrString::new_str("define_struct"), define_struct_fn);

        // ---- export CType scalar singletons ----
        exports.insert(ShrString::new_str("c_int8"), ctype_singleton!(I8));
        exports.insert(ShrString::new_str("c_int16"), ctype_singleton!(I16));
        exports.insert(ShrString::new_str("c_int32"), ctype_singleton!(I32));
        exports.insert(ShrString::new_str("c_int64"), ctype_singleton!(I64));
        exports.insert(ShrString::new_str("c_uint8"), ctype_singleton!(U8));
        exports.insert(ShrString::new_str("c_uint16"), ctype_singleton!(U16));
        exports.insert(ShrString::new_str("c_uint32"), ctype_singleton!(U32));
        exports.insert(ShrString::new_str("c_uint64"), ctype_singleton!(U64));
        exports.insert(ShrString::new_str("c_float"), ctype_singleton!(F32));
        exports.insert(ShrString::new_str("c_double"), ctype_singleton!(F64));
        exports.insert(ShrString::new_str("c_bool"), ctype_singleton!(Bool));
        exports.insert(ShrString::new_str("c_pointer"), ctype_singleton!(Pointer));
        exports.insert(ShrString::new_str("c_cstring"), ctype_singleton!(CString));

        // ---- export scalar wrapper classes (looked up by name at runtime) ----
        exports.insert(ShrString::new_str("CI8"), ci8_class);
        exports.insert(ShrString::new_str("CUint8"), cuint8_class);
        exports.insert(ShrString::new_str("CI16"), ci16_class);
        exports.insert(ShrString::new_str("CUint16"), cuint16_class);
        exports.insert(ShrString::new_str("CI32"), ci32_class);
        exports.insert(ShrString::new_str("CUint32"), cuint32_class);
        exports.insert(ShrString::new_str("CI64"), ci64_class);
        exports.insert(ShrString::new_str("CUint64"), cuint64_class);
        exports.insert(ShrString::new_str("CFloat"), cfloat_class);
        exports.insert(ShrString::new_str("CDouble"), cdouble_class);
        exports.insert(ShrString::new_str("CBool"), cbool_class);
        exports.insert(ShrString::new_str("CPointer"), cpointer_class);

        exports.insert(ShrString::new_str("CDynLib"), library_class);
        exports.insert(ShrString::new_str("CSymbol"), csymbol_class);
        exports.insert(ShrString::new_str("CFunction"), cfunction_class);
        exports.insert(ShrString::new_str("CType"), ctype_class);
        exports.insert(ShrString::new_str("CStruct"), struct_instance_class);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);

        self.obj_heap.get_class_mut(ci8_class).expect("CI8").module = Some(module);
        self.obj_heap.get_class_mut(cuint8_class).expect("CUint8").module = Some(module);
        self.obj_heap.get_class_mut(ci16_class).expect("CI16").module = Some(module);
        self.obj_heap.get_class_mut(cuint16_class).expect("CUint16").module = Some(module);
        self.obj_heap.get_class_mut(ci32_class).expect("CI32").module = Some(module);
        self.obj_heap.get_class_mut(cuint32_class).expect("CUint32").module = Some(module);
        self.obj_heap.get_class_mut(ci64_class).expect("CI64").module = Some(module);
        self.obj_heap.get_class_mut(cuint64_class).expect("CUint64").module = Some(module);
        self.obj_heap.get_class_mut(cfloat_class).expect("CFloat").module = Some(module);
        self.obj_heap.get_class_mut(cdouble_class).expect("CDouble").module = Some(module);
        self.obj_heap.get_class_mut(cbool_class).expect("CBool").module = Some(module);
        self.obj_heap.get_class_mut(cpointer_class).expect("CPointer").module = Some(module);

        self.obj_heap.get_class_mut(library_class).expect("CDynLib").module = Some(module);
        self.obj_heap.get_class_mut(csymbol_class).expect("CSymbol").module = Some(module);
        self.obj_heap.get_class_mut(cfunction_class).expect("CFunction").module = Some(module);
        self.obj_heap.get_class_mut(ctype_class).expect("CType").module = Some(module);
        self.obj_heap.get_class_mut(struct_instance_class).expect("CStruct").module = Some(module);

        Ok(module)
    }
}
