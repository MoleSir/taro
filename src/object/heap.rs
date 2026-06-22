use std::{collections::HashMap, sync::LazyLock};

use super::{
    Method, NativeFunction, Object, ObjectBool, ObjectBoundMethod, ObjectBytes, ObjectBytesIterator, ObjectClass, ObjectClosure,
    ObjectDict, ObjectDictIterator, ObjectFields, ObjectFloat, ObjectFunction, ObjectInstance, ObjectInstanceData, ObjectInt,
    ObjectIterEnd, ObjectList, ObjectListIterator, ObjectNativeFn, ObjectNil, ObjectSet, ObjectSetIterator, ObjectString,
    ObjectStringIterator, ObjectUpvalue, register_bool_builtins, register_bytes_builtins, register_dict_builtins, register_float_builtins,
    register_int_builtins, register_list_builtins, register_set_builtins, register_string_builtins,
};
use crate::{Chunk, ShrString};

/// Static nil object — backing for `ObjectHandle::NIL`.
static NIL_OBJECT: LazyLock<Object> = LazyLock::new(|| Object::Instance(ObjectInstance::new(ObjectHandle::NIL, Box::new(ObjectNil))));

/// Static IterEnd object — backing for `ObjectHandle::ITER_END`.
static ITER_END_OBJECT: LazyLock<Object> =
    LazyLock::new(|| Object::Instance(ObjectInstance::new(ObjectHandle::ITER_END, Box::new(ObjectIterEnd))));

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub usize);

/// Sentinel handle representing nil — no heap allocation needed.
impl ObjectHandle {
    pub const NIL: Self = ObjectHandle(0);
    pub const ITER_END: Self = ObjectHandle(1);

    pub fn is_nil(self) -> bool {
        self.0 == 0
    }
    pub fn is_iter_end(self) -> bool {
        self.0 == 1
    }
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
    pub bytes_class: ObjectHandle,
    pub module_class: ObjectHandle,

    /// Singleton instances for `true` and `false` so repeated use of
    /// boolean literals doesn't allocate.
    pub true_instance: ObjectHandle,
    pub false_instance: ObjectHandle,

