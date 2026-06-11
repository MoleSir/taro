mod op;
mod builtin;
mod error;
pub use error::*;
#[cfg(test)]
mod tests;
use crate::{BuiltinFn, Chunk, Instruction, Object, ObjectHandle, ObjectHeap, ShrString, Value};
use std::collections::HashMap;

pub struct VirtualMachine {
    pub obj_heap: ObjectHeap,
    pub frames: Vec<CallFrame>,
    pub stack: Vec<Value>,
    pub globals: HashMap<ShrString, Value>,
}

/// A single function-call frame.  `slots_start` is the index into
/// [`VirtualMachine::stack`] where this frame's locals begin — it serves the
/// same role as the `Value* slots` pointer in the C interpreter, but without
/// raw pointers.
pub struct CallFrame {
    pub function: ObjectHandle,
    pub ip: usize,
    pub slots_start: usize,
}

macro_rules! binary_op {
    ($vm:ident, $f:ident) => {{
        let rhs = $vm.pop_stack()?;
        let lhs = $vm.pop_stack()?;
        let res = $vm.$f(&lhs, &rhs)?;
        $vm.push_stack(res);
    }};
}

macro_rules! unary_op {
    ($vm:ident, $f:ident) => {{
        let v = $vm.pop_stack()?;
        let res = $vm.$f(&v)?;
        $vm.push_stack(res);
    }};
}

impl VirtualMachine {
    pub fn new() -> Self {
        let mut vm = Self {
            obj_heap: ObjectHeap::new(),
            frames: vec![],
            stack: vec![],
            globals: HashMap::new(),
        };
        vm.register_builtins();
        vm
    }

    fn register_builtins(&mut self) {
        self.define_builtin_fn("print", builtin::print);
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
        // Slot 0 holds the script function itself — the same layout as a
        // normal call where the callee occupies slot 0.
        self.stack = vec![Value::Object(function)];
        self.frames = vec![CallFrame { function, ip: 0, slots_start: 0 }];
        self.run().map_err(InterpretError::Runtime)
    }

    pub fn run(&mut self) -> ExecuteResult<()> {
        loop {
            // Copy `ip` out of the frame so we can work with a local
            // variable — this avoids a lingering mutable borrow on
            // `self.frames` that would prevent access to `self.stack`.
            let mut ip = self.frame()?.ip;

            // Decode the next instruction.  `read_instruction` only
            // needs an immutable reference to the chunk, so this
            // doesn't conflict with anything.
            let inst = {
                let chunk = match self.obj_heap.get(self.frame()?.function) {
                    Object::Function(function) => &function.chunk,
                    _ => unreachable!(),
                };
                chunk.read_instruction(&mut ip)?
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
                    // Function return: pop the return value, clean up the
                    // callee's stack window, and push the result back onto
                    // the caller's stack.
                    let result = self.pop_stack()?;
                    self.stack.truncate(frame.slots_start);
                    self.push_stack(result);
                    continue; // skip writing ip back — it was the callee's ip
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
                    if !Self::is_truthy(self.peek_stack(0)?) {
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
                    // before we push the callee frame.  Otherwise the
                    // bottom-of-loop write-back would clobber the callee's
                    // ip = 0 with the caller's post-Call ip.
                    self.frame_mut()?.ip = ip;
                    let callee = self.peek_stack(arg_count)?.clone();
                    self.call_value(callee, arg_count)?;
                    continue; // skip writing ip back — callee frame is now active
                }
            }

            // Persist the (potentially modified) ip back into the frame.
            self.frame_mut()?.ip = ip;
        }
    }

    /// Replace the current script with a new chunk, preserving globals.
    /// Used by the REPL to run each line in a shared global scope.
    pub fn reset(&mut self) {
        let function = self.obj_heap.alloc_function("script", 0, Chunk::new());
        self.frames = vec![CallFrame { function, ip: 0, slots_start: 0 }];
        self.stack = vec![Value::Object(function)];
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
                Object::Function(_) => self.call(handle, arg_count),
                Object::BuiltinFn(builtin_fn) => {
                    let args = &self.stack[self.stack.len() - arg_count..];
                    let result = (builtin_fn.function)(args)?;
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
    
    fn call(&mut self, handle: ObjectHandle, arg_count: usize) -> ExecuteResult<()> {
        let function = self.obj_heap.get(handle).as_function().expect("must func");
        if arg_count != function.arity {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: function.arity, got: arg_count })?;
        }
        
        let frame = CallFrame { function: handle, ip: 0, slots_start: self.stack.len() - arg_count - 1 };
        self.frames.push(frame);
        Ok(())
    }

    fn define_builtin_fn(&mut self, name: impl Into<ShrString>, function: BuiltinFn) {
        let function = self.obj_heap.alloc_builtin_fn(function);
        self.globals.insert(name.into(), Value::Object(function));
    }
}

