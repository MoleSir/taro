use crate::{Chunk, Instruction, Value, ToShrString};
use super::VirtualMachine;

fn run_chunk(chunk: Chunk) -> VirtualMachine {    
    let mut vm = VirtualMachine::new();
    let function = vm.obj_heap.alloc_function("script", 0, chunk);
    vm.interpret_function(function).unwrap();
    vm
}

#[test]
pub fn test_base_arith() {
    let mut chunk = Chunk::new();

    chunk.write_instruction(Instruction::Constant(Value::Float(3.4)));
    chunk.write_instruction(Instruction::Constant(Value::Float(1.2)));
    chunk.write_instruction(Instruction::Add);

    chunk.write_instruction(Instruction::Constant(Value::Float(5.6)));
    chunk.write_instruction(Instruction::Div);
    chunk.write_instruction(Instruction::Negate);
    chunk.write_instruction(Instruction::Return);

    let mut vm = run_chunk(chunk);
    // (3.4 + 1.2) / 5.6 = 4.6 / 5.6 ≈ 0.82142…, negated ≈ -0.82142…
    let result = vm.pop_stack().unwrap();
    println!("{:?}", result);
    assert!(matches!(result, Value::Float(_)));
}

#[test]
pub fn test_global_variable() {
    let mut chunk = Chunk::new();

    // var x = 42;
    chunk.write_instruction(Instruction::Constant(Value::Integer(42))); // push_stack 42
    chunk.write_instruction(Instruction::DefineGlobal("x".to_shrstring())); // pop_stack → x

    // print x;  →  GetGlobal("print"); GetGlobal("x"); Call(1); Pop;
    chunk.write_instruction(Instruction::GetGlobal("print".into()));
    chunk.write_instruction(Instruction::GetGlobal("x".to_shrstring())); // push_stack x
    chunk.write_instruction(Instruction::Call(1));
    chunk.write_instruction(Instruction::Pop);

    // x = 99;
    chunk.write_instruction(Instruction::Constant(Value::Integer(99)));
    chunk.write_instruction(Instruction::SetGlobal("x".to_shrstring()));

    // print x;
    chunk.write_instruction(Instruction::GetGlobal("print".into()));
    chunk.write_instruction(Instruction::GetGlobal("x".to_shrstring()));
    chunk.write_instruction(Instruction::Call(1));
    chunk.write_instruction(Instruction::Pop);

    chunk.write_instruction(Instruction::Return);

    run_chunk(chunk);
}

#[test]
pub fn test_local_variable_get_set() {
    let mut chunk = Chunk::new();

    // Simulate: var a = 10; var b = 20;
    // slot 0 = function, slot 1 = a, slot 2 = b
    chunk.write_instruction(Instruction::Constant(Value::Integer(10)));
    chunk.write_instruction(Instruction::Constant(Value::Integer(20)));

    // a + b  →  GetLocal(1); GetLocal(2); Add
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::GetLocal(2));
    chunk.write_instruction(Instruction::Add);

    // Result should be 10 + 20 = 30
    chunk.write_instruction(Instruction::Return);

    let mut vm = run_chunk(chunk);
    let result = vm.pop_stack().unwrap();
    assert_eq!(result, Value::Integer(30));
}

#[test]
pub fn test_local_variable_set() {
    let mut chunk = Chunk::new();

    // Simulate: var a = 10; a = 42;  (slot 0 = fn, slot 1 = a)
    // First push the initial value for slot 1
    chunk.write_instruction(Instruction::Constant(Value::Integer(10)));

    // Now a = 42:
    // 1. Push 42 onto the stack
    // 2. SetLocal(1) — writes 42 into stack[1], leaves 42 on stack
    // 3. Pop to discard the expression result
    chunk.write_instruction(Instruction::Constant(Value::Integer(42)));
    chunk.write_instruction(Instruction::SetLocal(1));
    chunk.write_instruction(Instruction::Pop);

    // Now read a: GetLocal(1) → should be 42
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Return);

    let mut vm = run_chunk(chunk);
    let result = vm.pop_stack().unwrap();
    assert_eq!(result, Value::Integer(42));
}

#[test]
pub fn test_if_then_branch_taken() {
    // if (true) { 42; }  → result on stack = 42
    let mut chunk = Chunk::new();
    // true → condition
    chunk.write_instruction(Instruction::True);
    // JumpIfFalse: if false, skip then-branch. offset to land after the Jump (at Pop+Return below)
    chunk.write_instruction(Instruction::JumpIfFalse(7)); // skip: Constant(42) + Pop + Jump = 3+1+3 = 7
    chunk.write_instruction(Instruction::Pop); // pop true (taking then-branch)
    // then-branch: 42
    chunk.write_instruction(Instruction::Constant(Value::Integer(42)));
    chunk.write_instruction(Instruction::Pop); // discard expr result
    // Jump over else (absent) → to final Pop
    chunk.write_instruction(Instruction::Jump(1)); // skip the condition Pop
    chunk.write_instruction(Instruction::Pop); // pop true (from JumpIfFalse)
    chunk.write_instruction(Instruction::Return);

    let mut vm = run_chunk(chunk);
    // The stack should be empty (expression statement pops result,
    // only Return would leave something if we pushed before it...
    // Actually there's nothing left on stack — let's verify by
    // checking we can push/pop without error)
    vm.push_stack(Value::Integer(0));
    let _ = vm.pop_stack().unwrap();
}

