pub mod types;
pub mod range;

use range::ObjectRangeIter;

use crate::{NativeFunction, ObjectClass};
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{Object, ObjectHandle};

impl VirtualMachine {
    pub fn init_builtins(&mut self) {
        // define builtin classes
        self.register_builtin_class("Int", self.obj_heap.int_class);
        self.register_builtin_class("Float", self.obj_heap.float_class);
        self.register_builtin_class("String", self.obj_heap.string_class);
        self.register_builtin_class("List", self.obj_heap.list_class);
        self.register_builtin_class("Dict", self.obj_heap.dict_class);
        self.register_builtin_class("Set", self.obj_heap.set_class);
        self.register_builtin_class("Bytes", self.obj_heap.bytes_class);
        self.register_builtin_class("Bool", self.obj_heap.bool_class);

        // global native functions
        self.register_builtin_fn("print", NativeFunction::var(print));
        self.register_builtin_fn("len", NativeFunction::a1(len));
        self.register_builtin_fn("type", NativeFunction::a1(VirtualMachine::typeof_val));
        self.register_builtin_fn("input", NativeFunction::var(input));
        self.register_builtin_fn("abs", NativeFunction::a1(abs));
        self.register_builtin_fn("min", NativeFunction::var(min));
        self.register_builtin_fn("max", NativeFunction::var(max));
        self.register_builtin_fn("sum", NativeFunction::var(sum));
        self.register_builtin_fn("id", NativeFunction::a1(id));
        self.register_builtin_fn("exit", NativeFunction::var(exit));
        self.register_builtin_fn("isinstance", NativeFunction::a2(isinstance));
        self.register_builtin_fn("format", NativeFunction::var(format));
        self.register_builtin_fn("printf", NativeFunction::var(printf));

        // types
        self.register_builtin_fn("int", NativeFunction::a1(types::int));
        self.register_builtin_fn("float", NativeFunction::a1(types::float));
        self.register_builtin_fn("str", NativeFunction::a1(types::str));
        self.register_builtin_fn("bool", NativeFunction::a1(types::bool));
        self.register_builtin_fn("list", NativeFunction::var(types::list));
        self.register_builtin_fn("dict", NativeFunction::a0(types::dict));
        self.register_builtin_fn("set", NativeFunction::var(types::set));
        self.register_builtin_fn("bytes", NativeFunction::a1(types::bytes));

        // IterEnd sentinel — signals end of iteration in __next__.
        self.builtins.insert("IterEnd".into(), ObjectHandle::ITER_END);
        self.register_builtin_fn("is_iter_end", NativeFunction::a1(is_iter_end));
        self.register_builtin_fn("iter", NativeFunction::a1(iter));
        self.register_builtin_fn("next", NativeFunction::a1(next));

        // range
        let mut range_iter_class = ObjectClass::new("RangeIter", self.obj_heap.builtins_module);
        range_iter_class.insert_method(&mut self.obj_heap, "__iter__", NativeFunction::a1(ObjectRangeIter::__iter__));
        range_iter_class.insert_method(&mut self.obj_heap, "__next__", NativeFunction::a1(ObjectRangeIter::__next__));
        range_iter_class.insert_method(&mut self.obj_heap, "__str__", NativeFunction::a1(ObjectRangeIter::__str__));
        range_iter_class.insert_method(&mut self.obj_heap, "__len__", NativeFunction::a1(ObjectRangeIter::__len__));
        let range_iter_class = self.obj_heap.alloc(range_iter_class);
        self.register_builtin_class("RangeIter", range_iter_class);
        self.register_builtin_fn("range", NativeFunction::var(range::range));
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

/// `isinstance(value, class)` — return true if value is an instance of class
/// or any of its subclasses (walking the superclass chain).
pub fn isinstance(vm: &mut VirtualMachine, obj: ObjectHandle, class: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    // Sentinel handles (nil, IterEnd) have no real class.
    if obj.is_nil() || obj.is_iter_end() {
        return Ok(vm.obj_heap.alloc_bool_instance(false));
    }
    let instance = match vm.obj_heap.get_instance(obj) {
        Some(inst) => inst,
        None => return Ok(vm.obj_heap.alloc_bool_instance(false)),
    };
    // Validate that `class` is actually a class handle.
    let _ = vm.obj_heap.expect_class(class)?;

    let mut cur = instance.class;
    loop {
        if cur == class {
            return Ok(vm.obj_heap.alloc_bool_instance(true));
        }
        let cls = vm.obj_heap.get_class(cur).expect("must class");
        match cls.superclass {
            Some(sc) => cur = sc,
            None => break,
        }
    }
    Ok(vm.obj_heap.alloc_bool_instance(false))
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

/// `sum(iterable, start=0)` — sum all elements of an iterable, using `__add__`.
/// Returns `start` (default 0) when the iterable is empty.
pub fn sum(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 1, got: 0 });
    }
    let (iterable, start) = if args.len() >= 2 {
        (args[0], args[1])
    } else {
        (args[0], vm.obj_heap.alloc_integer_instance(0))
    };

    let iterator = vm.__iter__(iterable)?;
    let mut accumulator = start;
    loop {
        let item = vm.__next__(iterator)?;
        if item.is_iter_end() {
            break;
        }
        accumulator = vm.__add__(accumulator, item)?;
    }
    Ok(accumulator)
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

pub fn exit(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    // let n = vm.__int__(arg)?;
    let code = if args.len() == 0 {
        0
    } else {
        *vm.obj_heap.expect_integer(args[0])? as i32
    };
    std::process::exit(code);
}

pub fn id(vm: &mut VirtualMachine, obj: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    Ok(vm.obj_heap.alloc_integer_instance(obj.0 as i64))
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
