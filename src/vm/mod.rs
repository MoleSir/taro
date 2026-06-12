mod magic;
mod builtin;
mod error;
mod gc;
pub use error::*;
#[cfg(test)]
mod tests;
use crate::{BuiltinFn, Instruction, Object, ObjectHandle, ObjectHeap, ShrString, Value};
use std::collections::HashMap;

pub struct VirtualMachine {
    pub obj_heap: ObjectHeap,
    frames: Vec<CallFrame>,
    stack: Vec<Value>,
    globals: HashMap<ShrString, Value>,
    /// Sorted (by descending location) linked list of open upvalues.
    open_upvalues: Vec<ObjectHandle>,
    gc_threshold: usize,
}

/// A single function-call frame.  `slots_start` is the index into
/// [`VirtualMachine::stack`] where this frame's locals begin — it serves the
/// same role as the `Value* slots` pointer in the C interpreter, but without
/// raw pointers.
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
            let res = $vm.[<__ $f __>](&lhs, &rhs)?;
            $vm.push_stack(res);
        }}
    };
}

macro_rules! unary_op {
    ($vm:ident, $f:ident) => {
        paste::paste! {{ 
            let v = $vm.pop_stack()?;
            let res = $vm.[<__ $f __>](&v)?;
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
            gc_threshold: 1024 * 1026,
        };
        vm.register_builtins();
        vm
    }

    fn register_builtins(&mut self) {
        self.define_builtin_fn("print", VirtualMachine::print);
        self.define_builtin_fn("str", VirtualMachine::str);
        self.define_builtin_fn("bool", VirtualMachine::bool);
        self.define_builtin_fn("len", VirtualMachine::len);
        self.define_builtin_fn("int", VirtualMachine::int);
        self.define_builtin_fn("float", VirtualMachine::float);
        self.define_builtin_fn("type", VirtualMachine::typeof_val);
        self.define_builtin_fn("input", VirtualMachine::input);
        self.define_builtin_fn("abs", VirtualMachine::abs);
        self.define_builtin_fn("min", VirtualMachine::min);
        self.define_builtin_fn("max", VirtualMachine::max);
        self.define_builtin_fn("clock", VirtualMachine::clock);
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
    ///
    /// This is the main entry point for running scripts — it encapsulates the
    /// manual stack/frame setup that every caller otherwise has to duplicate.
    /// Globals and the object heap are preserved across calls, which is what
    /// the REPL relies on to share state between lines.
    pub fn interpret(&mut self, source: &str) -> Result<(), InterpretError> {
        let function = crate::compile::compile(source, &mut self.obj_heap)
            .map_err(InterpretError::Compile)?;
        self.interpret_function(function)
    }

    pub(crate) fn interpret_function(&mut self, function: ObjectHandle) -> Result<(), InterpretError> {
        let closure = self.obj_heap.alloc_closure(function);
        self.reset();
        self.push_stack(Value::Object(closure));
        self.call(closure, 0).expect("can't failed in script call");
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

    /// Advance the VM by one instruction.  This is the core of the
    /// interpreter loop, extracted so that synchronous method calls
    /// (e.g. `__str__`) can re-enter it from within a builtin.
    ///
    /// For normal instructions, writes the updated `ip` back into the
    /// current frame before returning.  Callers that change frames
    /// (`Return`, `Call`, `Invoke`) skip that write-back.
    fn step(&mut self) -> ExecuteResult<()> {
        // Copy `ip` out of the frame so we can work with a local
        // variable — this avoids a lingering mutable borrow on
        // `self.frames` that would prevent access to `self.stack`.
        let mut ip = self.frame()?.ip;

        // Decode the next instruction.  `read_instruction` only
        // needs an immutable reference to the chunk, so this
        // doesn't conflict with anything.
        let inst = {
            let closure = self.obj_heap.get_closure(self.frame()?.closure).expect("must closure");
            let function = self.obj_heap.get_function(closure.function).expect("must function");
            function.chunk.read_instruction(&mut ip)?
        };

        match inst {
            Instruction::Constant(value) => self.push_stack(value),
            Instruction::DefineGlobal(name) => {
                let value = self.pop_stack()?;
                self.globals.insert(name, value);
            }
            Instruction::GetGlobal(name) => {
                let value = self.globals
                    .get(&name)
                    .ok_or_else(|| ExecuteError::VariableNotFound(name.as_str().to_string()))?
                    .clone();
                self.push_stack(value);
            }
            Instruction::SetGlobal(name) => {
                let value = self.stack
                    .last()
                    .ok_or(ExecuteError::StackEmpty)?
                    .clone();
                self.globals.insert(name, value);
            }
            Instruction::GetLocal(slot) => {
                let base = self.frame()?.slots_start;
                let index = base + slot;
                let value = self.stack
                    .get(index)
                    .ok_or_else(|| ExecuteError::StackIndexOutOfRange(index))?
                    .clone();
                self.push_stack(value);
            }
            Instruction::SetLocal(slot) => {
                // Assignment is an expression — the value stays on the
                // stack after being written into the local slot.
                let base = self.frame()?.slots_start;
                let index = base + slot;
                let value = self.stack
                    .last()
                    .ok_or(ExecuteError::StackEmpty)?
                    .clone();
                if index >= self.stack.len() {
                    return Err(ExecuteError::StackIndexOutOfRange(index));
                }
                self.stack[index] = value;
            }
            Instruction::Return => {
                let frame = self.frames.pop().expect("not empty frame");
                if self.frames.is_empty() {
                    // Top-level script finished — the return value (if any)
                    // stays on the stack so callers can inspect it.
                    return Ok(());
                }
                // Function return: pop the return value, close any
                // upvalues referencing this frame, clean up the callee's
                // stack window, and push the result onto the caller's stack.
                let result = self.pop_stack()?;
                self.close_upvalues(frame.slots_start)?;
                self.stack.truncate(frame.slots_start);
                self.push_stack(result);
                return Ok(()); // was `continue` — skip writing callee's ip
            }
            Instruction::Nil => self.push_stack(()),
            Instruction::True => self.push_stack(true),
            Instruction::False => self.push_stack(false),
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
                let value = self.peek_stack(0)?.clone();
                if !self.__bool__(&value)? {
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
                // Save the caller's ip (already advanced past Call)
                // before we push the callee frame.
                self.frame_mut()?.ip = ip;
                let callee = self.peek_stack(arg_count)?.clone();
                self.call_value(callee, arg_count)?;
                return Ok(()); // was `continue` — callee frame is now active
            }

            Instruction::Closure { function, upvalues } => {
                let function_handle = function.as_object().expect("must object");
                let closure_handle = self.obj_heap.alloc_closure(function_handle);
                // Capture upvalues from the enclosing frame/closure.
                for uv_desc in upvalues {
                    let upvalue = if uv_desc.is_local {
                        // Capture from the current frame's stack.
                        let slot = self.frame()?.slots_start + uv_desc.index;
                        self.capture_upvalue(slot)?
                    } else {
                        // Capture from the enclosing closure's upvalue array.
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
                    Some(stack_slot) => self.stack[stack_slot].clone(),
                    None => upvalue.closed.clone(),
                };
                self.push_stack(value);
            }

            Instruction::SetUpvalue(slot) => {
                let closure_handle = self.frame()?.closure;
                let closure = self.obj_heap.get_closure(closure_handle).expect("must closure");
                let upvalue_handle = closure.upvalues[slot];
                let upvalue = self.obj_heap.get_upvalue(upvalue_handle).expect("must upvalue");
                let value = self.peek_stack(0)?.clone();
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
            Instruction::GetProperty(field_name) => {
                let instance = self.peek_stack(0)?.as_object()?;
                let instance = self.obj_heap.get_instance(instance)?;

                // try field
                match instance.fields.get(&field_name).cloned() {
                    Some(value) => {
                        self.pop_stack()?;
                        self.push_stack(value);
                    }
                    None => {
                        // try method
                        let class = self.obj_heap.get_class(instance.class)?;
                        match class.methods.get(&field_name).cloned() {
                            Some(method) => {
                                let receiver = self.peek_stack(0)?.clone();
                                let bound_method = self.obj_heap.alloc_bound_method(receiver, method);
                                self.pop_stack()?;
                                self.push_stack(bound_method);
                            }
                            None => {
                                Err(ExecuteError::UndefinedProperty(field_name.to_string()))?
                            }
                        }
                    }
                }
            }
            Instruction::SetProperty(field_name) => {
                let value = self.peek_stack(0)?.clone();
                let instance = self.peek_stack(1)?.as_object()?;
                let instance = self.obj_heap.get_instance_mut(instance)?;
                instance.fields.insert(field_name, value);

                let value = self.pop_stack()?;
                self.pop_stack()?;
                self.push_stack(value);
            }
            Instruction::Inherit => {
                let superclass = self.peek_stack(0)?.as_object()?;
                let subclass = self.peek_stack(1)?.as_object()?;
                // Copy all methods from superclass into subclass.
                // Subclass methods (defined later) will override these.
                let super_methods = {
                    let sc = self.obj_heap.get_class(superclass)?;
                    sc.methods.clone()
                };
                let sub = self.obj_heap.get_class_mut(subclass)?;
                sub.superclass = Some(superclass);
                for (name, method) in super_methods {
                    sub.methods.entry(name).or_insert(method);
                }
                self.pop_stack()?; // pop superclass
            }
            Instruction::Method(method_name) => {
                self.define_method(method_name)?;
            }

            Instruction::Invoke(method_name, arg_count) => {
                // Look up the method on the receiver's class.
                let method_handle = {
                    let receiver = self.peek_stack(arg_count)?.clone();
                    let instance_handle = receiver.as_object()?;
                    let instance = self.obj_heap.get_instance(instance_handle)?;
                    let class = self.obj_heap.get_class(instance.class)?;
                    class
                        .methods
                        .get(&method_name)
                        .ok_or_else(|| {
                            ExecuteError::UndefinedProperty(method_name.as_str().to_string())
                        })?
                        .clone()
                }; // all borrows released before calling call_method()

                // Save ip before switching to callee frame (same pattern as Call).
                // +1 for the receiver (explicit self), which is already on the stack.
                self.frame_mut()?.ip = ip;
                self.call_method(method_handle, arg_count + 1)?;
                return Ok(()); // was `continue` — callee frame is now active
            }
        }

        // Persist the (potentially modified) ip back into the frame.
        self.frame_mut()?.ip = ip;
        Ok(())
    }

    /// Invoke a method on a receiver synchronously, running its bytecode 
    /// to completion and returning the result value.
    /// TODO: gc?
    fn invoke_method_sync(&mut self, receiver: ObjectHandle, method: ObjectHandle, extra_args: &[Value]) -> ExecuteResult<Value> {
        let method_handle = method;

        // Push the receiver and extra args onto the stack.
        // call_method expects: stack[slots_start] = receiver, then args.
        self.push_stack(receiver);
        for arg in extra_args {
            self.push_stack(arg.clone());
        }
        let saved_frame_count = self.frames.len();
        let total_args = 1 + extra_args.len(); // receiver + explicit args
        self.call_method(method_handle, total_args)?;

        // Sub-loop: run until the method's frame (and any frames it pushes) unwind back to the caller.
        while self.frames.len() > saved_frame_count {
            self.step()?;
        }

        // The method's return value is now on top of the stack.
        self.pop_stack()
    }

    pub fn reset(&mut self) {
        self.stack.clear();
        self.frames.clear();
    }

    #[inline]
    fn push_stack(&mut self, value: impl Into<Value>) {
        self.stack.push(value.into());
    }

    #[inline]
    pub fn pop_stack(&mut self) -> ExecuteResult<Value> {
        self.stack.pop().ok_or(ExecuteError::StackEmpty)
    }

    #[inline]
    pub fn peek_stack(&self, index: usize) -> ExecuteResult<&Value> {
        self.stack.iter().rev().nth(index).ok_or(ExecuteError::StackEmpty)
    }

    fn call_value(&mut self, callee: Value, arg_count: usize) -> ExecuteResult<()> {
        if let Value::Object(handle) = callee {
            let obj = self.obj_heap.get(handle);
            match obj {
                Object::Closure(_) => self.call(handle, arg_count),
                Object::Class(_) => {
                    // Look up __init__() before allocating the instance (avoid borrow conflict).
                    let init_handle = {
                        let class = self.obj_heap.get_class(handle)?;
                        class.methods.get("__init__").copied()
                    };
                    let instance = self.obj_heap.alloc_instance(handle);
                    let index = self.stack.len() - arg_count - 1;
                    self.stack[index] = Value::Object(instance);
                    if let Some(init_handle) = init_handle {
                        // Call the __init__() method on the fresh instance.
                        // +1 for the receiver (explicit self), already on the stack.
                        self.call_method(init_handle, arg_count + 1)
                    } else if arg_count != 0 {
                        Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count, })?;
                        unreachable!()
                    } else {
                        Ok(())
                    }
                }
                Object::BoundMethod(bound_method) => {
                    let index = self.stack.len() - arg_count - 1;
                    self.stack[index] = bound_method.receiver.clone();
                    // +1 for the receiver (explicit self), already on the stack.
                    self.call_method(bound_method.method, arg_count + 1)
                }
                Object::BuiltinFn(builtin_fn) => {
                    let result = (builtin_fn.function)(self, arg_count)?;
                    self.stack.truncate(self.stack.len() - arg_count - 1);
                    self.push_stack(result);
                    Ok(())
                }
                _ => Err(ExecuteError::CanNotCall(callee.type_name()))
            }
        } else {
            Err(ExecuteError::CanNotCall(callee.type_name()))
        }
    }
    
    /// Push a call frame for a regular function call.
    ///
    /// The closure lives on the stack at `slots_start` (slot 0), followed by the
    /// arguments at slots 1..=arg_count.  This matches the layout produced by
    /// `OP_CALL`.
    fn call(&mut self, closure_handle: ObjectHandle, arg_count: usize) -> ExecuteResult<()> {
        let closure = self.obj_heap.get_closure(closure_handle).expect("must closure");
        let function = self.obj_heap.get_function(closure.function).expect("must function");
        if arg_count != function.arity {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: function.arity, got: arg_count })?;
        }

        let frame = CallFrame { closure: closure_handle, ip: 0, slots_start: self.stack.len() - arg_count - 1 };
        self.frames.push(frame);
        Ok(())
    }

    /// Push a call frame for a method call (via `OP_INVOKE`, bound-method,
    /// or class instantiation with `__init__()`).
    ///
    /// The receiver is already on the stack at `slots_start` (slot 0) and
    /// counts toward `arg_count` (explicit `self`).  There is no closure on
    /// the stack, unlike `call()`.
    fn call_method(&mut self, closure_handle: ObjectHandle, arg_count: usize) -> ExecuteResult<()> {
        let closure = self.obj_heap.get_closure(closure_handle).expect("must closure");
        let function = self.obj_heap.get_function(closure.function).expect("must function");
        if arg_count != function.arity {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: function.arity, got: arg_count })?;
        }

        let frame = CallFrame { closure: closure_handle, ip: 0, slots_start: self.stack.len() - arg_count };
        self.frames.push(frame);
        Ok(())
    }

    fn define_builtin_fn(&mut self, name: &'static str, function: BuiltinFn) {
        let function = self.obj_heap.alloc_builtin_fn(name, function);
        self.globals.insert(name.into(), Value::Object(function));
    }

    fn define_method(&mut self, name: ShrString) -> ExecuteResult<()> {
        let method = self.peek_stack(0)?.as_object()?;
        let class = self.peek_stack(1)?.as_object()?;
        let class = self.obj_heap.get_class_mut(class)?;
        class.methods.insert(name, method);
        self.pop_stack()?;
        Ok(())
    }

    /// Capture a stack slot as an upvalue.  If an open upvalue already
    /// references this slot, reuse it; otherwise allocate a new one.
    fn capture_upvalue(&mut self, slot: usize) -> ExecuteResult<ObjectHandle> {
        // Walk the linked list of open upvalues rooted at this slot (if any
        // exist) to see whether we already have one.
        let mut prev: Option<ObjectHandle> = None;
        let mut curr = self.open_upvalues.last().copied();
        while let Some(handle) = curr {
            let uv = self.obj_heap.get_upvalue(handle).expect("must upvalue");
            if uv.location.map_or(true, |loc| loc < slot) {
                break;
            }
            if uv.location == Some(slot) {
                return Ok(handle); // reuse existing
            }
            prev = curr;
            curr = uv.next;
        }

        // Allocate a new open upvalue and insert it into the list, keeping
        // the list sorted by descending location.
        let new_handle = self.obj_heap.alloc_upvalue(Some(slot));
        if let Some(prev_handle) = prev {
            self.obj_heap.get_upvalue_mut(prev_handle).expect("must upvalue").next = Some(new_handle);
        } else {
            // This becomes the new head.  We don't have a direct head
            // pointer — we rely on `open_upvalues` tracking.  For now,
            // just push onto the tracker.
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
            let value = self.stack[location].clone();
            let uv_mut = self.obj_heap.get_upvalue_mut(handle).expect("must upvalue");
            uv_mut.closed = value;
            uv_mut.location = None;
            self.open_upvalues.pop();
        }
        Ok(())
    }
}

