mod magic;
mod builtin;
mod error;
mod gc;
mod utils;
pub use error::*;
#[cfg(test)]
mod tests;
use crate::{NativeFunction, Instruction, Method, Object, ObjectBoundMethod, ObjectNativeFn, ObjectClass, ObjectClosure, ObjectFunction, ObjectHandle, ObjectHeap, ObjectInstance, ObjectInstanceData, ObjectUpvalue, ShrString};
use std::collections::HashMap;

pub struct VirtualMachine {
    pub obj_heap: ObjectHeap,
    frames: Vec<CallFrame>,
    stack: Vec<ObjectHandle>,
    globals: HashMap<ShrString, ObjectHandle>,
    /// Sorted (by descending location) linked list of open upvalues.
    open_upvalues: Vec<ObjectHandle>,
    gc_threshold: usize,
    /// Handles that should always be treated as GC roots (used during import
    /// to keep the importing script's state alive while a module executes).
    extra_gc_roots: Vec<ObjectHandle>,
}

/// A single function-call frame.  `slots_start` is the index into
/// [`VirtualMachine::stack`] where this frame's locals begin.
pub struct CallFrame {
    pub closure: ObjectHandle,
    pub ip: usize,
    pub slots_start: usize,
}

macro_rules! binary_op {
    ($vm:ident, $f:ident) => {
        paste::paste! {{
            let rhs = $vm.pop_stack()?;
            let lhs = $vm.pop_stack()?;
            let res = $vm.[<__ $f __>](lhs, rhs)?;
            $vm.push_stack(res);
        }}
    };
}

macro_rules! unary_op {
    ($vm:ident, $f:ident) => {
        paste::paste! {{
            let v = $vm.pop_stack()?;
            let res = $vm.[<__ $f __>](v)?;
            $vm.push_stack(res);
        }}
    };
}

impl VirtualMachine {
    pub fn new() -> Self {
        let mut vm = Self {
            obj_heap: ObjectHeap::new(),
            frames: vec![],
            stack: vec![],
            globals: HashMap::new(),
            open_upvalues: vec![],
            gc_threshold: 1024 * 1024,
            extra_gc_roots: vec![],
        };
        vm.register_builtins();
        vm
    }

    /// Return a reference to the top-most (currently executing) call frame.
    #[inline]
    fn frame(&self) -> ExecuteResult<&CallFrame> {
        self.frames.last().ok_or(ExecuteError::CallFrameEmpty)
    }

    /// Return a mutable reference to the top-most call frame.
    #[inline]
    fn frame_mut(&mut self) -> ExecuteResult<&mut CallFrame> {
        self.frames.last_mut().ok_or(ExecuteError::StackEmpty)
    }

    /// Compile `source` and execute it on this VM.
    pub fn interpret(&mut self, source: &str) -> Result<(), InterpretError> {
        let function = crate::compile::compile(source, &mut self.obj_heap)
            .map_err(InterpretError::Compile)?;
        self.interpret_function(function)
    }

    pub(crate) fn interpret_function(&mut self, function: ObjectHandle) -> Result<(), InterpretError> {
        let closure = self.obj_heap.alloc_closure(function);
        self.reset();
        self.push_stack(closure);
        self.call_closure(closure, 0, true).expect("can't failed in script call");
        self.run().map_err(InterpretError::Runtime)
    }

    /// Import a module: read `path`, compile and execute it with isolated
    /// globals, then return a module object containing the top-level
    /// definitions (everything except the builtins).
    pub fn import_module(&mut self, path: &str) -> ExecuteResult<ObjectHandle> {
        // Virtual std/ modules — no file on disk.
        if let Some(module_name) = path.strip_prefix("std/") {
            return self.import_std_module(module_name);
        }

        // Read file content.
        let source = std::fs::read_to_string(path)
            .map_err(|e| ExecuteError::ImportError(format!("cannot read '{path}': {e}")))?;

        self.import_source_module(&source, path)
    }

