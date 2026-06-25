# Language overview

Taro is a small, class-based scripting language with closures, garbage collection, and Python-style magic methods.

## Values

- Nil: `nil`
- Booleans: `true`, `false`
- Numbers: integers (`42`, `-7`) and floats (`3.14`, `-2.7`)
- Strings: `"hello"`, concatenation with `+`
- Lists: `[1, 2, 3]`, indexing, nested lists
- Dicts: `{"key": value}`, key-value storage, indexing by key
- Bytes: binary data, created with `bytes("...")` or `bytes([...])`
- Sets: unique elements, created with `set(args...)`
- Objects: functions, classes, instances, closures, bound methods, lists, dicts, bytes, sets

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

## Bytes

```taro
var b = bytes("hello");         // from string → UTF-8 bytes
print(b);                        // b"hello"
print(len(b));                   // 5
print(b[0]);                     // 104 (ASCII 'h')
print(b[-1]);                    // 111 (ASCII 'o')

var c = bytes([65, 66, 67]);    // from list of ints (0–255)
print(c);                        // b"ABC"
print(bool(bytes([])));         // false
print(bool(bytes("x")));        // true

// Bytes methods
print(b.hex());                  // "68656c6c6f"
print(b.decode("utf-8"));       // "hello"
print(b.to_list());              // [104, 101, 108, 108, 111]

// Equality and concatenation
print(bytes("ab") == bytes("ab"));   // true
print(bytes("ab") + bytes("cd"));    // b"abcd"

// Membership test
print(104 in b);                 // true ('h')

// Iteration yields integer byte values
for byte in bytes("Hi") {
    print(byte);                 // 72, 105
}
```

Bytes support hashing, so they can be used as dict keys or set elements.

## Sets

```taro
var s = set(1, 2, 3);           // create a set
print(len(s));                   // 3
print(2 in s);                   // true
print(4 in s);                   // false
s.add(4);                        // add an element
print(4 in s);                   // true
s.remove(2);                     // remove an element
print(2 in s);                   // false
print(bool(set()));              // false
print(bool(set(1)));             // true

// Iteration
for item in s {
    print(item);
}
```

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

// Break — exit the loop early
for (var i = 0; i < 10; i = i + 1) {
    if (i == 3) { break; }
    print(i);          // 0, 1, 2
}

// Continue — skip the rest of the current iteration
// In for loops, the increment clause still runs
var sum = 0;
for (var i = 0; i < 5; i = i + 1) {
    if (i == 2) { continue; }
    sum = sum + i;
}
print(sum);            // 0 + 1 + 3 + 4 = 8
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

// Default parameters
fun greet(name, greeting = "Hello", punctuation = "!") {
    return greeting + " " + name + punctuation;
}
print(greet("World"));                    // Hello World!
print(greet("Taro", "Hi"));               // Hi Taro!
print(greet("Claude", punctuation = ".")); // Hello Claude.

// Keyword arguments (positional must come before keyword)
print(greet(name = "World"));             // Hello World!
print(greet("Bob", punctuation = "?"));   // Hello Bob?

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

Default values must be constant literals (numbers, strings, booleans, nil).
Parameters with defaults must come after required parameters.
Keyword arguments are matched by name at runtime; unknown or duplicate keyword
arguments produce runtime errors.

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
- Constructors support default parameters and keyword arguments.
- `instance.field = value` sets fields; `instance.field` reads them.
- `instance.method(args)` looks up methods on the instance's class.

```taro
class Point {
    fun __init__(self, x = 0, y = 0) {
        self.x = x;
        self.y = y;
    }
}

var a = Point();           // (0, 0)
var b = Point(5);          // (5, 0)
var c = Point(y = 7);      // (0, 7)
var d = Point(x = 3, y = 4); // (3, 4)
```

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
| `set(a, b, ...)` | Create a set from the given arguments (variadic). |
| `bytes(value)` | Create bytes from a string (UTF-8 encode) or list of ints (0–255). |
| `is_iter_end(value)` | Return `true` if the value is the `IterEnd` sentinel (used to detect end of iteration in custom iterators). |
| `format(fmt, args...)` | Substitute `{}` placeholders with `__str__` of each argument. `{{`/`}}` for literal braces. Errors on placeholder/arg count mismatch. |

