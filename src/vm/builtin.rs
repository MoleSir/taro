use crate::NativeFunction;
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{Object, ObjectBytes, ObjectHandle, ObjectSet};
use std::collections::HashMap;

impl VirtualMachine {
    pub fn init_builtins(&mut self) {
        // ---- define builtin classes ----
        self.register_builtin_class("Int", self.obj_heap.int_class);
        self.register_builtin_class("Float", self.obj_heap.float_class);
        self.register_builtin_class("String", self.obj_heap.string_class);
        self.register_builtin_class("List", self.obj_heap.list_class);
        self.register_builtin_class("Dict", self.obj_heap.dict_class);
        self.register_builtin_class("Set", self.obj_heap.set_class);
        self.register_builtin_class("Bytes", self.obj_heap.bytes_class);
        self.register_builtin_class("Bool", self.obj_heap.bool_class);

        // ---- global native functions ----
        self.register_builtin_fn("print", NativeFunction::var(print));
        self.register_builtin_fn("len", NativeFunction::a1(len));
        self.register_builtin_fn("type", NativeFunction::a1(VirtualMachine::typeof_val));
        self.register_builtin_fn("input", NativeFunction::var(input));
        self.register_builtin_fn("abs", NativeFunction::a1(abs));
        self.register_builtin_fn("min", NativeFunction::var(min));
        self.register_builtin_fn("max", NativeFunction::var(max));
        self.register_builtin_fn("clock", NativeFunction::a0(clock));
        self.register_builtin_fn("exit", NativeFunction::a1(exit));

        self.register_builtin_fn("int", NativeFunction::a1(int));
        self.register_builtin_fn("float", NativeFunction::a1(float));
        self.register_builtin_fn("str", NativeFunction::a1(str));
        self.register_builtin_fn("bool", NativeFunction::a1(bool));
        self.register_builtin_fn("list", NativeFunction::var(list));
        self.register_builtin_fn("dict", NativeFunction::a0(dict));
        self.register_builtin_fn("set", NativeFunction::var(set));
        self.register_builtin_fn("bytes", NativeFunction::a1(bytes));

        // IterEnd sentinel — signals end of iteration in __next__.
        self.builtins.insert("IterEnd".into(), ObjectHandle::ITER_END);
        self.register_builtin_fn("is_iter_end", NativeFunction::a1(is_iter_end));
        self.register_builtin_fn("iter", NativeFunction::a1(iter));
        self.register_builtin_fn("next", NativeFunction::a1(next));
        self.register_builtin_fn("format", NativeFunction::var(format));
        self.register_builtin_fn("printf", NativeFunction::var(printf));
    }

    fn register_builtin_fn(&mut self, name: &'static str, function: NativeFunction) {
        let function = self.obj_heap.alloc_native_fn(name, function);
        self.builtins.insert(name.into(), function);
    }

    fn register_builtin_class(&mut self, name: &'static str, class: ObjectHandle) {
        self.builtins.insert(name.into(), class);
    }

    /// `type(value)` — for Instance, return the class object;
    /// otherwise the type name string.
    pub fn typeof_val(&mut self, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let obj = self.obj_heap.get(arg);
        match obj {
            Object::Instance(inst) => return Ok(inst.class),
            _ => {
                let name = self.obj_heap.type_of(arg);
                Ok(self.obj_heap.alloc_string_instance(name.into()))
            }
        }
    }
}

pub fn print(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    for (i, &arg) in args.iter().enumerate() {
        if i == 0 {
            print!("{}", vm.__str__(arg)?);
        } else {
            print!(" {}", vm.__str__(arg)?);
        }
    }
    println!("");
    Ok(ObjectHandle::NIL)
}

/// `input()` or `input("prompt")` — read a line from stdin.
pub fn input(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if let Some(&prompt) = args.first() {
        print!("{}", vm.__str__(prompt)?);
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map_err(|e| RuntimeErrorKind::IoError(format!("failed to read stdin: {}", e)))?;
    // Trim the trailing newline (and optional \r).
    if line.ends_with('\n') {
        line.pop();
    }
    if line.ends_with('\r') {
        line.pop();
    }
    Ok(vm.obj_heap.alloc_string_instance(line.into()))
}

/// `abs(value)` — return the absolute value of a number.
pub fn abs(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let bi = vm.obj_heap.expect_instance(arg)?;
    if let Some(v) = bi.data.as_any_ref().downcast_ref::<crate::object::ObjectInt>() {
        Ok(vm.obj_heap.alloc_integer_instance(v.value.wrapping_abs()))
    } else if let Some(v) = bi.data.as_any_ref().downcast_ref::<crate::object::ObjectFloat>() {
        Ok(vm.obj_heap.alloc_float_instance(v.value.abs()))
    } else {
        Err(RuntimeErrorKind::UnexpectedType("number", vm.obj_heap.type_of(arg)))
    }
}

/// `min(a, b, ...)` — return the smallest argument.
pub fn min(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 1, got: 0 });
    }
    let mut min_val = args[0];
    for &arg in &args[1..] {
        let cmp = vm.__lt__(arg, min_val)?;
        if vm.__bool__(cmp)? {
            min_val = arg;
        }
    }
    Ok(min_val)
}

/// `max(a, b, ...)` — return the largest argument.
pub fn max(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 1, got: 0 });
    }
    let mut max_val = args[0];
    for &arg in &args[1..] {
        let cmp = vm.__gt__(arg, max_val)?;
        if vm.__bool__(cmp)? {
            max_val = arg;
        }
    }
    Ok(max_val)
}

