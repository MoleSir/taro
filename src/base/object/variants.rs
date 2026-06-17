use std::any::Any;
use std::collections::HashMap;

use crate::{vm::{ExecuteResult, VirtualMachine}, ShrString};
use super::{ObjectHandle, ObjectHeap};

// ========================================================================== //
//                    Method (unified user + native)
// ========================================================================== //

/// A callable method — either a user-defined closure or a Rust native function.
#[derive(Copy, Clone)]
pub enum Method {
    /// User-defined method (closure handle, compiled from Taro source).
    User(ObjectHandle),
    /// Native method (handle to an `ObjectNativeFn`).
    Native(ObjectHandle),
}

// ========================================================================== //
//                    Function, NativeFn, Upvalue
// ========================================================================== //

pub struct ObjectFunction {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: ShrString,
}

impl ObjectFunction {
    pub fn new(name: impl Into<ShrString>, arity: usize, chunk: Chunk) -> Self {
        Self { arity, name: name.into(), chunk }
    }
}

// ---- Arity-specific native-function pointer types -----------------------

pub type NativeFn0 = fn(&mut VirtualMachine) -> ExecuteResult<ObjectHandle>;
pub type NativeFn1 = fn(&mut VirtualMachine, ObjectHandle) -> ExecuteResult<ObjectHandle>;
pub type NativeFn2 = fn(&mut VirtualMachine, ObjectHandle, ObjectHandle) -> ExecuteResult<ObjectHandle>;
pub type NativeFn3 = fn(&mut VirtualMachine, ObjectHandle, ObjectHandle, ObjectHandle) -> ExecuteResult<ObjectHandle>;
pub type NativeFn4 = fn(&mut VirtualMachine, ObjectHandle, ObjectHandle, ObjectHandle, ObjectHandle) -> ExecuteResult<ObjectHandle>;
pub type NativeFn5 = fn(&mut VirtualMachine, ObjectHandle, ObjectHandle, ObjectHandle, ObjectHandle, ObjectHandle) -> ExecuteResult<ObjectHandle>;
pub type NativeFnN = fn(&mut VirtualMachine, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle>;

/// Tagged union over native function arities.
///
/// The VM dispatch layer uses this to validate `arg_count` and extract typed
/// arguments before calling the inner function, so individual native functions
/// never deal with raw stack indices.
#[derive(Clone, Copy)]
pub enum NativeFunction {
    Arity0(NativeFn0),
    Arity1(NativeFn1),
    Arity2(NativeFn2),
    Arity3(NativeFn3),
    Arity4(NativeFn4),
    Arity5(NativeFn5),
    Variadic(NativeFnN),
}

// `From` impls allow `.into()` on already-coerced function pointers.
impl From<NativeFn0> for NativeFunction { fn from(f: NativeFn0) -> Self { NativeFunction::Arity0(f) } }
impl From<NativeFn1> for NativeFunction { fn from(f: NativeFn1) -> Self { NativeFunction::Arity1(f) } }
impl From<NativeFn2> for NativeFunction { fn from(f: NativeFn2) -> Self { NativeFunction::Arity2(f) } }
impl From<NativeFn3> for NativeFunction { fn from(f: NativeFn3) -> Self { NativeFunction::Arity3(f) } }
impl From<NativeFn4> for NativeFunction { fn from(f: NativeFn4) -> Self { NativeFunction::Arity4(f) } }
impl From<NativeFn5> for NativeFunction { fn from(f: NativeFn5) -> Self { NativeFunction::Arity5(f) } }
impl From<NativeFnN> for NativeFunction { fn from(f: NativeFnN) -> Self { NativeFunction::Variadic(f) } }

// Explicit constructors — these trigger function-item → function-pointer
// coercion because the parameter type is concrete (not generic).
impl NativeFunction {
    pub fn a0(f: NativeFn0) -> Self { NativeFunction::Arity0(f) }
    pub fn a1(f: NativeFn1) -> Self { NativeFunction::Arity1(f) }
    pub fn a2(f: NativeFn2) -> Self { NativeFunction::Arity2(f) }
    pub fn a3(f: NativeFn3) -> Self { NativeFunction::Arity3(f) }
    pub fn a4(f: NativeFn4) -> Self { NativeFunction::Arity4(f) }
    pub fn a5(f: NativeFn5) -> Self { NativeFunction::Arity5(f) }
    pub fn var(f: NativeFnN) -> Self { NativeFunction::Variadic(f) }
}

// NativeFunction contains only function pointers — safe to share across threads
// in our single-threaded VM.
unsafe impl Send for NativeFunction {}
unsafe impl Sync for NativeFunction {}

pub struct ObjectNativeFn {
    pub name: ShrString,
    pub function: NativeFunction,
}

impl ObjectNativeFn {
    pub fn new(name: impl Into<ShrString>, function: NativeFunction) -> Self {
        Self { name: name.into(), function }
    }
}

pub struct ObjectUpvalue {
    /// Stack slot index when the upvalue is still "open" (the local variable
    /// is alive on the stack).  Set to `None` once the variable goes out of
    /// scope and the upvalue is "closed" — the value has been moved into
    /// `closed`.
    pub location: Option<usize>,
    pub closed: ObjectHandle,
    /// Intrusive linked list: the next open upvalue that refers to the same
    /// stack slot (or to a slot below this one).  Used by the VM to find all
    /// upvalues that need to be closed when a local goes out of scope.
    pub next: Option<ObjectHandle>,
}

// ========================================================================== //
//                    Class, Instance
// ========================================================================== //

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
    
    List(Vec<ObjectHandle>),
    Dict(HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>>),
    Set(HashMap<u64, Vec<ObjectHandle>>),

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

// ========================================================================== //
//                    Closure
// ========================================================================== //

pub struct ObjectClosure {
    pub function: ObjectHandle,
    pub upvalues: Vec<ObjectHandle>,
}

impl ObjectClosure {
    pub fn new(function: ObjectHandle) -> Self {
        Self {
            function,
            upvalues: vec![],
        }
    }
}

// ========================================================================== //
//                    BoundMethod
// ========================================================================== //

pub struct ObjectBoundMethod {
    pub receiver: ObjectHandle,
    pub method: Method,
}

impl ObjectBoundMethod {
    pub fn new(receiver: ObjectHandle, method: Method) -> Self {
        Self { receiver, method }
    }
}

// Re-export Chunk for use in ObjectFunction.
use crate::Chunk;