## Builtin methods

List, Dict, and String objects have builtin methods callable via dot syntax:

| Type | Method | Description |
|------|--------|-------------|
| List | `list.append(value)` | Add an item to the end; returns the value. |
| List | `list.pop()` | Remove and return the last item; errors on empty. |
| List | `list.extend(other)` | Add all items from another list. |
| Dict | `dict.get(key)` | Return the value for `key`, or `nil` if missing. |
| Dict | `dict.keys()` | Return a list of all keys. |
| Dict | `dict.values()` | Return a list of all values. |
| Dict | `dict.pop(key)` | Remove `key` and return its value; errors if missing. |
| String | `str.upper()` / `str.lower()` | Convert case (aliases: `to_uppercase()`, `to_lowercase()`). |
| String | `str.trim()` | Remove leading/trailing whitespace (aliases: `strip()`, `lstrip()`, `rstrip()`). |
| String | `str.starts_with(p)` / `str.ends_with(s)` | Check prefix/suffix. |
| String | `str.contains(sub)` | Check if string contains `sub`. |
| String | `str.replace(old, new)` | Replace all occurrences of `old` with `new`. |
| String | `str.split(delim)` | Split by delimiter, return list of strings. |
| String | `str.substr(start, len)` | Extract substring (negative start counts from end). |
| String | `str.find(sub)` / `str.rfind(sub)` | Return first/last index of `sub`, or -1 if not found. |
| String | `str.is_empty()` | Return `true` if the string is empty. |
| String | `str.repeat(n)` | Repeat the string `n` times. |
| String | `str.len()` | Return the string length in bytes. |
| Bytes | `bytes.hex()` | Return hex string representation. |
| Bytes | `bytes.decode(enc)` | Decode bytes to string (supports `"utf-8"`). |
| Bytes | `bytes.to_list()` | Return list of integer byte values. |
| Bytes | `bytes.len()` | Return the number of bytes. |

Methods can be assigned to variables and called later (bound methods):

```taro
var appender = my_list.append;
appender(42);   // same as my_list.append(42)

var getter = my_dict.get;
print(getter("key"));
```

## Imports and modules

Taro supports two kinds of module imports:

**File-based imports** load and execute a `.taro` script, exposing its top-level definitions
as fields on a module object:

```taro
import "tests/scripts/lib/math.taro";
print(math.PI);                  // 3.14159
print(math.add(10, 20));         // 30

// Import as expression
import "tests/scripts/lib/math.taro" as m;
print(m.mul(7, 6));              // 42

// Use classes from modules
var v = math.Vec(3, 4) + math.Vec(1, 2);
print(str(v));                   // Vec(4,6)
```

Module globals do not leak — `PI` is only accessible via `math.PI`.

**Virtual std modules** are built into the VM and imported via `import "std/<name>"`:

| Module | Description |
|--------|-------------|
| `std/argparse`  | CLI argument parser — flag definitions, type coercion (pure taro) |
| `std/ffi`        | Foreign Function Interface — dynamic library loading, C function calls, structs |
| `std/fs`        | File I/O — `File` class + standalone convenience functions |
| `std/itertools` | Lazy iterators — map, filter, zip, chain, take, drop, … (pure taro) |
| `std/json`      | JSON encoding (serialize) and decoding (parse) |
| `std/logging`   | Leveled logging with timestamps and named loggers (pure taro) |
| `std/math`      | Constants, trig, logarithms, rounding, angle conversion |
| `std/net`       | TCP networking — `Socket` client + `Server` listener |
| `std/os`        | Environment variables, process info, working directory, shell commands |
| `std/random`    | Random numbers, randint, uniform, choice, shuffle |
| `std/time`      | Unix timestamp, sleep, structured UTC time |

### `std/argparse`

Pure-taro CLI argument parser supporting flags with type coercion (string, int, float, bool).

```taro
import "std/argparse";

var p = argparse.Parser();
p.add_str("--name", "name", "default_name");
p.add_int("--count", "count", 1);
p.add_float("--rate", "rate", 0.5);
p.add_bool("--verbose", "verbose");

var args = p.parse(os.args());
print(args["name"]);      // from --name flag
print(args["verbose"]);   // true if --verbose given
```

