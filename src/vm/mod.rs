mod builtin;
pub(crate) mod call;
mod error;
mod gc;
mod magic;
pub(crate) mod module;
pub(crate) mod ops;
mod utils;
pub use error::*;

#[cfg(test)]
mod tests;
use crate::{ObjectHandle, ShrString};
use std::{collections::HashMap, path::PathBuf};

pub struct VirtualMachine {
    pub obj_heap: crate::ObjectHeap,
    pub(crate) frames: Vec<CallFrame>,
    pub(crate) stack: Vec<ObjectHandle>,
    /// Persistent root module (`__main__`).  All top-level definitions in
    /// directly executed scripts are stored in this module's fields, so they
    /// survive across `interpret()` calls (important for REPL use).
    pub(crate) main_module: ObjectHandle,
    pub(crate) builtins: HashMap<ShrString, ObjectHandle>,
    pub(crate) open_upvalues: Vec<ObjectHandle>,
    pub(crate) gc_threshold: usize,
    pub(crate) extra_gc_roots: Vec<ObjectHandle>,
    pub(crate) modules: module::Modules,
}

/// A single function-call frame.  `slots_start` is the index into
/// [`VirtualMachine::stack`] where this frame's locals begin.
pub struct CallFrame {
    pub closure: ObjectHandle,
    pub ip: usize,
    pub slots_start: usize,
}

impl VirtualMachine {
    pub fn new() -> Self {
        let obj_heap = crate::ObjectHeap::new();
        // The heap creates its own `__main__` module for builtin classes;
        // reuse it as the VM's root module for directly executed scripts.
        let main_module = obj_heap.builtins_module;
        let mut vm = Self {
            obj_heap,
            frames: vec![],
            stack: vec![],
            main_module,
            builtins: HashMap::new(),
            open_upvalues: vec![],
            gc_threshold: 1024 * 1024,
            extra_gc_roots: vec![],
            modules: module::Modules::default(),
        };
        vm.init_builtins();
        vm
    }

    /// Build the default module search path list.
    /// Always includes `"."` first, then any directories from the
    /// `TARO_PATH` environment variable (colon-separated).
    pub(crate) fn default_search_paths() -> Vec<PathBuf> {
        let mut paths = vec![PathBuf::from("."), PathBuf::from("src")];
        if let Ok(taro_path) = std::env::var("TARO_PATH") {
            for dir in taro_path.split(':') {
                let dir = dir.trim();
                if !dir.is_empty() {
                    paths.push(PathBuf::from(dir));
                }
            }
        }
        paths
    }

    /// Return a reference to the top-most (currently executing) call frame.
    #[inline]
    pub(crate) fn frame(&self) -> RuntimeResult<&CallFrame> {
        self.frames.last().ok_or(RuntimeErrorKind::CallFrameEmpty)
    }

    /// Return a mutable reference to the top-most call frame.
    #[inline]
    pub(crate) fn frame_mut(&mut self) -> RuntimeResult<&mut CallFrame> {
        self.frames.last_mut().ok_or(RuntimeErrorKind::StackEmpty)
    }

    /// Compile `source` and execute it on this VM.
    pub fn interpret(&mut self, source: &str) -> Result<(), InterpretError> {
        let function = crate::compile::compile(source, &mut self.obj_heap).map_err(InterpretError::Compile)?;
        self.interpret_function(function)
    }

    pub(crate) fn interpret_function(&mut self, function: ObjectHandle) -> Result<(), InterpretError> {
        let closure = self.obj_heap.alloc_closure(function, self.main_module);
        self.reset();
        self.push_stack(closure);
        self.call_closure(closure, 0, true).expect("can't failed in script call");
        self.run().map_err(InterpretError::Runtime)
    }

    pub fn run(&mut self) -> Result<(), RuntimeError> {
        loop {
            self.try_collect_garbage();
            if self.frames.is_empty() {
                return Ok(());
            }
            if let Err(reason) = self.step() {
                let (line, column) = self.get_current_source_pos();
                return Err(RuntimeError { line, column, reason });
            }
        }
    }

    /// Return the module object for the currently executing closure.
    ///
    /// During execution every closure has a `.module` set (either the root
    /// `__main__` module or an imported module).  This is the Taro equivalent
    /// of Python's `func.__globals__`.
    pub(crate) fn current_module(&self) -> ObjectHandle {
        let closure = self.obj_heap.get_closure(self.frame().expect("no frame").closure).expect("must closure");
        closure.module
    }

    #[inline]
    pub(crate) fn push_stack(&mut self, handle: ObjectHandle) {
        self.stack.push(handle);
    }

    #[inline]
    pub fn pop_stack(&mut self) -> RuntimeResult<ObjectHandle> {
        self.stack.pop().ok_or(RuntimeErrorKind::StackEmpty)
    }

    #[inline]
    pub fn peek_stack(&self, index: usize) -> RuntimeResult<ObjectHandle> {
        self.stack.iter().rev().nth(index).copied().ok_or(RuntimeErrorKind::StackEmpty)
    }

    /// Return the absolute stack index of the callee slot for a pending call
    /// with `arg_count` arguments already pushed above it.
    #[inline]
    pub(crate) fn callee_slot(&self, arg_count: usize) -> usize {
        self.stack.len() - arg_count - 1
    }

    /// Look up the source position (line, column) for the instruction currently
    /// being executed (the IP of the top-most call frame).
    fn get_current_source_pos(&self) -> (Option<usize>, Option<usize>) {
        let frame = match self.frame() {
            Ok(f) => f,
            Err(_) => return (None, None),
        };
        let closure = match self.obj_heap.get_closure(frame.closure) {
            Some(c) => c,
            None => return (None, None),
        };
        let function = match self.obj_heap.get_function(closure.function) {
            Some(f) => f,
            None => return (None, None),
        };
        match function.chunk.get_source_pos(frame.ip) {
            Some((line, col)) => (Some(line), Some(col)),
            None => (None, None),
        }
    }

    pub fn reset(&mut self) {
        self.stack.clear();
        self.frames.clear();
    }

    /// Capture a stack slot as an upvalue.
    pub(crate) fn capture_upvalue(&mut self, slot: usize) -> RuntimeResult<ObjectHandle> {
        let mut prev: Option<ObjectHandle> = None;
        let mut curr = self.open_upvalues.last().copied();
        while let Some(handle) = curr {
            let uv = self.obj_heap.get_upvalue(handle).expect("must upvalue");
            if uv.location.map_or(true, |loc| loc < slot) {
                break;
            }
            if uv.location == Some(slot) {
                return Ok(handle);
            }
            prev = curr;
            curr = uv.next;
        }

        let new_handle = self.obj_heap.alloc_upvalue(Some(slot));
        if let Some(prev_handle) = prev {
            self.obj_heap.get_upvalue_mut(prev_handle).expect("must upvalue").next = Some(new_handle);
        } else {
            self.open_upvalues.push(new_handle);
        }
        Ok(new_handle)
    }

    /// Close every open upvalue whose location is at or above `last`.
    pub(crate) fn close_upvalues(&mut self, last: usize) -> RuntimeResult<()> {
        while let Some(&handle) = self.open_upvalues.last() {
            let uv = self.obj_heap.get_upvalue(handle).expect("must upvalue");
            if uv.location.map_or(true, |loc| loc < last) {
                break;
            }
            let location = uv.location.expect("open upvalue must have location");
            let value = self.stack[location];
            let uv_mut = self.obj_heap.get_upvalue_mut(handle).expect("must upvalue");
            uv_mut.closed = value;
            uv_mut.location = None;
            self.open_upvalues.pop();
        }
        Ok(())
    }
}
