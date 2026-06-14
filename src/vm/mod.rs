mod magic;
mod builtin;
mod error;
mod gc;
pub use error::*;
#[cfg(test)]
mod tests;
use crate::{NativeFn, Instruction, Method, Object, ObjectBoundMethod, ObjectNativeFn, ObjectClass, ObjectClosure, ObjectFunction, ObjectHandle, ObjectHeap, ObjectInstance, ObjectInstanceData, ObjectUpvalue, ShrString};
use std::collections::HashMap;

pub struct VirtualMachine {
    pub obj_heap: ObjectHeap,
    frames: Vec<CallFrame>,
    stack: Vec<ObjectHandle>,
    globals: HashMap<ShrString, ObjectHandle>,
    /// Sorted (by descending location) linked list of open upvalues.
    open_upvalues: Vec<ObjectHandle>,
    gc_threshold: usize,
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
                if self.frames.is_empty() {
                    return Ok(());
                }
                let result = self.pop_stack()?;
                self.close_upvalues(frame.slots_start)?;
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

                // Extract the class handle.
                let class_handle = self.get_instance(receiver)?.class;

                // Look up the method in the class.
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
                let mut items = vec![];
                for _ in 0..count {
                    let val = self.pop_stack()?;
                    let key = self.pop_stack()?;
                    items.push((key, val));
                }
                items.reverse();
                let dict = self.obj_heap.alloc_dict_instance(items);
                self.push_stack(dict);
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
        }

        self.frame_mut()?.ip = ip;
        Ok(())
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
                let instance = self.obj_heap.alloc_fields_instance(callee);
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
        if arg_count != function.arity {
            Err(ExecuteError::ArgumentCountMismatch { expected: function.arity, got: arg_count })?;
        }
        let slots_start = if callee_on_stack {
            self.stack.len() - arg_count - 1
        } else {
            self.stack.len() - arg_count
        };
        self.frames.push(CallFrame { closure, ip: 0, slots_start });
        Ok(())
    }

    /// Call a native function.
    ///
    /// When `callee_on_stack` is true the callee is popped before the native
    /// function reads its arguments (see [`call_closure`] for the two stack layouts).
    fn call_native_fn(&mut self, native_fn: NativeFn, arg_count: usize, callee_on_stack: bool) -> ExecuteResult<()> {
        let actual_args = if callee_on_stack {
            let callee_idx = self.stack.len() - arg_count - 1;
            self.stack.remove(callee_idx);
            arg_count
        } else {
            arg_count
        };
        let result = native_fn(self, actual_args)?;
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
    impl_getters!(dict_instance, Vec<(ObjectHandle, ObjectHandle)>);
    impl_getters!(fields_instance, std::collections::HashMap<ShrString, ObjectHandle>);
}