use super::*;

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
        c.write_instruction(Instruction::GetProperty("double".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(5)), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
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
pub fn test_inherited_method_call() {
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
        // Base.m(self) — explicit class-qualified call
        dc.write_instruction(Instruction::Constant(base), 1, 1, h); // push Base class
        dc.write_instruction(Instruction::GetProperty("m".into()), 1, 1, h); // pop Base, push raw closure
        dc.write_instruction(Instruction::GetLocal(1), 1, 1, h); // push self
        dc.write_instruction(Instruction::Call(1), 1, 1, h); // call closure(self)
        dc.write_instruction(Instruction::Return, 1, 1, h);
        let dm = h.alloc_function("m", 1, 1, vec![], vec![], dc);
        let dm_cl = h.alloc_closure(dm, ObjectHandle::NIL);
        h.get_class_mut(derived).unwrap().methods.insert("m".into(), crate::Method::User(dm_cl));

        c.write_instruction(Instruction::Constant(derived), 1, 1, h);
        c.write_instruction(Instruction::Call(0), 1, 1, h);
        c.write_instruction(Instruction::GetProperty("m".into()), 1, 1, h);
        c.write_instruction(Instruction::Call(0), 1, 1, h);
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