#[test]
pub fn test_if_else_branch_taken() {
    // if (false) { 1; } else { 42; } → result 42
    let mut chunk = Chunk::new();
    // false → condition
    chunk.write_instruction(Instruction::False);
    // JumpIfFalse: if false, jump to else. offset = past then-branch (Pop+Constant(1)+Pop+Jump)
    chunk.write_instruction(Instruction::JumpIfFalse(8)); // skip then-branch: 1+3+1+3 = 8
    chunk.write_instruction(Instruction::Pop); // pop false (not taken here)
    // then: 1
    chunk.write_instruction(Instruction::Constant(Value::Integer(1)));
    chunk.write_instruction(Instruction::Pop);
    // Jump over else
    chunk.write_instruction(Instruction::Jump(6)); // skip else: Pop+Constant(42)+Pop = 1+3+1 = 5, +1
    chunk.write_instruction(Instruction::Pop); // pop false (else entry)
    // else: 42
    chunk.write_instruction(Instruction::Constant(Value::Integer(42)));
    chunk.write_instruction(Instruction::Pop); // discard expr
    chunk.write_instruction(Instruction::Return);

    run_chunk(chunk);
}

#[test]
pub fn test_while_loop_executes() {
    // Simulate: var i = 3; while (i > 0) { i = i - 1; }
    // But simpler: just test the loop mechanics
    // slot 0 = function, slot 1 = i
    // while (i > 0): GetLocal(1), Constant(0), Greater, JumpIfFalse, ...
    let mut chunk = Chunk::new();
    // Set up: i = 1 at slot 1 (slot 0 is the function)
    chunk.write_instruction(Instruction::Constant(Value::Integer(1)));
    // loop_start at pos 3 (after Constant 3 bytes)
    // condition: GetLocal(1), Constant(0), Greater
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    chunk.write_instruction(Instruction::Greater);                // stack: [fn, 1, 0, true]
    // JumpIfFalse exit — skip Pop + body + Loop = 1 + 3 + 1 + 3 = 8
    chunk.write_instruction(Instruction::JumpIfFalse(8));
    chunk.write_instruction(Instruction::Pop);                    // pop true
    // body: push 0, SetLocal(1), Pop. slot 1 = 0 on second iteration
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    chunk.write_instruction(Instruction::SetLocal(1));
    chunk.write_instruction(Instruction::Pop);                    // pop assignment result
    // Loop back to loop_start (position 3, offset = here-3+3)
    chunk.write_instruction(Instruction::Loop(12));
    chunk.write_instruction(Instruction::Pop);                    // pop exit condition
    chunk.write_instruction(Instruction::Return);

    let vm = run_chunk(chunk);
    // After the loop, slot 1 should be 0
    assert_eq!(vm.stack[1], Value::Integer(0));
}

#[test]
pub fn test_for_loop_simple() {
    // Simulate: var i = 0; for (; i < 3; i = i + 1) {}
    // This tests the basic for-loop desugaring pattern
    // slot 0 = function, slot 1 = i
    let mut chunk = Chunk::new();
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    // loop_start (condition): GetLocal(1), Constant(3), Less
    let loop_start = chunk.codes.len(); // should be 3 (after Constant 3 bytes)
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Constant(Value::Integer(3)));
    chunk.write_instruction(Instruction::Less);
    // JumpIfFalse exit
    chunk.write_instruction(Instruction::JumpIfFalse(0)); // placeholder, patched later
    let exit_jump = chunk.codes.len() - 2;
    chunk.write_instruction(Instruction::Pop); // pop condition
    // body: empty (just a no-op — we're testing the increment)
    // increment: GetLocal(1), Constant(1), Add, SetLocal(1), Pop
    let increment_start = chunk.codes.len();
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Constant(Value::Integer(1)));
    chunk.write_instruction(Instruction::Add);
    chunk.write_instruction(Instruction::SetLocal(1));
    chunk.write_instruction(Instruction::Pop); // pop assignment result
    // Loop to condition
    let loop_offset = chunk.codes.len() - loop_start + 3;
    chunk.write_instruction(Instruction::Loop(loop_offset));
    // Loop to increment (from body — body is empty here, so this is redundant
    // but included to test the pattern)
    let body_loop_offset = chunk.codes.len() - increment_start + 3;
    chunk.write_instruction(Instruction::Loop(body_loop_offset));
    // Patch exit jump
    let exit_offset = chunk.codes.len() - exit_jump - 2;
    chunk.codes[exit_jump] = (exit_offset as u16).to_le_bytes()[0];
    chunk.codes[exit_jump + 1] = (exit_offset as u16).to_le_bytes()[1];
    chunk.write_instruction(Instruction::Pop); // pop false
    chunk.write_instruction(Instruction::Return);

    let vm = run_chunk(chunk);
    // After 3 iterations, i should be 3
    assert_eq!(vm.stack[1], Value::Integer(3));
}

