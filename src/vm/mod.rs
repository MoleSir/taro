mod magic;
mod builtin;
pub mod builtin_methods;
mod error;
mod gc;
pub use error::*;
#[cfg(test)]
mod tests;
use crate::{BuiltinFn, Instruction, Method, Object, ObjectHandle, ObjectHeap, ShrString, Value};
use std::collections::HashMap;

pub struct VirtualMachine {
    pub obj_heap: ObjectHeap,
    frames: Vec<CallFrame>,
    stack: Vec<Value>,
    globals: HashMap<ShrString, Value>,
    /// Sorted (by descending location) linked list of open upvalues.
    open_upvalues: Vec<ObjectHandle>,
    gc_threshold: usize,
    /// Builtin class handles — allocated once at startup.
    pub list_class: ObjectHandle,
    pub dict_class: ObjectHandle,
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
            // Allocated immediately below, then methods registered in register_builtins().
            list_class: ObjectHandle(0),
            dict_class: ObjectHandle(0),
        };
        // Allocate the builtin class objects first so they are valid GC roots.
        vm.list_class = vm.obj_heap.alloc_class("list");
        vm.dict_class = vm.obj_heap.alloc_class("dict");
        vm.register_builtins();
        vm
    }

    fn register_builtins(&mut self) {
        // ---- builtin class methods ----
        {
            let list = self.obj_heap.get_class_mut(self.list_class)
                .expect("list_class is a Class");
            list.methods.insert("append".into(), Method::Builtin(VirtualMachine::list_append));
            list.methods.insert("pop".into(), Method::Builtin(VirtualMachine::list_pop));
            list.methods.insert("extend".into(), Method::Builtin(VirtualMachine::list_extend));
        }
        {
            let dict = self.obj_heap.get_class_mut(self.dict_class)
                .expect("dict_class is a Class");
            dict.methods.insert("get".into(), Method::Builtin(VirtualMachine::dict_get));
            dict.methods.insert("keys".into(), Method::Builtin(VirtualMachine::dict_keys));
            dict.methods.insert("values".into(), Method::Builtin(VirtualMachine::dict_values));
            dict.methods.insert("pop".into(), Method::Builtin(VirtualMachine::dict_pop));
        }

        // ---- global builtin functions ----
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
        self.define_builtin_fn("list", VirtualMachine::list);
        self.define_builtin_fn("dict", VirtualMachine::dict);
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

    /// Advance the VM by one instruction.
    fn step(&mut self) -> ExecuteResult<()> {
        let mut ip = self.frame()?.ip;

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
                    return Ok(());
                }
                let result = self.pop_stack()?;
                self.close_upvalues(frame.slots_start)?;
                self.stack.truncate(frame.slots_start);
                self.push_stack(result);
                return Ok(());
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
                self.frame_mut()?.ip = ip;
                let callee = self.peek_stack(arg_count)?.clone();
                self.call_value(callee, arg_count)?;
                return Ok(());
            }

            Instruction::Closure { function, upvalues } => {
                let function_handle = function.as_object().expect("must object");
                let closure_handle = self.obj_heap.alloc_closure(function_handle);
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

            // ---- GetProperty — unified dispatch ----
            Instruction::GetProperty(field_name) => {
                let receiver_val = self.peek_stack(0)?.clone();
                let receiver_handle = receiver_val.as_object()?;

                // Extract the class handle (and optional field value for Instance).
                let (class_handle, field_value) = {
                    let obj = self.obj_heap.get(receiver_handle);
                    match obj {
                        Object::Instance(inst) => {
                            let val = inst.fields.get(&field_name).cloned();
                            (inst.class, val)
                        }
                        Object::List(list) => (list.class, None),
                        Object::Dict(dict) => (dict.class, None),
                        _ => Err(ExecuteError::UndefinedProperty(field_name.to_string()))?,
                    }
                }; // immutable borrow released

                if let Some(value) = field_value {
                    self.pop_stack()?;
                    self.push_stack(value);
                } else {
                    let method = {
                        let class = self.obj_heap.get_class(class_handle)?;
                        class.methods.get(&field_name).cloned()
                            .ok_or_else(|| ExecuteError::UndefinedProperty(field_name.to_string()))?
                    };
                    let receiver = self.pop_stack()?;
                    let bound = self.obj_heap.alloc_bound_method(receiver, method);
                    self.push_stack(bound);
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
                let super_methods = {
                    let sc = self.obj_heap.get_class(superclass)?;
                    sc.methods.clone()
                };
                let sub = self.obj_heap.get_class_mut(subclass)?;
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
                let receiver = self.peek_stack(arg_count)?.clone();
                let receiver_handle = receiver.as_object()?;

                // Extract the class handle.
                let class_handle = {
                    let obj = self.obj_heap.get(receiver_handle);
                    match obj {
                        Object::Instance(inst) => inst.class,
                        Object::List(list) => list.class,
                        Object::Dict(dict) => dict.class,
                        _ => Err(ExecuteError::UndefinedProperty(method_name.as_str().to_string()))?,
                    }
                };

                // Look up the method in the class.
                let method = {
                    let class = self.obj_heap.get_class(class_handle)?;
                    class.methods.get(&method_name).cloned()
                        .ok_or_else(|| ExecuteError::UndefinedProperty(method_name.as_str().to_string()))?
                };

                match method {
                    Method::User(closure_handle) => {
                        self.frame_mut()?.ip = ip;
                        self.call_method(closure_handle, arg_count + 1)?;
                        return Ok(());
                    }
                    Method::Builtin(builtin_fn) => {
                        let result = builtin_fn(self, arg_count + 1)?;
                        self.stack.truncate(self.stack.len() - arg_count - 1);
                        self.push_stack(result);
                    }
                }
            }

            Instruction::SuperInvoke(method_name, arg_count) => {
                let method = {
                    let receiver = self.peek_stack(arg_count)?.clone();
                    let instance_handle = receiver.as_object()?;
                    let instance = self.obj_heap.get_instance(instance_handle)?;
                    let class = self.obj_heap.get_class(instance.class)?;
                    let superclass_handle = class
                        .superclass
                        .ok_or(ExecuteError::NoSuperclass)?;
                    let superclass = self.obj_heap.get_class(superclass_handle)?;
                    superclass
                        .methods
                        .get(&method_name)
                        .cloned()
                        .ok_or_else(|| {
                            ExecuteError::UndefinedProperty(method_name.as_str().to_string())
                        })?
                };

                match method {
                    Method::User(closure_handle) => {
                        self.frame_mut()?.ip = ip;
                        self.call_method(closure_handle, arg_count + 1)?;
                        return Ok(());
                    }
                    Method::Builtin(builtin_fn) => {
                        let result = builtin_fn(self, arg_count + 1)?;
                        self.stack.truncate(self.stack.len() - arg_count - 1);
                        self.push_stack(result);
                    }
                }
            }

            Instruction::BuildList(count) => {
                let mut items = vec![];
                for _ in 0..count {
                    items.push(self.pop_stack()?);
                }
                items.reverse();
                let list = self.obj_heap.alloc_list(self.list_class, items);
                self.push_stack(list);
            }
            Instruction::BuildDict(count) => {
                let mut items = HashMap::new();
                for _ in 0..count {
                    let val = self.pop_stack()?;
                    let key = self.pop_stack()?;
                    items.insert(key, val);
                }
                let dict = self.obj_heap.alloc_dict(self.dict_class, items);
                self.push_stack(dict);
            }
            Instruction::IndexGet => {
                let index = self.pop_stack()?;
                let collection = self.pop_stack()?;
                let result = self.__getitem__(&collection, &index)?;
                self.push_stack(result);
            }
            Instruction::IndexSet => {
                let value = self.pop_stack()?;
                let index = self.pop_stack()?;
                let collection = self.pop_stack()?;
                let result = self.__setitem__(&collection, &index, &value)?;
                self.push_stack(result);
            }
        }

        self.frame_mut()?.ip = ip;
        Ok(())
    }

    /// Invoke a method on a receiver synchronously, running its bytecode
    /// to completion and returning the result value.
    fn invoke_method_sync(&mut self, receiver: ObjectHandle, method: ObjectHandle, extra_args: &[Value]) -> ExecuteResult<Value> {
        self.push_stack(receiver);
        for arg in extra_args {
            self.push_stack(arg.clone());
        }
        let saved_frame_count = self.frames.len();
        let total_args = 1 + extra_args.len();
        self.call_method(method, total_args)?;

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
                    let init_method = {
                        let class = self.obj_heap.get_class(handle)?;
                        class.methods.get("__init__").cloned()
                    };
                    let instance = self.obj_heap.alloc_instance(handle);
                    let index = self.stack.len() - arg_count - 1;
                    self.stack[index] = Value::Object(instance);
                    if let Some(method) = init_method {
                        match method {
                            Method::User(closure_handle) => {
                                self.call_method(closure_handle, arg_count + 1)
                            }
                            Method::Builtin(_) => {
                                unreachable!("__init__ on builtin classes is not supported")
                            }
                        }
                    } else if arg_count != 0 {
                        Err(ExecuteError::ArgmentCountUnmatch { expcted: 0, got: arg_count })?;
                        unreachable!()
                    } else {
                        Ok(())
                    }
                }
                Object::BoundMethod(bound_method) => {
                    let index = self.stack.len() - arg_count - 1;
                    self.stack[index] = bound_method.receiver.clone();
                    match &bound_method.method {
                        Method::User(closure_handle) => {
                            self.call_method(*closure_handle, arg_count + 1)
                        }
                        Method::Builtin(builtin_fn) => {
                            let result = (*builtin_fn)(self, arg_count + 1)?;
                            self.stack.truncate(self.stack.len() - arg_count - 1);
                            self.push_stack(result);
                            Ok(())
                        }
                    }
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

    /// Push a call frame for a method call (user-defined closure only).
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
        let method_handle = self.peek_stack(0)?.as_object()?;
        let class_handle = self.peek_stack(1)?.as_object()?;
        let class = self.obj_heap.get_class_mut(class_handle)?;
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
            let value = self.stack[location].clone();
            let uv_mut = self.obj_heap.get_upvalue_mut(handle).expect("must upvalue");
            uv_mut.closed = value;
            uv_mut.location = None;
            self.open_upvalues.pop();
        }
        Ok(())
    }
}
