use super::VirtualMachine;
use super::module::ModuleKey;
use crate::{Chunk, Instruction, ObjectHandle, ObjectHeap};

/// Build a chunk and run it: creates VM first, then calls `build` with VM's heap.
fn run_chunk(build: impl FnOnce(&mut Chunk, &mut ObjectHeap)) -> VirtualMachine {
    let mut vm = VirtualMachine::new();
    // SAFETY: `vm.obj_heap` and the local `chunk` are separate allocations.
    // `build` only uses the heap for pushing constants; it never reads or
    // modifies vm internals.  The raw pointer is valid for the duration of
    // the call because `vm` is pinned on the stack.
    let heap_ptr = &mut vm.obj_heap as *mut ObjectHeap;
    let mut chunk = Chunk::new();
    unsafe {
        build(&mut chunk, &mut *heap_ptr);
    }
    let function = vm.obj_heap.alloc_function("script", 0, 0, vec![], vec![], chunk);
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
fn is_nil(handle: ObjectHandle) -> bool {
    handle.is_nil()
}

#[test]
pub fn test_base_arith() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_float_instance(3.4)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_float_instance(1.2)), 1, 1, h);
        c.write_instruction(Instruction::Add, 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_float_instance(5.6)), 1, 1, h);
        c.write_instruction(Instruction::Div, 1, 1, h);
        c.write_instruction(Instruction::Negate, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    let f = get_float(&vm, r);
    assert!((f + 0.82142857).abs() < 0.001);
}

#[test]
pub fn test_global_variable() {
    let _vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::DefineGlobal("x".into()), 1, 1, h);
        c.write_instruction(Instruction::GetGlobal("print".into()), 1, 1, h);
        c.write_instruction(Instruction::GetGlobal("x".into()), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Pop, 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(99)), 1, 1, h);
        c.write_instruction(Instruction::SetGlobal("x".into()), 1, 1, h);
        c.write_instruction(Instruction::GetGlobal("print".into()), 1, 1, h);
        c.write_instruction(Instruction::GetGlobal("x".into()), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Pop, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
}

#[test]
pub fn test_local_variable_get_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(20)), 1, 1, h);
        c.write_instruction(Instruction::GetLocal(1), 1, 1, h);
        c.write_instruction(Instruction::GetLocal(2), 1, 1, h);
        c.write_instruction(Instruction::Add, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 30);
}

#[test]
pub fn test_local_variable_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::SetLocal(1), 1, 1, h);
        c.write_instruction(Instruction::Pop, 1, 1, h);
        c.write_instruction(Instruction::GetLocal(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_scopes() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::GetLocal(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_jump_if_false() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::True, 1, 1, h);
        c.write_instruction(Instruction::JumpIfFalse(5), 1, 1, h);
        c.write_instruction(Instruction::Pop, 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_while_loop() {
    // Test while loop via compilation (return isn't allowed at top level).
    let mut vm = VirtualMachine::new();
    vm.interpret("{ var i = 0; while i < 3 { i = i + 1; } }").unwrap();
    // The script returns nil implicitly — verify execution succeeded.
}

#[test]
pub fn test_function_call() {
    let mut vm = run_chunk(|c, h| {
        let mut f = Chunk::new();
        f.write_instruction(Instruction::GetLocal(1), 1, 1, h);
        f.write_instruction(Instruction::GetLocal(2), 1, 1, h);
        f.write_instruction(Instruction::Add, 1, 1, h);
        f.write_instruction(Instruction::Return, 1, 1, h);
        let fn_h = h.alloc_function("add", 2, 2, vec![], vec![], f);
        let cl_h = h.alloc_closure(fn_h, ObjectHandle::NIL);
        c.write_instruction(Instruction::Constant(cl_h), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(20)), 1, 1, h);
        c.write_instruction(Instruction::Call(2), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 30);
}

#[test]
pub fn test_string_concat() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("hello".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance(" world".into())), 1, 1, h);
        c.write_instruction(Instruction::Add, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "hello world");
}

#[test]
pub fn test_string_equality() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), 1, 1, h);
        c.write_instruction(Instruction::Equal, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

#[test]
pub fn test_string_not_equality() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("xyz".into())), 1, 1, h);
        c.write_instruction(Instruction::NotEqual, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

#[test]
pub fn test_len_string() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("len".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("hello".into())), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        5
    );
}

#[test]
pub fn test_type_of_string() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("type".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("hi".into())), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert!(matches!(vm.obj_heap.get(r), crate::Object::Class(_)));
}

#[test]
pub fn test_bool_negate() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::True, 1, 1, h);
        c.write_instruction(Instruction::Not, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!(!{
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

#[test]
pub fn test_bool_truthy() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(0)), 1, 1, h);
        c.write_instruction(Instruction::Not, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

#[test]
pub fn test_build_list() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(3)), 1, 1, h);
        c.write_instruction(Instruction::BuildList(3), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_list_instance(r).unwrap().len(), 3);
}

#[test]
pub fn test_list_index_get() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::BuildList(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(0)), 1, 1, h);
        c.write_instruction(Instruction::IndexGet, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        10
    );
}

#[test]
pub fn test_list_index_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::BuildList(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(0)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::IndexSet, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_list_append_method() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::BuildList(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), 1, 1, h);
        c.write_instruction(Instruction::Invoke("append".into(), 1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        2
    );
}

#[test]
pub fn test_build_dict() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("a".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("b".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(2), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    let entry_count: usize = vm.obj_heap.get_dict_instance(r).unwrap().values().map(|b| b.len()).sum();
    assert_eq!(entry_count, 2);
}

#[test]
pub fn test_dict_index_get() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::IndexGet, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_dict_get_method() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Invoke("get".into(), 1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_dict_get_missing() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("x".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("y".into())), 1, 1, h);
        c.write_instruction(Instruction::Invoke("get".into(), 1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        is_nil(r)
    });
}

#[test]
pub fn test_dict_keys_method() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("a".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::Invoke("keys".into(), 0), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_list_instance(r).unwrap().len(), 1);
}

#[test]
pub fn test_dict_pop_method() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Invoke("pop".into(), 1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}

#[test]
pub fn test_class_instantiate() {
    let mut vm = run_chunk(|c, h| {
        let cls = h.alloc_class("Foo", h.builtins_module);
        c.write_instruction(Instruction::Constant(cls), 1, 1, h);
        c.write_instruction(Instruction::Call(0), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!(matches!(
        {
            let r = vm.pop_stack().unwrap();
            vm.obj_heap.get(r)
        },
        crate::Object::Instance(_)
    ));
}

#[test]
pub fn test_class_with_method() {
    let mut vm = run_chunk(|c, h| {
        let cls = h.alloc_class("Calc", h.builtins_module);
        let mut mc = Chunk::new();
        mc.write_instruction(Instruction::GetLocal(2), 1, 1, h);
        mc.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), 1, 1, h);
        mc.write_instruction(Instruction::Mul, 1, 1, h);
        mc.write_instruction(Instruction::Return, 1, 1, h);
        let mfn = h.alloc_function("double", 2, 2, vec![], vec![], mc);
        let mcl = h.alloc_closure(mfn, ObjectHandle::NIL);
        h.get_class_mut(cls).unwrap().methods.insert("double".into(), crate::Method::User(mcl));
        c.write_instruction(Instruction::Constant(cls), 1, 1, h);
        c.write_instruction(Instruction::Call(0), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(5)), 1, 1, h);
        c.write_instruction(Instruction::Invoke("double".into(), 1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        10
    );
}

#[test]
pub fn test_builtin_abs() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("abs".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(-5)), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        5
    );
}

#[test]
pub fn test_builtin_min() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("min".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(5)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(-3)), 1, 1, h);
        c.write_instruction(Instruction::Call(3), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        -3
    );
}

#[test]
pub fn test_builtin_type() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("type".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    match {
        let r = vm.pop_stack().unwrap();
        vm.obj_heap.get(r)
    } {
        crate::Object::Class(c) => assert_eq!(c.name.as_str(), "Int"),
        _ => panic!(),
    }
}