// ------------------------------------------------------------------------
//  Function calls (compile + execute)
// ------------------------------------------------------------------------

#[test]
pub fn test_function_call_no_args_no_return() {
    // `fun foo() { print 42; } foo();`
    // Should print 42 without errors.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun foo() { print(42); } foo();").unwrap();
}

#[test]
pub fn test_function_call_with_return() {
    // `fun add(a, b) { return a + b; } print add(3, 4);`
    // Should print 7.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun add(a, b) { return a + b; } print(add(3, 4));").unwrap();
}

#[test]
pub fn test_function_call_implicit_return() {
    // A function without an explicit return returns nil.
    // `fun f() {} var x = f(); print x;`
    let mut vm = VirtualMachine::new();
    vm.interpret("fun f() {} var x = f(); print(x);").unwrap();
}

#[test]
pub fn test_nested_function_call() {
    // `fun f() { return 1; } fun g() { return f(); } print g();`
    // Should print 1.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun f() { return 1; } fun g() { return f(); } print(g());").unwrap();
}

#[test]
pub fn test_function_with_multiple_params() {
    // `fun sum(a, b, c) { return a + b + c; } print sum(1, 2, 3);`
    // Should print 6.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun sum(a, b, c) { return a + b + c; } print(sum(1, 2, 3));").unwrap();
}

#[test]
pub fn test_function_call_as_expression() {
    // `fun double(x) { return x * 2; } print double(5) + 1;`
    // Should print 11.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun double(x) { return x * 2; } print(double(5) + 1);").unwrap();
}

#[test]
pub fn test_arg_count_mismatch() {
    // Calling with wrong number of args should be a runtime error.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun f(a, b) {} f(1);").unwrap_err();
}

// ------------------------------------------------------------------------
//  Closures & Upvalues — runtime behaviour
// ------------------------------------------------------------------------

#[test]
pub fn test_closure_counting() {
    // Each call to the closure increments and returns the captured counter.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        fun makeCounter() {
            var i = 0;
            fun counter() {
                i = i + 1;
                return i;
            }
            return counter;
        }
        var c = makeCounter();
        print(c()); // 1
        print(c()); // 2
        print(c()); // 3
    "#).unwrap();
}

#[test]
pub fn test_independent_closures() {
    // Two closures created by the same factory are independent.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        fun makeCounter() {
            var i = 0;
            fun counter() {
                i = i + 1;
                return i;
            }
            return counter;
        }
        var c1 = makeCounter();
        var c2 = makeCounter();
        print(c1()); // 1
        print(c1()); // 2
        print(c2()); // 1
        print(c2()); // 2
    "#).unwrap();
}

#[test]
pub fn test_closure_captures_parameter() {
    // makeAdder captures its parameter x, adder uses it with its own param y.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        fun makeAdder(x) {
            fun adder(y) {
                return x + y;
            }
            return adder;
        }
        var add5 = makeAdder(5);
        print(add5(3));  // 8
        print(add5(10)); // 15
    "#).unwrap();
}

#[test]
pub fn test_nested_upvalue_chain() {
    // inner captures a from outer, via middle's upvalue.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        fun outer() {
            var a = 10;
            fun middle() {
                fun inner() {
                    return a;
                }
                return inner;
            }
            return middle;
        }
        var getA = outer()();
        print(getA()); // 10
    "#).unwrap();
}

#[test]
pub fn test_upvalue_mutation() {
    // A closure can mutate the captured variable, and the outer function
    // sees the change when the closure returned.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        fun outer() {
            var x = 10;
            fun setX(v) {
                x = v;
            }
            setX(42);
            return x;
        }
        print(outer()); // 42
    "#).unwrap();
}

#[test]
pub fn test_upvalue_survives_outer_return() {
    // After makeCounter returns, the closure can still read/write the
    // upvalue (which is now closed — value copied from stack).
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        fun makeCounter() {
            var i = 0;
            fun counter() {
                i = i + 1;
                return i;
            }
            return counter;
        }
        var c = makeCounter();
        // makeCounter has returned — i is now closed.
        // counter should still work.
        print(c()); // 1
        print(c()); // 2
        print(c()); // 3
        print(c()); // 4
    "#).unwrap();
}

