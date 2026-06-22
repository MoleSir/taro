mod bool;
mod bytes;
mod dict;
mod float;
mod int;
mod list;
mod set;
mod string;

pub use bool::{ObjectBool, register_bool_builtins};
pub use bytes::{ObjectBytes, ObjectBytesIterator, register_bytes_builtins};
pub use dict::{ObjectDict, ObjectDictIterator, register_dict_builtins};
pub use float::{ObjectFloat, register_float_builtins};
pub use int::{ObjectInt, register_int_builtins};
pub use list::{ObjectList, ObjectListIterator, register_list_builtins};
pub use set::{ObjectSet, ObjectSetIterator, register_set_builtins};
pub use string::{ObjectString, ObjectStringIterator, register_string_builtins};

use super::{Method, ObjectHandle, ObjectHeap};
use crate::vm::{RuntimeResult, VirtualMachine};
use crate::{ShrString, ToShrString};
use std::any::Any;
use std::collections::HashMap;

// ==========================================================================
//  ObjectInstanceData trait
// ==========================================================================

/// Trait for all instance data stored inside a Taro object.
///
/// Types implementing this trait can be stored in `ObjectInstance::data` as
/// `Box<dyn ObjectInstanceData>`, and recovered via `as_any_ref().downcast_ref::<T>()`.
///
/// `Send + Sync + Any` bounds are required because `Object` is stored in
/// `LazyLock` statics.  The VM is single-threaded, so these bounds are harmless.
pub trait ObjectInstanceData: Any + Send + Sync {
    /// Called during GC marking.  Override to mark any [`ObjectHandle`]
    /// references held by this data.
    fn mark_references(&self, _heap: &mut ObjectHeap) {}

    /// Human-readable type name for error messages and `type()`.
    fn type_name(&self) -> &'static str;

    /// For downcasting back to concrete type.
    fn as_any_ref(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Macro to reduce boilerplate for simple `ObjectInstanceData` implementations
/// that contain no `ObjectHandle` references.
#[macro_export]
macro_rules! impl_object_instance_data {
    ($ty:ty, $type_name:expr) => {
        impl $crate::object::ObjectInstanceData for $ty {
            fn mark_references(&self, _heap: &mut $crate::object::ObjectHeap) {}
            fn type_name(&self) -> &'static str {
                $type_name
            }
            fn as_any_ref(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    };
}

// ==========================================================================
//  Concrete ObjectInstanceData types defined in this module
// ==========================================================================

/// Sentinel: nil.
pub struct ObjectNil;
impl_object_instance_data!(ObjectNil, "nil");

/// Sentinel: IterEnd (iteration sentinel).
pub struct ObjectIterEnd;
impl_object_instance_data!(ObjectIterEnd, "IterEnd");

/// User-defined class instances (field storage).
pub struct ObjectFields {
    pub fields: HashMap<ShrString, ObjectHandle>,
}

impl ObjectFields {
    pub fn new() -> Self {
        Self { fields: HashMap::new() }
    }
}

impl Default for ObjectFields {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectInstanceData for ObjectFields {
    fn mark_references(&self, heap: &mut ObjectHeap) {
        for &handle in self.fields.values() {
            heap.mark_object(handle);
        }
    }
    fn type_name(&self) -> &'static str {
        "instance"
    }
    fn as_any_ref(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ==========================================================================
//  ObjectClass
// ==========================================================================

pub struct ObjectClass {
    pub name: ShrString,
    pub methods: HashMap<ShrString, Method>,
    pub superclass: Option<ObjectHandle>,
    /// Owning module for classes created as part of a native std module.
    /// `None` for builtin classes and user-script classes.
    pub module: Option<ObjectHandle>,
}

impl ObjectClass {
    pub fn new(name: impl Into<ShrString>) -> Self {
        Self { name: name.into(), methods: HashMap::new(), superclass: None, module: None }
    }
}

// ==========================================================================
//  ObjectInstance
// ==========================================================================

pub struct ObjectInstance {
    pub class: ObjectHandle,
    pub data: Box<dyn ObjectInstanceData>,
}

impl ObjectInstance {
    pub fn new(class: ObjectHandle, data: Box<dyn ObjectInstanceData>) -> Self {
        Self { class, data }
    }

    pub fn get_data_mut<T: ObjectInstanceData>(&mut self) -> Option<&mut T> {
        self.data.as_any_mut().downcast_mut()
    }

    pub fn get_data_ref<T: ObjectInstanceData>(&self) -> Option<&T> {
        self.data.as_any_ref().downcast_ref()
    }
}

// ==========================================================================
//  IntoObjectInstance — convert Rust values into heap-allocated instances
// ==========================================================================

pub trait IntoObjectInstance {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle;
}

impl IntoObjectInstance for ShrString {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle {
        vm.obj_heap.alloc_string_instance(self)
    }
}

impl IntoObjectInstance for String {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle {
        vm.obj_heap.alloc_string_instance(self.to_shrstring())
    }
}

impl IntoObjectInstance for &'static str {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle {
        vm.obj_heap.alloc_string_instance(self.to_shrstring())
    }
}

impl IntoObjectInstance for bool {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle {
        vm.obj_heap.alloc_bool_instance(self)
    }
}

impl IntoObjectInstance for i64 {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle {
        vm.obj_heap.alloc_integer_instance(self)
    }
}

impl IntoObjectInstance for f64 {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle {
        vm.obj_heap.alloc_float_instance(self)
    }
}

impl IntoObjectInstance for Vec<ObjectHandle> {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle {
        vm.obj_heap.alloc_list_instance(self)
    }
}

impl IntoObjectInstance for Vec<u8> {
    fn into_object_instance(self, vm: &mut VirtualMachine) -> ObjectHandle {
        vm.obj_heap.alloc_bytes_instance(self)
    }
}

impl IntoObjectInstance for ObjectHandle {
    fn into_object_instance(self, _vm: &mut VirtualMachine) -> ObjectHandle {
        self
    }
}

impl IntoObjectInstance for () {
    fn into_object_instance(self, _vm: &mut VirtualMachine) -> ObjectHandle {
        ObjectHandle::NIL
    }
}

// ── FromObjectInstance: extract Rust values from ObjectHandle ────────────

/// Types that can be extracted from an [`ObjectHandle`] via the VM.
///
/// The lifetime parameter `'a` connects the returned reference (if any) to
/// the `vm` borrow — for `&'a ShrString` this avoids a clone; for owned types
/// (`ShrString`, `i64`, …) the lifetime is elided.
pub trait FromObjectInstance<'a>: Sized {
    fn from_object_instance(vm: &'a VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self>;
}

/// Borrowed string — zero-cost, no clone.
impl<'a> FromObjectInstance<'a> for &'a ShrString {
    fn from_object_instance(vm: &'a VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_string_instance(handle)
    }
}

impl<'a> FromObjectInstance<'a> for &'a Vec<ObjectHandle> {
    fn from_object_instance(vm: &'a VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_list_instance(handle)
    }
}

impl<'a> FromObjectInstance<'a> for &'a std::collections::HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>> {
    fn from_object_instance(vm: &'a VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_dict_instance(handle)
    }
}