#[test]
pub fn test_super_invoke() {
    let mut vm = run_chunk(|c, h| {
        let base = h.alloc_class("Base", h.builtins_module);
        let mut bc = Chunk::new();
        bc.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        bc.write_instruction(Instruction::Return, 1, 1, h);
        let bm = h.alloc_function("m", 1, 1, vec![], vec![], bc);
        let bm_cl = h.alloc_closure(bm, ObjectHandle::NIL);
        h.get_class_mut(base).unwrap().methods.insert("m".into(), crate::Method::User(bm_cl));

        let derived = h.alloc_class("Derived", h.builtins_module);
        h.get_class_mut(derived).unwrap().superclass = Some(base);
        let mut dc = Chunk::new();
        dc.write_instruction(Instruction::GetLocal(1), 1, 1, h);
        dc.write_instruction(Instruction::SuperInvoke("m".into(), 0), 1, 1, h);
        dc.write_instruction(Instruction::Return, 1, 1, h);
        let dm = h.alloc_function("m", 1, 1, vec![], vec![], dc);
        let dm_cl = h.alloc_closure(dm, ObjectHandle::NIL);
        h.get_class_mut(derived).unwrap().methods.insert("m".into(), crate::Method::User(dm_cl));

        c.write_instruction(Instruction::Constant(derived), 1, 1, h);
        c.write_instruction(Instruction::Call(0), 1, 1, h);
        c.write_instruction(Instruction::Invoke("m".into(), 0), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        1
    );
}

// ===========================================================================
// Regression tests for previously-fixed bugs
// ===========================================================================

/// Bug 1: NativeFn call stack corruption.
/// `call_native_fn` removed only args from stack, leaving the callee behind.
/// A subsequent call would read the wrong value.  This test exercises chained
/// and nested native calls that would have crashed or printed garbage.
#[test]
pub fn test_regression_chained_builtin_no_crash() {
    let mut vm = VirtualMachine::new();
    // len("hello") returns 5; print(5) should succeed.
    vm.interpret("print(len(\"hello\"));").unwrap();
}

#[test]
pub fn test_regression_multiple_builtin_calls_as_args() {
    let mut vm = VirtualMachine::new();
    // Multiple native calls whose results become arguments to print.
    vm.interpret("print(len(\"a\"), len(\"bc\"), len(\"def\"));").unwrap();
}

#[test]
pub fn test_regression_deeply_nested_builtin_calls() {
    let mut vm = VirtualMachine::new();
    // bool(len("hi")) — len returns 2, bool(2) returns true.
    vm.interpret("print(bool(len(\"hi\")));").unwrap();
}

#[test]
pub fn test_regression_builtin_call_chain_in_block() {
    let mut vm = VirtualMachine::new();
    // Sequential native calls should not interfere with each other.
    vm.interpret("{ var a = len(\"hello\"); var b = str(a); print(b); }").unwrap();
}

#[test]
pub fn test_regression_builtin_in_expression_context() {
    let mut vm = VirtualMachine::new();
    // Native result used in arithmetic expression.
    vm.interpret("{ var n = len(\"ab\") + len(\"cde\"); print(n); }").unwrap(); // 2 + 3 = 5
}

/// Bug 2: Non-Instance comparison crash.
/// `type(f) == Bar` crashed because type() returns a Class object, and
/// `dispatch_magic` calls `get_instance()` which rejects non-Instance objects.
#[test]
pub fn test_regression_type_equality_class() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "class Foo {} class Bar {} var f = Foo(); var b = Bar(); \
         print(type(f) == Foo); print(type(f) == Bar); print(type(b) == Bar);",
    )
    .unwrap();
}

#[test]
pub fn test_regression_native_fn_equality() {
    let mut vm = VirtualMachine::new();
    // NativeFn == NativeFn: same handle → true (fast path).
    vm.interpret("print(print == print);").unwrap();
}

#[test]
pub fn test_regression_native_fn_not_equal() {
    let mut vm = VirtualMachine::new();
    // NativeFn != NativeFn: different handles → true.
    vm.interpret("print(print != len);").unwrap();
}

#[test]
pub fn test_regression_class_not_equal() {
    let mut vm = VirtualMachine::new();
    // Different classes should not be equal.
    vm.interpret("class A {} class B {} print(A == B); print(A != B);").unwrap();
}

/// Bug 3 (nil): nil should not have method implementations.  Operations on nil
/// must check and reject directly rather than dispatching to a nil class.
#[test]
pub fn test_regression_nil_negate_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("-nil;").is_err());
}

#[test]
pub fn test_regression_nil_add_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil + 1;").is_err());
}

#[test]
pub fn test_regression_nil_sub_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil - 1;").is_err());
}

#[test]
pub fn test_regression_nil_mul_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil * 2;").is_err());
}

#[test]
pub fn test_regression_nil_div_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil / 2;").is_err());
}

#[test]
pub fn test_regression_nil_lt_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil < 1;").is_err());
}

#[test]
pub fn test_regression_nil_le_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil <= 1;").is_err());
}

#[test]
pub fn test_regression_nil_gt_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil > 1;").is_err());
}

#[test]
pub fn test_regression_nil_ge_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil >= 1;").is_err());
}

#[test]
pub fn test_regression_nil_len_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("len(nil);").is_err());
}

#[test]
pub fn test_regression_nil_int_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("int(nil);").is_err());
}

#[test]
pub fn test_regression_nil_float_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("float(nil);").is_err());
}

#[test]
pub fn test_regression_nil_index_get_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil[0];").is_err());
}

#[test]
pub fn test_regression_nil_eq_nil_true() {
    // nil == nil should always be true.
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Nil, 1, 1, h);
        c.write_instruction(Instruction::Nil, 1, 1, h);
        c.write_instruction(Instruction::Equal, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

#[test]
pub fn test_regression_nil_ne_nil_false() {
    // nil != nil should be false.
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Nil, 1, 1, h);
        c.write_instruction(Instruction::Nil, 1, 1, h);
        c.write_instruction(Instruction::NotEqual, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!(!{
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

#[test]
pub fn test_regression_nil_eq_int_false() {
    // nil == 42 should be false.
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Nil, 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::Equal, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!(!{
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

#[test]
pub fn test_regression_not_nil_is_true() {
    // !nil should be true (nil is falsy).
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Nil, 1, 1, h);
        c.write_instruction(Instruction::Not, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

#[test]
pub fn test_regression_str_nil() {
    // str(nil) should return "nil" without crashing.
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("str".into()), 1, 1, h);
        c.write_instruction(Instruction::Nil, 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "nil");
}

#[test]
pub fn test_regression_bool_nil_is_false() {
    // bool(nil) should be false.
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("bool".into()), 1, 1, h);
        c.write_instruction(Instruction::Nil, 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!(!{
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

/// `type(nil)` internally: nil is stored as an Instance with NIL class,
/// so type(nil) returns a handle.  Verify it doesn't crash.
#[test]
pub fn test_regression_type_nil_no_crash() {
    let mut vm = VirtualMachine::new();
    // Just verify this compiles and runs without crashing.
    vm.interpret("print(type(nil));").unwrap();
}

/// Bool interning: `true` always returns the same ObjectHandle.
#[test]
pub fn test_regression_bool_intern_same_vm() {
    let mut vm = run_chunk(|c, h| {
        // Push two `true` values; they should be the same handle.
        c.write_instruction(Instruction::True, 1, 1, h);
        c.write_instruction(Instruction::True, 1, 1, h);
        // They are the same handle, so equality fast-path catches it.
        c.write_instruction(Instruction::Equal, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

/// Bool interning: `false` always returns the same ObjectHandle.
#[test]
pub fn test_regression_bool_intern_false() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::False, 1, 1, h);
        c.write_instruction(Instruction::False, 1, 1, h);
        c.write_instruction(Instruction::Equal, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}

/// `abs()` on non-number types should error gracefully.
#[test]
pub fn test_regression_abs_on_string_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("abs(\"hello\");").is_err());
}

// ===========================================================================
// __call__ magic method — Python-style callable instances
// ===========================================================================

#[test]
pub fn test_call_magic_user_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Adder {
            fun __init__(self, n) { self.n = n; }
            fun __call__(self, x) { return self.n + x; }
        }
        var add5 = Adder(5);
        print(add5(3));    // 8
        print(add5(10));   // 15
    ",
    )
    .unwrap();
}

#[test]
pub fn test_call_magic_no_args() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Logger {
            fun __call__(self) { print(\"called!\"); }
        }
        var log = Logger();
        log();
    ",
    )
    .unwrap();
}

#[test]
pub fn test_call_magic_multiple_args() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Multiplier {
            fun __init__(self, factor) { self.factor = factor; }
            fun __call__(self, a, b) { return self.factor * a * b; }
        }
        var double = Multiplier(2);
        print(double(3, 4));  // 24
    ",
    )
    .unwrap();
}

#[test]
pub fn test_call_magic_on_non_callable_instance_error() {
    let mut vm = VirtualMachine::new();
    // Plain instance without __call__ should error when called.
    assert!(vm.interpret("class Foo {} var f = Foo(); f();").is_err());
}

#[test]
pub fn test_call_magic_chained() {
    let mut vm = VirtualMachine::new();
    // A callable that returns a callable.
    vm.interpret(
        "
        class Factory {
            fun __init__(self, n) { self.n = n; }
            fun __call__(self, x) { return self.n + x; }
        }
        print(Factory(10)(5));  // 15
    ",
    )
    .unwrap();
}

#[test]
pub fn test_call_magic_with_method() {
    let mut vm = VirtualMachine::new();
    // Instance with both regular methods and __call__.
    vm.interpret(
        "
        class Counter {
            fun __init__(self) { self.count = 0; }
            fun __call__(self) {
                self.count = self.count + 1;
                return self.count;
            }
            fun reset(self) { self.count = 0; }
        }
        var c = Counter();
        print(c());  // 1
        print(c());  // 2
        c.reset();
        print(c());  // 1
    ",
    )
    .unwrap();
}

// ===========================================================================
// Non-Instance type error messages — friendly errors for all operations
// ===========================================================================

/// Unary `-` on a non-Instance (class, native fn, closure) should give
/// a friendly error with the actual type name.
#[test]
pub fn test_type_error_negate_on_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} -Foo;").unwrap_err();
    assert!(err.to_string().contains("bad operand type for unary neg"), "got: {err}");
    assert!(err.to_string().contains("class"), "got: {err}");
}

#[test]
pub fn test_type_error_negate_on_native_fn() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("-print;").unwrap_err();
    assert!(err.to_string().contains("native function"), "got: {err}");
}

#[test]
pub fn test_type_error_negate_on_function() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("fun f() {} -f;").unwrap_err();
    assert!(err.to_string().contains("closure"), "got: {err}");
}

/// Binary `+` on a class object should mention both types.
#[test]
pub fn test_type_error_add_class_and_int() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo + 1;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for add"), "got: {err}");
    assert!(err.to_string().contains("class"), "got: {err}");
}

#[test]
pub fn test_type_error_add_native_fn_and_int() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("print + 1;").unwrap_err();
    assert!(err.to_string().contains("native function"), "got: {err}");
}

#[test]
pub fn test_type_error_sub_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo - 1;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for sub"), "got: {err}");
}