    /// Compile `source` as a module and execute it in an isolated scope,
    /// returning a module object with the top-level definitions.
    ///
    /// `display_name` is used only in error messages.
    fn import_source_module(&mut self, source: &str, display_name: &str) -> ExecuteResult<ObjectHandle> {
        // 1. Compile the module source (uses local scope so nested functions
        //    capture module-level names as upvalues).
        let function = crate::compile::compile_module(source, &mut self.obj_heap)
            .map_err(|e| ExecuteError::ImportError(format!("compile error in '{display_name}': {e:?}")))?;

        // 2. Save current VM execution state.
        let saved_frames = std::mem::take(&mut self.frames);
        let saved_stack = std::mem::take(&mut self.stack);
        let saved_globals = std::mem::take(&mut self.globals);
        let saved_upvalues = std::mem::take(&mut self.open_upvalues);
        let saved_gc_threshold = self.gc_threshold;

        // 3. Prevent GC from running while saved state is unreachable from VM roots.
        self.gc_threshold = usize::MAX;

        // 4. Populate extra_gc_roots so the GC (which always runs in test /
        //    gc-stress mode) keeps the importing script's state alive while the
        //    module executes.
        self.extra_gc_roots.clear();
        self.extra_gc_roots.extend_from_slice(&saved_stack);
        for frame in &saved_frames {
            self.extra_gc_roots.push(frame.closure);
        }
        for &handle in saved_globals.values() {
            self.extra_gc_roots.push(handle);
        }
        self.extra_gc_roots.extend_from_slice(&saved_upvalues);

        // 5. Set up fresh globals with builtins only (module top-level
        //    definitions use locals, but builtin functions like `print` still
        //    need to be accessible as globals).
        self.register_builtins();

        // 6. Execute the module function.
        let result = self.interpret_function(function);

        // 7. The module function returns a dict containing its top-level
        //    definitions.  Grab it from the stack before restoring state.
        let exports_dict = self.pop_stack().unwrap_or(ObjectHandle::NIL);

        // 8. Restore original VM state.
        self.frames = saved_frames;
        self.stack = saved_stack;
        self.open_upvalues = saved_upvalues;
        self.globals = saved_globals;
        self.gc_threshold = saved_gc_threshold;
        self.extra_gc_roots.clear();

        // 9. Propagate execution errors.
        result.map_err(|e| {
            ExecuteError::ImportError(format!("error in module '{display_name}': {e}"))
        })?;

        // 10. Convert the dict into a fields instance.
        let exports: HashMap<ShrString, ObjectHandle> = if let Some(entries) = self.obj_heap.get_dict_instance(exports_dict) {
            entries.values().flat_map(|bucket| bucket.iter().map(|&(k, v)| {
                let key = self.obj_heap.get_string_instance(k)
                    .cloned()
                    .unwrap_or_else(|| ShrString::new_str("?"));
                (key, v)
            })).collect()
        } else {
            HashMap::new()
        };

        // 11. Create module object with exported names as fields.
        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }

    pub fn run(&mut self) -> ExecuteResult<()> {
        loop {
            self.try_collect_garbage();
            if self.frames.is_empty() {
                return Ok(());
            }
            self.step()?;
        }
    }