/// `clock()` — return elapsed wall-clock time in seconds (fractional).
pub fn clock(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    let dur = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    Ok(vm.obj_heap.alloc_float_instance(dur.as_secs_f64()))
}

pub fn len(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let n = vm.__len__(arg)?;
    Ok(vm.obj_heap.alloc_integer_instance(n))
}

/// `is_iter_end(value)` — return true if value is the iteration-end sentinel.
pub fn is_iter_end(vm: &mut VirtualMachine, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    Ok(vm.obj_heap.alloc_bool_instance(value.is_iter_end()))
}

pub fn next(vm: &mut VirtualMachine, iterator: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    vm.__next__(iterator)
}

pub fn iter(vm: &mut VirtualMachine, iterator: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    vm.__iter__(iterator)
}

pub fn str(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.__str__(arg)?;
    Ok(vm.obj_heap.alloc_string_instance(s))
}

pub fn bool(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let b = vm.__bool__(arg)?;
    Ok(vm.obj_heap.alloc_bool_instance(b))
}

pub fn int(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let n = vm.__int__(arg)?;
    Ok(vm.obj_heap.alloc_integer_instance(n))
}

pub fn float(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let n = vm.__float__(arg)?;
    Ok(vm.obj_heap.alloc_float_instance(n))
}

/// list
pub fn list(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    Ok(vm.obj_heap.alloc_list_instance(args.to_vec()))
}

/// dict
pub fn dict(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    Ok(vm.obj_heap.alloc_dict_instance(std::collections::HashMap::new()))
}

/// `set(args...)` — create a new set from the given arguments.
pub fn set(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    let set_handle = vm.obj_heap.alloc_set_instance(HashMap::new());
    for &item in args {
        ObjectSet::add(vm, set_handle, item)?;
    }
    Ok(set_handle)
}

/// `bytes(value)` — create bytes from a string or list of ints.
pub fn bytes(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    // Snapshot what we need to decide, then drop the immutable borrow.
    let is_string = vm
        .obj_heap
        .get_instance(arg)
        .map(|inst| inst.data.as_any_ref().downcast_ref::<crate::object::ObjectString>().is_some())
        .unwrap_or(false);
    let is_list = vm
        .obj_heap
        .get_instance(arg)
        .map(|inst| inst.data.as_any_ref().downcast_ref::<crate::object::ObjectList>().is_some())
        .unwrap_or(false);

    if is_string {
        let s = vm.obj_heap.get_string_instance(arg).expect("must string").as_str().to_string();
        ObjectBytes::from_string(vm, s.as_str())
    } else if is_list {
        ObjectBytes::from_list(vm, arg)
    } else {
        Err(RuntimeErrorKind::UnexpectedType("string or list of ints", vm.obj_heap.type_of(arg)))
    }
}

pub fn exit(vm: &mut VirtualMachine, arg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let n = vm.__int__(arg)?;
    std::process::exit(n as i32)
}

/// `format(fmt, args...)` — substitute `{}` placeholders with `__str__` of
/// each argument.  `{{` and `}}` escape to literal `{` / `}`.
pub fn format(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    let s = format_impl(vm, args)?;
    Ok(vm.obj_heap.alloc_string_instance(s.into()))
}

pub fn printf(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    let s = format_impl(vm, args)?;
    println!("{}", s);
    Ok(ObjectHandle::NIL)
}

fn format_impl(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<String> {
    if args.is_empty() {
        return Err(RuntimeErrorKind::FormatError("missing format string".into()));
    }
    let fmt = vm.obj_heap.expect_string(args[0])?;
    let fmt_str = fmt.as_str().to_string(); // drop the immutable borrow before __str__ below
    let value_args = &args[1..];

    let mut result = String::new();
    let mut chars = fmt_str.chars().peekable();
    let mut arg_idx = 0;
    let mut placeholder_count = 0;

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                match chars.peek() {
                    Some(&'{') => {
                        // Escaped "{{" → literal "{"
                        chars.next();
                        result.push('{');
                    }
                    Some(&'}') => {
                        // Placeholder "{}"
                        chars.next();
                        placeholder_count += 1;
                        if arg_idx >= value_args.len() {
                            return Err(RuntimeErrorKind::FormatError(format!(
                                "not enough arguments: format string has at least {} placeholder(s), got {} argument(s)",
                                placeholder_count,
                                value_args.len()
                            )));
                        }
                        let s = vm.__str__(value_args[arg_idx])?;
                        result.push_str(s.as_str());
                        arg_idx += 1;
                    }
                    _ => {
                        return Err(RuntimeErrorKind::FormatError("unclosed '{' — use '{{' for a literal '{'".into()));
                    }
                }
            }
            '}' => {
                match chars.peek() {
                    Some(&'}') => {
                        // Escaped "}}" → literal "}"
                        chars.next();
                        result.push('}');
                    }
                    _ => {
                        return Err(RuntimeErrorKind::FormatError("stray '}' — use '}}' for a literal '}'".into()));
                    }
                }
            }
            other => result.push(other),
        }
    }

    if arg_idx < value_args.len() {
        return Err(RuntimeErrorKind::FormatError(format!(
            "too many arguments: {} placeholder(s), {} argument(s)",
            placeholder_count,
            value_args.len()
        )));
    }

    Ok(result)
}