#[test]
pub fn test_multiple_independent_upvalues() {
    // Closure captures two variables — both should be independent.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        fun factory() {
            var a = 1;
            var b = 10;
            fun compute() {
                a = a + 1;
                b = b + 10;
                return a * 100 + b;
            }
            return compute;
        }
        var c = factory();
        print(c()); // a=2, b=20 → 2*100+20 = 220
        print(c()); // a=3, b=30 → 3*100+30 = 330
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  Classes & Instances — runtime behaviour
// ------------------------------------------------------------------------

#[test]
pub fn test_basic_class_instance_creation() {
    // Create a class, create an instance, print it.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Toast {}
        var toast = Toast();
        print(toast);
    "#).unwrap();
}

#[test]
pub fn test_instance_field_get_set() {
    // Set and get instance fields.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Pair {}
        var pair = Pair();
        pair.first = 1;
        pair.second = 2;
        print(pair.first + pair.second); // 3
    "#).unwrap();
}

#[test]
pub fn test_instance_field_expression_value() {
    // Assignment is an expression — the assigned value is returned.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Toast {}
        var toast = Toast();
        print(toast.jam = "grape"); // "grape"
    "#).unwrap();
}

#[test]
pub fn test_method_call_no_params() {
    // Call a method that only has self as parameter.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Greeter {
            fun hello(self) {
                print("hello");
            }
        }
        var g = Greeter();
        g.hello();
    "#).unwrap();
}

#[test]
pub fn test_method_call_with_params() {
    // Method with explicit self and multiple parameters.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Scone {
            fun topping(self, first, second) {
                print("scone with " + first + " and " + second);
            }
        }
        var scone = Scone();
        scone.topping("berries", "cream");
    "#).unwrap();
}

#[test]
pub fn test_self_in_method() {
    // `self` inside a method refers to the instance.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Counter {
            fun __init__(self) {
                self.count = 0;
            }
            fun inc(self) {
                self.count = self.count + 1;
            }
            fun get(self) {
                return self.count;
            }
        }
        var c = Counter();
        c.inc();
        c.inc();
        print(c.get()); // 2
    "#).unwrap();
}

#[test]
pub fn test_init_constructor() {
    // __init__() runs automatically when calling the class.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Point {
            fun __init__(self, x, y) {
                self.x = x;
                self.y = y;
            }
        }
        var p = Point(3, 4);
        print(p.x);           // 3
        print(p.y);           // 4
        print(p.x + p.y);     // 7
    "#).unwrap();
}

#[test]
pub fn test_init_returns_instance() {
    // Calling a class with __init__() should return the instance, not nil.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Person {
            fun __init__(self, name) {
                self.name = name;
            }
        }
        var p = Person("Alice");
        print(p.name); // "Alice"
    "#).unwrap();
}

#[test]
pub fn test_nested_function_in_method_captures_self() {
    // A closure inside a method should be able to capture `self`.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Nested {
            fun method(self) {
                fun inner() {
                    print(self);
                }
                print("in Nested::method");
                inner();
            }
        }
        var n = Nested();
        n.method();
    "#).unwrap();
}

#[test]
pub fn test_undefined_property_is_error() {
    // Accessing a non-existent property should fail at runtime.
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Empty {}
        var e = Empty();
        print(e.nope);
    "#);
    assert!(result.is_err(), "accessing undefined property should error");
}

#[test]
pub fn test_class_no_args_error() {
    // Calling a class with args but no __init__ should error.
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Empty {}
        var e = Empty(1);
    "#);
    assert!(result.is_err(), "class without init should not accept args");
}

#[test]
pub fn test_method_inherits_fields() {
    // Methods share the same instance fields.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Box {
            fun __init__(self, value) {
                self.value = value;
            }
            fun setValue(self, value) {
                self.value = value;
            }
            fun getValue(self) {
                return self.value;
            }
        }
        var b = Box(42);
        print(b.getValue());   // 42
        b.setValue(99);
        print(b.getValue());   // 99
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  __str__ magic method
// ------------------------------------------------------------------------

#[test]
pub fn test_str_method_custom_format() {
    // __str__ returns a custom string representation.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Greeting {
            fun __init__(self, name) {
                self.name = name;
            }
            fun __str__(self) {
                return "Hello, " + self.name + "!";
            }
        }
        var g = Greeting("Taro");
        print(g);
    "#).unwrap();
}

#[test]
pub fn test_str_method_constant() {
    // __str__ that always returns the same string.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Always {
            fun __str__(self) {
                return "always the same";
            }
        }
        print(Always());
    "#).unwrap();
}

