use crate::{Chunk, Instruction, ObjectHandle, ObjectHeap};
use super::VirtualMachine;

/// Build a chunk and run it: creates VM first, then calls `build` with VM's heap.
fn run_chunk(build: impl FnOnce(&mut Chunk, &mut ObjectHeap)) -> VirtualMachine {
    let mut vm = VirtualMachine::new();
    let heap_ptr = &mut vm.obj_heap as *mut ObjectHeap;
    let mut chunk = Chunk::new();
    unsafe { build(&mut chunk, &mut *heap_ptr); }
    let function = vm.obj_heap.alloc_function("script", 0, chunk);
    vm.interpret_function(function).unwrap();
    vm
}

/// Helper: get integer value from an instance handle.
fn get_int(vm: &VirtualMachine, handle: ObjectHandle) -> i64 {
    *vm.obj_heap.get_integer_instance(handle).unwrap()
}

/// Helper: get float value.
fn get_float(vm: &VirtualMachine, handle: ObjectHandle) -> f64 {
    *vm.obj_heap.get_float_instance(handle).unwrap()
}

/// Helper: get bool value.
fn get_bool(vm: &VirtualMachine, handle: ObjectHandle) -> bool {
    *vm.obj_heap.get_bool_instance(handle).unwrap()
}

/// Helper: check nil.
fn is_nil(handle: ObjectHandle) -> bool { handle.is_nil() }

#[test]
pub fn test_base_arith() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_float_instance(3.4)), h);
        c.write_instruction(Instruction::Constant(h.alloc_float_instance(1.2)), h);
        c.write_instruction(Instruction::Add, h);
        c.write_instruction(Instruction::Constant(h.alloc_float_instance(5.6)), h);
        c.write_instruction(Instruction::Div, h);
        c.write_instruction(Instruction::Negate, h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    let f = get_float(&vm, r);
    assert!((f + 0.82142857).abs() < 0.001);
}

#[test]
pub fn test_global_variable() {
    let _vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::DefineGlobal("x".into()), h);
        c.write_instruction(Instruction::GetGlobal("print".into()), h);
        c.write_instruction(Instruction::GetGlobal("x".into()), h);
        c.write_instruction(Instruction::Call(1), h);
        c.write_instruction(Instruction::Pop, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(99)), h);
        c.write_instruction(Instruction::SetGlobal("x".into()), h);
        c.write_instruction(Instruction::GetGlobal("print".into()), h);
        c.write_instruction(Instruction::GetGlobal("x".into()), h);
        c.write_instruction(Instruction::Call(1), h);
        c.write_instruction(Instruction::Pop, h);
        c.write_instruction(Instruction::Return, h);
    });
}

#[test]
pub fn test_local_variable_get_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(20)), h);
        c.write_instruction(Instruction::GetLocal(1), h);
        c.write_instruction(Instruction::GetLocal(2), h);
        c.write_instruction(Instruction::Add, h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 30);
}

#[test]
pub fn test_local_variable_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::SetLocal(1), h);
        c.write_instruction(Instruction::Pop, h);
        c.write_instruction(Instruction::GetLocal(1), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_scopes() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::GetLocal(1), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_jump_if_false() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::True, h);
        c.write_instruction(Instruction::JumpIfFalse(5), h);
        c.write_instruction(Instruction::Pop, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_while_loop() {
    // Test while loop via compilation (return isn't allowed at top level).
    let mut vm = VirtualMachine::new();
    vm.interpret("{ var i = 0; while (i < 3) { i = i + 1; } }").unwrap();
    // The script returns nil implicitly — verify execution succeeded.
}

#[test]
pub fn test_function_call() {
    let mut vm = run_chunk(|c, h| {
        let mut f = Chunk::new();
        f.write_instruction(Instruction::GetLocal(1), h);
        f.write_instruction(Instruction::GetLocal(2), h);
        f.write_instruction(Instruction::Add, h);
        f.write_instruction(Instruction::Return, h);
        let fn_h = h.alloc_function("add", 2, f);
        let cl_h = h.alloc_closure(fn_h);
        c.write_instruction(Instruction::Constant(cl_h), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(20)), h);
        c.write_instruction(Instruction::Call(2), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 30);
}

#[test]
pub fn test_string_concat() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("hello".into())), h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance(" world".into())), h);
        c.write_instruction(Instruction::Add, h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "hello world");
}

