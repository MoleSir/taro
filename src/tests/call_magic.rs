use super::*;

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
            fun __call__(self, x) { return Base.__call__(self, x) * 10; }
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


