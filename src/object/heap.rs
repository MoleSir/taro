use std::{collections::HashMap, sync::LazyLock};

use crate::{Chunk, ShrString};
use super::{NativeFunction, Method, Object, ObjectBoundMethod, ObjectNativeFn, ObjectClass, ObjectClosure, ObjectFunction, ObjectInstance, ObjectInstanceData, ObjectUpvalue, register_int_builtins, register_float_builtins, register_bool_builtins, register_string_builtins, register_list_builtins, register_dict_builtins, register_set_builtins};

/// Static nil object — backing for `ObjectHandle::NIL`.
static NIL_OBJECT: LazyLock<Object> = LazyLock::new(|| {
    Object::Instance(ObjectInstance::new(ObjectHandle::NIL, ObjectInstanceData::Nil))
});

/// Static IterEnd object — backing for `ObjectHandle::ITER_END`.
static ITER_END_OBJECT: LazyLock<Object> = LazyLock::new(|| {
    Object::Instance(ObjectInstance::new(ObjectHandle::ITER_END, ObjectInstanceData::IterEnd))
});

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub usize);

/// Sentinel handle representing nil — no heap allocation needed.
impl ObjectHandle {
    pub const NIL: Self = ObjectHandle(0);
    pub const ITER_END: Self = ObjectHandle(1);

    pub fn is_nil(self) -> bool { self.0 == 0 }
    pub fn is_iter_end(self) -> bool { self.0 == 1 }
}

pub struct ObjectHeap {
    objects: Vec<Option<Object>>,
    marked: Vec<bool>,
    free_slots: Vec<usize>,
    gray_stack: Vec<ObjectHandle>,
    pub bytes_allocated: usize,

    // ---- interning caches ----
    /// Small-integer cache (-5..256) so arithmetic doesn't allocate every time.
    int_cache: HashMap<i64, ObjectHandle>,
    /// String-object handle cache keyed by ShrString.
    string_cache: HashMap<ShrString, ObjectHandle>,

    /// builtin class
    pub nil_class: ObjectHandle,
    pub int_class: ObjectHandle,
    pub float_class: ObjectHandle,
    pub bool_class: ObjectHandle,
    pub string_class: ObjectHandle,
    pub list_class: ObjectHandle,
    pub dict_class: ObjectHandle,
    pub set_class: ObjectHandle,
    pub module_class: ObjectHandle,
    /// Class handle for `net.Socket` — stored here so `Server.accept()` can
    /// create new Socket instances without needing access to the net module.
    pub socket_class: ObjectHandle,

    /// Singleton instances for `true` and `false` so repeated use of
    /// boolean literals doesn't allocate.
    pub true_instance: ObjectHandle,
    pub false_instance: ObjectHandle,

    /// Class handles for built-in iterator types.
    pub list_iter_class: ObjectHandle,
    pub string_iter_class: ObjectHandle,
    pub dict_iter_class: ObjectHandle,
    pub set_iter_class: ObjectHandle,
}