    /// Class handles for built-in iterator types.
    pub list_iter_class: ObjectHandle,
    pub string_iter_class: ObjectHandle,
    pub dict_iter_class: ObjectHandle,
    pub set_iter_class: ObjectHandle,
    pub bytes_iter_class: ObjectHandle,
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
            bytes_class: ObjectHandle::NIL,
            module_class: ObjectHandle::NIL,
            true_instance: ObjectHandle::NIL,
            false_instance: ObjectHandle::NIL,
            list_iter_class: ObjectHandle::NIL,
            string_iter_class: ObjectHandle::NIL,
            dict_iter_class: ObjectHandle::NIL,
            set_iter_class: ObjectHandle::NIL,
            bytes_iter_class: ObjectHandle::NIL,
        };

        heap.nil_class = heap.alloc_class("Nil");
        heap.int_class = heap.alloc_class("Int");
        heap.float_class = heap.alloc_class("Float");
        heap.bool_class = heap.alloc_class("Bool");
        heap.string_class = heap.alloc_class("String");
        heap.list_class = heap.alloc_class("List");
        heap.dict_class = heap.alloc_class("Dict");
        heap.set_class = heap.alloc_class("Set");
        heap.bytes_class = heap.alloc_class("Bytes");
        heap.module_class = heap.alloc_class("Module");
        heap.list_iter_class = heap.alloc_class("ObjectListIterator");
        heap.string_iter_class = heap.alloc_class("ObjectStringIterator");
        heap.dict_iter_class = heap.alloc_class("ObjectDictIterator");
        heap.set_iter_class = heap.alloc_class("ObjectSetIterator");
        heap.bytes_iter_class = heap.alloc_class("ObjectBytesIterator");

        // Allocate singleton bool instances (after bool_class exists).
        heap.true_instance = heap.alloc_instance(heap.bool_class, ObjectBool::new(true));
        heap.false_instance = heap.alloc_instance(heap.bool_class, ObjectBool::new(false));

        // Register built-in class magic methods during heap init.
        register_int_builtins(&mut heap);
        register_float_builtins(&mut heap);
        register_bool_builtins(&mut heap);
        register_string_builtins(&mut heap);
        register_list_builtins(&mut heap);
        register_dict_builtins(&mut heap);
        register_set_builtins(&mut heap);
        register_bytes_builtins(&mut heap);

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

    pub fn alloc_instance<D: ObjectInstanceData>(&mut self, class: ObjectHandle, data: D) -> ObjectHandle {
        let obj = ObjectInstance::new(class, Box::new(data));
        self.alloc(obj)
    }

    pub fn alloc_instance_dyn(&mut self, class: ObjectHandle, data: Box<dyn ObjectInstanceData>) -> ObjectHandle {
        let obj = ObjectInstance::new(class, data);
        self.alloc(obj)
    }

    pub fn alloc_bound_method(&mut self, receiver: ObjectHandle, method: Method) -> ObjectHandle {
        let obj = ObjectBoundMethod::new(receiver, method);
        self.alloc(obj)
    }

    #[inline]
    pub fn alloc_fields_instance(&mut self, class: ObjectHandle, fields: HashMap<ShrString, ObjectHandle>) -> ObjectHandle {
        self.alloc_instance(class, ObjectFields { fields })
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
            let handle = self.alloc_instance(self.int_class, ObjectInt::new(v));
            self.int_cache.insert(v, handle);
            return handle;
        }
        self.alloc_instance(self.int_class, ObjectInt::new(v))
    }

    #[inline]
    pub fn alloc_float_instance(&mut self, v: f64) -> ObjectHandle {
        self.alloc_instance(self.float_class, ObjectFloat { value: v })
    }

    pub fn alloc_string_instance(&mut self, s: ShrString) -> ObjectHandle {
        if let Some(&handle) = self.string_cache.get(&s) {
            return handle;
        }
        let handle = self.alloc_instance(self.string_class, ObjectString::new(s.clone()));
        self.string_cache.insert(s, handle);
        handle
    }

    #[inline]
    pub fn alloc_list_instance(&mut self, items: Vec<ObjectHandle>) -> ObjectHandle {
        self.alloc_instance(self.list_class, ObjectList::new(items))
    }

    #[inline]
    pub fn alloc_dict_instance(&mut self, items: HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>>) -> ObjectHandle {
        self.alloc_instance(self.dict_class, ObjectDict::new(items))
    }

    #[inline]
    pub fn alloc_set_instance(&mut self, items: HashMap<u64, Vec<ObjectHandle>>) -> ObjectHandle {
        self.alloc_instance(self.set_class, ObjectSet { entries: items })
    }

    #[inline]
    pub fn alloc_bytes_instance(&mut self, data: Vec<u8>) -> ObjectHandle {
        self.alloc_instance(self.bytes_class, ObjectBytes { data })
    }

    /// Register a native method on a builtin class.
    pub fn register_native_method(&mut self, class_handle: ObjectHandle, name: &'static str, function: NativeFunction) {
        let handle = self.alloc_native_fn(name, function);
        let class = self.get_class_mut(class_handle).expect("class");
        class.methods.insert(name.into(), Method::Native(handle));
    }

    pub fn alloc(&mut self, obj: impl Into<Object>) -> ObjectHandle {
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
    ($name:ident, $wrapper:ty, $inner:ty, $field:ident, $label:literal) => {
        paste::paste! {
            #[doc = "Return a reference to the inner `" $label "` data."]
            pub fn [<get_ $name _instance>](&self, handle: ObjectHandle) -> Option<&$inner> {
                let inst = self.get_instance(handle)?;
                inst.data.as_any_ref().downcast_ref::<$wrapper>().map(|d| &d.$field)
            }

            #[doc = "Return a mutable reference to the inner `" $label "` data."]
            pub fn [<get_ $name _instance_mut>](&mut self, handle: ObjectHandle) -> Option<&mut $inner> {
                let inst = self.get_instance_mut(handle)?;
                inst.data.as_any_mut().downcast_mut::<$wrapper>().map(|d| &mut d.$field)
            }
        }
    };
}

