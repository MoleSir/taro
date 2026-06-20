mod bool;
mod int;
mod float;
mod string;
mod list;
mod dict;
mod set;
pub use bool::{ObjectBool, register_bool_builtins};
pub use int::{ObjectInt, register_int_builtins};
pub use float::{ObjectFloat, register_float_builtins};
pub use string::{ObjectString, ObjectStringIterator, register_string_builtins};
pub use list::{ObjectList, ObjectListIterator, register_list_builtins};
pub use dict::{ObjectDict, ObjectDictIterator, register_dict_builtins};
pub use set::{ObjectSet, ObjectSetIterator, register_set_builtins};

use std::any::Any;
use std::collections::HashMap;
use crate::ShrString;
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