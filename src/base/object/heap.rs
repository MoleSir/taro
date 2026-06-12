use std::collections::HashMap;

use crate::{Chunk, ShrString, Value};
use super::{BuiltinFn, Method, Object, ObjectBoundMethod, ObjectBuiltinFn, ObjectClass, ObjectClosure, ObjectDict, ObjectError, ObjectFunction, ObjectInstance, ObjectList, ObjectUpvalue};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub usize);

pub struct ObjectHeap {
    objects: Vec<Option<Object>>,
    marked: Vec<bool>,
    free_slots: Vec<usize>,
    gray_stack: Vec<ObjectHandle>,
    pub bytes_allocated: usize,
}

impl ObjectHeap {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            marked: Vec::new(),
            free_slots: Vec::new(),
            gray_stack: Vec::new(),
            bytes_allocated: 0,
        }
    }
}

impl ObjectHeap {
    // ================================================================================== //
    //           Alloc
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
        let obj = ObjectUpvalue { location, closed: Value::Nil, next: None };
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

    pub fn alloc_instance(&mut self, class: ObjectHandle) -> ObjectHandle {
        let obj = ObjectInstance::new(class);
        self.alloc(obj)
    }

    pub fn alloc_bound_method(&mut self, receiver: Value, method: Method) -> ObjectHandle {
        let obj = ObjectBoundMethod::new(receiver, method);
        self.alloc(obj)
    }

    pub fn alloc_list(&mut self, class: ObjectHandle, items: Vec<Value>) -> ObjectHandle {
        let obj = ObjectList::new(class, items);
        self.alloc(obj)
    }

    pub fn alloc_dict(&mut self, class: ObjectHandle, items: HashMap<Value, Value>) -> ObjectHandle {
        let obj = ObjectDict::new(class, items);
        self.alloc(obj)
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
        self.objects[handle.0].as_ref().expect("Dangling handle accessed!")
    }

    pub fn get_mut(&mut self, handle: ObjectHandle) -> &mut Object {
        self.objects[handle.0].as_mut().expect("Dangling handle accessed!")
    }

    impl_getters!(function, ObjectFunction);
    impl_getters!(builtin_fn, ObjectBuiltinFn);
    impl_getters!(closure, ObjectClosure);
    impl_getters!(upvalue, ObjectUpvalue);
    impl_getters!(instance, ObjectInstance);
    impl_getters!(class, ObjectClass);
    impl_getters!(bound_method, ObjectBoundMethod);
    impl_getters!(list, ObjectList);
    impl_getters!(dict, ObjectDict);
}

impl ObjectHeap {
    // ================================================================================== //
    //           GC
    // ================================================================================== //

    pub fn collect_garbage(&mut self) {
        self.trace_references();
        self.sweep();
    }

    pub fn mark_value(&mut self, value: &Value) {
        if let Value::Object(handle) = value {
            self.mark_object(*handle);
        }
    }

    pub fn mark_object(&mut self, handle: ObjectHandle) {
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
                    for value in function.chunk.constants.iter() {
                        self.mark_value(value);
                    }
                }
                Object::Closure(closure) => {
                    self.mark_object(closure.function);
                    for &upvalue in &closure.upvalues {
                        self.mark_object(upvalue);
                    }
                }
                Object::Upvalue(upvalue) => {
                    self.mark_value(&upvalue.closed);
                    if let Some(next) = upvalue.next {
                        self.mark_object(next);
                    }
                }
                Object::Instance(instance) => {
                    self.mark_object(instance.class);
                    for value in instance.fields.values() {
                        self.mark_value(value);
                    }
                }
                Object::BoundMethod(bound) => {
                    if let Method::User(method_handle) = bound.method {
                        self.mark_object(method_handle);
                    }
                    self.mark_value(&bound.receiver);
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
                Object::List(list) => {
                    self.mark_object(list.class);
                    for item in list.items.iter() {
                        self.mark_value(item);
                    }
                }
                Object::Dict(dict) => {
                    self.mark_object(dict.class);
                    for (k, v) in dict.items.iter() {
                        self.mark_value(k);
                        self.mark_value(v);
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
        for i in 0..self.objects.len() {
            if self.objects[i].is_some() {
                if self.marked[i] {
                    self.marked[i] = false;
                } else {
                    #[cfg(feature = "debug-gc")]
                    println!("Sweeping object at {}", i);

                    self.objects[i] = None;
                    self.free_slots.push(i);
                    self.bytes_allocated -= std::mem::size_of::<Object>();
                }
            }
        }
    }
}
