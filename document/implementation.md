# Implementation

- Compiler: single-pass recursive-descent parser emitting bytecode for a stack-based VM.
- VM: direct threaded interpretation with `CallFrame` stack.
- GC: mark-and-sweep with gray-stack tracing.
- Objects: heap-allocated with handle-based access (`ObjectHandle`).

## Project structure

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
    ├── builtin_methods.rs # Builtin methods for List/Dict (append, pop, get, keys, …)
    ├── magic.rs          # Magic method implementations (__add__, __eq__, __str__, …)
    ├── error.rs          # Error types
    ├── gc.rs             # GC threshold & trigger
    └── tests.rs          # VM runtime tests

tests/scripts/
├── 10_class.taro         # Class & instance integration tests
├── 11_magic.taro         # Magic method integration tests
├── 12_builin.taro        # Builtin function integration tests
├── 13_super.taro         # Super method call integration tests
├── 14_list.taro          # List integration tests
├── 15_dict.taro          # Dict integration tests
└── 16_builtin_methods.taro  # Builtin methods (list.append, dict.get, …)
```