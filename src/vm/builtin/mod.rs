use crate::{NativeFunction, Method, ObjectHandle};

use super::VirtualMachine;

mod function;
mod bool;
mod int;
mod float;
mod string;
mod list;
mod dict;

impl VirtualMachine {
    pub fn register_builtins(&mut self) {
        // ---- define builtin classes as globals ----
        self.register_builtin_class("Int", self.obj_heap.int_class);
        self.register_builtin_class("Float", self.obj_heap.float_class);
        self.register_builtin_class("String", self.obj_heap.string_class);
        self.register_builtin_class("List", self.obj_heap.list_class);
        self.register_builtin_class("Dict", self.obj_heap.dict_class);
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
    }

    pub fn register_builtins_class_method(&mut self) {
        self.register_int_builtins();
        self.register_float_builtins();
        self.register_string_builtins();
        self.register_list_builtins();
        self.register_dict_builtins();
        self.register_bool_builtins();
    }

    pub(crate) fn register_native_method(&mut self, class_handle: ObjectHandle, name: &'static str, function: NativeFunction) {
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