impl<'a> FromObjectInstance<'a> for &'a std::collections::HashMap<u64, Vec<ObjectHandle>> {
    fn from_object_instance(vm: &'a VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_set_instance(handle)
    }
}

impl<'a> FromObjectInstance<'a> for &'a Vec<u8> {
    fn from_object_instance(vm: &'a VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_bytes_instance(handle)
    }
}

impl FromObjectInstance<'_> for ShrString {
    fn from_object_instance(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_string_instance(handle).cloned()
    }
}

impl FromObjectInstance<'_> for i64 {
    fn from_object_instance(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_integer_instance(handle).copied()
    }
}

impl FromObjectInstance<'_> for f64 {
    fn from_object_instance(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_float_instance(handle).copied()
    }
}

impl FromObjectInstance<'_> for bool {
    fn from_object_instance(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        vm.get_bool_instance(handle).copied()
    }
}

impl FromObjectInstance<'_> for ObjectHandle {
    fn from_object_instance(_vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Self> {
        Ok(handle)
    }
}

// ==========================================================================
//  Method-generation macros
// ==========================================================================
//
// Generic macros that eliminate the boilerplate of extracting arguments from
// ObjectHandle and wrapping the return value.  The compiler picks the right
// extraction / wrapping via `FromObjectInstance` / `IntoObjectInstance`.
#[macro_export]
macro_rules! native_a1 {
    ($name:ident, $a0:ident: $A0:ty, $body:block) => {
        pub fn $name(vm: &mut $crate::vm::VirtualMachine, $a0: $crate::ObjectHandle) -> $crate::vm::RuntimeResult<$crate::ObjectHandle> {
            let $a0 = <$A0 as $crate::object::FromObjectInstance<'_>>::from_object_instance(vm, $a0)?;
            let _result = $body;
            Ok($crate::object::IntoObjectInstance::into_object_instance(_result, vm))
        }
    };
}

/// Generate a native a2 method (receiver + 1 argument).
#[macro_export]
macro_rules! native_a2 {
    ($name:ident, $a0:ident: $A0:ty, $a1:ident: $A1:ty, $body:block) => {
        pub fn $name(
            vm: &mut $crate::vm::VirtualMachine,
            $a0: $crate::ObjectHandle,
            $a1: $crate::ObjectHandle,
        ) -> $crate::vm::RuntimeResult<$crate::ObjectHandle> {
            let $a0 = <$A0 as $crate::object::FromObjectInstance<'_>>::from_object_instance(vm, $a0)?;
            let $a1 = <$A1 as $crate::object::FromObjectInstance<'_>>::from_object_instance(vm, $a1)?;
            let _result = $body;
            Ok($crate::object::IntoObjectInstance::into_object_instance(_result, vm))
        }
    };
}

/// Generate a native a3 method (receiver + 2 arguments).
#[macro_export]
macro_rules! native_a3 {
    ($name:ident, $a0:ident: $A0:ty, $a1:ident: $A1:ty, $a2:ident: $A2:ty, $body:block) => {
        pub fn $name(
            vm: &mut $crate::vm::VirtualMachine,
            $a0: $crate::ObjectHandle,
            $a1: $crate::ObjectHandle,
            $a2: $crate::ObjectHandle,
        ) -> $crate::vm::RuntimeResult<$crate::ObjectHandle> {
            let $a0 = <$A0 as $crate::object::FromObjectInstance<'_>>::from_object_instance(vm, $a0)?;
            let $a1 = <$A1 as $crate::object::FromObjectInstance<'_>>::from_object_instance(vm, $a1)?;
            let $a2 = <$A2 as $crate::object::FromObjectInstance<'_>>::from_object_instance(vm, $a2)?;
            let _result = $body;
            Ok($crate::object::IntoObjectInstance::into_object_instance(_result, vm))
        }
    };
}