    /// Advance the VM by one instruction.
    fn step(&mut self) -> ExecuteResult<()> {
        let mut ip = self.frame()?.ip;

        let inst = {
            let closure = self.obj_heap.get_closure(self.frame()?.closure).expect("must closure");
            let function = self.obj_heap.get_function(closure.function).expect("must function");
            function.chunk.read_instruction(&mut ip, &self.obj_heap)?
        };

        match inst {
            Instruction::Constant(handle) => self.push_stack(handle),
            Instruction::DefineGlobal(name) => {
                let value = self.pop_stack()?;
                self.globals.insert(name, value);
            }
            Instruction::GetGlobal(name) => {
                let value = self.globals
                    .get(&name)
                    .copied()
                    .ok_or_else(|| ExecuteError::VariableNotFound(name.as_str().to_string()))?;
                self.push_stack(value);
            }
            Instruction::SetGlobal(name) => {
                let value = self.stack
                    .last()
                    .copied()
                    .ok_or(ExecuteError::StackEmpty)?;
                self.globals.insert(name, value);
            }
            Instruction::GetLocal(slot) => {
                let base = self.frame()?.slots_start;
                let index = base + slot;
                let value = self.stack
                    .get(index)
                    .copied()
                    .ok_or_else(|| ExecuteError::StackIndexOutOfRange(index))?;
                self.push_stack(value);
            }
            Instruction::SetLocal(slot) => {
                let base = self.frame()?.slots_start;
                let index = base + slot;
                let value = self.stack
                    .last()
                    .copied()
                    .ok_or(ExecuteError::StackEmpty)?;
                if index >= self.stack.len() {
                    return Err(ExecuteError::StackIndexOutOfRange(index));
                }
                self.stack[index] = value;
            }
            Instruction::Return => {
                let frame = self.frames.pop().expect("not empty frame");
                let result = self.pop_stack()?;
                self.close_upvalues(frame.slots_start)?;
                if self.frames.is_empty() {
                    // Last frame — keep only the result on the stack.
                    self.stack.truncate(0);
                    self.push_stack(result);
                    return Ok(());
                }
                self.stack.truncate(frame.slots_start);
                self.push_stack(result);
                return Ok(());
            }
            Instruction::Nil => self.push_stack(ObjectHandle::NIL),
            Instruction::True => { let h = self.obj_heap.alloc_bool_instance(true); self.push_stack(h); }
            Instruction::False => { let h = self.obj_heap.alloc_bool_instance(false); self.push_stack(h); }
            Instruction::Negate => unary_op!(self, neg),
            Instruction::Not => unary_op!(self, not),
            Instruction::Add => binary_op!(self, add),
            Instruction::Sub => binary_op!(self, sub),
            Instruction::Mul => binary_op!(self, mul),
            Instruction::Div => binary_op!(self, div),
            Instruction::Mod => binary_op!(self, mod),
            Instruction::FloorDiv => binary_op!(self, floordiv),
            Instruction::Equal => binary_op!(self, eq),
            Instruction::NotEqual => binary_op!(self, ne),
            Instruction::Greater => binary_op!(self, gt),
            Instruction::GreaterEqual => binary_op!(self, ge),
            Instruction::Less => binary_op!(self, lt),
            Instruction::LessEqual => binary_op!(self, le),
            Instruction::Pop => {
                self.pop_stack()?;
            }
            Instruction::JumpIfFalse(offset) => {
                let value = self.peek_stack(0)?;
                if !self.__bool__(value)? {
                    ip += offset;
                }
            }
            Instruction::Jump(offset) => {
                ip += offset;
            }
            Instruction::Loop(offset) => {
                ip -= offset;
            }

            Instruction::Call(arg_count) => {
                self.frame_mut()?.ip = ip;
                let callee = self.peek_stack(arg_count)?;
                self.call_value(callee, arg_count)?;
                return Ok(());
            }

            Instruction::CallKw { pos_count, kw_count, ref kw_names } => {
                self.frame_mut()?.ip = ip;
                let total_on_stack = pos_count + kw_count;
                let callee = self.peek_stack(total_on_stack)?;
                self.call_value_kw(callee, pos_count, kw_count, kw_names)?;
                return Ok(());
            }

            Instruction::Closure { function, upvalues } => {
                let closure_handle = self.obj_heap.alloc_closure(function);
                for uv_desc in upvalues {
                    let upvalue = if uv_desc.is_local {
                        let slot = self.frame()?.slots_start + uv_desc.index;
                        self.capture_upvalue(slot)?
                    } else {
                        let enclosing_closure = self.obj_heap
                            .get_closure(self.frame()?.closure)
                            .expect("must closure");
                        enclosing_closure.upvalues[uv_desc.index]
                    };
                    self.obj_heap
                        .get_closure_mut(closure_handle)
                        .expect("must closure")
                        .upvalues
                        .push(upvalue);
                }
                self.push_stack(closure_handle);
            }

            Instruction::GetUpvalue(slot) => {
                let closure_handle = self.frame()?.closure;
                let closure = self.obj_heap.get_closure(closure_handle).expect("must closure");
                let upvalue_handle = closure.upvalues[slot];
                let upvalue = self.obj_heap.get_upvalue(upvalue_handle).expect("must upvalue");
                let value = match upvalue.location {
                    Some(stack_slot) => self.stack[stack_slot],
                    None => upvalue.closed,
                };
                self.push_stack(value);
            }

            Instruction::SetUpvalue(slot) => {
                let closure_handle = self.frame()?.closure;
                let closure = self.obj_heap.get_closure(closure_handle).expect("must closure");
                let upvalue_handle = closure.upvalues[slot];
                let upvalue = self.obj_heap.get_upvalue(upvalue_handle).expect("must upvalue");
                let value = self.peek_stack(0)?;
                match upvalue.location {
                    Some(stack_slot) => self.stack[stack_slot] = value,
                    None => {
                        let uv = self.obj_heap.get_upvalue_mut(upvalue_handle).expect("must upvalue");
                        uv.closed = value;
                    }
                }
            }

            Instruction::CloseUpvalue => {
                let top_slot = self.stack.len() - 1;
                self.close_upvalues(top_slot)?;
                self.pop_stack()?;
            }

            Instruction::Class(class_name) => {
                let class = self.obj_heap.alloc_class(class_name);
                self.push_stack(class);
            }

            // ---- GetProperty — unified dispatch ----
            Instruction::GetProperty(field_name) => {
                let receiver = self.peek_stack(0)?;

                let obj = self.obj_heap.get(receiver); 
                match obj {
                    Object::Instance(instance) => {
                        if let ObjectInstanceData::Fields(fields) = &instance.data && let Some(value) = fields.get(&field_name).cloned() {
                            self.pop_stack()?;
                            self.push_stack(value);
                        } else {
                            let method = {
                                let class = self.get_class(instance.class)?;
                                class.methods
                                    .get(&field_name).copied()
                                    .ok_or_else(|| ExecuteError::UndefinedProperty(field_name.to_string()))?
                            };
                            let receiver = self.pop_stack()?;
                            let bound = self.obj_heap.alloc_bound_method(receiver, method);
                            self.push_stack(bound);
                        }
                    }
                    Object::Class(class) => {
                        if let Some(method) = class.methods.get(&field_name).copied() {
                            self.pop_stack()?; // discard the class
                            match method {
                                Method::User(closure_handle) => {
                                    self.push_stack(closure_handle);
                                }
                                Method::Native(handle) => {
                                    // Unbound: push the NativeFn object directly.
                                    self.push_stack(handle);
                                }
                            }
                            self.frame_mut()?.ip = ip;
                            return Ok(());
                        }
                        return Err(ExecuteError::UndefinedProperty(field_name.to_string()));
                    }
                    _ => Err(ExecuteError::UndefinedProperty(field_name.to_string()))?,
                }
            }

            Instruction::SetProperty(field_name) => {
                let value = self.peek_stack(0)?;
                let instance = self.peek_stack(1)?;
                let instance = self.get_instance_mut(instance)?;
                match &mut instance.data {
                    ObjectInstanceData::Fields(fields) => fields.insert(field_name, value),
                    _ => Err(ExecuteError::CannotSetProperty(self.value_type_name(value)))?,
                };

                let value = self.pop_stack()?;
                self.pop_stack()?;
                self.push_stack(value);
            }

            Instruction::Inherit => {
                let superclass = self.peek_stack(0)?;
                let subclass = self.peek_stack(1)?;
                let super_methods = {
                    let sc = self.get_class(superclass)?;
                    sc.methods.clone()
                };
                let sub = self.get_class_mut(subclass)?;
                sub.superclass = Some(superclass);
                for (name, method) in super_methods {
                    sub.methods.entry(name).or_insert(method);
                }
                self.pop_stack()?;
            }

            Instruction::Method(method_name) => {
                self.define_method(method_name)?;
            }

            // ---- Invoke — unified dispatch ----
            Instruction::Invoke(method_name, arg_count) => {
                let receiver = self.peek_stack(arg_count)?;

                // First, check if the receiver is an Instance with a field
                // named `method_name`.  If so, try to call it directly (this
                // makes module exports and dynamically-assigned callable fields
                // work naturally).
                let field_value = match self.obj_heap.get(receiver) {
                    Object::Instance(inst) => {
                        match &inst.data {
                            ObjectInstanceData::Fields(fields) => fields.get(&method_name).copied(),
                            _ => None,
                        }
                    }
                    _ => None,
                };

                if let Some(handle) = field_value {
                    // Replace the receiver slot with the field value and call it.
                    let index = self.callee_slot(arg_count);
                    self.stack[index] = handle;
                    self.frame_mut()?.ip = ip;
                    self.call_value(handle, arg_count)?;
                    return Ok(());
                }

                // Not a field — fall back to class method lookup.
                let class_handle = self.get_instance(receiver)?.class;

                let method = {
                    let class_ = self.get_class(class_handle)?;
                    class_.methods.get(&method_name).copied()
                        .ok_or_else(|| ExecuteError::UndefinedProperty(method_name.as_str().to_string()))?
                };

                match method {
                    Method::User(closure_handle) => {
                        self.frame_mut()?.ip = ip;
                        self.call_closure(closure_handle, arg_count + 1, false)?;
                        return Ok(());
                    }
                    Method::Native(handle) => {
                        let native_fn = self.get_native_fn(handle).expect("must fn").function;
                        self.call_native_fn(native_fn, arg_count + 1, false)?;
                    }
                }
            }

            Instruction::SuperInvoke(method_name, arg_count) => {
                let method = {
                    let receiver = self.peek_stack(arg_count)?;
                    let instance = self.get_instance(receiver)?;
                    let class = self.get_class(instance.class)?;
                    let superclass_handle = class
                        .superclass
                        .ok_or(ExecuteError::NoSuperclass)?;
                    let superclass = self.get_class(superclass_handle)?;
                    superclass
                        .methods
                        .get(&method_name)
                        .copied()
                        .ok_or_else(|| {
                            ExecuteError::UndefinedProperty(method_name.as_str().to_string())
                        })?
                };

                match method {
                    Method::User(closure_handle) => {
                        self.frame_mut()?.ip = ip;
                        self.call_closure(closure_handle, arg_count + 1, false)?;
                        return Ok(());
                    }
                    Method::Native(handle) => {
                        let native_fn = self.get_native_fn(handle).expect("must fn").function;
                        self.call_native_fn(native_fn, arg_count + 1, false)?;
                    }
                }
            }

            Instruction::BuildList(count) => {
                let mut items = vec![];
                for _ in 0..count {
                    items.push(self.pop_stack()?);
                }
                items.reverse();
                let list = self.obj_heap.alloc_list_instance(items);
                self.push_stack(list);
            }
            Instruction::BuildDict(count) => {
                let mut map: HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>> = HashMap::new();
                for _ in 0..count {
                    let val = self.pop_stack()?;
                    let key = self.pop_stack()?;
                    let hash = self.__hash__(key)?;
                    map.entry(hash).or_default().push((key, val));
                }
                let dict = self.obj_heap.alloc_dict_instance(map);
                self.push_stack(dict);
            }
            Instruction::BuildSet(count) => {
                let mut map: HashMap<u64, Vec<ObjectHandle>> = HashMap::new();
                for _ in 0..count {
                    let item = self.pop_stack()?;
                    let hash = self.__hash__(item)?;
                    map.entry(hash).or_default().push(item);
                }
                let set = self.obj_heap.alloc_set_instance(map);
                self.push_stack(set);
            }
            Instruction::IndexGet => {
                let index = self.pop_stack()?;
                let collection = self.pop_stack()?;
                let result = self.__getitem__(collection, index)?;
                self.push_stack(result);
            }
            Instruction::IndexSet => {
                let value = self.pop_stack()?;
                let index = self.pop_stack()?;
                let collection = self.pop_stack()?;
                let result = self.__setitem__(collection, index, value)?;
                self.push_stack(result);
            }

            Instruction::Import(file_path) => {
                self.frame_mut()?.ip = ip;
                let module = self.import_module(file_path.as_str())?;
                self.push_stack(module);
                return Ok(());
            }

            // ---- iteration ----
            Instruction::IterEnd => {
                self.push_stack(ObjectHandle::ITER_END);
            }

            Instruction::ForInIter => {
                let iterable = self.pop_stack()?;
                let iterator = self.__iter__(iterable)?;
                self.push_stack(iterator);
            }

            Instruction::ForInNext(offset) => {
                let iterator = self.peek_stack(0)?;
                let result = self.__next__(iterator)?;
                // dispatch_magic internally pushes/pops receiver; the iterator
                // on the stack (pushed by GetLocal) is untouched.
                if result.is_iter_end() {
                    ip += offset as usize;
                } else {
                    self.push_stack(result);
                }
            }
        }

        self.frame_mut()?.ip = ip;
        Ok(())
    }

