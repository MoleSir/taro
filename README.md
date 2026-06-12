# Taro

A dynamically-typed scripting language with a bytecode VM, written in Rust. Inspired by [Crafting Interpreters](https://craftinginterpreters.com/).



## Quick start

```bash
cargo run -- tests/scripts/10_class.taro
```

Run all tests:

```bash
cargo test
```



## Language overview

Taro is a small, class-based scripting language with closures, garbage collection, and Python-style magic methods.

### Values

- Nil: `nil`
- Booleans: `true`, `false`
- Numbers: integers (`42`, `-7`) and floats (`3.14`, `-2.7`)
- Strings: `"hello"`, concatenation with `+`
- Objects: functions, classes, instances, closures, bound methods

### Variables and scope

```taro
var x = 10;        // declaration
x = 42;            // assignment
print(x);          // access

// Block scope
{
    var y = 99;
    print(y);      // 99
}
// y is out of scope here
```

### Control flow

```taro
// Conditionals
if (score > 90) {
    print("A");
} else if (score > 75) {
    print("B");
} else {
    print("C");
}

// While loop
var i = 0;
while (i < 5) {
    print(i);
    i = i + 1;
}

// For loop
for (var i = 0; i < 10; i = i + 1) {
    print(i);
}
```

### Logical operators

`and` and `or` short-circuit:

```taro
print(true and 42);    // 42
print(false or 99);    // 99
print(nil or "fallback");
```

### Functions and closures

```taro
fun add(a, b) {
    return a + b;
}
print(add(3, 4));  // 7

// Closures capture variables from enclosing scope
fun makeCounter() {
    var i = 0;
    fun counter() {
        i = i + 1;
        return i;
    }
    return counter;
}

var c = makeCounter();
print(c());  // 1
print(c());  // 2
print(c());  // 3
```

### Classes and instances

```taro
class Point {
    fun __init__(self, x, y) {
        self.x = x;
        self.y = y;
    }

    fun distance(self) {
        return (self.x + self.y);
    }
}

var p = Point(3, 4);
print(p.x);          // 3
print(p.distance()); // 7
```

- Methods declare `self` as the first parameter (explicit receiver).
- `Class(args)` calls `__init__` on a fresh instance.
- Class without `__init__` takes zero arguments.
- `instance.field = value` sets fields; `instance.field` reads them.
- `instance.method(args)` looks up methods on the instance's class.

### Inheritance

```taro
class Animal {
    fun speak(self) {
        print("animal speaks");
    }
}

class Dog extends Animal {
    fun bark(self) {
        print("woof");
    }
}

var d = Dog();
d.speak();  // inherited
d.bark();   // own

// Override
class Cat extends Animal {
    fun speak(self) {
        print("meow");
    }
}

// Multi-level
class A { fun a(self) { print("A"); } }
class B extends A { fun b(self) { print("B"); } }
class C extends B { fun c(self) { print("C"); } }
var x = C();
x.a();  // A
x.b();  // B
x.c();  // C
```

Methods are copied from superclass to subclass at class-creation time. Subclass methods override inherited ones.

### Magic methods

Python-style magic methods let instances customize operator and builtin behavior:

| Method | Triggered by |
|--------|-------------|
| `__str__(self)` | `str()`, `print()` |
| `__bool__(self)` | `bool()`, conditionals, `!` |
| `__neg__(self)` | `-instance` |
| `__not__(self)` | `!instance` (explicit); falls back to `__bool__` + invert |
| `__add__(self, other)` | `instance + x` |
| `__sub__(self, other)` | `instance - x` |
| `__mul__(self, other)` | `instance * x` |
| `__div__(self, other)` | `instance / x` |
| `__eq__(self, other)` | `instance == x` |
| `__ne__(self, other)` | `instance != x` (falls back to `__eq__` + invert) |
| `__gt__(self, other)` | `instance > x` |
| `__ge__(self, other)` | `instance >= x` (falls back to `__lt__` + invert) |
| `__lt__(self, other)` | `instance < x` |
| `__le__(self, other)` | `instance <= x` (falls back to `__gt__` + invert) |
| `__len__(self)` | `len()` |
| `__int__(self)` | `int()` |
| `__float__(self)` | `float()` |

```taro
class Vec {
    fun __init__(self, x, y) {
        self.x = x;
        self.y = y;
    }
    fun __add__(self, other) {
        return Vec(self.x + other.x, self.y + other.y);
    }
    fun __str__(self) {
        return "(" + str(self.x) + "," + str(self.y) + ")";
    }
    fun __bool__(self) {
        return self.x != 0 or self.y != 0;
    }
}

var v1 = Vec(1, 2);
var v2 = Vec(3, 4);
print(v1 + v2);       // (4,6)
print(bool(v1));      // true
```

Comparison fallback mechanism: `!=` works with only `__eq__`, `>=` works with only `__lt__`, `<=` works with only `__gt__`.

### Builtin functions

| Function | Description |
|----------|-------------|
| `print(a, b, ...)` | Print values to stdout, space-separated. |
| `str(value)` | Convert any value to a string. |
| `bool(value)` | Convert any value to a boolean. |
| `len(value)` | Length of a string, or `__len__` on an instance. |
| `int(value)` | Convert to integer (truncates float); dispatches to `__int__`. |
| `float(value)` | Convert to float (promotes integer); dispatches to `__float__`. |
| `type(value)` | For instances, returns the class object; otherwise the type-name string. |
| `abs(value)` | Absolute value of an integer or float. |
| `min(a, b, ...)` | Smallest argument (variadic). |
| `max(a, b, ...)` | Largest argument (variadic). |
| `input(prompt?)` | Read a line from stdin, with optional prompt (no trailing newline). |
| `clock()` | Wall-clock time in seconds since Unix epoch (as float). |



## Implementation

- Compiler: single-pass recursive-descent parser emitting bytecode for a stack-based VM.
- VM: direct threaded interpretation with `CallFrame` stack.
- GC: mark-and-sweep with gray-stack tracing.
- Objects: heap-allocated with handle-based access (`ObjectHandle`).

### Project structure

```
src/
├── base/
│   ├── chunk.rs          # Bytecode chunk (write/read instructions)
│   ├── instruct.rs       # ByteCode & Instruction enums
│   ├── value.rs          # Value enum (Nil, Integer, Float, Bool, String, Object)
│   └── object/
│       ├── mod.rs        # Object enum & type-checking helpers
│       ├── heap.rs       # ObjectHeap — allocation, GC mark/sweep
│       └── variants.rs   # Object variants (Function, Class, Instance, Closure, …)
├── compile/
│   ├── mod.rs            # Compiler entry point
│   ├── parse.rs          # Parser — Pratt parsing, statement/expression compilation
│   ├── scan.rs           # Lexer / scanner
│   └── token.rs          # Token & TokenKind
└── vm/
    ├── mod.rs            # VirtualMachine — execution loop, call frames, stack ops
    ├── builtin.rs        # Builtin functions (print, str, len, min, max, …)
    ├── magic.rs          # Magic method implementations (__add__, __eq__, __str__, …)
    ├── error.rs          # Error types
    ├── gc.rs             # GC threshold & trigger
    └── tests.rs          # VM runtime tests

tests/scripts/
├── 10_class.taro         # Class & instance integration tests
├── 11_magic.taro         # Magic method integration tests
└── 12_builin.taro        # Builtin function integration tests
```



## References

- [Crafting Interpreters](https://craftinginterpreters.com/a-bytecode-virtual-machine.html) — the book that inspired this project.
