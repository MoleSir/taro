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
use crate::ShrString;
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
    /// Owning module for this class.  Every class belongs to exactly one module
    /// — builtin classes use `__main__`, user-defined classes use the current
    /// module, and native std classes use their owning module.
    pub module: ObjectHandle,
}

impl ObjectClass {
    pub fn new(name: impl Into<ShrString>, module: ObjectHandle) -> Self {
        Self { name: name.into(), methods: HashMap::new(), superclass: None, module }
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