    /// Handle virtual std module imports.
    ///
    /// Returns a module instance (Instance with Fields) containing the module's
    /// exports — just like a real file-based module would.  The returned value
    /// is indistinguishable from a compiled `.taro` module.
    pub fn import_std_module(&mut self, module_name: &str) -> ExecuteResult<ObjectHandle> {
        match module_name {
            "argparse" => {
                let source = include_str!("../std/argparse.taro");
                self.import_source_module(source, "std/argparse")
            }
            "logging" => {
                let source = include_str!("../std/logging.taro");
                self.import_source_module(source, "std/logging")
            }
            "fs" => self.create_fs_module(),
            "itertools" => {
                let source = include_str!("../std/itertools.taro");
                self.import_source_module(source, "std/itertools")
            }
            "json" => self.create_json_module(),
            "math" => self.create_math_module(),
            "net" => self.create_net_module(),
            "os" => self.create_os_module(),
            "random" => self.create_random_module(),
            "time" => self.create_time_module(),
            _ => Err(ExecuteError::ImportError(format!(
                "unknown std module '{module_name}'"
            ))),
        }
    }

    /// Invoke a method on a receiver synchronously, running its bytecode
    /// to completion and returning the result handle.
    fn invoke_method_sync(&mut self, receiver: ObjectHandle, method: ObjectHandle, extra_args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        self.push_stack(receiver);
        for &arg in extra_args {
            self.push_stack(arg);
        }
        let saved_frame_count = self.frames.len();
        let total_args = 1 + extra_args.len();
        self.call_closure(method, total_args, false)?;

        while self.frames.len() > saved_frame_count {
            self.step()?;
        }

        self.pop_stack()
    }