| Method | Description |
|--------|-------------|
| `p.add_str(flag, dest, default)` | Add a string flag. |
| `p.add_int(flag, dest, default)` | Add an integer flag. |
| `p.add_float(flag, dest, default)` | Add a float flag. |
| `p.add_bool(flag, dest)` | Add a boolean switch (no value; sets to `true` when present). |
| `p.parse(args_list)` | Parse a list of tokens (e.g. `os.args()`), return a dict mapping dest→value. Returns `nil` on error (with diagnostic printed). |

### `std/logging`

Pure-taro leveled logging with timestamps and named loggers.

```taro
import "std/logging" as log;

// Level constants: log.DEBUG, log.INFO, log.WARN, log.ERROR
log.set_level(log.INFO);

log.info("server started");
log.warn("disk usage at 80%");
log.error("connection refused");

// Named logger with its own level
var db = log.get_logger("database", log.WARN);
db.error("query timeout");   // shown
db.info("query ok");         // suppressed (WARN > INFO)
```

| Function | Description |
|----------|-------------|
| `log.get_logger(name, level)` | Create a named logger with the given level. |
| `log.set_level(level)` | Set the root logger's minimum level. |
| `log.debug(msg)` | Log at DEBUG level (level 0). |
| `log.info(msg)` | Log at INFO level (level 1). |
| `log.warn(msg)` | Log at WARN level (level 2). |
| `log.error(msg)` | Log at ERROR level (level 3). |

Log format: `[LEVEL] YYYY-MM-DD HH:MM:SS [name] message`

### `std/math`

```taro
import "std/math";

// Constants
print(math.PI);                  // 3.141592653589793
print(math.E);                   // 2.718281828459045
print(math.TAU);                 // 6.283185307179586

// Trig
print(math.sin(math.PI / 2));    // 1
print(math.cos(0));              // 1
print(math.tan(math.PI / 4));    // ~1
print(math.asin(0));             // 0
print(math.atan2(1, 1));         // π/4

// Power / log
print(math.sqrt(16));            // 4
print(math.pow(2, 10));          // 1024
print(math.ln(math.E));          // 1
print(math.log2(8));             // 3
print(math.log10(100));          // 2
print(math.hypot(3, 4));         // 5

// Rounding
print(math.floor(3.7));          // 3
print(math.ceil(3.1));           // 4
print(math.round(3.5));          // 4

// Angle conversion
print(math.degrees(math.PI));    // 180
print(math.radians(180));        // π
```

All functions accept both int and float arguments, returning float.

### `std/fs`

```taro
import "std/fs";

// Standalone convenience functions
fs.write("/tmp/hello.txt", "Hello from Taro!");
print(fs.read("/tmp/hello.txt"));
print(fs.read_bytes("/tmp/hello.txt")); // b"Hello from Taro!"
print(fs.exists("/tmp/hello.txt"));     // true
print(fs.is_file("/tmp/hello.txt"));    // true
print(fs.is_dir("/tmp"));               // true
fs.rename("/tmp/hello.txt", "/tmp/hi.txt");
fs.remove("/tmp/hi.txt");

fs.mkdir("/tmp/taro_demo");
fs.list_dir("/tmp");                    // list of entry names

// File class — open, read/write line-by-line, seek
var f = fs.File("/tmp/demo.txt", "w");
f.write("line1\nline2\nline3");
f.close();

var g = fs.File("/tmp/demo.txt", "r");
print(g.readline());                    // line1
print(g.tell());                        // 6
g.seek(0);
print(g.read());                        // all content
g.seek(0);
print(g.read_bytes());                  // all content as bytes
g.close();

print(str(g));                          // <File path='...' mode='r' status=closed>
```

File modes: `"r"` (read), `"w"` (write/create), `"a"` (append).

### `std/random`

```taro
import "std/random";

// Random float in [0, 1)
print(random.random());

// Random integer in [min, max] inclusive
print(random.randint(1, 6));          // e.g. 4

// Random float in [min, max)
print(random.uniform(0.0, 10.0));     // e.g. 7.234

// Random element from a list
var colors = ["red", "green", "blue"];
print(random.choice(colors));         // e.g. "blue"

// Shuffle a list in place
random.shuffle(colors);
print(colors);                        // e.g. ["blue", "red", "green"]
```

