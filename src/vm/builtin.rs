use crate::{Object, ObjectHandle, ObjectInstanceData, ObjectSet};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};
use crate::{NativeFunction, Method};
use std::collections::HashMap;

impl VirtualMachine {
    pub fn register_builtins(&mut self) {
        // ---- define builtin classes as globals ----
        self.register_builtin_class("Int", self.obj_heap.int_class);
        self.register_builtin_class("Float", self.obj_heap.float_class);
        self.register_builtin_class("String", self.obj_heap.string_class);
        self.register_builtin_class("List", self.obj_heap.list_class);
        self.register_builtin_class("Dict", self.obj_heap.dict_class);
        self.register_builtin_class("Set", self.obj_heap.set_class);
        self.register_builtin_class("Bool", self.obj_heap.bool_class);

        // ---- global native functions ----
        self.register_native_fn("print", NativeFunction::var(VirtualMachine::print));
        self.register_native_fn("len",   NativeFunction::a1(VirtualMachine::len));
        self.register_native_fn("type",  NativeFunction::a1(VirtualMachine::typeof_val));
        self.register_native_fn("input", NativeFunction::var(VirtualMachine::input));
        self.register_native_fn("abs",   NativeFunction::a1(VirtualMachine::abs));
        self.register_native_fn("min",   NativeFunction::var(VirtualMachine::min));
        self.register_native_fn("max",   NativeFunction::var(VirtualMachine::max));
        self.register_native_fn("clock", NativeFunction::a0(VirtualMachine::clock));
        self.register_native_fn("exit", NativeFunction::a1(VirtualMachine::exit));

        self.register_native_fn("int",   NativeFunction::a1(VirtualMachine::int));
        self.register_native_fn("float", NativeFunction::a1(VirtualMachine::float));
        self.register_native_fn("str",   NativeFunction::a1(VirtualMachine::str));
        self.register_native_fn("bool",  NativeFunction::a1(VirtualMachine::bool));
        self.register_native_fn("list",  NativeFunction::var(VirtualMachine::list));
        self.register_native_fn("dict",  NativeFunction::a0(VirtualMachine::dict));
        self.register_native_fn("set",   NativeFunction::var(VirtualMachine::set));

        // IterEnd sentinel — signals end of iteration in __next__.
        self.globals.insert("IterEnd".into(), ObjectHandle::ITER_END);
        self.register_native_fn("is_iter_end", NativeFunction::a1(VirtualMachine::is_iter_end));
        self.register_native_fn("iter",   NativeFunction::a1(VirtualMachine::iter));
        self.register_native_fn("next",   NativeFunction::a1(VirtualMachine::next));
    }

    pub(crate) fn register_native_method(&mut self, class_handle: ObjectHandle, name: &'static str, function: impl Into<NativeFunction>) {
        let handle = self.obj_heap.alloc_native_fn(name, function);
        let class = self.obj_heap.get_class_mut(class_handle).expect("class");
        class.methods.insert(name.into(), Method::Native(handle));
    }

    fn register_native_fn(&mut self, name: &'static str, function: NativeFunction) {
        let function = self.obj_heap.alloc_native_fn(name, function);
        self.globals.insert(name.into(), function);
    }

    fn register_builtin_class(&mut self, name: &'static str, class: ObjectHandle) {
        self.globals.insert(name.into(), class);
    }
}


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

    pub fn len(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let n = self.__len__(arg)?;
        Ok(self.obj_heap.alloc_integer_instance(n))
    }

    /// `is_iter_end(value)` — return true if value is the iteration-end sentinel.
    pub fn is_iter_end(&mut self, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        Ok(self.obj_heap.alloc_bool_instance(value.is_iter_end()))
    }

    pub fn next(&mut self, iterator: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.__next__(iterator)
    }

    pub fn iter(&mut self, iterator: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.__iter__(iterator)
    }

    pub fn str(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = self.__str__(arg)?;
        Ok(self.obj_heap.alloc_string_instance(s))
    }

    pub fn bool(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let b = self.__bool__(arg)?;
        Ok(self.obj_heap.alloc_bool_instance(b))
    }

    pub fn int(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let n = self.__int__(arg)?;
        Ok(self.obj_heap.alloc_integer_instance(n))
    }

    pub fn float(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let n = self.__float__(arg)?;
        Ok(self.obj_heap.alloc_float_instance(n))
    }

    /// list
    pub fn list(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        Ok(self.obj_heap.alloc_list_instance(args.to_vec()))
    }

    /// dict
    pub fn dict(&mut self) -> ExecuteResult<ObjectHandle> {
        Ok(self.obj_heap.alloc_dict_instance(std::collections::HashMap::new()))
    }

    /// `set(args...)` — create a new set from the given arguments.
    pub fn set(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        let set_handle = self.obj_heap.alloc_set_instance(HashMap::new());
        for &item in args {
            ObjectSet::add(self, set_handle, item)?;
        }
        Ok(set_handle)
    }

    pub fn exit(&mut self, arg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let n = self.__int__(arg)?;
        std::process::exit(n as i32)
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
}