#[test]
pub fn test_type_error_mul_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo * 2;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for mul"), "got: {err}");
}

#[test]
pub fn test_type_error_div_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo / 2;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for div"), "got: {err}");
}

/// Comparison on a non-Instance should error with the type name.
#[test]
pub fn test_type_error_lt_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo < 1;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for lt"), "got: {err}");
}

#[test]
pub fn test_type_error_gt_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo > 1;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for gt"), "got: {err}");
}

#[test]
pub fn test_type_error_le_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo <= 1;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for le"), "got: {err}");
}

#[test]
pub fn test_type_error_ge_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo >= 1;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for ge"), "got: {err}");
}

/// rhs non-Instance should also be caught.
#[test]
pub fn test_type_error_lt_int_and_native_fn() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("1 < print;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for lt"), "got: {err}");
}

/// `len()` on non-Instance gives friendly error.
#[test]
pub fn test_type_error_len_on_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} len(Foo);").unwrap_err();
    assert!(err.to_string().contains("is not object with __len__"), "got: {err}");
}

#[test]
pub fn test_type_error_len_on_native_fn() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("len(print);").unwrap_err();
    assert!(err.to_string().contains("native function"), "got: {err}");
}

/// `int()` / `float()` on non-Instance.
#[test]
pub fn test_type_error_int_on_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} int(Foo);").unwrap_err();
    assert!(err.to_string().contains("is not object with __int__"), "got: {err}");
}

#[test]
pub fn test_type_error_float_on_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} float(Foo);").unwrap_err();
    assert!(err.to_string().contains("is not object with __float__"), "got: {err}");
}

/// `[]` index on non-Instance.
#[test]
pub fn test_type_error_getitem_on_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo[0];").unwrap_err();
    assert!(err.to_string().contains("is not object with __getitem__"), "got: {err}");
}

#[test]
pub fn test_type_error_setitem_on_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo[0] = 1;").unwrap_err();
    assert!(err.to_string().contains("is not object with __setitem__"), "got: {err}");
}

/// `()` call on a non-callable (e.g. string literal).
#[test]
pub fn test_type_error_call_on_non_callable() {
    let mut vm = VirtualMachine::new();
    // "hello" is a string instance (not callable).
    let err = vm.interpret("\"hello\"();").unwrap_err();
    assert!(err.to_string().contains("Can't call"), "got: {err}");
}

// ===========================================================================
// __call__ magic method — additional edge cases
// ===========================================================================

/// Instance with inherited __call__ should work.
#[test]
pub fn test_call_magic_inherited() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Base {
            fun __call__(self, x) { return x * 2; }
        }
        class Child extends Base {}
        var c = Child();
        print(c(21));  // 42
    ",
    )
    .unwrap();
}

/// __call__ with super invocation.
#[test]
pub fn test_call_magic_with_super() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Base {
            fun __call__(self, x) { return x + 1; }
        }
        class Derived extends Base {
            fun __call__(self, x) { return super.__call__(x) * 10; }
        }
        var d = Derived();
        print(d(5));  // (5 + 1) * 10 = 60
    ",
    )
    .unwrap();
}

/// Callable that mutates self and returns accumulated state.
#[test]
pub fn test_call_magic_accumulator() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Accum {
            fun __init__(self) { self.total = 0; }
            fun __call__(self, n) {
                self.total = self.total + n;
                return self.total;
            }
        }
        var a = Accum();
        print(a(5));   // 5
        print(a(3));   // 8
        print(a(2));   // 10
    ",
    )
    .unwrap();
}

/// Callable stored in a list and invoked.
#[test]
pub fn test_call_magic_in_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Greet {
            fun __init__(self, name) { self.name = name; }
            fun __call__(self) { print(\"Hi \" + self.name); }
        }
        var gs = [Greet(\"A\"), Greet(\"B\")];
        gs[0]();
        gs[1]();
    ",
    )
    .unwrap();
}

/// __call__ returning a value used in an expression.
#[test]
pub fn test_call_magic_in_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Doubler {
            fun __call__(self, n) { return n * 2; }
        }
        var d = Doubler();
        print(d(3) + d(4));  // 6 + 8 = 14
    ",
    )
    .unwrap();
}

// ===========================================================================
// type_of — friendly type names in error messages
// ===========================================================================

#[test]
pub fn test_type_of_nil() {
    let vm = VirtualMachine::new();
    assert_eq!(vm.obj_heap.type_of(ObjectHandle::NIL), "nil");
}

#[test]
pub fn test_type_of_instance_fields() {
    let mut vm = VirtualMachine::new();
    vm.interpret("class Foo {} var f = Foo();").unwrap();
    // The instance 'f' should report as "instance" (Fields variant).
    // We verify by getting an error message that includes the type name.
    let err = vm.interpret("class Bar {} var b = Bar(); -b;").unwrap_err();
    // User-defined instances with no __neg__ get UnaryOpTypeMismatch
    assert!(err.to_string().contains("instance"), "got: {err}");
}

#[test]
pub fn test_type_of_builtin_types() {
    let mut vm = VirtualMachine::new();
    // Integer handle
    let int_handle = vm.obj_heap.alloc_integer_instance(42);
    assert_eq!(vm.obj_heap.type_of(int_handle), "int");
    // Float handle
    let float_handle = vm.obj_heap.alloc_float_instance(3.14);
    assert_eq!(vm.obj_heap.type_of(float_handle), "float");
    // Bool handle
    assert_eq!(vm.obj_heap.type_of(vm.obj_heap.true_instance), "bool");
    assert_eq!(vm.obj_heap.type_of(vm.obj_heap.false_instance), "bool");
    // String handle
    let str_handle = vm.obj_heap.alloc_string_instance("hello".into());
    assert_eq!(vm.obj_heap.type_of(str_handle), "string");
}

// ===========================================================================
// More magic method edge cases
// ===========================================================================

/// Custom __bool__ returning false should make the instance falsy.
#[test]
pub fn test_custom_bool_falsy() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class AlwaysFalse {
            fun __bool__(self) { return false; }
        }
        var af = AlwaysFalse();
        print(!af);              // true  — !false == true
        print(af or 42);         // 42   — false is falsy
        print(af and 42);        // <instance> — short-circuit
        print(bool(af));         // false
    ",
    )
    .unwrap();
}