impl ObjectHeap {
    pub fn new() -> Self {
        // Slots 0 and 1 are reserved for nil and IterEnd sentinels.
        let objects = vec![None, None];
        let marked = vec![false, false];

        let mut heap = Self {
            objects,
            marked,
            free_slots: Vec::new(),
            gray_stack: Vec::new(),
            bytes_allocated: 0,
            int_cache: HashMap::new(),
            string_cache: HashMap::new(),
            nil_class: ObjectHandle::NIL,
            int_class: ObjectHandle::NIL,
            float_class: ObjectHandle::NIL,
            bool_class: ObjectHandle::NIL,
            string_class: ObjectHandle::NIL,
            list_class: ObjectHandle::NIL,
            dict_class: ObjectHandle::NIL,
            set_class: ObjectHandle::NIL,
            module_class: ObjectHandle::NIL,
            socket_class: ObjectHandle::NIL,
            true_instance: ObjectHandle::NIL,
            false_instance: ObjectHandle::NIL,
            list_iter_class: ObjectHandle::NIL,
            string_iter_class: ObjectHandle::NIL,
            dict_iter_class: ObjectHandle::NIL,
            set_iter_class: ObjectHandle::NIL,
        };

        heap.nil_class = heap.alloc_class("Nil");
        heap.int_class = heap.alloc_class("Int");
        heap.float_class = heap.alloc_class("Float");
        heap.bool_class = heap.alloc_class("Bool");
        heap.string_class = heap.alloc_class("String");
        heap.list_class = heap.alloc_class("List");
        heap.dict_class = heap.alloc_class("Dict");
        heap.set_class = heap.alloc_class("Set");
        heap.module_class = heap.alloc_class("Module");
        heap.list_iter_class = heap.alloc_class("ListIterator");
        heap.string_iter_class = heap.alloc_class("StringIterator");
        heap.dict_iter_class = heap.alloc_class("DictIterator");
        heap.set_iter_class = heap.alloc_class("SetIterator");

        // Allocate singleton bool instances (after bool_class exists).
        heap.true_instance = heap.alloc_instance(heap.bool_class, ObjectInstanceData::Bool(true));
        heap.false_instance = heap.alloc_instance(heap.bool_class, ObjectInstanceData::Bool(false));

        // Register built-in class magic methods during heap init.
        register_int_builtins(&mut heap);
        register_float_builtins(&mut heap);
        register_bool_builtins(&mut heap);
        register_string_builtins(&mut heap);
        register_list_builtins(&mut heap);
        register_dict_builtins(&mut heap);
        register_set_builtins(&mut heap);

        heap
    }
}

impl ObjectHeap {
    // ================================================================================== //
    //           Alloc — convenience helpers
    // ================================================================================== //

    pub fn alloc_closure(&mut self, function: ObjectHandle) -> ObjectHandle {
        let obj = ObjectClosure::new(function);
        self.alloc(obj)
    }

    pub fn alloc_function(
        &mut self,
        name: impl Into<ShrString>,
        arity: usize,
        required_arity: usize,
        param_names: Vec<ShrString>,
        defaults: Vec<ObjectHandle>,
        chunk: Chunk,
    ) -> ObjectHandle {
        let obj = ObjectFunction::new(name, arity, required_arity, param_names, defaults, chunk);
        self.alloc(obj)
    }

    pub fn alloc_upvalue(&mut self, location: Option<usize>) -> ObjectHandle {
        let obj = ObjectUpvalue { location, closed: ObjectHandle::NIL, next: None };
        self.alloc(obj)
    }

    pub fn alloc_native_fn(&mut self, name: impl Into<ShrString>, function: impl Into<NativeFunction>) -> ObjectHandle {
        let obj = ObjectNativeFn::new(name, function.into());
        self.alloc(obj)
    }

    pub fn alloc_class(&mut self, name: impl Into<ShrString>) -> ObjectHandle {
        let obj = ObjectClass::new(name);
        self.alloc(obj)
    }

    pub fn alloc_instance(&mut self, class: ObjectHandle, data: ObjectInstanceData) -> ObjectHandle {
        let obj = ObjectInstance::new(class, data);
        self.alloc(obj)
    }

    pub fn alloc_bound_method(&mut self, receiver: ObjectHandle, method: Method) -> ObjectHandle {
        let obj = ObjectBoundMethod::new(receiver, method);
        self.alloc(obj)
    }

    #[inline]
    pub fn alloc_fields_instance(&mut self, class: ObjectHandle, fields: HashMap<ShrString, ObjectHandle>) -> ObjectHandle {
        self.alloc_instance(class, ObjectInstanceData::Fields(fields))
    }

    #[inline]
    pub fn alloc_bool_instance(&mut self, v: bool) -> ObjectHandle {
        if v { self.true_instance } else { self.false_instance }
    }

