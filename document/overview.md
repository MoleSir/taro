# Language overview

Taro is a small, class-based scripting language with closures, garbage collection, and Python-style magic methods.

## Values

- Nil: `nil`
- Booleans: `true`, `false`
- Numbers: integers (`42`, `-7`) and floats (`3.14`, `-2.7`)
- Strings: `"hello"`, concatenation with `+`
- Lists: `[1, 2, 3]`, indexing, nested lists
- Dicts: `{"key": value}`, key-value storage, indexing by key
- Objects: functions, classes, instances, closures, bound methods, lists, dicts

## Variables and scope

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

## Lists

```taro
var a = [1, 2, 3];       // literal
print(a[0]);              // 1 — indexing
print(a[-1]);             // 3 — negative index
a[0] = 99;                // mutation
print(len(a));            // 3
print(bool([]));          // false
print(bool([1]));         // true

// Nested lists
var m = [[1, 2], [3, 4]];
print(m[0][1]);           // 2

// list() builtin (variadic)
var b = list(1, 2, 3);    // same as [1, 2, 3]
var c = list();           // []

// Loop over a list
var sum = 0;
var i = 0;
while (i < len(a)) {
    sum = sum + a[i];
    i = i + 1;
}

// List methods
a.append(4);               // add to end
print(a.pop());            // remove and return last item
a.extend([5, 6]);          // add all items from another list
```

## Dicts

```taro
var d = {"a": 1, "b": 2, "c": 3};  // literal
print(d["a"]);              // 1 — indexing by key
print(len(d));              // 3
print(bool({}));            // false
print(bool({"a": 1}));      // true

// Mutation
d["a"] = 99;                // update existing key
d["new"] = 42;              // add new key

// Mixed key types
var m = {1: "one", "two": 2, true: "yes"};

// Nested dicts
var nested = {"a": {"x": 1, "y": 2}, "b": {"x": 3, "y": 4}};
print(nested["a"]["x"]);    // 1

// dict() builtin (creates empty dict)
var e = dict();
e["hello"] = "world";

// Loop over dict keys via list
var keys = ["a", "b", "c"];
var i = 0;
while (i < len(keys)) {
    print(d[keys[i]]);
    i = i + 1;
}

// Dict methods
print(d.get("a"));          // 1 — get with nil default for missing
print(d.get("z"));          // nil
var key_list = d.keys();    // list of all keys
var val_list = d.values();  // list of all values
print(d.pop("a"));          // 1 — remove and return value
```

Dict keys use `Value` equality and hashing — same-type comparisons apply.

## Control flow

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

## Logical operators

`and` and `or` short-circuit:

```taro
print(true and 42);    // 42
print(false or 99);    // 99
print(nil or "fallback");
```

## Functions and closures

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

## Classes and instances

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

## Inheritance

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

## Magic methods

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
| `__getitem__(self, key)` | `instance[key]` |
| `__setitem__(self, key, value)` | `instance[key] = value` |

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

## Builtin functions

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
| `list(a, b, ...)` | Create a list from the given arguments (variadic). |
| `dict()` | Create an empty dict. |

## Builtin methods

List and Dict objects have builtin methods callable via dot syntax:

| Type | Method | Description |
|------|--------|-------------|
| List | `list.append(value)` | Add an item to the end; returns the value. |
| List | `list.pop()` | Remove and return the last item; errors on empty. |
| List | `list.extend(other)` | Add all items from another list. |
| Dict | `dict.get(key)` | Return the value for `key`, or `nil` if missing. |
| Dict | `dict.keys()` | Return a list of all keys. |
| Dict | `dict.values()` | Return a list of all values. |
| Dict | `dict.pop(key)` | Remove `key` and return its value; errors if missing. |

Methods can be assigned to variables and called later (bound methods):

```taro
var appender = my_list.append;
appender(42);   // same as my_list.append(42)

var getter = my_dict.get;
print(getter("key"));
```
