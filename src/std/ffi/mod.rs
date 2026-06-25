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

use super::ModuleBuilder;
use crate::vm::{RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle};
use types::CType;

// ===========================================================================
// Module factory
// ===========================================================================

impl VirtualMachine {
    pub(crate) fn create_ffi_module(&mut self) -> RuntimeResult<ObjectHandle> {
        let mut m = ModuleBuilder::new(&mut self.obj_heap, "ffi");

        // ---- main classes ----
        m.define_class("CDynLib", |class| {
            class.method("__new__", NativeFunction::var(library::CDynLib::__new__));
            class.method("symbol", NativeFunction::a2(library::CDynLib::symbol));
            class.method("bind", NativeFunction::a4(library::CDynLib::bind));
        });

        m.define_class("CSymbol", |class| {
            class.method("__new__", NativeFunction::var(library::CSymbol::__new__));
        });

        m.define_class("CFunction", |class| {
            class.method("__new__", NativeFunction::var(function::CFunction::__new__));
            class.method("__call__", NativeFunction::var(function::CFunction::__call__));
        });

        let ctype_class = m.define_class("CType", |class| {
            class.method("__new__", NativeFunction::var(types::CType::__new__));
            class.method("__call__", NativeFunction::var(types::CType::__call__));
        });

        m.define_class("CStruct", |class| {
            class.method("__new__", NativeFunction::var(types::CStruct::__new__));
            class.method("__getattr__", NativeFunction::var(types::CStruct::__getattr__));
            class.method("__setattr__", NativeFunction::var(types::CStruct::__setattr__));
        });

        // ---- scalar wrapper classes ----
        macro_rules! define_scalar_class {
            ($name:ident) => {
                m.define_class(stringify!($name), |class| {
                    class.method("__new__", NativeFunction::var(types::scalar_new_error));
                    class.method("__getattr__", NativeFunction::var(types::scalar_getattr));
                    class.method("__setattr__", NativeFunction::var(types::scalar_setattr));
                });
            };
        }

        define_scalar_class!(CI8);
        define_scalar_class!(CUint8);
        define_scalar_class!(CI16);
        define_scalar_class!(CUint16);
        define_scalar_class!(CI32);
        define_scalar_class!(CUint32);
        define_scalar_class!(CI64);
        define_scalar_class!(CUint64);
        define_scalar_class!(CFloat);
        define_scalar_class!(CDouble);
        define_scalar_class!(CBool);
        define_scalar_class!(CPointer);

        // ---- export functions ----
        m.define_fn("call", NativeFunction::var(function::call));
        m.define_fn("define_struct", NativeFunction::var(types::define_struct));

        // ---- CType scalar singletons ----
        macro_rules! ctype_val {
            ($m:ident, $name:literal, $variant:ident) => {{
                let v = $m.alloc_instance(ctype_class, CType::$variant);
                $m.define_value($name, v);
            }};
        }
        ctype_val!(m, "c_int8", I8);
        ctype_val!(m, "c_int16", I16);
        ctype_val!(m, "c_int32", I32);
        ctype_val!(m, "c_int64", I64);
        ctype_val!(m, "c_uint8", U8);
        ctype_val!(m, "c_uint16", U16);
        ctype_val!(m, "c_uint32", U32);
        ctype_val!(m, "c_uint64", U64);
        ctype_val!(m, "c_float", F32);
        ctype_val!(m, "c_double", F64);
        ctype_val!(m, "c_bool", Bool);
        ctype_val!(m, "c_pointer", Pointer);
        ctype_val!(m, "c_cstring", CString);

        Ok(m.build())
    }
}