    pub fn alloc_integer_instance(&mut self, v: i64) -> ObjectHandle {
        // Small-integer interning: -5..256
        if (-5..=256).contains(&v) {
            if let Some(&handle) = self.int_cache.get(&v) {
                return handle;
            }
            let handle = self.alloc_instance(self.int_class, ObjectInstanceData::Integer(v));
            self.int_cache.insert(v, handle);
            return handle;
        }
        self.alloc_instance(self.int_class, ObjectInstanceData::Integer(v))
    }

    #[inline]
    pub fn alloc_float_instance(&mut self, v: f64) -> ObjectHandle {
        self.alloc_instance(self.float_class, ObjectInstanceData::Float(v))
    }

    pub fn alloc_string_instance(&mut self, s: ShrString) -> ObjectHandle {
        if let Some(&handle) = self.string_cache.get(&s) {
            return handle;
        }
        let handle = self.alloc_instance(self.string_class, ObjectInstanceData::String(s.clone()));
        self.string_cache.insert(s, handle);
        handle
    }

    #[inline]
    pub fn alloc_list_instance(&mut self, items: Vec<ObjectHandle>) -> ObjectHandle {
        self.alloc_instance(self.list_class, ObjectInstanceData::List(items))
    }

    #[inline]
    pub fn alloc_dict_instance(&mut self, items: HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>>) -> ObjectHandle {
        self.alloc_instance(self.dict_class, ObjectInstanceData::Dict(items))
    }

    #[inline]
    pub fn alloc_set_instance(&mut self, items: HashMap<u64, Vec<ObjectHandle>>) -> ObjectHandle {
        self.alloc_instance(self.set_class, ObjectInstanceData::Set(items))
    }

    /// Register a native method on a builtin class.
    pub fn register_native_method(&mut self, class_handle: ObjectHandle, name: &'static str, function: NativeFunction) {
        let handle = self.alloc_native_fn(name, function);
        let class = self.get_class_mut(class_handle).expect("class");
        class.methods.insert(name.into(), Method::Native(handle));
    }

    fn alloc(&mut self, obj: impl Into<Object>) -> ObjectHandle {
        let obj = obj.into();
        self.bytes_allocated += std::mem::size_of::<Object>();
        let handle = if let Some(index) = self.free_slots.pop() {
            self.objects[index] = Some(obj);
            self.marked[index] = false;
            ObjectHandle(index)
        } else {
            let index = self.objects.len();
            self.objects.push(Some(obj));
            self.marked.push(false);
            ObjectHandle(index)
        };

        #[cfg(feature = "debug-gc")]
        println!("Allocated {} at {:?}", self.bytes_allocated, handle);

        handle
    }

    pub fn alloc_nil(&mut self) -> ObjectHandle {
        // Nil is a singleton — always handle 0, backed by the static NIL_OBJECT.
        ObjectHandle::NIL
    }
}

macro_rules! impl_getters {
    ($name:ident, $ty:ty) => {
        paste::paste! {
            #[inline]
            pub fn [<get_ $name>](&self, handle: ObjectHandle) -> Option<&$ty> {
                self.get(handle).[<as_ $name>]()
            }

            #[inline]
            pub fn [<get_ $name _mut>](&mut self, handle: ObjectHandle) -> Option<&mut $ty> {
                self.get_mut(handle).[<as_ $name _mut>]()
            }
        }
    };
}

impl ObjectHeap {
    // ================================================================================== //
    //           Get
    // ================================================================================== //

    pub fn get(&self, handle: ObjectHandle) -> &Object {
        if handle.is_nil() {
            return &NIL_OBJECT;
        }
        if handle.is_iter_end() {
            return &ITER_END_OBJECT;
        }
        self.objects[handle.0].as_ref().expect("Dangling handle accessed!")
    }