### `std/net`

TCP networking with `Socket` (client) and `Server` (listener) classes.

```taro
import "std/net";

// ---- TCP client ----
var sock = net.Socket();
sock.connect("httpbin.org", 80);
sock.send("GET /get HTTP/1.0\r\nHost: httpbin.org\r\n\r\n");
var data = sock.recv(4096);
print(data);
sock.close();

// ---- TCP server ----
var server = net.Server();
server.bind(8080);                    // bind to 0.0.0.0:8080
// server.bind("127.0.0.1", 8080);   // or specific host
// server.bind("0.0.0.0:8080");      // or single string

var client = server.accept();         // wait for a connection
var msg = client.recv(1024);          // read up to 1024 bytes
client.send("echo: " + msg);          // send reply
client.close();
server.close();
```

**Socket** methods:

| Method | Description |
|--------|-------------|
| `sock.connect(host, port)` | Connect to a remote address. Also `sock.connect("host:port")`. |
| `sock.send(data)` | Send a string. |
| `sock.recv(bufsize)` | Receive up to `bufsize` bytes, return as string. |
| `sock.close()` | Close the socket. |
| `sock.settimeout(seconds)` | Set read timeout (float). |

**Server** methods:

| Method | Description |
|--------|-------------|
| `server.bind(port)` | Bind to `0.0.0.0:port`. Also `bind(host, port)` or `bind("host:port")`. |
| `server.accept()` | Accept a connection, return a `Socket` instance. |
| `server.close()` | Close the listener. |

### `std/os`

Operating system interaction — environment variables, process info, working directory, and shell commands.

```taro
import "std/os";

// Environment variables
print(os.getenv("HOME"));             // "/home/user" (or nil if not set)
os.setenv("MY_VAR", "hello");
print(os.getenv("MY_VAR"));           // "hello"

// All env vars as a dict
var all = os.env();
print(all["PATH"]);                   // system PATH

// Process info
print(os.pid());                      // e.g. 12345
print(os.args());                     // command-line arguments as list

// Working directory
var here = os.cwd();
os.chdir("/tmp");                     // change directory
print(os.cwd());                      // "/tmp"
os.chdir(here);                       // restore

// Temp directory
print(os.tmpdir());                   // e.g. "/tmp"

// Run a shell command
var code = os.system("echo hello");   // prints "hello", returns exit code
print(code);                          // 0
```

| Function | Description |
|----------|-------------|
| `os.args()` | List of command-line arguments. |
| `os.getenv(name)` | Value of environment variable, or `nil`. |
| `os.setenv(key, value)` | Set an environment variable. |
| `os.env()` | Dict of all environment variables. |
| `os.cwd()` | Current working directory path. |
| `os.chdir(path)` | Change working directory. |
| `os.pid()` | Current process ID. |
| `os.tmpdir()` | System temporary directory path. |
| `os.system(cmd)` | Run a shell command, return exit code. |

### `std/time`

Time functions — Unix timestamp, sleep, and structured UTC time.

```taro
import "std/time";

// Unix timestamp
print(time.time());                   // e.g. 1781706916.163

// Sleep (seconds, fractional ok)
time.sleep(0.5);                      // pause 500 ms

// Structured UTC time
var n = time.now();
print(n.year);                        // 2026
print(n.month);                       // 6
print(n.day);                         // 17
print(n.hour);                        // 14
print(n.min);                         // 35
print(n.sec);                         // 16.163
print(n.wday);                        // 3 (= Wednesday, 0=Sun)
print(n.yday);                        // 168 (day of year)
print(n.timestamp);                   // same as time.time()
```

| Function | Description |
|----------|-------------|
| `time.time()` | Current Unix timestamp in seconds (float). |
| `time.sleep(secs)` | Pause execution for `secs` seconds (may be fractional). |
| `time.now()` | Current UTC time as a structured object with fields: `year`, `month`, `day`, `hour`, `min`, `sec`, `wday` (0=Sun), `yday` (1–366), `timestamp`. |

All time fields are in UTC.

### `std/ffi`

Foreign Function Interface — load dynamic libraries, call C functions, and define C-compatible
struct types. Backed by `libffi` for ABI-level calling and `libloading` for `dlopen`/`dlsym`.

