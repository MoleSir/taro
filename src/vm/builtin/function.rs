use crate::{Object, ObjectHandle, ObjectInstanceData};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

macro_rules! get_args {
    ($vm:ident, $arg_count:ident) => {
        &$vm.stack[$vm.stack.len() - $arg_count..]
    };
}

macro_rules! get_1_arg {
    ($vm:ident, $arg_count:ident) => {{
        if $arg_count != 1 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: $arg_count })?;
        }
        let args = get_args!($vm, $arg_count);
        args[0]
    }};
}

#[allow(unused)]
macro_rules! get_2_arg {
    ($vm:ident, $arg_count:ident) => {{
        if $arg_count != 2 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 2, got: $arg_count })?;
        }
        let args = get_args!($vm, $arg_count);
        (args[0], args[1])
    }};
}

#[allow(unused)]
macro_rules! get_3_arg {
    ($vm:ident, $arg_count:ident) => {{
        if $arg_count != 3 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 3, got: $arg_count })?;
        }
        let args = get_args!($vm, $arg_count);
        (args[0], args[1], args[2])
    }};
}

impl VirtualMachine {
    pub fn print(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let args = get_args!(self, arg_count).to_vec();
        for (i, &arg) in args.iter().enumerate() {
            if i == 0 {
                print!("{}", self.__str__(arg)?);
            } else {
                print!(" {}", self.__str__(arg)?);
            }
        }
        println!("");
        Ok(ObjectHandle::NIL)
    }

    pub fn str(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let arg = get_1_arg!(self, arg_count);
        let s = self.__str__(arg)?;
        Ok(self.obj_heap.alloc_string_instance(s))
    }

    pub fn bool(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let arg = get_1_arg!(self, arg_count);
        let b = self.__bool__(arg)?;
        Ok(self.obj_heap.alloc_bool_instance(b))
    }

    pub fn len(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let arg = get_1_arg!(self, arg_count);
        let n = self.__len__(arg)?;
        Ok(self.obj_heap.alloc_integer_instance(n))
    }

    pub fn int(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let arg = get_1_arg!(self, arg_count);
        let n = self.__int__(arg)?;
        Ok(self.obj_heap.alloc_integer_instance(n))
    }

    pub fn float(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let arg = get_1_arg!(self, arg_count);
        let n = self.__float__(arg)?;
        Ok(self.obj_heap.alloc_float_instance(n))
    }

    /// `type(value)` — for Instance, return the class object;
    /// otherwise the type name string.
    pub fn typeof_val(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let arg = get_1_arg!(self, arg_count);
        let obj = self.obj_heap.get(arg);
        match obj {
            Object::Instance(inst) => return Ok(inst.class),
            _ => {
                let name = self.value_type_name(arg);
                Ok(self.obj_heap.alloc_string_instance(name.into()))
            }
        }
    }

    /// `input()` or `input("prompt")` — read a line from stdin.
    pub fn input(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count > 0 {
            let prompt = get_1_arg!(self, arg_count);
            print!("{}", self.__str__(prompt)?);
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
        Ok(self.obj_heap.alloc_string_instance(line.into()))
    }

    /// `abs(value)` — return the absolute value of a number.
    pub fn abs(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let arg = get_1_arg!(self, arg_count);
        let bi = self.get_instance(arg)?;
        match &bi.data {
            ObjectInstanceData::Integer(v) => Ok(self.obj_heap.alloc_integer_instance(v.wrapping_abs())),
            ObjectInstanceData::Float(v) => Ok(self.obj_heap.alloc_float_instance(v.abs())),
            _ => Err(ExecuteError::UnexpectedType("number", self.value_type_name(arg))),
        }
    }

    /// `min(a, b, ...)` — return the smallest argument.
    pub fn min(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count == 0 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: 0 })?;
        }
        let args = get_args!(self, arg_count).to_vec();
        let mut min_val = args[0];
        for &arg in &args[1..] {
            let cmp = self.__lt__(arg, min_val)?;
            if self.__bool__(cmp)? {
                min_val = arg;
            }
        }
        Ok(min_val)
    }

    /// `max(a, b, ...)` — return the largest argument.
    pub fn max(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        if arg_count == 0 {
            Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: 0 })?;
        }
        let args = get_args!(self, arg_count).to_vec();
        let mut max_val = args[0];
        for &arg in &args[1..] {
            let cmp = self.__gt__(arg, max_val)?;
            if self.__bool__(cmp)? {
                max_val = arg;
            }
        }
        Ok(max_val)
    }

    /// `clock()` — return elapsed wall-clock time in seconds (fractional).
    pub fn clock(&mut self, _arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let dur = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Ok(self.obj_heap.alloc_float_instance(dur.as_secs_f64()))
    }

    /// list
    pub fn list(&mut self, arg_count: usize) -> ExecuteResult<ObjectHandle> {
        let items: Vec<ObjectHandle> = get_args!(self, arg_count).to_vec();
        Ok(self.obj_heap.alloc_list_instance(items))
    }

    /// dict
    pub fn dict(&mut self, _arg_count: usize) -> ExecuteResult<ObjectHandle> {
        Ok(self.obj_heap.alloc_dict_instance(vec![]))
    }
}
