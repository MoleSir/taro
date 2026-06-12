use std::collections::HashMap;

use crate::{vm::VirtualMachine, Object, Value};
use super::{ExecuteError, ExecuteResult};

macro_rules! get_args {
    ($vm:ident, $arg_count:ident) => {
        &$vm.stack[$vm.stack.len() - $arg_count..]
    };
}

macro_rules! get_1_arg {
    ($vm:ident, $arg_count:ident) => {{
        if $arg_count != 1 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: $arg_count })?;
        }
        let args = get_args!($vm, $arg_count);
        args[0].clone()
    }};
}

#[allow(unused)]
macro_rules! get_2_arg {
    ($vm:ident, $arg_count:ident) => {{
        if $arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 2, got: $arg_count })?;
        }
        let args = get_args!($vm, $arg_count);
        (args[0].clone(), args[1].clone())
    }};
}

#[allow(unused)]
macro_rules! get_3_arg {
    ($vm:ident, $arg_count:ident) => {{
        if $arg_count != 2 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 3, got: $arg_count })?;
        }
        let args = get_args!($vm, $arg_count);
        (args[0].clone(), args[1].clone(), args[2].clone())
    }};
}

impl VirtualMachine {
    pub fn print(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let args: Vec<Value> = get_args!(self, arg_count).to_vec();
        for (i, arg) in args.iter().enumerate() {
            if i == 0 {
                print!("{}", self.__str__(arg)?);
            } else {
                print!(" {}", self.__str__(arg)?);
            }
        }
        println!("");
        Ok(Value::Nil)
    }

    pub fn str(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let arg = get_1_arg!(self, arg_count);
        self.__str__(&arg).map(Value::String)
    }

    pub fn bool(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let arg = get_1_arg!(self, arg_count);
        self.__bool__(&arg).map(Value::Bool)
    }

    pub fn len(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let arg = get_1_arg!(self, arg_count);
        self.__len__(&arg).map(Value::Integer)
    }

    pub fn int(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let arg = get_1_arg!(self, arg_count);
        self.__int__(&arg).map(Value::Integer)
    }

    pub fn float(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let arg = get_1_arg!(self, arg_count);
        self.__float__(&arg).map(Value::Float)
    }

    /// `type(value)` — for instances, return the class object; otherwise the type name string.
    pub fn typeof_val(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let arg = get_1_arg!(self, arg_count);
        match &arg {
            Value::Object(h) => {
                if let Object::Instance(instance) = self.obj_heap.get(*h) {
                    return Ok(Value::Object(instance.class));
                }
                Ok(Value::String(self.value_type_name(&arg).into()))
            }
            _ => Ok(Value::String(self.value_type_name(&arg).into()))
        }
    }

    /// `input()` or `input("prompt")` — read a line from stdin.
    pub fn input(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        if arg_count > 0 {
            let prompt = get_1_arg!(self, arg_count);
            print!("{}", self.__str__(&prompt)?);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|e| ExecuteError::IoError(format!("failed to read stdin: {}", e)))?;
        // Trim the trailing newline (and optional \r).
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }
        Ok(Value::String(line.into()))
    }

    /// `abs(value)` — return the absolute value of a number.
    pub fn abs(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let arg = get_1_arg!(self, arg_count);
        match arg {
            Value::Integer(v) => Ok(Value::Integer(v.wrapping_abs())),
            Value::Float(v) => Ok(Value::Float(v.abs())),
            ref other => Err(ExecuteError::UnexpectType("number", self.value_type_name(other)))
        }
    }

    /// `min(a, b, ...)` — return the smallest argument.
    pub fn min(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        if arg_count == 0 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: 0 })?;
        }
        let args = get_args!(self, arg_count).to_vec();
        let mut min_val = args[0].clone();
        for arg in &args[1..] {
            let cmp = self.__lt__(arg, &min_val)?;
            if self.__bool__(&cmp)? {
                min_val = arg.clone();
            }
        }
        Ok(min_val)
    }

    /// `max(a, b, ...)` — return the largest argument.
    pub fn max(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        if arg_count == 0 {
            Err(ExecuteError::ArgmentCountUnmatch { expcted: 1, got: 0 })?;
        }
        let args = get_args!(self, arg_count).to_vec();
        let mut max_val = args[0].clone();
        for arg in &args[1..] {
            let cmp = self.__gt__(arg, &max_val)?;
            if self.__bool__(&cmp)? {
                max_val = arg.clone();
            }
        }
        Ok(max_val)
    }

    /// `clock()` — return elapsed wall-clock time in seconds (fractional).
    pub fn clock(&mut self, _arg_count: usize) -> ExecuteResult<Value> {
        let dur = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Ok(Value::Float(dur.as_secs_f64()))
    }

    /// list 
    pub fn list(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        let items: Vec<Value> = get_args!(self, arg_count).to_vec();
        Ok(Value::Object(self.obj_heap.alloc_list(items)))
    }

    /// dict 
    pub fn dict(&mut self, _arg_count: usize) -> ExecuteResult<Value> {
        Ok(Value::Object(self.obj_heap.alloc_dict(HashMap::new())))
    }
}
