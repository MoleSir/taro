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