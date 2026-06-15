use crate::{NativeFunc, Method, ObjectHandle};

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
        self.register_native_fn("print", NativeFunc::var(VirtualMachine::print));
        self.register_native_fn("str",   NativeFunc::a1(VirtualMachine::str));
        self.register_native_fn("bool",  NativeFunc::a1(VirtualMachine::bool));
        self.register_native_fn("len",   NativeFunc::a1(VirtualMachine::len));
        self.register_native_fn("int",   NativeFunc::a1(VirtualMachine::int));
        self.register_native_fn("float", NativeFunc::a1(VirtualMachine::float));
        self.register_native_fn("type",  NativeFunc::a1(VirtualMachine::typeof_val));
        self.register_native_fn("input", NativeFunc::var(VirtualMachine::input));
        self.register_native_fn("abs",   NativeFunc::a1(VirtualMachine::abs));
        self.register_native_fn("min",   NativeFunc::var(VirtualMachine::min));
        self.register_native_fn("max",   NativeFunc::var(VirtualMachine::max));
        self.register_native_fn("clock", NativeFunc::a0(VirtualMachine::clock));
        self.register_native_fn("list",  NativeFunc::var(VirtualMachine::list));
        self.register_native_fn("dict",  NativeFunc::a0(VirtualMachine::dict));
    }

    pub fn register_builtins_class_method(&mut self) {
        self.register_int_builtins();
        self.register_float_builtins();
        self.register_string_builtins();
        self.register_list_builtins();
        self.register_dict_builtins();
        self.register_bool_builtins();
    }

    pub(crate) fn register_native_method(&mut self, class_handle: ObjectHandle, name: &'static str, function: NativeFunc) {
        let handle = self.obj_heap.alloc_native_fn(name, function);
        let class = self.obj_heap.get_class_mut(class_handle).expect("class");
        class.methods.insert(name.into(), Method::Native(handle));
    }

    fn register_native_fn(&mut self, name: &'static str, function: NativeFunc) {
        let function = self.obj_heap.alloc_native_fn(name, function);
        self.globals.insert(name.into(), function);
    }

    fn register_builtin_class(&mut self, name: &'static str, class: ObjectHandle) {
        self.globals.insert(name.into(), class);
    }
}