    pub fn reset(&mut self) {
        self.stack.clear();
        self.frames.clear();
    }

    #[inline]
    fn push_stack(&mut self, handle: ObjectHandle) {
        self.stack.push(handle);
    }

    #[inline]
    pub fn pop_stack(&mut self) -> ExecuteResult<ObjectHandle> {
        self.stack.pop().ok_or(ExecuteError::StackEmpty)
    }

    #[inline]
    pub fn peek_stack(&self, index: usize) -> ExecuteResult<ObjectHandle> {
        self.stack.iter().rev().nth(index).copied().ok_or(ExecuteError::StackEmpty)
    }

    /// Return the absolute stack index of the callee slot for a pending call
    /// with `arg_count` arguments already pushed above it.
    #[inline]
    fn callee_slot(&self, arg_count: usize) -> usize {
        self.stack.len() - arg_count - 1
    }

    fn call_value(&mut self, callee: ObjectHandle, arg_count: usize) -> ExecuteResult<()> {
        let obj = self.obj_heap.get(callee);
        match obj {
            Object::Closure(_) => self.call_closure(callee, arg_count, true),
            Object::Class(_) => {
                let init_method = {
                    let class = self.get_class(callee)?;
                    class.methods.get("__init__").copied()
                };
                let instance = self.obj_heap.alloc_fields_instance(callee, Default::default());
                let index = self.callee_slot(arg_count);
                self.stack[index] = instance;
                if let Some(method) = init_method {
                    match method {
                        Method::User(closure_handle) => {
                            self.call_closure(closure_handle, arg_count + 1, false)
                        }
                        Method::Native(handle) => {
                            let native_fn = self.get_native_fn(handle)?.function;
                            self.call_native_fn(native_fn, arg_count + 1, false)
                        }
                    }
                } else if arg_count != 0 {
                    Err(ExecuteError::ArgumentCountMismatch { expected: 0, got: arg_count })?;
                    unreachable!()
                } else {
                    Ok(())
                }
            }
            Object::Instance(_) => self.__call__(callee, arg_count)
                .map_err(|e| match e {
                    ExecuteError::NoImplementMethod(_, _) => ExecuteError::CanNotCall(self.value_type_name(callee)),
                    other => other,
                }),
            Object::BoundMethod(bound_method) => {
                let index = self.callee_slot(arg_count);
                self.stack[index] = bound_method.receiver;
                match &bound_method.method {
                    Method::User(closure_handle) => {
                        self.call_closure(*closure_handle, arg_count + 1, false)
                    }
                    Method::Native(handle) => {
                        let native_fn = self.get_native_fn(*handle)?.function;
                        self.call_native_fn(native_fn, arg_count + 1, false)
                    }
                }
            }
            Object::NativeFn(native_fn) => {
                self.call_native_fn(native_fn.function, arg_count, true)
            }
            _ => Err(ExecuteError::CanNotCall(self.value_type_name(callee)))
        }
    }