```taro
import "std/ffi";

// ---- Load libraries and bind functions ----
var libm = ffi.CDynLib("libm.so.6");

// bind caches the type info — subsequent calls are fast (no string parsing)
var cos = libm.bind("cos", "double", ["double"]);
print(cos(0.0));                        // 1.0
print(cos(3.141592653589793));          // ~-1.0

// libc examples
var libc = ffi.CDynLib("libc.so.6");
var abs = libc.bind("abs", "int32", ["int32"]);
print(abs(-42));                        // 42

// void return functions
var srand = libc.bind("srand", "void", ["uint32"]);
print(srand(12345));                    // nil

// ---- Low-level one-shot call (no pre-binding) ----
var sym = libm.symbol("cos");
var val = ffi.call(sym, "double", ["double"], [0.0]);
print(val);                             // 1.0

// ---- Scalar CType singletons (for use as type arguments) ----
// ffi.c_int8, c_uint8, c_int16, c_uint16, c_int32, c_uint32,
// ffi.c_int64, c_uint64, c_float, c_double, c_bool, c_pointer, c_cstring

// ---- Struct definition (positional fields) ----
var Color = ffi.define_struct(["uint8", "uint8", "uint8", "uint8"]);
var c = Color(255, 128, 64, 255);
print(c[0]);                            // 255 (positional access)

// ---- Struct definition (named fields) ----
var Vec3 = ffi.define_struct([
    ["x", "float"],
    ["y", "float"],
    ["z", "float"],
]);
var v = Vec3(1.0, 2.0, 3.0);
print(v.x);                             // 1.0 (named field access)
v.y = 5.0;                              // field mutation

// ---- Nested structs via CType singletons ----
var Vector3 = ffi.define_struct([
    ["x", ffi.c_float],
    ["y", ffi.c_float],
    ["z", ffi.c_float],
]);
var Camera3D = ffi.define_struct([
    ["position", Vector3],
    ["target", Vector3],
    ["up", Vector3],
    ["fovy", ffi.c_float],
    ["projection", ffi.c_int32],
]);

// ---- Scalar value wrapping ----
var n = ffi.c_int32(42);                // wraps 42 as a typed C int32
print(n.value);                         // 42
n.value = 99;                           // mutation
```

**Classes:**

| Class | Description |
|-------|-------------|
| `CDynLib(path)` | Load a dynamic library (`.so` / `.dylib` / `.dll`). Methods: `symbol(name)`, `bind(name, ret, params)`. |
| `CSymbol` | Opaque symbol pointer — returned by `lib.symbol()`, passed to `ffi.call()`. |
| `CFunction` | Bound C function — returned by `lib.bind()`. Callable directly with Taro values. |
| `CType` | Type descriptor for a C type. Scalar singletons (e.g. `ffi.c_float`) are pre-built `CType` instances; `ffi.define_struct(...)` creates struct `CType` instances. Both are callable: scalar CType converts a value (returns a scalar wrapper), struct CType constructs a `CStruct` instance. |
| `CStruct` | Concrete struct instance with named field access (`s.field`) and mutation (`s.field = v`). |

**`CDynLib` methods:**

| Method | Description |
|--------|-------------|
| `lib.symbol(name)` | Look up a symbol by name, return a `CSymbol`. |
| `lib.bind(name, ret_type, param_types)` | Look up a symbol and bind it as a `CFunction`. `ret_type` and `param_types` accept both string names (`"int32"`, `"double"`, …) and `CType` instances (for struct return/param types). |

**`CFunction` call semantics:**

| Feature | Description |
|---------|-------------|
| Argument marshalling | Taro integers/floats are converted to the declared C types. Strings become `CString` (`*const c_char`). Struct instances are serialized to raw byte buffers. |
| Return marshalling | Scalar returns become Taro integers/floats. `void` returns `nil`. `CString` return is converted to a Taro string. Struct return re-reads from the raw buffer into a `CStruct` instance. |
| Struct return | When `ret_type` is a struct `CType` instance, the result is returned as a `CStruct` with named fields. |
| Error handling | Argument count mismatch, type errors, and FFI-level errors produce runtime errors with descriptive messages. |