impl ObjectHeap {
    impl_instance_data_getter!(integer, ObjectInt, i64, value, "integer");
    impl_instance_data_getter!(float, ObjectFloat, f64, value, "float");
    impl_instance_data_getter!(bool, ObjectBool, bool, value, "bool");
    impl_instance_data_getter!(string, ObjectString, ShrString, value, "string");
    impl_instance_data_getter!(list, ObjectList, Vec<ObjectHandle>, items, "list");
    impl_instance_data_getter!(dict, ObjectDict, HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>>, entries, "dict");
    impl_instance_data_getter!(set, ObjectSet, HashMap<u64, Vec<ObjectHandle>>, entries, "set");
    impl_instance_data_getter!(bytes, ObjectBytes, Vec<u8>, data, "bytes");
    impl_instance_data_getter!(fields, ObjectFields, HashMap<ShrString, ObjectHandle>, fields, "fields");

    /// Generic helper: return a shared reference to instance data downcast to `T`.
    pub fn get_instance_data<T: ObjectInstanceData>(&self, handle: ObjectHandle) -> Option<&T> {
        let inst = self.get_instance(handle)?;
        inst.data.as_any_ref().downcast_ref::<T>()
    }

    /// Generic helper: return a mutable reference to instance data downcast to `T`.
    pub fn get_instance_data_mut<T: ObjectInstanceData>(&mut self, handle: ObjectHandle) -> Option<&mut T> {
        let inst = self.get_instance_mut(handle)?;
        inst.data.as_any_mut().downcast_mut::<T>()
    }

    // Convenience aliases for iterator getters — delegates to the generic helper.
    pub fn get_list_iter(&self, handle: ObjectHandle) -> Option<&ObjectListIterator> {
        self.get_instance_data(handle)
    }
    pub fn get_list_iter_mut(&mut self, handle: ObjectHandle) -> Option<&mut ObjectListIterator> {
        self.get_instance_data_mut(handle)
    }
    pub fn get_dict_iter(&self, handle: ObjectHandle) -> Option<&ObjectDictIterator> {
        self.get_instance_data(handle)
    }
    pub fn get_dict_iter_mut(&mut self, handle: ObjectHandle) -> Option<&mut ObjectDictIterator> {
        self.get_instance_data_mut(handle)
    }
    pub fn get_set_iter(&self, handle: ObjectHandle) -> Option<&ObjectSetIterator> {
        self.get_instance_data(handle)
    }
    pub fn get_set_iter_mut(&mut self, handle: ObjectHandle) -> Option<&mut ObjectSetIterator> {
        self.get_instance_data_mut(handle)
    }
    pub fn get_string_iter(&self, handle: ObjectHandle) -> Option<&ObjectStringIterator> {
        self.get_instance_data(handle)
    }
    pub fn get_string_iter_mut(&mut self, handle: ObjectHandle) -> Option<&mut ObjectStringIterator> {
        self.get_instance_data_mut(handle)
    }
    pub fn get_bytes_iter(&self, handle: ObjectHandle) -> Option<&ObjectBytesIterator> {
        self.get_instance_data(handle)
    }
    pub fn get_bytes_iter_mut(&mut self, handle: ObjectHandle) -> Option<&mut ObjectBytesIterator> {
        self.get_instance_data_mut(handle)
    }

    /// Return a shared reference to the native data stored in `handle`,
    /// downcast to `T`.
    pub fn get_native<T: ObjectInstanceData>(&self, handle: ObjectHandle) -> Option<&T> {
        self.get_instance_data(handle)
    }

    /// Return a mutable reference to the native data stored in `handle`,
    /// downcast to `T`.
    pub fn get_native_mut<T: ObjectInstanceData>(&mut self, handle: ObjectHandle) -> Option<&mut T> {
        self.get_instance_data_mut(handle)
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
                    instance.data.mark_references(self);
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
                    if let Some(module) = class.module {
                        self.mark_object(module);
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