    /// Call a value with keyword arguments — reorder args to match parameter
    /// order and fill in defaults.
    fn call_value_kw(
        &mut self,
        callee: ObjectHandle,
        pos_count: usize,
        kw_count: usize,
        kw_names: &[ShrString],
    ) -> ExecuteResult<()> {
        let total_on_stack = pos_count + kw_count;
        let callee_idx = self.stack.len() - total_on_stack - 1;
        let obj = self.obj_heap.get(callee);

        match obj {
            Object::Class(_) => {
                // Class construction: get __init__ params, skipping `self`.
                let (param_names, arity, required_arity, defaults) = self.get_callable_info(callee)?;
                // param_names[0] is "self" — skip it for keyword matching.
                let user_param_names: Vec<ShrString> = param_names.iter().skip(1).cloned().collect();
                let user_arity = arity.saturating_sub(1);
                let _user_required = required_arity.saturating_sub(1);
                // defaults correspond to user params (not including self).
                let default_offset = user_arity.saturating_sub(defaults.len());

                let mut resolved: Vec<Option<ObjectHandle>> = vec![None; user_arity];

                for i in 0..pos_count.min(user_arity) {
                    resolved[i] = Some(self.stack[callee_idx + 1 + i]);
                }
                for (kw_idx, kw_name) in kw_names.iter().enumerate() {
                    let kw_val = self.stack[callee_idx + 1 + pos_count + kw_idx];
                    if let Some(param_idx) = user_param_names.iter().position(|n| n == kw_name) {
                        if resolved[param_idx].is_some() {
                            return Err(ExecuteError::DuplicateKeywordArg(kw_name.to_string()));
                        }
                        resolved[param_idx] = Some(kw_val);
                    } else {
                        return Err(ExecuteError::UnknownKeywordArg(kw_name.to_string()));
                    }
                }
                for i in 0..user_arity {
                    if resolved[i].is_none() {
                        if i >= default_offset {
                            resolved[i] = Some(defaults[i - default_offset]);
                        } else {
                            return Err(ExecuteError::MissingArgument(user_param_names[i].to_string()));
                        }
                    }
                }

                // Build new stack: callee + resolved args.
                self.stack.truncate(callee_idx);
                self.stack.push(callee);
                for arg_opt in &resolved {
                    self.stack.push(arg_opt.unwrap());
                }
                // call_value(Class) will create instance and call __init__.
                self.call_value(callee, user_arity)
            }
            _ => {
                // Closure, BoundMethod, etc.
                let (param_names, arity, _required_arity, defaults) = self.get_callable_info(callee)?;
                let default_offset = arity.saturating_sub(defaults.len());

                let mut resolved: Vec<Option<ObjectHandle>> = vec![None; arity];

                for i in 0..pos_count.min(arity) {
                    resolved[i] = Some(self.stack[callee_idx + 1 + i]);
                }
                for (kw_idx, kw_name) in kw_names.iter().enumerate() {
                    let kw_val = self.stack[callee_idx + 1 + pos_count + kw_idx];
                    if let Some(param_idx) = param_names.iter().position(|n| n == kw_name) {
                        if resolved[param_idx].is_some() {
                            return Err(ExecuteError::DuplicateKeywordArg(kw_name.to_string()));
                        }
                        resolved[param_idx] = Some(kw_val);
                    } else {
                        return Err(ExecuteError::UnknownKeywordArg(kw_name.to_string()));
                    }
                }
                for i in 0..arity {
                    if resolved[i].is_none() {
                        if i >= default_offset {
                            resolved[i] = Some(defaults[i - default_offset]);
                        } else {
                            return Err(ExecuteError::MissingArgument(param_names[i].to_string()));
                        }
                    }
                }

                self.stack.truncate(callee_idx);
                self.stack.push(callee);
                for arg_opt in &resolved {
                    self.stack.push(arg_opt.unwrap());
                }
                self.call_value(callee, arity)
            }
        }
    }