    pub fn get_mut(&mut self, handle: ObjectHandle) -> &mut Object {
        if handle.is_nil() {
            panic!("Cannot mutate nil object");
        }
        if handle.is_iter_end() {
            panic!("Cannot mutate IterEnd object");
        }
        self.objects[handle.0].as_mut().expect("Dangling handle accessed!")
    }

    impl_getters!(function, ObjectFunction);
    impl_getters!(native_fn, ObjectNativeFn);
    impl_getters!(closure, ObjectClosure);
    impl_getters!(upvalue, ObjectUpvalue);
    impl_getters!(instance, ObjectInstance);
    impl_getters!(class, ObjectClass);
    impl_getters!(bound_method, ObjectBoundMethod);
}

macro_rules! impl_instance_data_getter {
    ($name:ident, $variant:ident, $ty:ty, $label:literal) => {
        paste::paste! {
            /// Return a reference to the inner data — panics on type mismatch
            /// (intended for use in tests and internal code that has already
            /// verified the type).
            #[doc = "Return a reference to the inner `" $label "` data."]
            pub fn [<get_ $name _instance>](&self, handle: ObjectHandle) -> Option<&$ty> {
                let inst = self.get_instance(handle)?;
                match &inst.data {
                    ObjectInstanceData::$variant(v) => Some(v),
                    _ => None,
                }
            }

            #[doc = "Return a mutable reference to the inner `" $label "` data."]
            pub fn [<get_ $name _instance_mut>](&mut self, handle: ObjectHandle) -> Option<&mut $ty> {
                let inst = self.get_instance_mut(handle)?;
                match &mut inst.data {
                    ObjectInstanceData::$variant(v) => Some(v),
                    _ => None,
                }
            }
        }
    };
}

impl ObjectHeap {
    impl_instance_data_getter!(integer, Integer, i64, "integer");
    impl_instance_data_getter!(float, Float, f64, "float");
    impl_instance_data_getter!(bool, Bool, bool, "bool");
    impl_instance_data_getter!(string, String, ShrString, "string");
    impl_instance_data_getter!(list, List, Vec<ObjectHandle>, "list");
    impl_instance_data_getter!(dict, Dict, HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>>, "dict");
    impl_instance_data_getter!(set, Set, HashMap<u64, Vec<ObjectHandle>>, "set");
    impl_instance_data_getter!(fields, Fields, HashMap<ShrString, ObjectHandle>, "fields");

    /// Return a mutable reference to the native data stored in `handle`,
    /// downcast to `T`.  Returns `None` if the handle is not an Instance
    /// with `Native` data, or if the concrete type doesn't match `T`.
    pub fn get_native_mut<T: super::ToNativeData>(&mut self, handle: ObjectHandle) -> Option<&mut T> {
        let inst = self.get_instance_mut(handle)?;
        match &mut inst.data {
            ObjectInstanceData::Native(native) => native.downcast_mut::<T>(),
            _ => None,
        }
    }

    /// Return a shared reference to the native data stored in `handle`,
    /// downcast to `T`.
    pub fn get_native<T: super::ToNativeData>(&self, handle: ObjectHandle) -> Option<&T> {
        let inst = self.get_instance(handle)?;
        match &inst.data {
            ObjectInstanceData::Native(native) => native.downcast_ref::<T>(),
            _ => None,
        }
    }
}

impl ObjectHeap {
    // ================================================================================== //
    //           GC
    // ================================================================================== //

    pub fn collect_garbage(&mut self) {
        // Mark interning caches so cached objects don't get swept.
        let int_handles: Vec<ObjectHandle> = self.int_cache.values().copied().collect();
        let str_handles: Vec<ObjectHandle> = self.string_cache.values().copied().collect();
        for handle in int_handles {
            self.mark_object(handle);
        }
        for handle in str_handles {
            self.mark_object(handle);
        }
        self.trace_references();
        self.sweep();
    }

    pub fn mark_object(&mut self, handle: ObjectHandle) {
        if handle.is_nil() || handle.is_iter_end() {
            return;
        }
        let index = handle.0;
        if self.marked[index] {
            return;
        }

        #[cfg(feature = "debug-gc")]
        println!("Marking {:?}", handle);

        self.marked[index] = true;
        self.gray_stack.push(handle);
    }