/// Custom __len__ on a user class.
#[test]
pub fn test_custom_len_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class MyCollection {
            fun __init__(self) { self.items = [10, 20, 30]; }
            fun __len__(self) { return len(self.items); }
        }
        var mc = MyCollection();
        print(len(mc));  // 3
    ",
    )
    .unwrap();
}

/// Custom __getitem__ on a user class.
#[test]
pub fn test_custom_getitem_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class MySeq {
            fun __init__(self) { self.data = [1, 2, 4, 8]; }
            fun __getitem__(self, i) { return self.data[i]; }
        }
        var ms = MySeq();
        print(ms[0]);  // 1
        print(ms[3]);  // 8
    ",
    )
    .unwrap();
}

/// Custom __setitem__ on a user class.
#[test]
pub fn test_custom_setitem_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class MyMutable {
            fun __init__(self) { self.data = [0, 0, 0]; }
            fun __getitem__(self, i) { return self.data[i]; }
            fun __setitem__(self, i, v) { self.data[i] = v; return v; }
        }
        var mm = MyMutable();
        mm[1] = 99;
        print(mm[1]);  // 99
    ",
    )
    .unwrap();
}

/// Custom __int__ / __float__ on a user class.
#[test]
pub fn test_custom_int_float_methods() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Number {
            fun __init__(self, n) { self.n = n; }
            fun __int__(self) { return self.n; }
            fun __float__(self) { return self.n + 0.5; }
        }
        var n = Number(7);
        print(int(n));    // 7
        print(float(n));  // 7.5
    ",
    )
    .unwrap();
}

/// __not__ on a custom class (explicit override).
#[test]
pub fn test_custom_not_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Inverter {
            fun __init__(self, val) { self.val = val; }
            fun __not__(self) { return !self.val; }
        }
        var inv = Inverter(true);
        print(!inv);  // false
    ",
    )
    .unwrap();
}

/// __eq__ on a custom class.
#[test]
pub fn test_custom_eq_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Pair {
            fun __init__(self, a, b) { self.a = a; self.b = b; }
            fun __eq__(self, other) { return self.a == other.a and self.b == other.b; }
        }
        var p1 = Pair(1, 2);
        var p2 = Pair(1, 2);
        var p3 = Pair(3, 4);
        print(p1 == p2);  // true
        print(p1 == p3);  // false
        print(p1 != p3);  // true
    ",
    )
    .unwrap();
}

/// __neg__ on a custom class.
#[test]
pub fn test_custom_neg_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Vec {
            fun __init__(self, x, y) { self.x = x; self.y = y; }
            fun __neg__(self) {
                return Vec(-self.x, -self.y);
            }
            fun __str__(self) { return \"Vec(\" + str(self.x) + \",\" + str(self.y) + \")\"; }
        }
        var v = Vec(3, -5);
        print(str(-v));  // Vec(-3,5)
    ",
    )
    .unwrap();
}

/// __add__ / __mul__ on a custom class.
#[test]
pub fn test_custom_add_mul_methods() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Vec {
            fun __init__(self, x, y) { self.x = x; self.y = y; }
            fun __add__(self, other) { return Vec(self.x + other.x, self.y + other.y); }
            fun __mul__(self, s) { return Vec(self.x * s, self.y * s); }
            fun __str__(self) { return \"(\" + str(self.x) + \",\" + str(self.y) + \")\"; }
        }
        var a = Vec(1, 2);
        var b = Vec(3, 4);
        print(str(a + b));    // (4,6)
        print(str(a * 3));    // (3,6)
    ",
    )
    .unwrap();
}

/// Chained magic method operations.
#[test]
pub fn test_custom_magic_chained() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Num {
            fun __init__(self, v) { self.v = v; }
            fun __add__(self, o) { return Num(self.v + o.v); }
            fun __eq__(self, o) { return self.v == o.v; }
            fun __bool__(self) { return self.v > 0; }
        }
        var a = Num(1);
        var b = Num(2);
        var c = Num(3);
        print((a + b) == c);   // true
        print(bool(a + b));    // true
    ",
    )
    .unwrap();
}

// ===========================================================================
// List / Dict edge cases
// ===========================================================================

/// List nested operations.
#[test]
pub fn test_list_nested() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var matrix = [[1, 2], [3, 4]];
        print(matrix[0][0]);  // 1
        print(matrix[1][1]);  // 4
        matrix[0][1] = 99;
        print(matrix[0][1]);  // 99
    ",
    )
    .unwrap();
}

/// Dict with mixed key types (string only — cross-type eq not yet supported).
#[test]
pub fn test_dict_mixed_values() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var d = {\"int\": 1, \"str\": \"two\", \"bool\": true};
        print(d[\"int\"]);   // 1
        print(d[\"str\"]);   // two
        print(d[\"bool\"]);  // true
        print(len(d));       // 3
    ",
    )
    .unwrap();
}

/// Dict builtin methods: keys(), get(), values().
#[test]
pub fn test_dict_keys_values() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var d = {\"a\": 1, \"b\": 2, \"c\": 3};
        var keys = d.keys();
        print(len(keys));  // 3
        var val = d.get(\"b\");
        print(val);        // 2
        var missing = d.get(\"z\");
        print(missing);    // nil
        d[\"d\"] = 4;
        print(d.get(\"d\"));  // 4
    ",
    )
    .unwrap();
}

/// Basic string operations: concatenation, length, equality.
#[test]
pub fn test_string_operations() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var s = \"Hello\";
        print(s + \" World\");    // Hello World
        print(len(s));           // 5
        print(s == \"Hello\");    // true
        print(s != \"Bye\");      // true
        print(str(s));           // Hello
    ",
    )
    .unwrap();
}

/// Bool conversion via `int()` and `float()` builtins.
#[test]
pub fn test_bool_int_float_conversion() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        // int() and float() on bool via builtin functions
        print(int(true));    // 1
        print(int(false));   // 0
        print(float(true));  // 1
        print(float(false)); // 0
    ",
    )
    .unwrap();
}

/// ! on non-Instance should not error — it falls back to __bool__ + invert.
#[test]
pub fn test_not_on_non_instance() {
    let mut vm = VirtualMachine::new();
    // !class should work: class is truthy (non-nil), so !class == false.
    // This should NOT dispatch to __not__ magic, but fall back to __bool__.
    vm.interpret("class Foo {} print(!Foo);").unwrap(); // false
    vm.interpret("fun f() {} print(!f);").unwrap(); // false (closure is truthy)
}

/// eq/ne on non-Instance should return false/true without error.
#[test]
pub fn test_eq_on_non_instance() {
    let mut vm = VirtualMachine::new();
    // Class == int → false (different Object variants)
    vm.interpret("class Foo {} print(Foo == 1);").unwrap(); // false
    vm.interpret("class Foo {} print(Foo != 1);").unwrap(); // true
    // native fn == class → false
    vm.interpret("class Foo {} print(print == Foo);").unwrap(); // false
}

/// Calling a non-callable Instance (no __call__) should error.
#[test]
pub fn test_call_on_plain_instance_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} var f = Foo(); f(1, 2);").unwrap_err();
    assert!(err.to_string().contains("Can't call"), "got: {err}");
}

// ===========================================================================
// Import tests
// ===========================================================================

/// Helper: get the test module path for "lib/math.taro" relative to the
/// project root (where `cargo test` runs).
fn math_module_path() -> String {
    "tests/scripts/lib/math.taro".to_string()
}

#[test]
pub fn test_import_statement_defines_global() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; print(math);")).unwrap();
}

#[test]
pub fn test_import_module_pi_value() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; print(math.PI);")).unwrap();
}

#[test]
pub fn test_import_module_call_function() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; print(math.add(10, 20));")).unwrap();
}

#[test]
pub fn test_import_module_mul_function() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; print(math.mul(7, 6));")).unwrap();
}

#[test]
pub fn test_import_module_use_class() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; var v = math.Vec(1, 2); print(str(v));")).unwrap();
}

#[test]
pub fn test_import_as_expression() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("var m = import \"{path}\"; print(m.PI); print(m.add(1, 2));")).unwrap();
}

#[test]
pub fn test_import_file_not_found_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"nonexistent_file.taro\";").unwrap_err();
    assert!(err.to_string().contains("import error"), "got: {err}");
}

#[test]
pub fn test_import_file_not_exists_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"nonexistent_file_xyz.taro\";").unwrap_err();
    assert!(err.to_string().contains("import error"), "got: {err}");
    assert!(err.to_string().contains("module not found"), "got: {err}");
}

