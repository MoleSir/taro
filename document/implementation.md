# Implementation

- Compiler: single-pass recursive-descent parser emitting bytecode for a stack-based VM.
- VM: direct threaded interpretation with `CallFrame` stack.
- GC: mark-and-sweep with gray-stack tracing.
- Objects: heap-allocated with handle-based access (`ObjectHandle`).
- Strings: interned via `ShrString` (shared, cheap clone / hash / equality).

## Project structure

```
src/
├── base/
│   ├── chunk.rs              # Bytecode chunk (write/read instructions)
│   ├── instruct.rs           # ByteCode & Instruction enums
│   ├── string.rs             # ShrString — interned, copy-on-write string type
│   └── object/
│       ├── mod.rs            # Object enum & type-checking helpers
│       ├── heap.rs           # ObjectHeap — allocation, GC mark/sweep, interning caches
│       ├── variants.rs       # Object variants (Function, Class, Instance, Closure, …)
│       └── instance/
│           ├── mod.rs        # ObjectInstanceData enum (Bool, Int, Float, String, List, Dict, Set, Bytes, …)
│           ├── list.rs       # List builtin methods
│           ├── dict.rs       # Dict builtin methods
│           ├── set.rs        # Set builtin methods
│           ├── string.rs     # String builtin methods
│           └── bytes.rs      # Bytes builtin methods
├── compile/
│   ├── mod.rs                # Compiler entry point
│   ├── parse.rs              # Parser — Pratt parsing, statement/expression compilation
│   ├── scan.rs               # Lexer / scanner
│   └── token.rs              # Token & TokenKind
└── vm/
    ├── mod.rs                # VirtualMachine — execution loop, call frames, stack ops,
    │                         #   import_module, call_native_fn dispatch (NativeFunction enum)
    ├── error.rs              # ExecuteError & InterpretError types (thiserror-based)
    ├── gc.rs                 # GC threshold & trigger
    ├── utils.rs              # VM utility helpers (get_args, value_type_name)
    ├── magic.rs              # Magic method bindings (__add__, __getitem__, …)
    ├── builtin.rs            # Global built-in functions & class registration
    │                         #   (print, str, len, type, abs, min, max, input, clock,
    │                         #    is_iter_end, int, float, bool, list, dict, set, bytes, exit)
    ├── std/                  # Virtual std modules (no .taro file needed)
    │   ├── mod.rs            #   import_std_module dispatcher
    │   ├── argparse.taro     #   std/argparse — CLI argument parser (pure taro)
    │   ├── fs.rs             #   std/fs — File class (read/write/read_bytes/readline/seek/tell/close) + standalone functions
    │   ├── itertools.taro    #   std/itertools — lazy iterators (pure taro)
    │   ├── json.rs           #   std/json — JSON encode/decode via serde_json
    │   ├── logging.taro      #   std/logging — leveled logging with timestamps (pure taro)
    │   ├── math.rs           #   std/math — trig, log, rounding, conversion + constants
    │   ├── net.rs            #   std/net — TCP Socket client + Server listener
    │   ├── os.rs             #   std/os — env vars, pid, cwd, shell commands
    │   ├── random.rs         #   std/random — random numbers, randint, choice, shuffle
    │   └── time.rs           #   std/time — timestamp, sleep, structured UTC time
    └── tests.rs              # VM runtime tests (411 tests)

tests/scripts/                # Integration test scripts
├── 00_test.taro             # Smoke test
├── 01–09_*.taro             # Language features (literals, arithmetic, control flow, closures, …)
├── 10_class.taro            # Class & instance
├── 11_magic.taro            # Magic methods (arithmetic, comparison, conversion)
├── 12_builin.taro           # Builtin functions
├── 13_super.taro            # Inheritance & super calls
├── 14_list.taro             # List operations
├── 15_dict.taro             # Dict operations
├── 16_builtin_methods.taro  # List/Dict builtin methods
├── 17_regression.taro       # Regression tests
├── 18_call_magic.taro       # __call__ magic method
├── 19_import.taro           # File-based module import
├── 20_std_fs.taro           # std/fs integration tests
├── 21_std_math.taro         # std/math integration tests
├── 22_std_random.taro       # std/random integration tests
├── 23_std_os.taro           # std/os integration tests
├── 24_std_time.taro         # std/time integration tests
├── 25_std_json.taro         # std/json integration tests
├── 26_std_itertools.taro    # std/itertools integration tests
├── 27_std_argparse.taro     # std/argparse integration tests
└── 28_std_logging.taro      # std/logging integration tests
tests/scripts/lib/           # File-based modules used by import tests
└── math.taro                #   Sample module (add, mul, PI, Vec class)
examples/                    # Runnable example scripts
├── echo_server.taro         #   TCP echo server using std/net
├── echo_client.taro         #   TCP echo client using std/net
└── web.taro                 #   HTTP web server using std/net
```

