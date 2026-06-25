use super::*;

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