#[test]
pub fn test_import_does_not_leak_globals() {
    // Verify that importing a module doesn't add its internals to the
    // importing script's global scope (only the module name is added).
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    // PI is defined in math.taro but should not be a global.
    vm.interpret(&format!("import \"{path}\";")).unwrap();
    // `math` should exist as a global.
    // `PI` should NOT be a global (it's only accessible via math.PI).
    let err = vm.interpret("print(PI);").unwrap_err();
    assert!(err.to_string().contains("not found"), "PI should not be a global, got: {err}");
}

#[test]
pub fn test_import_nested() {
    // Test that a module can import another module.
    // We need a module that imports math.
    let mut vm = VirtualMachine::new();
    let math_path = math_module_path();
    // This script imports math and uses it — the import system should handle
    // nested import (import inside a module).
    vm.interpret(&format!("import \"{math_path}\"; var m = math; print(m.add(100, 200));")).unwrap();
}

/// File module importing another file module — exercises nested
/// `with_module_scope` and verifies `extra_gc_roots` stacking.
#[test]
pub fn test_import_nested_file_modules() {
    let inner_path = tmp_file_path("nested_inner.taro");
    let outer_path = tmp_file_path("nested_outer.taro");

    // Inner module exports a function.
    std::fs::write(&inner_path, "fun double(x) { return x * 2; }\n").unwrap();

    // Outer module imports inner and wraps the export.
    std::fs::write(
        &outer_path,
        &format!("import \"{inner_path}\" as inner;\nfun quadruple(x) {{ return inner.double(inner.double(x)); }}\n"),
    )
    .unwrap();

    let mut vm = VirtualMachine::new();
    vm.interpret(&format!("import \"{outer_path}\" as outer; print(outer.quadruple(5));")).unwrap();

    std::fs::remove_file(&inner_path).ok();
    std::fs::remove_file(&outer_path).ok();
}

#[test]
pub fn test_import_module_caching() {
    // Verify that importing the same module twice returns the cached module
    // (i.e. the module is executed at most once, matching Python's semantics).
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("cache_test.taro");

    // Write a module that defines a variable.
    std::fs::write(&path, "var x = 42;\n").unwrap();

    // First import — module is compiled and executed.
    vm.interpret(&format!("import \"{path}\";")).unwrap();

    // The module name is derived from the file stem.
    let module_name = std::path::Path::new(&path).file_stem().unwrap().to_str().unwrap();
    // Cache key is the canonical absolute path.
    let canonical = std::fs::canonicalize(&path).unwrap();
    let module_handle = vm.modules.loaded.get(&ModuleKey::File(canonical.clone())).copied().unwrap();

    // After first import, x == 42.
    let fields = vm.obj_heap.get_module(module_handle).map(|m| &m.fields).unwrap();
    let x_handle = fields.get(&crate::ShrString::new_str("x")).copied().unwrap();
    assert_eq!(*vm.obj_heap.expect_integer(x_handle).unwrap(), 42);

    // Modify the module's field via script.
    vm.interpret(&format!("{module_name}.x = 100;")).unwrap();

    // Verify the modification took effect on the module object.
    let fields = vm.obj_heap.get_module(module_handle).map(|m| &m.fields).unwrap();
    let x_handle = fields.get(&crate::ShrString::new_str("x")).copied().unwrap();
    assert_eq!(*vm.obj_heap.expect_integer(x_handle).unwrap(), 100);

    // Second import — should return the cached module (x still == 100, not 42).
    vm.interpret(&format!("import \"{path}\";")).unwrap();

    // The global should still point to the same cached module with x == 100.
    let fields = vm.obj_heap.get_module(module_handle).map(|m| &m.fields).unwrap();
    let x_handle = fields.get(&crate::ShrString::new_str("x")).copied().unwrap();
    assert_eq!(
        *vm.obj_heap.expect_integer(x_handle).unwrap(),
        100,
        "second import should return the cached module with x == 100, not a fresh module with x == 42"
    );

    std::fs::remove_file(&path).ok();
}

// ===========================================================================
// Std module tests — File
// ===========================================================================

/// Helper: return a temporary file path that can be used in tests.
fn tmp_file_path(name: &str) -> String {
    format!("/tmp/taro_test_{name}")
}

/// Clean up a temporary test file.
fn rm_tmp(name: &str) {
    let path = tmp_file_path(name);
    let _ = std::fs::remove_file(&path);
}

#[test]
pub fn test_std_file_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/fs\"; print(fs);").unwrap();
}

#[test]
pub fn test_std_file_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var f = import \"std/fs\"; print(f); print(f.File);").unwrap();
}

#[test]
pub fn test_file_write_and_read() {
    rm_tmp("write_read");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("write_read");
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         f.write(\"hello world\"); \
         f.close(); \
         var g = fs.File(\"{path}\", \"r\"); \
         print(g.read()); \
         g.close();"
    ))
    .unwrap();
    rm_tmp("write_read");
}

#[test]
pub fn test_file_str() {
    rm_tmp("str_test");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("str_test");
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         print(str(f)); \
         f.close(); \
         print(str(f));"
    ))
    .unwrap();
    rm_tmp("str_test");
}

#[test]
pub fn test_file_readline() {
    rm_tmp("readline");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("readline");
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         f.write(\"line1\\nline2\\nline3\"); \
         f.close(); \
         var g = fs.File(\"{path}\", \"r\"); \
         print(g.readline()); \
         print(g.readline()); \
         print(g.readline()); \
         var eof = g.readline(); \
         print(eof == nil); \
         g.close();"
    ))
    .unwrap();
    rm_tmp("readline");
}

#[test]
pub fn test_file_seek_and_tell() {
    rm_tmp("seek_tell");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("seek_tell");
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         f.write(\"abcdefghij\"); \
         f.close(); \
         var g = fs.File(\"{path}\", \"r\"); \
         print(g.tell()); \
         g.seek(5); \
         print(g.tell()); \
         print(g.read()); \
         g.close();"
    ))
    .unwrap();
    rm_tmp("seek_tell");
}

#[test]
pub fn test_file_not_found_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/fs\"; var f = fs.File(\"/tmp/nonexistent_xyz_12345\", \"r\");").unwrap_err();
    assert!(err.to_string().contains("cannot open"), "got: {err}");
}

#[test]
pub fn test_file_closed_error() {
    rm_tmp("closed_err");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("closed_err");
    // Create the file first so we can close it and then try reading.
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         f.write(\"data\"); \
         f.close();"
    ))
    .unwrap();
    // Now try reading from a closed file.
    let err = vm
        .interpret(&format!(
            "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"r\"); \
         f.close(); \
         f.read();"
        ))
        .unwrap_err();
    assert!(err.to_string().contains("file is closed"), "got: {err}");
    rm_tmp("closed_err");
}

#[test]
pub fn test_file_wrong_arg_count() {
    let mut vm = VirtualMachine::new();
    // read() takes no arguments
    let err = vm.interpret("import \"std/fs\"; var f = fs.File(\"/tmp/t\", \"w\"); f.read(\"extra\");").unwrap_err();
    assert!(err.to_string().contains("argument"), "got: {err}");
}

// ===========================================================================
// Std module tests — Math
// ===========================================================================

#[test]
pub fn test_std_math_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/math\"; print(math);").unwrap();
}

#[test]
pub fn test_std_math_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var m = import \"std/math\"; print(m); print(m.PI);").unwrap();
}

#[test]
pub fn test_math_constants() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/math\"; print(math.PI); print(math.E); print(math.TAU);").unwrap();
}

#[test]
pub fn test_math_trig() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.sin(0)); \
         print(math.cos(0)); \
         print(math.tan(0)); \
         print(math.asin(0)); \
         print(math.acos(1)); \
         print(math.atan(0)); \
         print(math.sin(1.5)); \
         print(math.atan2(1, 1));",
    )
    .unwrap();
}

#[test]
pub fn test_math_power_log() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.sqrt(16)); \
         print(math.pow(2, 10)); \
         print(math.exp(1)); \
         print(math.ln(math.E)); \
         print(math.log2(8)); \
         print(math.log10(100)); \
         print(math.hypot(3, 4));",
    )
    .unwrap();
}

#[test]
pub fn test_math_rounding() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.floor(3.7)); \
         print(math.ceil(3.1)); \
         print(math.round(3.5));",
    )
    .unwrap();
}

#[test]
pub fn test_math_conversion() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.degrees(math.PI)); \
         print(math.radians(180));",
    )
    .unwrap();
}

