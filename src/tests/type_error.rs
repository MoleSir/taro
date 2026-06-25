use super::*;

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
fn test_non_iterable_error() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret("for x in 42 {}");
    assert!(result.is_err());
}

// ==========================================================================
//  Floor division & modulo
// ==========================================================================