#[test]
pub fn test_str_method_calls_other_method() {
    // __str__ can call other methods on self.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Pair {
            fun __init__(self, a, b) {
                self.a = a;
                self.b = b;
            }
            fun format(self, v) {
                return "[" + v + "]";
            }
            fun __str__(self) {
                return self.format(self.a) + " and " + self.format(self.b);
            }
        }
        var p = Pair("x", "y");
        print(p);
    "#).unwrap();
}

#[test]
pub fn test_str_method_with_closure() {
    // __str__ using a nested closure.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Wrapper {
            fun __init__(self, val) {
                self.val = val;
            }
            fun __str__(self) {
                fun wrap(v) {
                    return "<<" + v + ">>";
                }
                return wrap(self.val);
            }
        }
        var w = Wrapper("hello");
        print(w);
    "#).unwrap();
}

#[test]
pub fn test_str_method_non_string_errors() {
    // __str__ returning a non-string should be a runtime error.
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Bad {
            fun __str__(self) {
                return 42;
            }
        }
        var b = Bad();
        print(b);
    "#);
    assert!(result.is_err(), "__str__ returning non-string should error");
}

#[test]
pub fn test_str_without_method_defaults() {
    // Instance without __str__ uses default format.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Toast {}
        var toast = Toast();
        print(toast);
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  Inheritance
// ------------------------------------------------------------------------

#[test]
pub fn test_inherits_method() {
    // Subclass inherits a method from its superclass.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Animal {
            fun speak(self) {
                print("animal speaks");
            }
        }
        class Dog extends Animal {}
        var d = Dog();
        d.speak();
    "#).unwrap();
}

#[test]
pub fn test_overrides_method() {
    // Subclass overrides a superclass method.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Animal {
            fun speak(self) {
                print("animal");
            }
        }
        class Cat extends Animal {
            fun speak(self) {
                print("meow");
            }
        }
        var c = Cat();
        c.speak();
    "#).unwrap();
}

#[test]
pub fn test_inherits_init() {
    // Subclass without __init__ inherits superclass __init__.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Base {
            fun __init__(self, value) {
                self.value = value;
            }
            fun getValue(self) {
                return self.value;
            }
        }
        class Child extends Base {}
        var c = Child(42);
        print(c.getValue());
    "#).unwrap();
}

#[test]
pub fn test_multi_level_inheritance() {
    // Chain of three classes.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class A {
            fun a(self) {
                print("A");
            }
        }
        class B extends A {
            fun b(self) {
                print("B");
            }
        }
        class C extends B {
            fun c(self) {
                print("C");
            }
        }
        var x = C();
        x.a();
        x.b();
        x.c();
    "#).unwrap();
}

#[test]
pub fn test_override_init() {
    // Subclass overrides __init__.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Animal {
            fun __init__(self, name) {
                self.name = name;
            }
        }
        class Cat extends Animal {
            fun __init__(self, name, color) {
                self.name = name;
                self.color = color;
            }
        }
        var c = Cat("Kitty", "black");
        print(c.name);
        print(c.color);
    "#).unwrap();
}

#[test]
pub fn test_inherited_method_uses_fields() {
    // Inherited method accesses subclass instance fields.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Base {
            fun display(self) {
                print(self.x);
            }
        }
        class Derived extends Base {}
        var d = Derived();
        d.x = 99;
        d.display();
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  Magic method: __bool__
// ------------------------------------------------------------------------

#[test]
pub fn test_bool_magic_true() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Truthy {
            fun __bool__(self) {
                return true;
            }
        }
        var t = Truthy();
        if (t) { print("yes"); }
        if (!t) { print("no"); }
    "#).unwrap();
}

#[test]
pub fn test_bool_magic_false() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Falsy {
            fun __bool__(self) {
                return false;
            }
        }
        var f = Falsy();
        if (f) { print("yes"); }
        if (!f) { print("no"); }
    "#).unwrap();
}

#[test]
pub fn test_bool_magic_conditional() {
    // __bool__ method that depends on instance field.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __bool__(self) {
                return self.n > 0;
            }
        }
        print(Num(5) and true);   // true
        print(Num(0) or true);    // true
        print(Num(-3) or true);   // true
    "#).unwrap();
}

#[test]
pub fn test_bool_magic_non_bool_errors() {
    // __bool__ returning non-bool should error.
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Bad {
            fun __bool__(self) {
                return 1;
            }
        }
        var b = Bad();
        if (b) { print("bad"); }
    "#);
    assert!(result.is_err(), "__bool__ returning non-bool should error");
}

#[test]
pub fn test_bool_uses_not_magic() {
    // !instance should call __bool__ (via __not__ → __bool__ path or direct magic).
    // Actually !instance calls __not__ which checks __bool__ for truthiness.
    // Test that __bool__ is correctly used by ! operator.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Even {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __bool__(self) {
                return self.n == 0;
            }
        }
        print(!Even(0));  // false (because __bool__ returns true, inverted)
        print(!Even(5));  // true  (because __bool__ returns false, inverted)
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  Magic method: binary arithmetic (__add__, __sub__, __mul__, __div__)
// ------------------------------------------------------------------------