#[test]
pub fn test_math_accepts_int_args() {
    // All math functions should accept int and treat it as float.
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.sin(0)); \
         print(math.sqrt(16)); \
         print(math.pow(2, 10)); \
         print(math.floor(3)); \
         print(math.ceil(3));",
    )
    .unwrap();
}

#[test]
pub fn test_math_type_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/math\"; math.sin(\"hello\");").unwrap_err();
    assert!(err.to_string().contains("unsupported operand"), "got: {err}");
}

#[test]
pub fn test_math_wrong_arg_count() {
    let mut vm = VirtualMachine::new();
    // sin() takes 1 argument
    let err = vm.interpret("import \"std/math\"; math.sin();").unwrap_err();
    assert!(err.to_string().contains("argument"), "got: {err}");

    let mut vm2 = VirtualMachine::new();
    // sqrt() takes 1 argument
    let err2 = vm2.interpret("import \"std/math\"; math.sqrt(4, 5);").unwrap_err();
    assert!(err2.to_string().contains("argument"), "got: {err2}");
}

// ==========================================================================
//  std/random
// ==========================================================================

#[test]
pub fn test_std_random_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/random\"; print(random);").unwrap();
}

#[test]
pub fn test_std_random_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var r = import \"std/random\"; print(r); print(r.random());").unwrap();
}

#[test]
pub fn test_random_random_in_range() {
    let mut vm = VirtualMachine::new();
    // random() should return values in [0, 1).
    vm.interpret(
        "import \"std/random\"; \
         var sum = 0.0; \
         for (var i = 0; i < 100; i = i + 1) { \
             var v = random.random(); \
             if v < 0 or v >= 1 { print(\"out of range: \" + str(v)); } \
         }",
    )
    .unwrap();
}

#[test]
pub fn test_random_randint() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var v = random.randint(1, 10); \
         if v < 1 or v > 10 { print(\"out of range: \" + str(v)); } \
         var w = random.randint(-5, 5); \
         if w < -5 or w > 5 { print(\"out of range: \" + str(w)); }",
    )
    .unwrap();
}

#[test]
pub fn test_random_randint_min_equals_max() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var v = random.randint(7, 7); \
         if v != 7 { print(\"expected 7, got \" + str(v)); }",
    )
    .unwrap();
}

#[test]
pub fn test_random_randint_error_on_float() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/random\"; random.randint(1.5, 10);").unwrap_err();
    assert!(err.to_string().contains("unsupported"), "got: {err}");
}

#[test]
pub fn test_random_randint_error_min_gt_max() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/random\"; random.randint(10, 1);").unwrap_err();
    assert!(err.to_string().contains("must be <="), "got: {err}");
}

#[test]
pub fn test_random_uniform() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var v = random.uniform(1.0, 5.0); \
         if v < 1.0 or v >= 5.0 { print(\"out of range: \" + str(v)); } \
         var w = random.uniform(-2.5, 3.5); \
         if w < -2.5 or w >= 3.5 { print(\"out of range: \" + str(w)); }",
    )
    .unwrap();
}

#[test]
pub fn test_random_uniform_accepts_int_args() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var v = random.uniform(0, 10); \
         if v < 0 or v >= 10 { print(\"out of range: \" + str(v)); }",
    )
    .unwrap();
}

#[test]
pub fn test_random_choice() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var items = [\"a\", \"b\", \"c\"]; \
         var v = random.choice(items); \
         if v != \"a\" and v != \"b\" and v != \"c\" { print(\"unexpected element: \" + str(v)); }",
    )
    .unwrap();
}

#[test]
pub fn test_random_choice_empty_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/random\"; random.choice([]);").unwrap_err();
    assert!(err.to_string().contains("empty"), "got: {err}");
}

#[test]
pub fn test_random_choice_non_list_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/random\"; random.choice(42);").unwrap_err();
    assert!(err.to_string().contains("unsupported"), "got: {err}");
}

#[test]
pub fn test_random_shuffle() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var items = [1, 2, 3, 4, 5]; \
         var result = random.shuffle(items); \
         if len(result) != 5 { print(\"wrong length: \" + str(len(result))); }",
    )
    .unwrap();
}

// ===========================================================================
// Break / Continue — runtime tests
// ===========================================================================

#[test]
pub fn test_while_break_terminates() {
    let mut vm = VirtualMachine::new();
    // Infinite while loop with break should terminate.
    vm.interpret("while true { break; }").unwrap();
}

#[test]
pub fn test_while_break_early_exit() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        while i < 10 {
            i = i + 1;
            if i == 3 { break; }
        }
        print(i);  // 3
    ",
    )
    .unwrap();
}

#[test]
pub fn test_while_continue_skips_rest() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        var sum = 0;
        while i < 5 {
            i = i + 1;
            if i == 3 { continue; }
            sum = sum + i;
        }
        print(sum);  // 1 + 2 + 4 + 5 = 12
    ",
    )
    .unwrap();
}

#[test]
pub fn test_for_break() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        for (i = 0; i < 10; i = i + 1) {
            if i == 4 { break; }
        }
        print(i);  // 4
    ",
    )
    .unwrap();
}

#[test]
pub fn test_for_continue_increment_runs() {
    // Critical: verify the increment clause executes on continue.
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var results = [];
        for (var i = 0; i < 5; i = i + 1) {
            if i == 2 { continue; }
            results.append(i);
        }
        // results should be [0, 1, 3, 4]
        print(len(results));  // 4
        print(results[0]);    // 0
        print(results[1]);    // 1
        print(results[2]);    // 3
        print(results[3]);    // 4
    ",
    )
    .unwrap();
}

#[test]
pub fn test_nested_break() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        while i < 3 {
            var j = 0;
            while j < 3 {
                j = j + 1;
                if j == 2 { break; }
            }
            i = i + 1;
        }
        print(i);  // 3 — outer loop runs all iterations
    ",
    )
    .unwrap();
}

#[test]
pub fn test_nested_continue() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var sum = 0;
        var i = 0;
        while i < 3 {
            i = i + 1;
            var j = 0;
            while j < 3 {
                j = j + 1;
                if j == 2 { continue; }
                sum = sum + 1;
            }
        }
        // Inner loop: j=1(sum+1), j=2(skip), j=3(sum+1) => 2 per outer iter
        // 3 outer iterations => 6
        print(sum);  // 6
    ",
    )
    .unwrap();
}

#[test]
pub fn test_break_in_for_without_condition() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        for (;;) {
            i = i + 1;
            if i == 5 { break; }
        }
        print(i);  // 5
    ",
    )
    .unwrap();
}

#[test]
pub fn test_continue_in_for_without_increment() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var sum = 0;
        for (var i = 0; i < 5;) {
            i = i + 1;
            if i == 3 { continue; }
            sum = sum + i;
        }
        print(sum);  // 1 + 2 + 4 + 5 = 12
    ",
    )
    .unwrap();
}

#[test]
pub fn test_break_inside_if_inside_loop() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        while i < 10 {
            i = i + 1;
            if i == 7 { break; }
        }
        print(i);  // 7
    ",
    )
    .unwrap();
}

#[test]
pub fn test_break_outside_loop_compile_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("break;").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("break"), "got: {msg}");
}

#[test]
pub fn test_continue_outside_loop_compile_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("continue;").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("continue"), "got: {msg}");
}

#[test]
pub fn test_break_inside_nested_block_in_while() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        while i < 10 {
            {
                i = i + 1;
                if i == 5 { break; }
            }
        }
        print(i);  // 5
    ",
    )
    .unwrap();
}

#[test]
pub fn test_continue_respects_increment_in_for() {
    // Make sure the increment is evaluated exactly once per continue.
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var count = 0;
        for (var i = 0; i < 10; i = i + 1) {
            count = count + 1;
            if i < 7 { continue; }
            print(i);  // 7, 8, 9
        }
        // count should be 10 (body executed 10 times)
        print(count);
    ",
    )
    .unwrap();
}

// ------------------------------------------------------------------------
//  For-in — VM tests
// ------------------------------------------------------------------------

#[test]
fn test_for_in_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = \"\";
        for x in [1, 2, 3] { acc = acc + str(x); }
        print(acc);
    ",
    )
    .unwrap();
}

#[test]
fn test_for_in_string() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = \"\";
        for c in \"ab\" { acc = acc + c; }
        print(acc);
    ",
    )
    .unwrap();
}

#[test]
fn test_for_in_empty_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var count = 0;
        for x in [] { count = count + 1; }
        print(count);  // 0
    ",
    )
    .unwrap();
}

