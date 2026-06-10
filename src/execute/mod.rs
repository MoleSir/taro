mod op;
mod error;
pub use error::*;
#[cfg(test)]
mod tests;
use crate::{Chunk, Instruction, ShrString, Value};
use std::collections::HashMap;

pub struct VirtualMachine {
    pub chunk: Chunk,
    pub ip: usize,
    pub stack: Vec<Value>,
    pub globals: HashMap<ShrString, Value>,
}

macro_rules! binary_op {
    ($vm:ident, $f:ident) => {{
        let rhs = $vm.pop_stack()?;
        let lhs = $vm.pop_stack()?;
        let res = Value::$f(&lhs, &rhs)?;
        $vm.push_stack(res);
    }};
}

macro_rules! unary_op {
    ($vm:ident, $f:ident) => {{
        let v = $vm.pop_stack()?;
        let res = Value::$f(&v)?;
        $vm.push_stack(res);
    }};
}

impl VirtualMachine {
    pub fn new(chunk: Chunk) -> Self {
        Self {
            chunk,
            ip: 0,
            stack: vec![],
            globals: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> ExecuteResult<()> {
        loop {
            let inst = self.chunk.read_instruction(&mut self.ip)?;
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
                    let value = self.stack
                        .get(slot)
                        .ok_or_else(|| ExecuteError::StackIndexOutOfRange(slot))?
                        .clone();
                    self.push_stack(value);
                }
                Instruction::SetLocal(slot) => {
                    // Assignment is an expression — the value stays on the
                    // stack after being written into the local slot.
                    let value = self.stack
                        .last()
                        .ok_or(ExecuteError::StackEmpty)?
                        .clone();
                    if slot >= self.stack.len() {
                        return Err(ExecuteError::StackIndexOutOfRange(slot));
                    }
                    self.stack[slot] = value;
                }
                Instruction::Return => return Ok(()),
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
                Instruction::Print => {
                    let v = self.pop_stack()?;
                    println!("{v}");
                }
                Instruction::Pop => {
                    self.pop_stack()?;
                }
                Instruction::JumpIfFalse(offset) => {
                    if !Value::is_truthy(self.peek_stack(0)?) {
                        self.ip += offset;
                    }
                }
                Instruction::Jump(offset) => {
                    self.ip += offset;
                }
                Instruction::Loop(offset) => {
                    self.ip -= offset;
                }
            }
        }
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
}

