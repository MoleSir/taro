use super::*;

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
pub fn test_regression_nil_floordiv_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil ~/ 2;").is_err());
}


#[test]
pub fn test_regression_nil_mod_error() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("nil % 2;").is_err());
}
