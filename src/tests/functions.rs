use super::*;

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

// ===========================================================================
// Lambda (anonymous function) expressions — runtime tests
// ===========================================================================


#[test]
pub fn test_lambda_assigned_and_called() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var double = fun(x) { return x * 2; };
        print(double(21));  // 42
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_no_params() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var answer = fun() { return 42; };
        print(answer());  // 42
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_passed_as_argument() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun apply(f, x) { return f(x); }
        var result = apply(fun(n) { return n + 10; }, 5);
        print(result);  // 15
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_closure_captures_local() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun makeAdder(n) {
            return fun(x) { return x + n; };
        }
        var add5 = makeAdder(5);
        print(add5(10));  // 15
        print(add5(100)); // 105
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_closure_multiple_upvalues() {
    // Single lambda capturing two parameters.
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun combine(a, b) {
            return fun() { return a * b; };
        }
        var fn12 = combine(3, 4);
        print(fn12());  // 12
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_immediately_called() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var result = fun(x) { return x * 3; }(7);
        print(result);  // 21
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_in_list_comprehension_style() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun map(f, items) {
            var result = [];
            for item in items {
                result.append(f(item));
            }
            return result;
        }
        var doubled = map(fun(x) { return x * 2; }, [1, 2, 3]);
        print(doubled[0]);  // 2
        print(doubled[1]);  // 4
        print(doubled[2]);  // 6
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_nested_in_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var square = fun(x) { return x * x; };
        print(square(4) + square(3));  // 16 + 9 = 25
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_with_default_param() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var greet = fun(name, greeting = \"Hi\") {
            return greeting + \" \" + name;
        };
        print(greet(\"Taro\"));            // Hi Taro
        print(greet(\"Taro\", \"Hello\"));  // Hello Taro
    ",
    )
    .unwrap();
}


#[test]
pub fn test_lambda_recursive_indirect() {
    // Lambda calls itself via a variable that captures it.
    // Just verify it compiles and runs without crash.
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var sum_to = nil;
        sum_to = fun(n) {
            if n <= 0 { return 0; }
            return n + sum_to(n - 1);
        };
        print(sum_to(5));  // 15
    ",
    )
    .unwrap();
}

// ===========================================================================
// Two closures capturing the same upvalues
// ===========================================================================
// Exercises `capture_upvalue` linked-list maintenance: when two closures in
// the same enclosing function capture the same upvalues, the second closure
// must reuse the existing upvalue objects (not create duplicates).


#[test]
pub fn test_two_closures_same_upvalues_named() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun outer(a, b) {
            fun inner1() { return a + b; }
            fun inner2() { return a * b; }
            return [inner1, inner2];
        }
        var funcs = outer(3, 4);
        print(funcs[0]());  // 7
        print(funcs[1]());  // 12
    ",
    )
    .unwrap();
}


#[test]
pub fn test_two_lambdas_same_upvalues() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        fun outer(a, b) {
            var f1 = fun() { return a + b; };
            var f2 = fun() { return a * b; };
            return [f1, f2];
        }
        var funcs = outer(3, 4);
        print(funcs[0]());  // 7
        print(funcs[1]());  // 12
    ",
    )
    .unwrap();
}

