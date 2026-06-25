use super::*;

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
pub fn test_call_on_plain_instance_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("class Foo {} var f = Foo(); f(1, 2);").unwrap_err();
    assert!(err.to_string().contains("Can't call"), "got: {err}");
}

// ===========================================================================
// Import tests
// ==========================================================================

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

// ==========================================================================
//  Range
// ==========================================================================

#[test]
fn test_range_basic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(5) { acc.append(i); }
        print(acc);  // [0, 1, 2, 3, 4]
    ",
    )
    .unwrap();
}

#[test]
fn test_range_start_stop() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(3, 7) { acc.append(i); }
        print(acc);  // [3, 4, 5, 6]
    ",
    )
    .unwrap();
}

#[test]
fn test_range_with_step() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(0, 10, 2) { acc.append(i); }
        print(acc);  // [0, 2, 4, 6, 8]
    ",
    )
    .unwrap();
}

#[test]
fn test_range_negative_step() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(5, 0, -1) { acc.append(i); }
        print(acc);  // [5, 4, 3, 2, 1]
    ",
    )
    .unwrap();
}

#[test]
fn test_range_negative_step_larger() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(10, 0, -2) { acc.append(i); }
        print(acc);  // [10, 8, 6, 4, 2]
    ",
    )
    .unwrap();
}

#[test]
fn test_range_empty() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(0) { acc.append(i); }
        print(len(acc));  // 0
    ",
    )
    .unwrap();
}

#[test]
fn test_range_empty_reverse() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(1, 5, -1) { acc.append(i); }
        print(len(acc));  // 0
    ",
    )
    .unwrap();
}

#[test]
fn test_range_len() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        print(len(range(5)));       // 5
        print(len(range(3, 7)));    // 4
        print(len(range(0, 10, 2)));// 5
        print(len(range(10, 0, -1)));// 10
        print(len(range(0)));       // 0
    ",
    )
    .unwrap();
}

#[test]
fn test_range_sum() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        print(sum(range(5)));       // 10
        print(sum(range(1, 6)));    // 15
        print(sum(range(2, 10, 2)));// 20
    ",
    )
    .unwrap();
}

#[test]
fn test_range_in_nested_loops() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = 0;
        for i in range(3) {
            for j in range(3) {
                acc = acc + 1;
            }
        }
        print(acc);  // 9
    ",
    )
    .unwrap();
}

#[test]
fn test_range_multiple_independent() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var a = [];
        var b = [];
        for i in range(0, 6, 2) { a.append(i); }
        for i in range(1, 6, 2) { b.append(i); }
        print(a);  // [0, 2, 4]
        print(b);  // [1, 3, 5]
    ",
    )
    .unwrap();
}

#[test]
fn test_range_reuse_is_exhausted() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var r = range(2);
        var a = []; for i in r { a.append(i); }
        var b = []; for i in r { b.append(i); }
        print(a);  // [0, 1]
        print(b);  // [] (exhausted)
    ",
    )
    .unwrap();
}

#[test]
fn test_range_step_zero_error() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret("range(1, 10, 0);");
    assert!(result.is_err());
}

#[test]
fn test_range_bad_arg_count() {
    let mut vm = VirtualMachine::new();
    assert!(vm.interpret("range();").is_err());
    assert!(vm.interpret("range(1, 2, 3, 4);").is_err());
}

#[test]
fn test_range_negative_bounds() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(-3, 3) { acc.append(i); }
        print(acc);  // [-3, -2, -1, 0, 1, 2]
    ",
    )
    .unwrap();
}

#[test]
fn test_range_negative_bounds_reverse() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = [];
        for i in range(3, -3, -1) { acc.append(i); }
        print(acc);  // [3, 2, 1, 0, -1, -2]
    ",
    )
    .unwrap();
}

#[test]
fn test_range_str() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        print(str(range(0, 5, 1)));   // range(0, 5, 1)
        print(str(range(10, 0, -1))); // range(10, 0, -1)
    ",
    )
    .unwrap();
}

