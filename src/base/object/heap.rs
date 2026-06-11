use crate::{Chunk, ShrString, Value};
use super::{BuiltinFn, Object, ObjectBuiltinFn, ObjectClass, ObjectClosure, ObjectError, ObjectFunction, ObjectInstance, ObjectUpvalue};

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

    // ================================================================================== // 
    //           Get
    // ================================================================================== // 

    pub fn get(&self, handle: ObjectHandle) -> &Object {
        self.objects[handle.0].as_ref().expect("Dangling handle accessed!")
    }

    pub fn get_mut(&mut self, handle: ObjectHandle) -> &mut Object {
        self.objects[handle.0].as_mut().expect("Dangling handle accessed!")
    }

    #[inline]
    pub fn get_function(&self, handle: ObjectHandle) -> Result<&ObjectFunction, ObjectError> {
        self.get(handle).as_function()
    }

    #[inline]
    pub fn get_function_mut(&mut self, handle: ObjectHandle) -> Result<&mut ObjectFunction, ObjectError> {
        self.get_mut(handle).as_function_mut()
    }

    #[inline]
    pub fn get_builtin_fn(&self, handle: ObjectHandle) -> Result<&ObjectBuiltinFn, ObjectError> {
        self.get(handle).as_builtin_fn()
    }

    #[inline]
    pub fn get_builtin_fn_mut(&mut self, handle: ObjectHandle) -> Result<&mut ObjectBuiltinFn, ObjectError> {
        self.get_mut(handle).as_builtin_fn_mut()
    }

    #[inline]
    pub fn get_closure(&self, handle: ObjectHandle) -> Result<&ObjectClosure, ObjectError> {
        self.get(handle).as_closure()
    }

    #[inline]
    pub fn get_closure_mut(&mut self, handle: ObjectHandle) -> Result<&mut ObjectClosure, ObjectError> {
        self.get_mut(handle).as_closure_mut()
    }

    #[inline]
    pub fn get_upvalue(&self, handle: ObjectHandle) -> Result<&ObjectUpvalue, ObjectError> {
        self.get(handle).as_upvalue()
    }

    #[inline]
    pub fn get_upvalue_mut(&mut self, handle: ObjectHandle) -> Result<&mut ObjectUpvalue, ObjectError> {
        self.get_mut(handle).as_upvalue_mut()
    }

    #[inline]
    pub fn get_instance(&self, handle: ObjectHandle) -> Result<&ObjectInstance, ObjectError> {
        self.get(handle).as_instance()
    }

    #[inline]
    pub fn get_instance_mut(&mut self, handle: ObjectHandle) -> Result<&mut ObjectInstance, ObjectError> {
        self.get_mut(handle).as_instance_mut()
    }

    #[inline]
    pub fn get_class(&self, handle: ObjectHandle) -> Result<&ObjectClass, ObjectError> {
        self.get(handle).as_class()
    }

    #[inline]
    pub fn get_class_mut(&mut self, handle: ObjectHandle) -> Result<&mut ObjectClass, ObjectError> {
        self.get_mut(handle).as_class_mut()
    }

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
                    self.mark_object(bound.method);
                    self.mark_value(&bound.receiver);
                }
                _ => {}
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