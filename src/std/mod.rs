mod ffi;
mod fs;
mod json;
mod math;
mod net;
mod os;
mod random;
mod time;

use std::collections::HashMap;
use crate::{NativeFunction, ObjectHandle, ObjectHeap, ObjectInstanceData, ShrString};

// ============================================================================
// ModuleBuilder — constructs std modules with less boilerplate
// ============================================================================

/// Builder for constructing a standard-library module.
///
/// Eliminates the boilerplate of calling `alloc_class`, `register_native_method`,
/// and `exports.insert` separately — the class name is written once and
/// automatically exported.
///
/// # Example
///
/// ```ignore
/// let mut m = ModuleBuilder::new(&mut self.obj_heap, "net");
/// m.define_class("Socket", |class| {
///     class.method("connect", NativeFunction::var(Socket::connect));
///     class.method("send", NativeFunction::a2(Socket::send));
/// });
/// let module = m.build();
/// ```
pub struct ModuleBuilder<'a> {
    heap: &'a mut ObjectHeap,
    module: ObjectHandle,
    exports: HashMap<ShrString, ObjectHandle>,
}

impl<'a> ModuleBuilder<'a> {
    pub fn new(heap: &'a mut ObjectHeap, name: impl Into<ShrString>) -> Self {
        let module = heap.alloc_module(name);
        Self { heap, module, exports: HashMap::new() }
    }

    /// Define a class, register its methods, and export it — all in one call.
    pub fn define_class(&mut self, name: &'static str, f: impl FnOnce(&mut ClassMethods<'_>)) -> ObjectHandle {
        let class = self.heap.alloc_class(name, self.module);
        f(&mut ClassMethods { heap: &mut *self.heap, class });
        self.exports.insert(ShrString::new_str(name), class);
        class
    }

    /// Define and export a native function.
    pub fn define_fn(&mut self, name: &'static str, f: impl Into<NativeFunction>) -> ObjectHandle {
        let handle = self.heap.alloc_native_fn(name, f);
        self.exports.insert(ShrString::new_str(name), handle);
        handle
    }

    /// Export an arbitrary value under `name` (e.g. a builtin instance or class handle).
    pub fn define_value(&mut self, name: &'static str, value: ObjectHandle) {
        self.exports.insert(ShrString::new_str(name), value);
    }

    /// Allocate an instance on the heap while the builder holds the module.
    pub fn alloc_instance<D: ObjectInstanceData>(&mut self, class: ObjectHandle, data: D) -> ObjectHandle {
        self.heap.alloc_instance(class, data)
    }

    /// Consume the builder and return the completed module.
    pub fn build(self) -> ObjectHandle {
        self.heap.get_module_mut(self.module).expect("module").fields = self.exports;
        self.module
    }
}

/// Scoped helper for registering methods on a class being defined via
/// [`ModuleBuilder::define_class`].
pub struct ClassMethods<'a> {
    heap: &'a mut ObjectHeap,
    class: ObjectHandle,
}

impl<'a> ClassMethods<'a> {
    pub fn method(&mut self, name: &'static str, f: impl Into<NativeFunction>) {
        self.heap.register_native_method(self.class, name, f.into());
    }
}