#[test]
pub fn test_string_equality() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), h);
        c.write_instruction(Instruction::Equal, h);
        c.write_instruction(Instruction::Return, h);
    });
    assert!({let r=vm.pop_stack().unwrap(); get_bool(&vm, r)});
}

#[test]
pub fn test_string_not_equality() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("xyz".into())), h);
        c.write_instruction(Instruction::NotEqual, h);
        c.write_instruction(Instruction::Return, h);
    });
    assert!({let r=vm.pop_stack().unwrap(); get_bool(&vm, r)});
}

#[test]
pub fn test_len_string() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("len".into()), h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("hello".into())), h);
        c.write_instruction(Instruction::Call(1), h);
        c.write_instruction(Instruction::Return, h);
    });
    assert_eq!({let r=vm.pop_stack().unwrap(); get_int(&vm, r)}, 5);
}

#[test]
pub fn test_type_of_string() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("type".into()), h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("hi".into())), h);
        c.write_instruction(Instruction::Call(1), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert!(matches!(vm.obj_heap.get(r), crate::Object::Class(_)));
}

#[test]
pub fn test_bool_negate() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::True, h);
        c.write_instruction(Instruction::Not, h);
        c.write_instruction(Instruction::Return, h);
    });
    assert!(!{let r=vm.pop_stack().unwrap(); get_bool(&vm, r)});
}

#[test]
pub fn test_bool_truthy() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(0)), h);
        c.write_instruction(Instruction::Not, h);
        c.write_instruction(Instruction::Return, h);
    });
    assert!({let r=vm.pop_stack().unwrap(); get_bool(&vm, r)});
}

#[test]
pub fn test_build_list() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(3)), h);
        c.write_instruction(Instruction::BuildList(3), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_list_instance(r).unwrap().len(), 3);
}

#[test]
pub fn test_list_index_get() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), h);
        c.write_instruction(Instruction::BuildList(1), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(0)), h);
        c.write_instruction(Instruction::IndexGet, h);
        c.write_instruction(Instruction::Return, h);
    });
    assert_eq!({let r=vm.pop_stack().unwrap(); get_int(&vm, r)}, 10);
}

#[test]
pub fn test_list_index_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), h);
        c.write_instruction(Instruction::BuildList(1), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(0)), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::IndexSet, h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_list_append_method() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), h);
        c.write_instruction(Instruction::BuildList(1), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), h);
        c.write_instruction(Instruction::Invoke("append".into(), 1), h);
        c.write_instruction(Instruction::Return, h);
    });
    assert_eq!({let r=vm.pop_stack().unwrap(); get_int(&vm, r)}, 2);
}

#[test]
pub fn test_build_dict() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("a".into())), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("b".into())), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), h);
        c.write_instruction(Instruction::BuildDict(2), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_dict_instance(r).unwrap().len(), 2);
}

#[test]
pub fn test_dict_index_get() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::BuildDict(1), h);
        c.write_instruction(Instruction::Constant(k), h);
        c.write_instruction(Instruction::IndexGet, h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_dict_get_method() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::BuildDict(1), h);
        c.write_instruction(Instruction::Constant(k), h);
        c.write_instruction(Instruction::Invoke("get".into(), 1), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_dict_get_missing() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("x".into())), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::BuildDict(1), h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("y".into())), h);
        c.write_instruction(Instruction::Invoke("get".into(), 1), h);
        c.write_instruction(Instruction::Return, h);
    });
    assert!({let r=vm.pop_stack().unwrap(); is_nil(r)});
}

#[test]
pub fn test_dict_keys_method() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("a".into())), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), h);
        c.write_instruction(Instruction::BuildDict(1), h);
        c.write_instruction(Instruction::Invoke("keys".into(), 0), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_list_instance(r).unwrap().len(), 1);
}