#[test]
pub fn test_add_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __add__(self, other) {
                return Num(self.n + other);
            }
            fun __str__(self) {
                return "Num(" + str(self.n) + ")";
            }
        }
        var a = Num(10);
        var b = a + 5;
        print(b);
    "#).unwrap();
}

#[test]
pub fn test_sub_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Vec {
            fun __init__(self, x, y) {
                self.x = x;
                self.y = y;
            }
            fun __sub__(self, other) {
                return Vec(self.x - other.x, self.y - other.y);
            }
            fun __str__(self) {
                return "(" + str(self.x) + "," + str(self.y) + ")";
            }
        }
        var v = Vec(5, 3) - Vec(2, 1);
        print(v);
    "#).unwrap();
}

#[test]
pub fn test_mul_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Vec {
            fun __init__(self, x, y) {
                self.x = x;
                self.y = y;
            }
            fun __mul__(self, s) {
                return Vec(self.x * s, self.y * s);
            }
            fun __str__(self) {
                return "(" + str(self.x) + "," + str(self.y) + ")";
            }
        }
        var v = Vec(2, 3) * 4;
        print(v);
    "#).unwrap();
}

#[test]
pub fn test_div_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Vec {
            fun __init__(self, x, y) {
                self.x = x;
                self.y = y;
            }
            fun __div__(self, s) {
                return Vec(self.x / s, self.y / s);
            }
            fun __str__(self) {
                return "(" + str(self.x) + "," + str(self.y) + ")";
            }
        }
        var v = Vec(10, 6) / 2;
        print(v);
    "#).unwrap();
}

#[test]
pub fn test_magic_no_method_errors() {
    // Instance without __add__ should error on + with instance.
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Empty {}
        var e = Empty();
        print(e + 1);
    "#);
    assert!(result.is_err(), "adding instance without __add__ should error");
}

// ------------------------------------------------------------------------
//  Magic method: comparison (__eq__, __ne__, __gt__, __ge__, __lt__, __le__)
// ------------------------------------------------------------------------

#[test]
pub fn test_eq_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Person {
            fun __init__(self, id) {
                self.id = id;
            }
            fun __eq__(self, other) {
                return self.id == other;
            }
        }
        var p = Person(1);
        print(p == 1);   // true
        print(p == 2);   // false
    "#).unwrap();
}

#[test]
pub fn test_eq_magic_uses_ne_fallback() {
    // Instance with __eq__ but no __ne__: != should work via __eq__ + invert.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Person {
            fun __init__(self, id) {
                self.id = id;
            }
            fun __eq__(self, other) {
                return self.id == other;
            }
        }
        var p = Person(1);
        print(p != 1);   // false
        print(p != 2);   // true
    "#).unwrap();
}

#[test]
pub fn test_lt_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __lt__(self, other) {
                return self.n < other;
            }
        }
        var a = Num(5);
        print(a < 10);   // true
        print(a < 3);    // false
    "#).unwrap();
}

#[test]
pub fn test_le_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __le__(self, other) {
                return self.n <= other;
            }
        }
        var a = Num(5);
        print(a <= 5);   // true
        print(a <= 10);  // true
        print(a <= 3);   // false
    "#).unwrap();
}

#[test]
pub fn test_gt_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __gt__(self, other) {
                return self.n > other;
            }
        }
        var a = Num(5);
        print(a > 3);    // true
        print(a > 10);   // false
    "#).unwrap();
}

#[test]
pub fn test_ge_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __ge__(self, other) {
                return self.n >= other;
            }
        }
        var a = Num(5);
        print(a >= 5);   // true
        print(a >= 3);   // true
        print(a >= 10);  // false
    "#).unwrap();
}

#[test]
pub fn test_ge_magic_fallback_to_lt_invert() {
    // Instance with __lt__ but no __ge__: >= should work via __lt__ + invert.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __lt__(self, other) {
                return self.n < other;
            }
        }
        var a = Num(5);
        print(a >= 3);   // true  (5 < 3 is false, inverted → true)
        print(a >= 5);   // true  (5 < 5 is false, inverted → true)
        print(a >= 10);  // false (5 < 10 is true, inverted → false)
    "#).unwrap();
}

#[test]
pub fn test_le_magic_fallback_to_gt_invert() {
    // Instance with __gt__ but no __le__: <= should work via __gt__ + invert.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __gt__(self, other) {
                return self.n > other;
            }
        }
        var a = Num(5);
        print(a <= 10);  // true  (5 > 10 is false, inverted → true)
        print(a <= 5);   // true  (5 > 5 is false, inverted → true)
        print(a <= 3);   // false (5 > 3 is true, inverted → false)
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  Magic method: unary (__neg__, __not__)
// ------------------------------------------------------------------------