#[test]
fn test_for_in_break() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = \"\";
        for x in [1, 2, 3, 4] {
            if x > 2 { break; }
            acc = acc + str(x);
        }
        print(acc);  // \"12\"
    ",
    )
    .unwrap();
}

#[test]
fn test_for_in_continue() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = \"\";
        for x in [1, 2, 3] {
            if x == 2 { continue; }
            acc = acc + str(x);
        }
        print(acc);  // \"13\"
    ",
    )
    .unwrap();
}

#[test]
fn test_for_in_dict_keys() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var count = 0;
        for k in {\"a\": 1, \"b\": 2} { count = count + 1; }
        print(count);  // 2
    ",
    )
    .unwrap();
}

#[test]
fn test_for_in_nested() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = 0;
        for x in [1, 2] {
            for y in [10, 20] { acc = acc + x + y; }
        }
        print(acc);  // 1+10 + 1+20 + 2+10 + 2+20 = 66
    ",
    )
    .unwrap();
}

#[test]
fn test_iter_end_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        print(str(IterEnd));  // IterEnd
    ",
    )
    .unwrap();
}

#[test]
fn test_non_iterable_error() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret("for x in 42 {}");
    assert!(result.is_err());
}

// ==========================================================================
//  Floor division & modulo
// ==========================================================================

#[test]
pub fn test_floordiv_basic() {
    let mut vm = VirtualMachine::new();
    vm.interpret("print(7 ~/ 3);").unwrap(); // 2
    vm.interpret("print(10 ~/ 3);").unwrap(); // 3
}

#[test]
pub fn test_floordiv_negative() {
    let mut vm = VirtualMachine::new();
    vm.interpret("print(-7 ~/ 3);").unwrap(); // -3 (Python floor)
    vm.interpret("print(7 ~/ -3);").unwrap(); // -3
    vm.interpret("print(-7 ~/ -3);").unwrap(); // 2
}

#[test]
pub fn test_floordiv_float() {
    let mut vm = VirtualMachine::new();
    vm.interpret("print(10.5 ~/ 3.0);").unwrap(); // 3.0
    vm.interpret("print(10 ~/ 3.0);").unwrap(); // 3.0
    vm.interpret("print(10.0 ~/ 3);").unwrap(); // 3.0
}

#[test]
pub fn test_mod_basic() {
    let mut vm = VirtualMachine::new();
    vm.interpret("print(7 % 3);").unwrap(); // 1
    vm.interpret("print(10 % 3);").unwrap(); // 1
    vm.interpret("print(8 % 2);").unwrap(); // 0
}

#[test]
pub fn test_mod_negative() {
    let mut vm = VirtualMachine::new();
    vm.interpret("print(-7 % 3);").unwrap(); // 2 (Python-style)
    vm.interpret("print(7 % -3);").unwrap(); // -2
    vm.interpret("print(-7 % -3);").unwrap(); // -1
}

#[test]
pub fn test_mod_float() {
    let mut vm = VirtualMachine::new();
    vm.interpret("print(10.5 % 3.0);").unwrap(); // 1.5
    vm.interpret("print(7.0 % 2.0);").unwrap(); // 1.0
}

#[test]
pub fn test_floordiv_divide_by_zero() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("1 ~/ 0;").is_err());
    assert!(vm.interpret("1.0 ~/ 0.0;").is_err());
}

#[test]
pub fn test_mod_divide_by_zero() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("1 % 0;").is_err());
    assert!(vm.interpret("1.0 % 0.0;").is_err());
}

#[test]
pub fn test_regression_nil_floordiv_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil ~/ 2;").is_err());
}

#[test]
pub fn test_regression_nil_mod_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil % 2;").is_err());
}

#[test]
pub fn test_type_error_floordiv_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo ~/ 2;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for floordiv"), "got: {err}");
}

#[test]
pub fn test_type_error_mod_class() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} Foo % 2;").unwrap_err();
    assert!(err.to_string().contains("unsupported operand type(s) for mod"), "got: {err}");
}

#[test]
pub fn test_floordiv_magic_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Vec {
            fun __init__(self, x) { self.x = x; }
            fun __floordiv__(self, s) { return Vec(self.x / s); }
            fun __str__(self) { return str(self.x); }
        }
        print(Vec(10) ~/ 2);
    ",
    )
    .unwrap();
}

#[test]
pub fn test_mod_magic_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Vec {
            fun __init__(self, x) { self.x = x; }
            fun __mod__(self, s) { return Vec(self.x % s); }
            fun __str__(self) { return str(self.x); }
        }
        print(Vec(10) % 3);
    ",
    )
    .unwrap();
}

// ===========================================================================
// Std module tests — OS
// ===========================================================================

#[test]
pub fn test_std_os_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; print(os);").unwrap();
}

#[test]
pub fn test_std_os_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var m = import \"std/os\"; print(m);").unwrap();
}

#[test]
pub fn test_os_pid() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var p = os.pid(); print(p > 0);").unwrap();
}

#[test]
pub fn test_os_cwd() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; print(os.cwd()); var d = os.cwd(); print(len(d) > 0);").unwrap();
}

#[test]
pub fn test_os_getenv() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var v = os.getenv(\"PATH\"); print(v != nil);").unwrap();
}

#[test]
pub fn test_os_getenv_missing() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var v = os.getenv(\"TARO_NO_SUCH_VAR_XYZ\"); print(v == nil);").unwrap();
}

#[test]
pub fn test_os_setenv_and_getenv() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/os\"; \
         os.setenv(\"TARO_TEST_VAR\", \"hello\"); \
         print(os.getenv(\"TARO_TEST_VAR\") == \"hello\");",
    )
    .unwrap();
}

#[test]
pub fn test_os_env_returns_dict() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/os\"; \
         var e = os.env(); \
         print(type(e) == Dict); \
         print(len(e) > 0);",
    )
    .unwrap();
}

#[test]
pub fn test_os_tmpdir() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var t = os.tmpdir(); print(len(t) > 0);").unwrap();
}

#[test]
pub fn test_os_args_returns_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var a = os.args(); print(type(a) == List);").unwrap();
}

// ===========================================================================
// Std module tests — Time
// ===========================================================================

#[test]
pub fn test_std_time_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/time\"; print(time);").unwrap();
}

#[test]
pub fn test_std_time_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var m = import \"std/time\"; print(m);").unwrap();
}

#[test]
pub fn test_time_time_returns_number() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/time\"; \
         var t = time.time(); \
         print(t > 1700000000);", // well after 2023
    )
    .unwrap();
}

#[test]
pub fn test_time_sleep() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/time\"; \
         var t0 = time.time(); \
         time.sleep(0.01); \
         var t1 = time.time(); \
         print(t1 >= t0);",
    )
    .unwrap();
}

#[test]
pub fn test_time_now_returns_object() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/time\"; \
         var n = time.now(); \
         print(n.year > 2023); \
         print(n.month >= 1); \
         print(n.month <= 12); \
         print(n.day >= 1); \
         print(n.day <= 31); \
         print(n.hour >= 0); \
         print(n.hour <= 23); \
         print(n.min >= 0); \
         print(n.min <= 59); \
         print(n.sec >= 0); \
         print(n.sec < 60); \
         print(n.wday >= 0); \
         print(n.wday <= 6); \
         print(n.yday >= 1); \
         print(n.yday <= 366); \
         print(n.timestamp > 1700000000);",
    )
    .unwrap();
}

#[test]
pub fn test_time_sleep_negative_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/time\"; time.sleep(-1);").unwrap_err();
    assert!(err.to_string().contains("negative duration"), "got: {err}");
}

// ===========================================================================
// Std module tests — JSON
// ===========================================================================

#[test]
pub fn test_std_json_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json);").unwrap();
}

#[test]
pub fn test_json_encode_nil() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json.encode(nil) == \"null\");").unwrap();
}

#[test]
pub fn test_json_encode_bool() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.encode(true) == \"true\"); \
         print(json.encode(false) == \"false\");",
    )
    .unwrap();
}

#[test]
pub fn test_json_encode_int() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json.encode(42) == \"42\");").unwrap();
}

#[test]
pub fn test_json_encode_float() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json.encode(3.14) == \"3.14\");").unwrap();
}

#[test]
pub fn test_json_encode_string() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.encode(\"hello\") == \"\\\"hello\\\"\"); \
         print(json.encode(\"a\\nb\") == \"\\\"a\\\\nb\\\"\");",
    )
    .unwrap();
}

#[test]
pub fn test_json_encode_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.encode([1, 2, 3]) == \"[1,2,3]\"); \
         print(json.encode([\"a\", 1, true]) == \"[\\\"a\\\",1,true]\");",
    )
    .unwrap();
}

