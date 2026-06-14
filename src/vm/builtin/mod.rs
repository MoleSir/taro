use crate::{BuiltinFn, Method, ObjectHandle};

use super::VirtualMachine;

mod function;
mod nil;
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
        self.define_builtin_class("Int", self.int_class);
        self.define_builtin_class("Float", self.float_class);
        self.define_builtin_class("String", self.string_class);
        self.define_builtin_class("List", self.list_class);
        self.define_builtin_class("Dict", self.dict_class);
        self.define_builtin_class("Bool", self.bool_class);

        // ---- int methods ----
        let ic = self.int_class;
        self.reg_builtin_method(ic, "__neg__", VirtualMachine::int_neg);
        self.reg_builtin_method(ic, "__not__", VirtualMachine::int_not);
        self.reg_builtin_method(ic, "__add__", VirtualMachine::int_add);
        self.reg_builtin_method(ic, "__sub__", VirtualMachine::int_sub);
        self.reg_builtin_method(ic, "__mul__", VirtualMachine::int_mul);
        self.reg_builtin_method(ic, "__div__", VirtualMachine::int_div);
        self.reg_builtin_method(ic, "__eq__", VirtualMachine::int_eq);
        self.reg_builtin_method(ic, "__ne__", VirtualMachine::int_ne);
        self.reg_builtin_method(ic, "__gt__", VirtualMachine::int_gt);
        self.reg_builtin_method(ic, "__ge__", VirtualMachine::int_ge);
        self.reg_builtin_method(ic, "__lt__", VirtualMachine::int_lt);
        self.reg_builtin_method(ic, "__le__", VirtualMachine::int_le);
        self.reg_builtin_method(ic, "__str__", VirtualMachine::int_str);
        self.reg_builtin_method(ic, "__bool__", VirtualMachine::int_bool);
        self.reg_builtin_method(ic, "__int__", VirtualMachine::int_int);
        self.reg_builtin_method(ic, "__float__", VirtualMachine::int_float);

        // ---- float methods ----
        let fc = self.float_class;
        self.reg_builtin_method(fc, "__neg__", VirtualMachine::float_neg);
        self.reg_builtin_method(fc, "__not__", VirtualMachine::float_not);
        self.reg_builtin_method(fc, "__add__", VirtualMachine::float_add);
        self.reg_builtin_method(fc, "__sub__", VirtualMachine::float_sub);
        self.reg_builtin_method(fc, "__mul__", VirtualMachine::float_mul);
        self.reg_builtin_method(fc, "__div__", VirtualMachine::float_div);
        self.reg_builtin_method(fc, "__eq__", VirtualMachine::float_eq);
        self.reg_builtin_method(fc, "__ne__", VirtualMachine::float_ne);
        self.reg_builtin_method(fc, "__gt__", VirtualMachine::float_gt);
        self.reg_builtin_method(fc, "__ge__", VirtualMachine::float_ge);
        self.reg_builtin_method(fc, "__lt__", VirtualMachine::float_lt);
        self.reg_builtin_method(fc, "__le__", VirtualMachine::float_le);
        self.reg_builtin_method(fc, "__str__", VirtualMachine::float_str);
        self.reg_builtin_method(fc, "__bool__", VirtualMachine::float_bool);
        self.reg_builtin_method(fc, "__int__", VirtualMachine::float_int);
        self.reg_builtin_method(fc, "__float__", VirtualMachine::float_float);

        // ---- string methods ----
        let sc = self.string_class;
        self.reg_builtin_method(sc, "__not__", VirtualMachine::string_not);
        self.reg_builtin_method(sc, "__add__", VirtualMachine::string_add);
        self.reg_builtin_method(sc, "__eq__", VirtualMachine::string_eq);
        self.reg_builtin_method(sc, "__ne__", VirtualMachine::string_ne);
        self.reg_builtin_method(sc, "__gt__", VirtualMachine::string_gt);
        self.reg_builtin_method(sc, "__ge__", VirtualMachine::string_ge);
        self.reg_builtin_method(sc, "__lt__", VirtualMachine::string_lt);
        self.reg_builtin_method(sc, "__le__", VirtualMachine::string_le);
        self.reg_builtin_method(sc, "__str__", VirtualMachine::string_str);
        self.reg_builtin_method(sc, "__bool__", VirtualMachine::string_bool);
        self.reg_builtin_method(sc, "__int__", VirtualMachine::string_int);
        self.reg_builtin_method(sc, "__float__", VirtualMachine::string_float);
        self.reg_builtin_method(sc, "__len__", VirtualMachine::string_len);
        self.reg_builtin_method(sc, "__getitem__", VirtualMachine::string_getitem);

        // ---- list methods ----
        let lc = self.list_class;
        self.reg_builtin_method(lc, "__not__", VirtualMachine::list_not);
        self.reg_builtin_method(lc, "__add__", VirtualMachine::list_add);
        self.reg_builtin_method(lc, "__eq__", VirtualMachine::list_eq);
        self.reg_builtin_method(lc, "__ne__", VirtualMachine::list_ne);
        self.reg_builtin_method(lc, "__str__", VirtualMachine::list_str);
        self.reg_builtin_method(lc, "__bool__", VirtualMachine::list_bool);
        self.reg_builtin_method(lc, "__len__", VirtualMachine::list_len);
        self.reg_builtin_method(lc, "__getitem__", VirtualMachine::list_getitem);
        self.reg_builtin_method(lc, "__setitem__", VirtualMachine::list_setitem);
        self.reg_builtin_method(lc, "append", VirtualMachine::list_append);
        self.reg_builtin_method(lc, "pop", VirtualMachine::list_pop);
        self.reg_builtin_method(lc, "extend", VirtualMachine::list_extend);

        // ---- dict methods ----
        let dc = self.dict_class;
        self.reg_builtin_method(dc, "__not__", VirtualMachine::dict_not);
        self.reg_builtin_method(dc, "__str__", VirtualMachine::dict_str);
        self.reg_builtin_method(dc, "__bool__", VirtualMachine::dict_bool);
        self.reg_builtin_method(dc, "__len__", VirtualMachine::dict_len);
        self.reg_builtin_method(dc, "__getitem__", VirtualMachine::dict_getitem);
        self.reg_builtin_method(dc, "__setitem__", VirtualMachine::dict_setitem);
        self.reg_builtin_method(dc, "get", VirtualMachine::dict_get);
        self.reg_builtin_method(dc, "keys", VirtualMachine::dict_keys);
        self.reg_builtin_method(dc, "values", VirtualMachine::dict_values);
        self.reg_builtin_method(dc, "pop", VirtualMachine::dict_pop);

        // ---- bool methods ----
        let bc = self.bool_class;
        self.reg_builtin_method(bc, "__neg__", VirtualMachine::bool_neg);
        self.reg_builtin_method(bc, "__not__", VirtualMachine::bool_not);
        self.reg_builtin_method(bc, "__add__", VirtualMachine::bool_add);
        self.reg_builtin_method(bc, "__sub__", VirtualMachine::bool_sub);
        self.reg_builtin_method(bc, "__mul__", VirtualMachine::bool_mul);
        self.reg_builtin_method(bc, "__div__", VirtualMachine::bool_div);
        self.reg_builtin_method(bc, "__eq__", VirtualMachine::bool_eq);
        self.reg_builtin_method(bc, "__ne__", VirtualMachine::bool_ne);
        self.reg_builtin_method(bc, "__gt__", VirtualMachine::bool_gt);
        self.reg_builtin_method(bc, "__ge__", VirtualMachine::bool_ge);
        self.reg_builtin_method(bc, "__lt__", VirtualMachine::bool_lt);
        self.reg_builtin_method(bc, "__le__", VirtualMachine::bool_le);
        self.reg_builtin_method(bc, "__str__", VirtualMachine::bool_str);
        self.reg_builtin_method(bc, "__bool__", VirtualMachine::bool_bool);
        self.reg_builtin_method(bc, "__int__", VirtualMachine::bool_int);
        self.reg_builtin_method(bc, "__float__", VirtualMachine::bool_float);

        // ---- nil methods ----
        let nc = self.nil_class;
        self.reg_builtin_method(nc, "__not__", VirtualMachine::nil_not);
        self.reg_builtin_method(nc, "__eq__", VirtualMachine::nil_eq);
        self.reg_builtin_method(nc, "__ne__", VirtualMachine::nil_ne);
        self.reg_builtin_method(nc, "__str__", VirtualMachine::nil_str);
        self.reg_builtin_method(nc, "__bool__", VirtualMachine::nil_bool);

        // ---- global builtin functions ----
        self.define_builtin_fn("print", VirtualMachine::print);
        self.define_builtin_fn("str", VirtualMachine::str);
        self.define_builtin_fn("bool", VirtualMachine::bool);
        self.define_builtin_fn("len", VirtualMachine::len);
        self.define_builtin_fn("int", VirtualMachine::int);
        self.define_builtin_fn("float", VirtualMachine::float);
        self.define_builtin_fn("type", VirtualMachine::typeof_val);
        self.define_builtin_fn("input", VirtualMachine::input);
        self.define_builtin_fn("abs", VirtualMachine::abs);
        self.define_builtin_fn("min", VirtualMachine::min);
        self.define_builtin_fn("max", VirtualMachine::max);
        self.define_builtin_fn("clock", VirtualMachine::clock);
        self.define_builtin_fn("list", VirtualMachine::list);
        self.define_builtin_fn("dict", VirtualMachine::dict);
    }

    fn reg_builtin_method(&mut self, class_handle: ObjectHandle, name: &'static str, function: BuiltinFn) {
        let handle = self.obj_heap.alloc_builtin_fn(name, function);
        let class = self.obj_heap.get_class_mut(class_handle).expect("class");
        class.methods.insert(name.into(), Method::Builtin(handle));
    }

    fn define_builtin_fn(&mut self, name: &'static str, function: BuiltinFn) {
        let function = self.obj_heap.alloc_builtin_fn(name, function);
        self.globals.insert(name.into(), function);
    }

    fn define_builtin_class(&mut self, name: &'static str, class: ObjectHandle) {
        self.globals.insert(name.into(), class);
    }
}
