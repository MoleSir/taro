use crate::{NativeFn, Method, ObjectHandle};

use super::VirtualMachine;

mod function;
mod bool;
mod int;
mod float;
mod string;
mod list;
mod dict;

mod utils;

impl VirtualMachine {
    pub fn register_builtins(&mut self) {
        // ---- define builtin classes as globals ----
        self.define_builtin_class("Int", self.obj_heap.int_class);
        self.define_builtin_class("Float", self.obj_heap.float_class);
        self.define_builtin_class("String", self.obj_heap.string_class);
        self.define_builtin_class("List", self.obj_heap.list_class);
        self.define_builtin_class("Dict", self.obj_heap.dict_class);
        self.define_builtin_class("Bool", self.obj_heap.bool_class);

        // ---- per-type method registration ----
        self.register_int_builtins();
        self.register_float_builtins();
        self.register_string_builtins();
        self.register_list_builtins();
        self.register_dict_builtins();
        self.register_bool_builtins();

        // ---- global native functions ----
        self.define_native_fn("print", VirtualMachine::print);
        self.define_native_fn("str", VirtualMachine::str);
        self.define_native_fn("bool", VirtualMachine::bool);
        self.define_native_fn("len", VirtualMachine::len);
        self.define_native_fn("int", VirtualMachine::int);
        self.define_native_fn("float", VirtualMachine::float);
        self.define_native_fn("type", VirtualMachine::typeof_val);
        self.define_native_fn("input", VirtualMachine::input);
        self.define_native_fn("abs", VirtualMachine::abs);
        self.define_native_fn("min", VirtualMachine::min);
        self.define_native_fn("max", VirtualMachine::max);
        self.define_native_fn("clock", VirtualMachine::clock);
        self.define_native_fn("list", VirtualMachine::list);
        self.define_native_fn("dict", VirtualMachine::dict);
    }

    fn reg_native_method(&mut self, class_handle: ObjectHandle, name: &'static str, function: NativeFn) {
        let handle = self.obj_heap.alloc_native_fn(name, function);
        let class = self.obj_heap.get_class_mut(class_handle).expect("class");
        class.methods.insert(name.into(), Method::Native(handle));
    }

    fn define_native_fn(&mut self, name: &'static str, function: NativeFn) {
        let function = self.obj_heap.alloc_native_fn(name, function);
        self.globals.insert(name.into(), function);
    }

    fn define_builtin_class(&mut self, name: &'static str, class: ObjectHandle) {
        self.globals.insert(name.into(), class);
    }
}
