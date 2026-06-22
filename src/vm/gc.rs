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

        // mark extra GC roots (used by import to keep importing state alive)
        for &handle in &self.extra_gc_roots {
            self.obj_heap.mark_object(handle);
        }

        // mark loaded modules (keeps module-owned classes alive via back-references)
        for &handle in self.loaded_modules.values() {
            self.obj_heap.mark_object(handle);
        }

        // mark builtin class handles (always reachable)
        self.obj_heap.mark_object(self.obj_heap.nil_class);
        self.obj_heap.mark_object(self.obj_heap.int_class);
        self.obj_heap.mark_object(self.obj_heap.float_class);
        self.obj_heap.mark_object(self.obj_heap.bool_class);
        self.obj_heap.mark_object(self.obj_heap.string_class);
        self.obj_heap.mark_object(self.obj_heap.list_class);
        self.obj_heap.mark_object(self.obj_heap.dict_class);
        self.obj_heap.mark_object(self.obj_heap.set_class);
        self.obj_heap.mark_object(self.obj_heap.bytes_class);
        self.obj_heap.mark_object(self.obj_heap.module_class);
        self.obj_heap.mark_object(self.obj_heap.list_iter_class);
        self.obj_heap.mark_object(self.obj_heap.string_iter_class);
        self.obj_heap.mark_object(self.obj_heap.dict_iter_class);
        self.obj_heap.mark_object(self.obj_heap.set_iter_class);
        self.obj_heap.mark_object(self.obj_heap.bytes_iter_class);

        // mark singleton bool instances (always reachable)
        self.obj_heap.mark_object(self.obj_heap.true_instance);
        self.obj_heap.mark_object(self.obj_heap.false_instance);

        // collect_garbage by gc
        self.obj_heap.collect_garbage();

        // update gc_threshold
        self.gc_threshold = self.obj_heap.bytes_allocated * 2;

        #[cfg(feature = "debug-gc")]
        println!("-- GC end");
    }
}
