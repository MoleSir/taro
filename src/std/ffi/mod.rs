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
//! Color = ffi.struct_def(["uint8", "uint8", "uint8", "uint8"]);
//! c     = Color(255, 0, 0, 255);
//!
//! // Struct support — named fields
//! Vec3 = ffi.struct_def([["x", "float"], ["y", "float"], ["z", "float"]]);
//! v    = Vec3(1.0, 2.0, 3.0);
//! print(v.x);  // 1.0  — named field access via dot
//!
//! ffi.dlclose(lib);
//! ```

mod bound;
mod call;
mod library;
mod structs;
mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::vm::{RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle, ShrString};

// ===========================================================================
// Module factory
// ===========================================================================

impl VirtualMachine {
    pub(crate) fn create_ffi_module(&mut self) -> RuntimeResult<ObjectHandle> {
        let bound_fn_class = self.obj_heap.alloc_class("BoundFn");
        self.register_native_method(bound_fn_class, "__new__", NativeFunction::var(bound::BoundFn::__new__));
        self.register_native_method(bound_fn_class, "__call__", NativeFunction::var(bound::BoundFn::__call__));

        let struct_def_class = self.obj_heap.alloc_class("StructDef");
        self.register_native_method(struct_def_class, "__new__", NativeFunction::var(structs::StructDef::__new__));
        self.register_native_method(struct_def_class, "__init__", NativeFunction::var(structs::StructDef::__init__));
        self.register_native_method(struct_def_class, "__call__", NativeFunction::var(structs::StructDef::__call__));

        let struct_instance_class = self.obj_heap.alloc_class("Struct");
        self.register_native_method(struct_instance_class, "__new__", NativeFunction::var(structs::Struct::__new__));
        self.register_native_method(struct_instance_class, "__getattr__", NativeFunction::var(structs::Struct::__getattr__));
        self.register_native_method(struct_instance_class, "__setattr__", NativeFunction::var(structs::Struct::__setattr__));

        // ---- export functions ----
        let dlopen_fn = self.obj_heap.alloc_native_fn("dlopen", NativeFunction::a1(library::dlopen));
        let dlsym_fn = self.obj_heap.alloc_native_fn("dlsym", NativeFunction::a2(library::dlsym));
        let dlclose_fn = self.obj_heap.alloc_native_fn("dlclose", NativeFunction::a1(library::dlclose));
        let call_fn = self.obj_heap.alloc_native_fn("call", NativeFunction::var(call::ffi_call));
        let bind_fn = self.obj_heap.alloc_native_fn("bind", NativeFunction::a4(bound::bind));

        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("dlopen"), dlopen_fn);
        exports.insert(ShrString::new_str("dlsym"), dlsym_fn);
        exports.insert(ShrString::new_str("dlclose"), dlclose_fn);
        exports.insert(ShrString::new_str("call"), call_fn);
        exports.insert(ShrString::new_str("bind"), bind_fn);

        // Internal classes are not user-visible, but are stored as exports
        // so module-level functions can resolve them via the module registry.
        exports.insert(ShrString::new_str("BoundFn"), bound_fn_class);
        exports.insert(ShrString::new_str("StructDef"), struct_def_class);
        exports.insert(ShrString::new_str("Struct"), struct_instance_class);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);

        // Back-link all internal classes to the module so method-level lookups
        // (StructDef.__call__, BoundFn.__call__) can find sibling classes.
        self.obj_heap.get_class_mut(bound_fn_class).expect("BoundFn").module = Some(module);
        self.obj_heap.get_class_mut(struct_def_class).expect("StructDef").module = Some(module);
        self.obj_heap.get_class_mut(struct_instance_class).expect("Struct").module = Some(module);

        Ok(module)
    }
}