**Functions:**

| Function | Description |
|----------|-------------|
| `ffi.call(symbol, ret_type, param_types[, args])` | One-shot FFI call. `symbol` is a `CSymbol`, `ret_type` and `param_types` are type descriptors (strings or `CType` instances), `args` is an optional list of argument values. |
| `ffi.define_struct(descriptors)` | Define a C struct type. `descriptors` is a list of either type strings (positional fields) or `[name, type]` pairs (named fields). Returns a callable `CType` instance that constructs `CStruct` instances. |

**Scalar wrapper types:**

Each scalar has a wrapper class (e.g. `CI8`, `CUint8`, `CFloat`, `CBool`, `CPointer`)
that stores a single native C value. Scalar wrappers support `.value` get/set for reading
and mutating the wrapped value. When accessed as struct fields via `CStruct.__getattr__`,
scalar wrappers are auto-unwrapped to plain Taro values.

**Supported C types:**

| Type string | C type | Size | Wrapper class |
|-------------|--------|------|---------------|
| `"int8"` | `i8` | 1 | `CI8` |
| `"uint8"` | `u8` | 1 | `CUint8` |
| `"int16"` | `i16` | 2 | `CI16` |
| `"uint16"` | `u16` | 2 | `CUint16` |
| `"int32"` | `i32` | 4 | `CI32` |
| `"uint32"` | `u32` | 4 | `CUint32` |
| `"int64"` | `i64` | 8 | `CI64` |
| `"uint64"` | `u64` | 8 | `CUint64` |
| `"float"` | `f32` | 4 | `CFloat` |
| `"double"` | `f64` | 8 | `CDouble` |
| `"bool"` | `u8` | 1 | `CBool` |
| `"pointer"` | `*const c_void` | 8 | `CPointer` |
| `"cstring"` | `*const c_char` | 8 | — |
| `"void"` | — | — | — (return only) |

### `std/json`

JSON encoding (serialization) and decoding (parsing), backed by `serde_json`.

```taro
import "std/json";

// ---- encode: Taro value → JSON string ----
json.encode(nil);                     // "null"
json.encode(42);                      // "42"
json.encode(true);                    // "true"
json.encode("hello");                 // "\"hello\""
json.encode([1, 2, 3]);               // "[1,2,3]"

var d = dict();
d["name"] = "taro";
d["version"] = 1;
json.encode(d);                       // "{\"name\":\"taro\",\"version\":1}"

// ---- decode: JSON string → Taro value ----
json.decode("null");                  // nil
json.decode("42");                    // 42 (Int)
json.decode("3.14");                  // 3.14 (Float)
json.decode("[1, 2, 3]");             // [1, 2, 3] (List)
json.decode("{\"x\": 10}");           // {"x": 10} (Dict)

// ---- roundtrip ----
var original = {"a": 1, "b": [2, 3]};
var encoded = json.encode(original);
var decoded = json.decode(encoded);
print(decoded["a"]);                  // 1
print(decoded["b"][1]);               // 3
```

| Function | Description |
|----------|-------------|
| `json.encode(value)` | Serialize a Taro value to a JSON string. Supports nil, bool, int, float, string, list, dict, and set. Errors on non-serializable types (functions, classes, native objects). |
| `json.decode(string)` | Parse a JSON string into a Taro value. JSON null→nil, bool→Bool, number→Int/Float, string→String, array→List, object→Dict. |

**Type mapping:**

| Taro → JSON | JSON → Taro |
|---|---|
| `nil` → `null` | `null` → `nil` |
| `Bool` → `true`/`false` | `true`/`false` → `Bool` |
| `Int` → number | number → `Int` (if integral) or `Float` |
| `Float` → number | — |
| `String` → string | string → `String` |
| `List` → `[...]` | `[...]` → `List` |
| `Dict` → `{...}` (string keys) | `{...}` → `Dict` |
| `Set` → `[...]` (as array) | — |

NaN and Infinity are encoded as `null` (JSON does not support them). Dict keys must be strings; non-string keys cause an error.

### `std/itertools`

Pure-taro lazy iterator library. All lazy iterators implement the iteration protocol
(`__iter__` / `__next__`) and work directly in `for`-`in` loops. The library is implemented
in taro itself (`src/std/itertools.taro`), compiled and executed at import time.