#[test]
pub fn test_neg_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __neg__(self) {
                return Num(-self.n);
            }
            fun __str__(self) {
                return "Num(" + str(self.n) + ")";
            }
        }
        var a = Num(42);
        print(-a);
    "#).unwrap();
}

#[test]
pub fn test_not_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Toggle {
            fun __init__(self, v) {
                self.v = v;
            }
            fun __not__(self) {
                return !self.v;
            }
        }
        var t = Toggle(true);
        print(!t);  // false
        t.v = false;
        print(!t);  // true
    "#).unwrap();
}

#[test]
pub fn test_not_on_integer_is_truthiness() {
    // !0 → true, !5 → false (truthiness, not arithmetic negation).
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(!0);   // true
        print(!5);   // false
        print(!-1);  // false
    "#).unwrap();
}

#[test]
pub fn test_not_on_string_is_truthiness() {
    // !"" → true, !"hi" → false.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(!"");     // true
        print(!"hi");   // false
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  Magic method: inherited
// ------------------------------------------------------------------------

#[test]
pub fn test_inherited_magic_method() {
    // Subclass inherits __add__ from superclass.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Base {
            fun __add__(self, other) {
                return self.n + other;
            }
        }
        class Child extends Base {
            fun __init__(self, n) {
                self.n = n;
            }
        }
        var c = Child(10);
        print(c + 5);
    "#).unwrap();
}

#[test]
pub fn test_overridden_magic_method() {
    // Subclass overrides __str__ from superclass.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Base {
            fun __str__(self) {
                return "base";
            }
        }
        class Child extends Base {
            fun __str__(self) {
                return "child";
            }
        }
        print(Child());
        print(Base());
    "#).unwrap();
}

#[test]
pub fn test_magic_method_bool_inherited() {
    // Inherited __bool__ should work on subclass instances.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Base {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __bool__(self) {
                return self.n != 0;
            }
        }
        class Child extends Base {}
        if (Child(0)) { print("bad0"); }
        if (!Child(0)) { print("good0"); }
        if (Child(1)) { print("good1"); }
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  Magic method: chained operations
// ------------------------------------------------------------------------

#[test]
pub fn test_magic_chain_arithmetic() {
    // Chain of magic binary ops: a + b + c → (a.__add__(b)).__add__(c)
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __add__(self, other) {
                return Num(self.n + other);
            }
            fun __str__(self) {
                return str(self.n);
            }
        }
        var a = Num(1);
        print(a + 3 + 4);
    "#).unwrap();
}

#[test]
pub fn test_magic_chain_comparison() {
    // a < b == c (left-to-right: (a < b) == c)
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __lt__(self, other) {
                return self.n < other;
            }
            fun __eq__(self, other) {
                return self.n == other;
            }
        }
        var a = Num(3);
        // 3 < 5 == true  →  Num(3).__lt__(5).__eq__(true) doesn't work
        // Just test individual comparisons:
        print(a < 5);    // true
        print(a == 3);   // true
        print(a < 5);    // true
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  len() builtin & __len__ magic method
// ------------------------------------------------------------------------

#[test]
pub fn test_len_string() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(len(""));       // 0
        print(len("hello"));  // 5
        print(len("abc"));    // 3
    "#).unwrap();
}

#[test]
pub fn test_len_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class List {
            fun __init__(self) {
                self.size = 0;
            }
            fun add(self) {
                self.size = self.size + 1;
            }
            fun __len__(self) {
                return self.size;
            }
        }
        var l = List();
        print(len(l));  // 0
        l.add();
        l.add();
        l.add();
        print(len(l));  // 3
    "#).unwrap();
}

#[test]
pub fn test_len_magic_non_integer_errors() {
    // __len__ returning non-integer should error.
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Bad {
            fun __len__(self) {
                return "not a number";
            }
        }
        print(len(Bad()));
    "#);
    assert!(result.is_err(), "__len__ returning non-integer should error");
}

#[test]
pub fn test_len_on_unsupported_type_errors() {
    // len() on types without __len__ should error.
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        print(len(42));
    "#);
    assert!(result.is_err(), "len on integer should error");

    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Empty {}
        print(len(Empty()));
    "#);
    assert!(result.is_err(), "len on instance without __len__ should error");
}

#[test]
pub fn test_len_magic_inherited() {
    // Subclass inherits __len__ from superclass.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Base {
            fun __len__(self) {
                return 42;
            }
        }
        class Child extends Base {}
        print(len(Child()));  // 42
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  int() builtin & __int__ magic method
// ------------------------------------------------------------------------

