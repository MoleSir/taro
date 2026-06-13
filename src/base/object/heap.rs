use std::{collections::HashMap, sync::LazyLock};

use crate::{Chunk, ShrString};
use super::{BuiltinFn, Method, Object, ObjectBoundMethod, ObjectBuiltinFn, ObjectClass, ObjectClosure, ObjectError, ObjectFunction, ObjectInstance, ObjectInstanceData, ObjectUpvalue};

/// Static nil object — backing for `ObjectHandle::NIL`.
static NIL_OBJECT: LazyLock<Object> = LazyLock::new(|| {
    Object::Instance(ObjectInstance::new(ObjectHandle::NIL, ObjectInstanceData::Nil))
});

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub usize);

/// Sentinel handle representing nil — no heap allocation needed.
impl ObjectHandle {
    pub const NIL: Self = ObjectHandle(0);
    pub fn is_nil(self) -> bool { self.0 == 0 }
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
}

impl ObjectHeap {
    pub fn new() -> Self {
        // Slot 0 is reserved for nil sentinel.
        let objects = vec![None];
        let marked = vec![false];
        
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
        };

        heap.nil_class = heap.alloc_class("nil");
        heap.int_class = heap.alloc_class("int");
        heap.float_class = heap.alloc_class("float");
        heap.bool_class = heap.alloc_class("bool");
        heap.string_class = heap.alloc_class("string");
        heap.list_class = heap.alloc_class("list");
        heap.dict_class = heap.alloc_class("dict");

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

    pub fn alloc_function(&mut self, name: impl Into<ShrString>, arity: usize, chunk: Chunk) -> ObjectHandle {
        let obj = ObjectFunction::new(name, arity, chunk);
        self.alloc(obj)
    }

    pub fn alloc_upvalue(&mut self, location: Option<usize>) -> ObjectHandle {
        let obj = ObjectUpvalue { location, closed: ObjectHandle::NIL, next: None };
        self.alloc(obj)
    }

    pub fn alloc_builtin_fn(&mut self, name: &'static str, function: BuiltinFn) -> ObjectHandle {
        let obj = ObjectBuiltinFn::new(name, function);
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
    pub fn alloc_fields_instance(&mut self, class: ObjectHandle) -> ObjectHandle {
        self.alloc_instance(class, ObjectInstanceData::Fields(Default::default()))
    }

    #[inline]
    pub fn alloc_bool(&mut self, v: bool) -> ObjectHandle {
        self.alloc_instance(self.bool_class, ObjectInstanceData::Bool(v))
    }

    pub fn alloc_integer(&mut self, v: i64) -> ObjectHandle {
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
    pub fn alloc_float(&mut self, v: f64) -> ObjectHandle {
        self.alloc_instance(self.float_class, ObjectInstanceData::Float(v))
    }

    pub fn alloc_string(&mut self, s: ShrString) -> ObjectHandle {
        if let Some(&handle) = self.string_cache.get(&s) {
            return handle;
        }
        let handle = self.alloc_instance(self.string_class, ObjectInstanceData::String(s.clone()));
        self.string_cache.insert(s, handle);
        handle
    }

    #[inline]
    pub fn alloc_list(&mut self, items: Vec<ObjectHandle>) -> ObjectHandle {
        self.alloc_instance(self.list_class, ObjectInstanceData::List(items))
    }

    #[inline]
    pub fn alloc_dict(&mut self, items: Vec<(ObjectHandle, ObjectHandle)>) -> ObjectHandle {
        self.alloc_instance(self.dict_class, ObjectInstanceData::Dict(items))
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
            pub fn [<get_ $name>](&self, handle: ObjectHandle) -> Result<&$ty, ObjectError> {
                self.get(handle).[<as_ $name>]()
            }

            #[inline]
            pub fn [<get_ $name _mut>](&mut self, handle: ObjectHandle) -> Result<&mut $ty, ObjectError> {
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
        self.objects[handle.0].as_ref().expect("Dangling handle accessed!")
    }

    pub fn get_mut(&mut self, handle: ObjectHandle) -> &mut Object {
        if handle.is_nil() {
            panic!("Cannot mutate nil object");
        }
        self.objects[handle.0].as_mut().expect("Dangling handle accessed!")
    }

    impl_getters!(function, ObjectFunction);
    impl_getters!(builtin_fn, ObjectBuiltinFn);
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
            pub fn [<get_ $name _instance>](&self, handle: ObjectHandle) -> &$ty {
                let inst = self.get_instance(handle).expect("must be Instance");
                match &inst.data {
                    ObjectInstanceData::$variant(v) => v,
                    _ => panic!("expected {}", $label),
                }
            }

            #[doc = "Return a mutable reference to the inner `" $label "` data."]
            pub fn [<get_ $name _instance_mut>](&mut self, handle: ObjectHandle) -> &mut $ty {
                let inst = self.get_instance_mut(handle).expect("must be Instance");
                match &mut inst.data {
                    ObjectInstanceData::$variant(v) => v,
                    _ => panic!("expected {}", $label),
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
    impl_instance_data_getter!(dict, Dict, Vec<(ObjectHandle, ObjectHandle)>, "dict");
    impl_instance_data_getter!(fields, Fields, std::collections::HashMap<ShrString, ObjectHandle>, "fields");
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
        if handle.is_nil() {
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
                        ObjectInstanceData::Nil | ObjectInstanceData::Bool(_) | ObjectInstanceData::Integer(_) | ObjectInstanceData::Float(_) => {
                            // leaf types — no heap references
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
                            for &(k, v) in entries {
                                self.mark_object(k);
                                self.mark_object(v);
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
                    if let Method::User(method_handle) = bound.method {
                        self.mark_object(method_handle);
                    }
                    self.mark_object(bound.receiver);
                }
                Object::Class(class) => {
                    if let Some(superclass) = class.superclass {
                        self.mark_object(superclass);
                    }
                    for method in class.methods.values() {
                        if let Method::User(method_handle) = method {
                            self.mark_object(*method_handle);
                        }
                    }
                }
                Object::BuiltinFn(_) => {
                    // Builtin functions own no heap references.
                }
            }
        }

        self.objects[handle.0] = object;
    }

    pub fn sweep(&mut self) {
        for i in 1..self.objects.len() {
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