    /// Return (param_names, arity, required_arity, defaults) for a callable.
    fn get_callable_info(
        &self,
        callee: ObjectHandle,
    ) -> ExecuteResult<(Vec<ShrString>, usize, usize, Vec<ObjectHandle>)> {
        let obj = self.obj_heap.get(callee);
        match obj {
            Object::Closure(closure) => {
                let func = self.obj_heap.get_function(closure.function).expect("must function");
                Ok((
                    func.param_names.clone(),
                    func.arity,
                    func.required_arity,
                    func.defaults.clone(),
                ))
            }
            Object::BoundMethod(bound) => {
                let handle = match bound.method {
                    Method::User(h) => h,
                    Method::Native(_) => {
                        // Native methods don't support keyword args yet.
                        return Err(ExecuteError::UnsupportedMethodCall("keyword", "native method"));
                    }
                };
                let closure = self.obj_heap.get_closure(handle).expect("must closure");
                let func = self.obj_heap.get_function(closure.function).expect("must function");
                Ok((
                    func.param_names.clone(),
                    func.arity,
                    func.required_arity,
                    func.defaults.clone(),
                ))
            }
            Object::Class(class) => {
                // For class construction, look up __init__.
                if let Some(Method::User(init_handle)) = class.methods.get("__init__").copied() {
                    let closure = self.obj_heap.get_closure(init_handle).expect("must closure");
                    let func = self.obj_heap.get_function(closure.function).expect("must function");
                    Ok((
                        func.param_names.clone(),
                        func.arity,
                        func.required_arity,
                        func.defaults.clone(),
                    ))
                } else {
                    // No init — no params.
                    Ok((vec![], 0, 0, vec![]))
                }
            }
            _ => Err(ExecuteError::CanNotCall(self.value_type_name(callee))),
        }
    }

    /// Push a call frame for a user-defined closure.
    ///
    /// `callee_on_stack` distinguishes two stack layouts:
    /// - `true`:  the callee (closure) sits on the stack at slot 0 of the new
    ///   frame.  `arg_count` is the number of *explicit* arguments (does not
    ///   include the callee itself).
    /// - `false`: the callee slot has been replaced by a receiver (e.g. via
    ///   BoundMethod or Invoke).  Slot 0 is the receiver.  `arg_count` already
    ///   includes the receiver.
    fn call_closure(&mut self, closure: ObjectHandle, arg_count: usize, callee_on_stack: bool) -> ExecuteResult<()> {
        let closure_obj = self.obj_heap.get_closure(closure).expect("must closure");
        let function = self.obj_heap.get_function(closure_obj.function).expect("must function");
        // Allow arg_count in [required_arity, arity]; fill defaults for missing args.
        if arg_count < function.required_arity || arg_count > function.arity {
            Err(ExecuteError::ArgumentCountRange {
                min: function.required_arity,
                max: function.arity,
                got: arg_count,
            })?;
        }
        let slots_start = if callee_on_stack {
            self.stack.len() - arg_count - 1
        } else {
            self.stack.len() - arg_count
        };

        // Fill in default values for missing arguments.
        let num_missing = function.arity - arg_count;
        if num_missing > 0 {
            let default_start = function.arity - function.defaults.len();
            // Push defaults for missing trailing parameters.
            for i in (function.arity - num_missing)..function.arity {
                let default_idx = i - default_start;
                let default_val = function.defaults[default_idx];
                self.stack.push(default_val);
            }
        }

        self.frames.push(CallFrame { closure, ip: 0, slots_start });
        Ok(())
    }

