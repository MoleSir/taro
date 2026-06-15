use crate::{Object, ObjectHandle, ObjectInstanceData};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

impl VirtualMachine {
    pub fn print(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
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

    pub fn str(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = self.__str__(arg)?;
        Ok(self.obj_heap.alloc_string_instance(s))
    }

    pub fn bool(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let b = self.__bool__(arg)?;
        Ok(self.obj_heap.alloc_bool_instance(b))
    }

    pub fn len(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let n = self.__len__(arg)?;
        Ok(self.obj_heap.alloc_integer_instance(n))
    }

    pub fn int(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let n = self.__int__(arg)?;
        Ok(self.obj_heap.alloc_integer_instance(n))
    }

    pub fn float(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let n = self.__float__(arg)?;
        Ok(self.obj_heap.alloc_float_instance(n))
    }

    /// `type(value)` — for Instance, return the class object;
    /// otherwise the type name string.
    pub fn typeof_val(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
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
    pub fn input(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        if let Some(&prompt) = args.first() {
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
    pub fn abs(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let bi = self.get_instance(arg)?;
        match &bi.data {
            ObjectInstanceData::Integer(v) => Ok(self.obj_heap.alloc_integer_instance(v.wrapping_abs())),
            ObjectInstanceData::Float(v) => Ok(self.obj_heap.alloc_float_instance(v.abs())),
            _ => Err(ExecuteError::UnexpectedType("number", self.value_type_name(arg))),
        }
    }

    /// `min(a, b, ...)` — return the smallest argument.
    pub fn min(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        if args.is_empty() {
            return Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: 0 });
        }
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
    pub fn max(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        if args.is_empty() {
            return Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: 0 });
        }
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
    pub fn clock(&mut self) -> ExecuteResult<ObjectHandle> {
        let dur = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Ok(self.obj_heap.alloc_float_instance(dur.as_secs_f64()))
    }

    /// list
    pub fn list(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        Ok(self.obj_heap.alloc_list_instance(args.to_vec()))
    }

    /// dict
    pub fn dict(&mut self) -> ExecuteResult<ObjectHandle> {
        Ok(self.obj_heap.alloc_dict_instance(vec![]))
    }
}
