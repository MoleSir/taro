# Taro

A dynamically-typed scripting language with a bytecode VM, written in Rust. Inspired by [Crafting Interpreters](https://craftinginterpreters.com/).

```taro
var prompt = "Hello World!";
print(prompt);
```



## Quick start

```bash
cargo run -- tests/scripts/10_class.taro
```

Run all tests:

```bash
cargo test
```



## [Language overview](./document/overview.md)



## [Implementation](./document/implementation.md)



## References

- [Crafting Interpreters](https://craftinginterpreters.com/a-bytecode-virtual-machine.html) — the book that inspired this project.
