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
│       └── variants.rs       # Object variants (Function, Class, Instance, Closure, …)
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
    ├── builtin/              # Builtin method & standalone function implementations
    │   ├── mod.rs            #   Registration dispatcher (register_builtins / register_builtins_class_method)
    │   ├── bool.rs           #   Bool methods (__neg__, __add__, __sub__, __eq__, __str__, …)
    │   ├── int.rs            #   Int arithmetic & comparison macros
    │   ├── float.rs          #   Float arithmetic & comparison macros
    │   ├── string.rs         #   String methods (__add__, __getitem__, __len__, __eq__, …)
    │   ├── list.rs           #   List methods (append, pop, extend, __getitem__, __setitem__, __len__, …)
    │   ├── dict.rs           #   Dict methods (get, pop, keys, values, __getitem__, __setitem__, __len__, …)
    │   └── function.rs       #   Global functions (print, str, len, type, min, max, abs, input, clock)
    ├── stdlib/               # Virtual std modules (no .taro file needed)
    │   ├── mod.rs            #   import_std_module dispatcher
    │   ├── fs.rs             #   std/fs — File class + standalone fs functions
    │   ├── math.rs           #   std/math — trig, log, rounding, conversion functions + constants
    |   └── random.rs         #   std/random
    └── tests.rs              # VM runtime tests (267 tests)

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
└── 21_std_math.taro         # std/math integration tests
tests/scripts/lib/           # File-based modules used by import tests
└── math.taro                #   Sample module (add, mul, PI, Vec class)
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
- `ObjectInstanceData` carries type-specific data (Bool, Integer, Float, String, List, Dict, Fields, Native).
- `NativeObject` stores type-erased Rust data (`Box<T>` behind a raw pointer) — used by `FileData` in fs module.
- Small integers (-5..256) and strings are interned via caches on `ObjectHeap`.

### String interning

`ShrString` is an `Arc<str>` wrapper that makes string copies O(1) and equality/hash O(1). The `ObjectHeap` maintains a `string_cache` so identical strings share the same handle.

### GC

Mark-and-sweep, triggered when `bytes_allocated` exceeds `gc_threshold` (1 MiB). Mark roots: VM stack, call frames, globals, open upvalues, extra_gc_roots, interning caches. Sweep removes unmarked objects and recycles slots into `free_slots`.

### Error handling

`ExecuteError` (27 variants) uses `thiserror::Error` with `#[error("...")]` for Display. A `context_error` derive from `thiserrorctx` adds context-tracking for better error messages. `InterpretError` wraps compile + runtime errors.