## Key design decisions

### Native function dispatch

Native functions (builtins, methods, stdlib) use a `NativeFunction` enum with arity variants:

```rust
pub enum NativeFunction {
    Arity0(NativeFn0),    // fn(&mut VirtualMachine) -> ExecuteResult<ObjectHandle>
    Arity1(NativeFn1),    // fn(&mut VirtualMachine, ObjectHandle) -> ExecuteResult<ObjectHandle>
    Arity2(NativeFn2),    // fn(&mut VirtualMachine, ObjectHandle, ObjectHandle) -> ...
    Arity3(NativeFn3),
    Variadic(NativeFnN),  // fn(&mut VirtualMachine, &[ObjectHandle]) -> ...
}
```

The VM dispatch (`call_native_fn`) validates argument counts and extracts typed arguments before calling the function — individual native functions never deal with raw stack indices.

### Object model

- `ObjectHandle(usize)` — cheap Copy handle, index into `ObjectHeap.objects`.
- Slot 0 is the nil sentinel; `ObjectHandle::NIL` never allocates.
- `ObjectInstanceData` carries type-specific data (Bool, Integer, Float, String, List, Dict, Set, Bytes, Fields, Native).
- `NativeObject` stores type-erased Rust data (`Box<T>` behind a raw pointer) — used by `FileData` in fs module.
- Small integers (-5..256) and strings are interned via caches on `ObjectHeap`.

### Functions, default parameters & keyword arguments

`ObjectFunction` stores function metadata alongside the bytecode chunk:

```rust
pub struct ObjectFunction {
    pub arity: usize,               // total parameter count (incl. those with defaults)
    pub required_arity: usize,      // parameter count without defaults
    pub param_names: Vec<ShrString>, // names of all parameters in declaration order
    pub defaults: Vec<ObjectHandle>, // default values for the last N parameters
    pub chunk: Chunk,
    pub name: ShrString,
}
```

Default parameter values must be constant literals (numbers, strings, booleans, nil).
They are stored as `ObjectHandle` constants in the function object.  Parameters with
defaults must come after required parameters (a `RequiredAfterOptional` parse error is
emitted otherwise).

Keyword arguments use a dedicated instruction:

```rust
CallKw { pos_count: usize, kw_count: usize, kw_names: Vec<ShrString> }
```

At runtime the VM matches keyword names to parameter names, validates for duplicates and
unknown names, fills defaults for missing trailing arguments, and reorders the stack into
parameter-declaration order.  Positional arguments must precede keyword arguments.
Class constructors receive special treatment: the `self` parameter (slot 0) is excluded
from user-facing keyword matching.

### String interning

`ShrString` is an `Arc<str>` wrapper that makes string copies O(1) and equality/hash O(1). The `ObjectHeap` maintains a `string_cache` so identical strings share the same handle.

### GC

Mark-and-sweep, triggered when `bytes_allocated` exceeds `gc_threshold` (1 MiB). Mark roots: VM stack, call frames, globals, open upvalues, extra_gc_roots, interning caches. Sweep removes unmarked objects and recycles slots into `free_slots`.

### Error handling

`ExecuteError` (35+ variants) uses `thiserror::Error` with `#[error("...")]` for Display. A `context_error` derive from `thiserrorctx` adds context-tracking for better error messages. `InterpretError` wraps compile + runtime errors.

### String methods

The `String` class implements 15+ built-in methods (in `src/object/instance/string.rs`):

| Category | Methods |
|----------|---------|
| Case | `upper()`, `lower()` (aliases: `to_uppercase()`, `to_lowercase()`) |
| Whitespace | `trim()`, `trim_start()`, `trim_end()` (aliases: `strip()`, `lstrip()`, `rstrip()`) |
| Search | `find(sub)`, `rfind(sub)` (aliases: `index_of()`, `last_index_of()`) |
| Predicate | `starts_with(prefix)`, `ends_with(suffix)`, `contains(sub)`, `is_empty()` |
| Transform | `replace(old, new)`, `split(delim)`, `substr(start, length)`, `repeat(n)` |

All search methods (`find`, `rfind`) return -1 when the substring is not found.
`substr` supports negative start indices (count from end).

### Bytes methods

The `Bytes` class implements built-in methods (in `src/object/instance/bytes.rs`):

| Category | Methods |
|----------|---------|
| Conversion | `hex()`, `decode(encoding)`, `to_list()` |
| Creation | `from_string(s)`, `from_list(lst)` |
| Introspection | `len()` |

Bytes are stored as `Vec<u8>` and indexed by integer position (returns byte value as `i64`).
Iteration yields integer byte values one at a time via `ObjectBytesIterator`.
Bytes support hashing, making them usable as dict keys and set elements.