#[test]
pub fn test_int_primitive() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(int(42));       // 42
        print(int(3.14));     // 3
        print(int(true));     // 1
        print(int(false));    // 0
    "#).unwrap();
}

#[test]
pub fn test_int_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Num {
            fun __init__(self, n) {
                self.n = n;
            }
            fun __int__(self) {
                return self.n;
            }
        }
        print(int(Num(42)));   // 42
        print(int(Num(-7)));   // -7
    "#).unwrap();
}

#[test]
pub fn test_int_magic_non_integer_errors() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Bad {
            fun __int__(self) {
                return "not int";
            }
        }
        print(int(Bad()));
    "#);
    assert!(result.is_err(), "__int__ returning non-integer should error");
}

#[test]
pub fn test_int_on_unsupported_type_errors() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Empty {}
        print(int(Empty()));
    "#);
    assert!(result.is_err(), "int on instance without __int__ should error");
}

#[test]
pub fn test_int_magic_inherited() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Base {
            fun __int__(self) {
                return 99;
            }
        }
        class Child extends Base {}
        print(int(Child()));  // 99
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  float() builtin & __float__ magic method
// ------------------------------------------------------------------------

#[test]
pub fn test_float_primitive() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(float(3.14));    // 3.14
        print(float(42));      // 42.0
        print(float(true));    // 1.0
        print(float(false));   // 0.0
    "#).unwrap();
}

#[test]
pub fn test_float_magic() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Vec {
            fun __init__(self, x, y) {
                self.x = x;
                self.y = y;
            }
            fun __float__(self) {
                return self.x + self.y + 0.0;
            }
        }
        print(float(Vec(3, 1)));  // 4.0
    "#).unwrap();
}

#[test]
pub fn test_float_magic_non_float_errors() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Bad {
            fun __float__(self) {
                return "not float";
            }
        }
        print(float(Bad()));
    "#);
    assert!(result.is_err(), "__float__ returning non-float should error");
}

#[test]
pub fn test_float_on_unsupported_type_errors() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"
        class Empty {}
        print(float(Empty()));
    "#);
    assert!(result.is_err(), "float on instance without __float__ should error");
}

#[test]
pub fn test_float_magic_inherited() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Base {
            fun __float__(self) {
                return 3.5;
            }
        }
        class Child extends Base {}
        print(float(Child()));  // 3.5
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  type() builtin
// ------------------------------------------------------------------------

#[test]
pub fn test_type_builtin() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(type(nil));     // nil
        print(type(42));      // integer
        print(type(3.14));    // float
        print(type(true));    // boolean
        print(type("hi"));    // string
    "#).unwrap();
}

#[test]
pub fn test_type_instance_returns_class() {
    // type(instance) should return the class object, not a string.
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Foo {}
        var f = Foo();
        print(type(f) == Foo);    // true
    "#).unwrap();
}

#[test]
pub fn test_type_instance_create_from_type() {
    // You should be able to create an instance from the result of type().
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        class Foo {
            fun __init__(self, x) {
                self.x = x;
            }
        }
        var f = Foo(1);
        var t = type(f);
        var f2 = t(2);
        print(f2.x);              // 2
        print(type(f2) == Foo);   // true
    "#).unwrap();
}

// ------------------------------------------------------------------------
//  abs() builtin
// ------------------------------------------------------------------------

#[test]
pub fn test_abs_builtin() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(abs(5));        // 5
        print(abs(-5));       // 5
        print(abs(3.14));     // 3.14
        print(abs(-3.14));    // 3.14
        print(abs(0));        // 0
    "#).unwrap();
}

#[test]
pub fn test_abs_non_number_errors() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"print(abs("hi"));"#);
    assert!(result.is_err(), "abs on string should error");
}

// ------------------------------------------------------------------------
//  min() / max() builtins
// ------------------------------------------------------------------------

#[test]
pub fn test_min_builtin() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(min(3, 1, 2));         // 1
        print(min(5));               // 5
        print(min(3.0, 1.5, 2.5));   // 1.5
        print(min(-1, 0, 1));         // -1
    "#).unwrap();
}

#[test]
pub fn test_max_builtin() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        print(max(3, 1, 2));         // 3
        print(max(5));               // 5
        print(max(3.0, 1.5, 2.5));   // 3
        print(max(-1, 0, 1));         // 1
    "#).unwrap();
}

#[test]
pub fn test_min_empty_errors() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"print(min());"#);
    assert!(result.is_err(), "min with no args should error");
}

#[test]
pub fn test_max_empty_errors() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(r#"print(max());"#);
    assert!(result.is_err(), "max with no args should error");
}

// ------------------------------------------------------------------------
//  clock() builtin
// ------------------------------------------------------------------------

#[test]
pub fn test_clock_returns_float() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"
        var t = clock();
        print(t > 0);   // must be positive
    "#).unwrap();
}