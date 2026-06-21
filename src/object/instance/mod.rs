mod bool;
mod int;
mod float;
mod string;
mod list;
mod dict;
mod set;
mod bytes;

pub use bool::{ObjectBool, register_bool_builtins};
pub use int::{ObjectInt, register_int_builtins};
pub use float::{ObjectFloat, register_float_builtins};
pub use string::{ObjectString, ObjectStringIterator, register_string_builtins};
pub use list::{ObjectList, ObjectListIterator, register_list_builtins};
pub use dict::{ObjectDict, ObjectDictIterator, register_dict_builtins};
pub use set::{ObjectSet, ObjectSetIterator, register_set_builtins};

use std::any::Any;
use std::collections::HashMap;
use crate::{ShrString, ToShrString};
use crate::vm::{RuntimeResult, VirtualMachine};
use super::{Method, ObjectHandle, ObjectHeap};

pub struct ObjectClass {
    pub name: ShrString,
    pub methods: HashMap<ShrString, Method>,
    pub superclass: Option<ObjectHandle>,
}

impl ObjectClass {
    pub fn new(name: impl Into<ShrString>) -> Self {
        Self {
            name: name.into(),
            methods: HashMap::new(),
            superclass: None,
        }
    }
}

pub enum ObjectInstanceData {
    Nil,
    IterEnd,

    Bool(bool),
    Integer(i64),
    Float(f64),

    String(ShrString),
    StringIter(ObjectStringIterator),

    List(Vec<ObjectHandle>),
    ListIter(ObjectListIterator),

    Dict(HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>>),
    DictIter(ObjectDictIterator),

    Set(HashMap<u64, Vec<ObjectHandle>>),
    SetIter(ObjectSetIterator),

    Fields(HashMap<ShrString, ObjectHandle>),

    Native(NativeData),
}

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

/// Trait for native Rust data stored inside a Taro object via
/// [`NativeData`].  Implies `Any` so the concrete type can be recovered
/// safely with [`NativeData::downcast_ref`] / [`NativeData::downcast_mut`].
///
/// `Send + Sync` are required because `Object` is stored in `LazyLock`
/// statics.  The VM is single-threaded, so these bounds are harmless.
pub trait ToNativeData: Any + Send + Sync {
    /// Called during GC marking.  Override to mark any [`ObjectHandle`]
    /// references held by this native data.
    fn mark_inner_object(&self, _heap: &mut ObjectHeap) {}
}

impl dyn ToNativeData {
    pub fn downcast_ref<T: ToNativeData>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
    pub fn downcast_mut<T: ToNativeData>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut::<T>()
    }
}

/// Type-erased native Rust value stored inline in a Taro object.
pub struct NativeData {
    data: Box<dyn ToNativeData>,
}

impl NativeData {
    /// Create a [`NativeData`] from any type implementing [`ToNativeData`].
    pub fn new<T: ToNativeData>(data: T) -> Self {
        NativeData { data: Box::new(data) }
    }

    /// Call the GC trace callback (if any) to mark embedded handles.
    pub fn mark_inner_object(&self, heap: &mut ObjectHeap) {
        self.data.mark_inner_object(heap);
    }

    /// Downcast to a shared reference of the stored type.
    /// Returns `None` if `T` doesn't match the concrete type.
    pub fn downcast_ref<T: ToNativeData>(&self) -> Option<&T> {
        self.data.as_ref().downcast_ref::<T>()
    }

    /// Downcast to a mutable reference of the stored type.
    /// Returns `None` if `T` doesn't match the concrete type.
    pub fn downcast_mut<T: ToNativeData>(&mut self) -> Option<&mut T> {
        self.data.as_mut().downcast_mut::<T>()
    }
}

pub struct ObjectInstance {
    pub class: ObjectHandle,
    pub data: ObjectInstanceData,
}

impl ObjectInstance {
    pub fn new(class: ObjectHandle, data: ObjectInstanceData) -> Self {
        Self {
            class,
            data,
        }
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