    /// Call a native function.
    ///
    /// When `callee_on_stack` is true the callee is popped before the native
    /// function reads its arguments (see [`call_closure`] for the two stack layouts).
    fn call_native_fn(&mut self, native_func: NativeFunction, arg_count: usize, callee_on_stack: bool) -> ExecuteResult<()> {
        let actual_args = if callee_on_stack {
            let callee_idx = self.stack.len() - arg_count - 1;
            self.stack.remove(callee_idx);
            arg_count
        } else {
            arg_count
        };
        let result = match native_func {
            NativeFunction::Arity0(f) => {
                self.get_0_args(actual_args)?;
                f(self)?
            }
            NativeFunction::Arity1(f) => {
                let a0 = self.get_1_args(actual_args)?;
                f(self, a0)?
            }
            NativeFunction::Arity2(f) => {
                let (a0, a1) = self.get_2_args(actual_args)?;
                f(self, a0, a1)?
            }
            NativeFunction::Arity3(f) => {
                let (a0, a1, a2) = self.get_3_args(actual_args)?;
                f(self, a0, a1, a2)?
            }
            NativeFunction::Arity4(f) => {
                let (a0, a1, a2, a3) = self.get_4_args(actual_args)?;
                f(self, a0, a1, a2, a3)?
            }
            NativeFunction::Arity5(f) => {
                let (a0, a1, a2, a3, a4) = self.get_5_args(actual_args)?;
                f(self, a0, a1, a2, a3, a4)?
            }
            NativeFunction::Variadic(f) => {
                let args = self.get_args(actual_args).to_vec();
                f(self, &args)?
            }
        };
        self.stack.truncate(self.stack.len() - actual_args);
        self.push_stack(result);
        Ok(())
    }

    fn define_method(&mut self, name: ShrString) -> ExecuteResult<()> {
        let method_handle = self.peek_stack(0)?;
        let class_handle = self.peek_stack(1)?;
        let class = self.get_class_mut(class_handle)?;
        class.methods.insert(name, Method::User(method_handle));
        self.pop_stack()?;
        Ok(())
    }

    /// Capture a stack slot as an upvalue.
    fn capture_upvalue(&mut self, slot: usize) -> ExecuteResult<ObjectHandle> {
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
    fn close_upvalues(&mut self, last: usize) -> ExecuteResult<()> {
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

macro_rules! impl_getters {
    ($name:ident, $ty:ty) => {
        paste::paste! {
            #[inline]
            pub fn [<get_ $name>](&self, handle: ObjectHandle) -> ExecuteResult<&$ty> {
                self.obj_heap.[<get_ $name>](handle).ok_or_else(|| ExecuteError::TypeMismatch { expected: stringify!($name), found: self.value_type_name(handle) })
            }

            #[inline]
            pub fn [<get_ $name _mut>](&mut self, handle: ObjectHandle) -> ExecuteResult<&mut $ty> {
                let found_msg = self.value_type_name(handle);
                if let Some(v) = self.obj_heap.[<get_ $name _mut>](handle) {
                    Ok(v)
                } else {
                    Err(ExecuteError::TypeMismatch { expected: stringify!($name), found: found_msg })
                }
            }
        }
    };
}

impl VirtualMachine {
    impl_getters!(function, ObjectFunction);
    impl_getters!(native_fn, ObjectNativeFn);
    impl_getters!(closure, ObjectClosure);
    impl_getters!(upvalue, ObjectUpvalue);
    impl_getters!(instance, ObjectInstance);
    impl_getters!(class, ObjectClass);
    impl_getters!(bound_method, ObjectBoundMethod);
    impl_getters!(integer_instance, i64);
    impl_getters!(float_instance, f64);
    impl_getters!(bool_instance, bool);
    impl_getters!(string_instance, ShrString);
    impl_getters!(list_instance, Vec<ObjectHandle>);
    impl_getters!(dict_instance, HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>>);
    impl_getters!(set_instance, HashMap<u64, Vec<ObjectHandle>>);
    impl_getters!(fields_instance, HashMap<ShrString, ObjectHandle>);

    pub fn get_native_mut<T: crate::ToNativeData>(&mut self, handle: ObjectHandle) -> ExecuteResult<&mut T> {
        let found = self.value_type_name(handle);
        self.obj_heap
            .get_native_mut::<T>(handle)
            .ok_or_else(|| ExecuteError::TypeMismatch { expected: "native", found } )
    }

    pub fn get_native<T: crate::ToNativeData>(&self, handle: ObjectHandle) -> ExecuteResult<&T> {
        self.obj_heap
            .get_native::<T>(handle)
            .ok_or_else(|| ExecuteError::TypeMismatch { expected: "native", found: self.value_type_name(handle) } )
    }
}