use super::VirtualMachine;

impl VirtualMachine {
    pub fn try_collect_garbage(&mut self) {
        if cfg!(any(feature = "gc-stress", test)) {
            self.collect_garbage();
        } else {
            if self.obj_heap.bytes_allocated > self.gc_threshold {
                self.collect_garbage();
            }
        }
    }

    pub fn collect_garbage(&mut self) {
        #[cfg(feature = "debug-gc")]
        println!("-- GC begin");

        // mark stacks — stack is Vec<ObjectHandle> now
        for &handle in &self.stack {
            self.obj_heap.mark_object(handle);
        }

        // mark globals
        for &handle in self.globals.values() {
            self.obj_heap.mark_object(handle);
        }

        // mark frames
        for frame in self.frames.iter() {
            self.obj_heap.mark_object(frame.closure);
        }

        // mark open_upvalues
        for obj in self.open_upvalues.iter() {
            self.obj_heap.mark_object(*obj);
        }

        // mark builtin class handles (always reachable)
        self.obj_heap.mark_object(self.nil_class);
        self.obj_heap.mark_object(self.int_class);
        self.obj_heap.mark_object(self.float_class);
        self.obj_heap.mark_object(self.bool_class);
        self.obj_heap.mark_object(self.string_class);
        self.obj_heap.mark_object(self.list_class);
        self.obj_heap.mark_object(self.dict_class);

        // collect_garbage by gc
        self.obj_heap.collect_garbage();

        // update gc_threshold
        self.gc_threshold = self.obj_heap.bytes_allocated * 2;

        #[cfg(feature = "debug-gc")]
        println!("-- GC end");
    }
}