#[test]
pub fn test_json_encode_dict() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var d = dict(); \
         d[\"name\"] = \"taro\"; \
         d[\"ver\"] = 1; \
         var s = json.encode(d); \
         print(s == \"{\\\"name\\\":\\\"taro\\\",\\\"ver\\\":1}\" \
               or s == \"{\\\"ver\\\":1,\\\"name\\\":\\\"taro\\\"}\");",
    )
    .unwrap();
}

#[test]
pub fn test_json_encode_deeply_nested() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var v = [1, [2, [3, 4]], {\"k\": [5, 6]}]; \
         print(json.encode(v) == \"[1,[2,[3,4]],{\\\"k\\\":[5,6]}]\");",
    )
    .unwrap();
}

#[test]
pub fn test_json_decode_null() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json.decode(\"null\") == nil);").unwrap();
}

#[test]
pub fn test_json_decode_bool() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.decode(\"true\") == true); \
         print(json.decode(\"false\") == false);",
    )
    .unwrap();
}

#[test]
pub fn test_json_decode_number() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.decode(\"42\") == 42); \
         print(json.decode(\"3.14\") == 3.14); \
         print(json.decode(\"-100\") == -100);",
    )
    .unwrap();
}

#[test]
pub fn test_json_decode_string() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.decode(\"\\\"hello\\\"\") == \"hello\"); \
         print(json.decode(\"\\\"a\\\\nb\\\"\") == \"a\\nb\");",
    )
    .unwrap();
}

#[test]
pub fn test_json_decode_array() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var a = json.decode(\"[1, 2, 3]\"); \
         print(len(a) == 3); \
         print(a[0] == 1); \
         print(a[2] == 3);",
    )
    .unwrap();
}

#[test]
pub fn test_json_decode_object() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var d = json.decode(\"{\\\"x\\\": 10, \\\"y\\\": 20}\"); \
         print(d[\"x\"] == 10); \
         print(d[\"y\"] == 20);",
    )
    .unwrap();
}

#[test]
pub fn test_json_decode_nested() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var v = json.decode(\"{\\\"items\\\": [1, {\\\"nested\\\": true}]}\"); \
         print(v[\"items\"][0] == 1); \
         print(v[\"items\"][1][\"nested\"] == true);",
    )
    .unwrap();
}

#[test]
pub fn test_json_roundtrip() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var original = {\"a\": 1, \"b\": [2, 3], \"c\": {\"d\": \"hello\"}}; \
         var encoded = json.encode(original); \
         var decoded = json.decode(encoded); \
         print(decoded[\"a\"] == 1); \
         print(decoded[\"b\"][1] == 3); \
         print(decoded[\"c\"][\"d\"] == \"hello\");",
    )
    .unwrap();
}

#[test]
pub fn test_json_encode_unsupported_type() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/json\"; json.encode(print);").unwrap_err();
    assert!(err.to_string().contains("cannot serialize"), "got: {err}");
}

#[test]
pub fn test_json_decode_invalid() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/json\"; json.decode(\"not json\");").unwrap_err();
    assert!(err.to_string().contains("json.decode"), "got: {err}");
}

// ===========================================================================
// Default parameters & keyword arguments
// ===========================================================================

#[test]
pub fn test_default_param_basic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun greet(name, greeting = \"Hello\") {
            print(greeting + \" \" + name);
        }
        greet(\"World\");
        greet(\"Taro\", \"Hi\");
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_multiple() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun add(a, b = 10, c = 20) {
            return a + b + c;
        }
        print(add(1));          // 1 + 10 + 20 = 31
        print(add(1, 2));       // 1 + 2 + 20 = 23
        print(add(1, 2, 3));    // 1 + 2 + 3 = 6
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_all_optional() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun wrap(x = 0, y = 0) {
            return x * 10 + y;
        }
        print(wrap());          // 0
        print(wrap(3));         // 30
        print(wrap(3, 5));      // 35
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_nil() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun maybe(value = nil) {
            print(value == nil);
        }
        maybe();
        maybe(42);
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_bool() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun flag(on = true) {
            print(on);
        }
        flag();
        flag(false);
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_string() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun echo(msg = \"default\") {
            print(msg);
        }
        echo();
        echo(\"custom\");
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_negative_number() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun offset(x, delta = -1) {
            return x + delta;
        }
        print(offset(5));       // 4
        print(offset(5, 3));    // 8
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_negative_float() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun shift(x = -1.5) {
            print(x);
        }
        shift();
        shift(2.5);
    ",
    )
    .unwrap();
}

#[test]
pub fn test_keyword_arg_basic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun greet(name, greeting) {
            print(greeting + \" \" + name);
        }
        greet(name = \"World\", greeting = \"Hello\");
        greet(greeting = \"Hi\", name = \"Taro\");
    ",
    )
    .unwrap();
}

#[test]
pub fn test_keyword_arg_mixed_positional() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun describe(name, age, city = \"unknown\") {
            print(name + \" is \" + str(age) + \" from \" + city);
        }
        describe(\"Alice\", age = 30);
        describe(\"Bob\", city = \"NYC\", age = 25);
    ",
    )
    .unwrap();
}

#[test]
pub fn test_keyword_arg_with_defaults() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun build(a = 1, b = 2, c = 3) {
            return a * 100 + b * 10 + c;
        }
        print(build());              // 123
        print(build(9));             // 923
        print(build(c = 7));         // 127
        print(build(b = 5, c = 6));  // 156
        print(build(9, c = 0));      // 900
    ",
    )
    .unwrap();
}

#[test]
pub fn test_keyword_arg_positional_after_keyword_error() {
    let mut vm = VirtualMachine::new();
    let err = vm
        .interpret(
            "
        fun f(a, b, c) {}
        f(a = 1, 2);
    ",
        )
        .unwrap_err();
    assert!(err.to_string().contains("Positional"), "got: {err}");
}

#[test]
pub fn test_keyword_arg_unknown_error() {
    let mut vm = VirtualMachine::new();
    let err = vm
        .interpret(
            "
        fun f(x, y) { print(x + y); }
        f(x = 1, z = 3);
    ",
        )
        .unwrap_err();
    assert!(err.to_string().contains("unknown keyword"), "got: {err}");
}

#[test]
pub fn test_keyword_arg_duplicate_error() {
    let mut vm = VirtualMachine::new();
    let err = vm
        .interpret(
            "
        fun f(x, y) { print(x + y); }
        f(x = 1, x = 2);
    ",
        )
        .unwrap_err();
    assert!(err.to_string().contains("Duplicate"), "got: {err}");
}

#[test]
pub fn test_default_param_required_after_optional_error() {
    let mut vm = VirtualMachine::new();
    let err = vm
        .interpret(
            "
        fun bad(a = 1, b) {}
    ",
        )
        .unwrap_err();
    assert!(err.to_string().contains("Required"), "got: {err}");
}

#[test]
pub fn test_arg_count_range_error() {
    let mut vm = VirtualMachine::new();
    let err = vm
        .interpret(
            "
        fun f(a, b = 2, c = 3) {}
        f();
    ",
        )
        .unwrap_err();
    assert!(err.to_string().contains("arguments"), "got: {err}");
}

#[test]
pub fn test_keyword_arg_in_class_constructor() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Point {
            fun __init__(self, x = 0, y = 0) {
                self.x = x;
                self.y = y;
            }
        }
        var p = Point(x = 5, y = 3);
        print(p.x);  // 5
        print(p.y);  // 3
        var q = Point(y = 10);
        print(q.x);  // 0
        print(q.y);  // 10
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_in_class_constructor() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Point {
            fun __init__(self, x = 0, y = 0) {
                self.x = x;
                self.y = y;
            }
        }
        var p = Point();
        print(p.x);  // 0
        print(p.y);  // 0
        var q = Point(7);
        print(q.x);  // 7
        print(q.y);  // 0
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_method_positional() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Greeter {
            fun greet(self, name, punctuation = \"!\") {
                print(\"Hello \" + name + punctuation);
            }
        }
        var g = Greeter();
        g.greet(\"World\");
        g.greet(\"Taro\", \"?\");
    ",
    )
    .unwrap();
}

#[test]
pub fn test_default_param_method() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        class Greeter {
            fun greet(self, name, punctuation = \"!\") {
                print(\"Hello \" + name + punctuation);
            }
        }
        var g = Greeter();
        g.greet(\"World\");
        g.greet(\"Taro\", \"?\");  // Invoke still uses positional
    ",
    )
    .unwrap();
}
