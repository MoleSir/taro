use crate::{Chunk, ShrString, Value};
use super::{BuiltinFn, Object, ObjectBuiltinFn, ObjectFunction};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub usize);

pub struct ObjectHeap {
    objects: Vec<Option<Object>>,
    marked: Vec<bool>,
    
    // 空闲链表：存放被回收的索引，以便 O(1) 复杂度复用空间
    free_slots: Vec<usize>,

    // 三色标记法中的 "灰色栈"
    gray_stack: Vec<ObjectHandle>,
    
    // 避免借用冲突的临时缓冲
    children_buffer: Vec<ObjectHandle>,

    // GC 触发策略
    bytes_allocated: usize,
    
    #[allow(unused)]
    next_gc: usize,
}

impl ObjectHeap {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            marked: Vec::new(),
            free_slots: Vec::new(),
            gray_stack: Vec::new(),
            children_buffer: Vec::new(),
            bytes_allocated: 0,
            next_gc: 1024 * 1024, // 初始阈值 1MB
        }
    }

    pub fn alloc_function(&mut self, name: impl Into<ShrString>, arity: usize, chunk: Chunk) -> ObjectHandle {
        let obj = ObjectFunction { arity, chunk, name: name.into() };
        self.alloc(obj)
    }

    pub fn alloc_builtin_fn(&mut self, function: BuiltinFn) -> ObjectHandle {
        let obj = ObjectBuiltinFn { function };
        self.alloc(obj)
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

    pub fn get(&self, handle: ObjectHandle) -> &Object {
        self.objects[handle.0].as_ref().expect("Dangling handle accessed!")
    }

    pub fn get_mut(&mut self, handle: ObjectHandle) -> &mut Object {
        self.objects[handle.0].as_mut().expect("Dangling handle accessed!")
    }

    pub fn mark_value(&mut self, value: Value) {
        if let Value::Object(handle) = value {
            self.mark_object(handle);
        }
    }

    /// 标记单个对象 (三色标记：白色变灰色)
    pub fn mark_object(&mut self, handle: ObjectHandle) {
        let index = handle.0;
        
        // 如果已经标记过(已经是灰色或黑色)，直接返回，防止循环引用导致死循环！
        if self.marked[index] {
            return;
        }

        #[cfg(feature = "debug-gc")]
        println!("Marking {:?}", handle);

        // 标记为存活
        self.marked[index] = true;
        // 加入灰色工作栈，等待后续追踪其引用的子对象
        self.gray_stack.push(handle);
    }

    /// 追踪对象引用 (三色标记：灰色变黑色)
    pub fn trace_references(&mut self) {
        // 不断从工作栈弹出灰色对象，直到工作栈为空
        while let Some(handle) = self.gray_stack.pop() {
            self.blacken_object(handle);
        }
    }

    fn blacken_object(&mut self, handle: ObjectHandle) {
        #[cfg(feature = "debug-gc")]
        println!("Blackening {:?}", handle);

        // 清空缓冲区
        self.children_buffer.clear();

        // 1. 获取对象，并提取其引用的所有子句柄
        // 【Rust 小技巧】：我们将提取的句柄放入缓冲中。这不仅避免了在调用 mark_object 
        // 时发生 `self` 被同时借用为可变和不可变的编译错误，还避免了分配临时 Vec。
        if let Some(obj) = &self.objects[handle.0] {
            obj.extract_children(&mut self.children_buffer);
        }

        // 2. 标记所有子对象（这会修改 self.marked 并将其推入 gray_stack）
        // 因为 children_buffer 只是临时存了 Copy 语义的 ObjectHandle，这里的循环是安全的。
        let children = self.children_buffer.clone(); // 只是 clone 一些 usize，开销极小
        for child_handle in children {
            self.mark_object(child_handle);
        }
    }

    /// 清除所有未标记的对象 (Sweep)
    pub fn sweep(&mut self) {
        for i in 0..self.objects.len() {
            // 如果槽位有对象
            if self.objects[i].is_some() {
                if self.marked[i] {
                    // 对象存活，取消标记，为下一次 GC 准备 (黑色变回白色)
                    self.marked[i] = false;
                } else {
                    // 对象已死，回收它
                    #[cfg(feature = "debug-gc")]
                    println!("Sweeping object at {}", i);
                    
                    self.objects[i] = None;
                    self.free_slots.push(i);
                    // 此处可以精确扣减 bytes_allocated
                }
            }
        }
    }
}