    pub fn trace_references(&mut self) {
        while let Some(handle) = self.gray_stack.pop() {
            self.blacken_object(handle);
        }
    }

    fn blacken_object(&mut self, handle: ObjectHandle) {
        #[cfg(feature = "debug-gc")]
        println!("Blackening {:?}", handle);

        let object = self.objects[handle.0].take();
        if let Some(ref obj) = object {
            match obj {
                Object::Function(function) => {
                    for &const_handle in function.chunk.constants.iter() {
                        self.mark_object(const_handle);
                    }
                    for &default_handle in &function.defaults {
                        self.mark_object(default_handle);
                    }
                }
                Object::Closure(closure) => {
                    self.mark_object(closure.function);
                    for &upvalue in &closure.upvalues {
                        self.mark_object(upvalue);
                    }
                }
                Object::Upvalue(upvalue) => {
                    self.mark_object(upvalue.closed);
                    if let Some(next) = upvalue.next {
                        self.mark_object(next);
                    }
                }
                Object::Instance(instance) => {
                    self.mark_object(instance.class);
                    match &instance.data {
                        ObjectInstanceData::Nil | ObjectInstanceData::IterEnd | ObjectInstanceData::Bool(_) | ObjectInstanceData::Integer(_) | ObjectInstanceData::Float(_) => {}
                        ObjectInstanceData::Native(native) => {
                            native.mark_inner_object(self);
                        }
                        ObjectInstanceData::String(_s) => {
                            // ShrString is internally Arc'd — no ObjectHandle refs
                        }
                        ObjectInstanceData::List(items) => {
                            for &item in items {
                                self.mark_object(item);
                            }
                        }
                        ObjectInstanceData::Dict(entries) => {
                            for bucket in entries.values() {
                                for &(k, v) in bucket {
                                    self.mark_object(k);
                                    self.mark_object(v);
                                }
                            }
                        }
                        ObjectInstanceData::Set(entries) => {
                            for bucket in entries.values() {
                                for v in bucket {
                                    self.mark_object(*v);
                                }
                            }
                        }
                        ObjectInstanceData::Fields(fields) => {
                            for &field_handle in fields.values() {
                                self.mark_object(field_handle);
                            }
                        }
                    }
                }
                Object::BoundMethod(bound) => {
                    match bound.method {
                        Method::User(method_handle) => self.mark_object(method_handle),
                        Method::Native(handle) => self.mark_object(handle),
                    }
                    self.mark_object(bound.receiver);
                }
                Object::Class(class) => {
                    if let Some(superclass) = class.superclass {
                        self.mark_object(superclass);
                    }
                    for method in class.methods.values() {
                        match method {
                            Method::User(method_handle) => self.mark_object(*method_handle),
                            Method::Native(handle) => self.mark_object(*handle),
                        }
                    }
                }
                Object::NativeFn(_) => {
                    // Native functions own no heap references.
                }
            }
        }

        self.objects[handle.0] = object;
    }

    pub fn sweep(&mut self) {
        // Skip slots 0 (nil) and 1 (IterEnd) — they are reserved sentinels.
        for i in 2..self.objects.len() {
            if self.objects[i].is_some() {
                if self.marked[i] {
                    self.marked[i] = false;
                } else {
                    #[cfg(feature = "debug-gc")]
                    println!("Sweeping object at {}", i);

                    // Remove from interning caches.
                    let swept_handle = ObjectHandle(i);
                    self.int_cache.retain(|_, &mut h| h != swept_handle);
                    self.string_cache.retain(|_, &mut h| h != swept_handle);

                    self.objects[i] = None;
                    self.free_slots.push(i);
                    self.bytes_allocated -= std::mem::size_of::<Object>();
                }
            }
        }
    }
}