#[test]
pub fn test_dict_pop_method() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::BuildDict(1), h);
        c.write_instruction(Instruction::Constant(k), h);
        c.write_instruction(Instruction::Invoke("pop".into(), 1), h);
        c.write_instruction(Instruction::Return, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_class_instantiate() {
    let mut vm = run_chunk(|c, h| {
        let cls = h.alloc_class("Foo");
        c.write_instruction(Instruction::Constant(cls), h);
        c.write_instruction(Instruction::Call(0), h);
        c.write_instruction(Instruction::Return, h);
    });
    assert!(matches!({let r=vm.pop_stack().unwrap(); vm.obj_heap.get(r)}, crate::Object::Instance(_)));
}

#[test]
pub fn test_class_with_method() {
    let mut vm = run_chunk(|c, h| {
        let cls = h.alloc_class("Calc");
        let mut mc = Chunk::new();
        mc.write_instruction(Instruction::GetLocal(1), h);
        mc.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), h);
        mc.write_instruction(Instruction::Mul, h);
        mc.write_instruction(Instruction::Return, h);
        let mfn = h.alloc_function("double", 2, mc);
        let mcl = h.alloc_closure(mfn);
        h.get_class_mut(cls).unwrap().methods.insert("double".into(), crate::Method::User(mcl));
        c.write_instruction(Instruction::Constant(cls), h);
        c.write_instruction(Instruction::Call(0), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(5)), h);
        c.write_instruction(Instruction::Invoke("double".into(), 1), h);
        c.write_instruction(Instruction::Return, h);
    });
    assert_eq!({let r=vm.pop_stack().unwrap(); get_int(&vm, r)}, 10);
}

#[test]
pub fn test_builtin_abs() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("abs".into()), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(-5)), h);
        c.write_instruction(Instruction::Call(1), h);
        c.write_instruction(Instruction::Return, h);
    });
    assert_eq!({let r=vm.pop_stack().unwrap(); get_int(&vm, r)}, 5);
}

#[test]
pub fn test_builtin_min() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("min".into()), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(5)), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(-3)), h);
        c.write_instruction(Instruction::Call(3), h);
        c.write_instruction(Instruction::Return, h);
    });
    assert_eq!({let r=vm.pop_stack().unwrap(); get_int(&vm, r)}, -3);
}

#[test]
pub fn test_builtin_type() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("type".into()), h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), h);
        c.write_instruction(Instruction::Call(1), h);
        c.write_instruction(Instruction::Return, h);
    });
    match {let r=vm.pop_stack().unwrap(); vm.obj_heap.get(r)} {
        crate::Object::Class(c) => assert_eq!(c.name.as_str(), "Int"),
        _ => panic!(),
    }
}

#[test]
pub fn test_super_invoke() {
    let mut vm = run_chunk(|c, h| {
        let base = h.alloc_class("Base");
        let mut bc = Chunk::new();
        bc.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), h);
        bc.write_instruction(Instruction::Return, h);
        let bm = h.alloc_function("m", 1, bc);
        let bm_cl = h.alloc_closure(bm);
        h.get_class_mut(base).unwrap().methods.insert("m".into(), crate::Method::User(bm_cl));

        let derived = h.alloc_class("Derived");
        h.get_class_mut(derived).unwrap().superclass = Some(base);
        let mut dc = Chunk::new();
        dc.write_instruction(Instruction::GetLocal(0), h);
        dc.write_instruction(Instruction::SuperInvoke("m".into(), 0), h);
        dc.write_instruction(Instruction::Return, h);
        let dm = h.alloc_function("m", 1, dc);
        let dm_cl = h.alloc_closure(dm);
        h.get_class_mut(derived).unwrap().methods.insert("m".into(), crate::Method::User(dm_cl));

        c.write_instruction(Instruction::Constant(derived), h);
        c.write_instruction(Instruction::Call(0), h);
        c.write_instruction(Instruction::Invoke("m".into(), 0), h);
        c.write_instruction(Instruction::Return, h);
    });
    assert_eq!({let r=vm.pop_stack().unwrap(); get_int(&vm, r)}, 1);
}