```taro
import "std/itertools" as it;

// ---- Lazy iterators (work in for-in, compose with each other) ----
it.map(fn, source)            // apply fn to each element
it.filter(fn, source)         // keep elements where fn(elem) is truthy
it.enumerate(source, start)   // yield [index, value] pairs
it.zip(left, right)           // pair elements from two iterables; stops at shorter
it.chain(first, second)       // exhaust first, then second
it.take(n, source)            // yield at most n elements
it.drop(n, source)            // skip the first n elements
it.flatten(source)            // yield elements from each inner iterable
it.cycle(source)              // repeat an iterable endlessly (use with `break`)
it.count(start, step)         // infinite arithmetic progression (use with `break`)
it.repeat(value, n)           // repeat value n times (n < 0 = infinite)

// ---- Eager consumers ----
it.collect(iterable)          // collect all elements into a list
it.reduce(fn, iterable, init) // left fold
it.all(iterable)              // true if every element is truthy (empty → true)
it.any(iterable)              // true if any element is truthy (empty → false)
it.find(iterable, fn)         // first element where fn(elem) is truthy, or nil
it.nth(iterable, n)           // nth element (0-based), or nil if out of range
it.sorted(iterable, key)      // new sorted list (insertion sort, stable);
                              // pass `nil` for key to compare elements directly
```

| Lazy iterator | Description |
|---------------|-------------|
| `map(fn, source)` | Apply `fn` to each element. |
| `filter(fn, source)` | Keep elements where `fn(elem)` is truthy. |
| `enumerate(source, start)` | Yield `[index, value]` pairs. |
| `zip(left, right)` | Pair elements from two iterables; stops at the shorter one. |
| `chain(first, second)` | Exhaust `first`, then `second`. |
| `take(n, source)` | Yield at most `n` elements. |
| `drop(n, source)` | Skip the first `n` elements. |
| `flatten(source)` | Yield elements from each inner iterable in turn (one level). |
| `cycle(source)` | Repeat an iterable endlessly (empty source yields nothing). |
| `count(start, step)` | Infinite arithmetic progression `start`, `start+step`, … |
| `repeat(value, n)` | Repeat `value` `n` times (`n < 0` for infinite). |

| Eager consumer | Description |
|----------------|-------------|
| `collect(iterable)` | Collect all elements into a list. |
| `reduce(fn, iterable, init)` | Left fold: `fn(acc, elem)` for each element. |
| `all(iterable)` | `true` if every element is truthy (empty → `true`). |
| `any(iterable)` | `true` if any element is truthy (empty → `false`). |
| `find(iterable, fn)` | First element where `fn(elem)` is truthy, or `nil`. |
| `nth(iterable, n)` | Nth element (0-based), or `nil` if out of range. |
| `sorted(iterable, key)` | Return a new sorted list. `key` is an optional extraction function (`nil` for identity). Uses stable insertion sort. |

**Examples:**

```taro
import "std/itertools" as it;

// Map and filter
var squares = it.map(fun(x) { return x * x; }, [1, 2, 3]);
print(it.collect(squares));                    // [1, 4, 9]

// Composition pipeline
var pipeline = it.take(2, it.map(fun(x) { return x * x; },
                      it.filter(fun(x) { return x % 2 == 0; }, [1..6])));
print(it.collect(pipeline));                   // [4, 16]

// Zip and enumerate
for pair in it.zip(["a", "b"], [1, 2]) {
    print(pair);                               // ["a", 1], ["b", 2]
}

// Sorting
print(it.sorted([3, 1, 4, 1, 5, 9], nil));    // [1, 1, 3, 4, 5, 9]
print(it.sorted(["xyz", "a", "bc"], len));     // ["a", "bc", "xyz"]

// Infinite + break
var i = 0;
for x in it.count(10, 3) {
    print(x);                                  // 10, 13, 16, 19, 22
    i = i + 1;
    if (i >= 5) { break; }
}
```

**Design note:** `item == IterEnd` does **not** work in taro because `IterEnd` is not
an `Instance` and `__eq__` magic dispatch requires both operands to be Instances.
Use the `is_iter_end(value)` builtin instead when writing custom iterators.
