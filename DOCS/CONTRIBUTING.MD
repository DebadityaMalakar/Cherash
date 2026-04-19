# Contributing to Cherash

This document describes how the Cherash codebase is developed — testing methodology, commit discipline, coding standards, and how to add new features.

---

## Development Philosophy

1. **Tests first.** Write the test that validates the exit criterion before writing the implementation.
2. **One sub-task at a time.** Implement, test, commit. Then move to the next sub-task.
3. **Loud failures over silent bugs.** Produce a detailed error with location. Never swallow errors silently.
4. **No premature optimization.** Tree-walking interpreter is the correct target for the MVP. Bytecode VM and JIT are post-Phase-8.
5. **Never skip phases.** Phase N depends on Phase N-1 being complete and tested.

---

## Project Structure

```
cherash/
├── src/
│   ├── lib.rs              top-level run_source() API
│   ├── main.rs             CLI entry point
│   ├── lexer/              Phase 1
│   ├── parser/             Phase 2
│   ├── typechecker/        Phase 4
│   ├── interpreter/        Phase 3 (evaluator, methods, environment)
│   ├── runtime/            Phases 7–8 (value, gc)
│   └── stdlib/             Phase 8 (math, io, json, sys, os, time, random, threading)
├── tests/
│   ├── phase1_lexer.rs
│   ├── phase3_evaluator.rs
│   ├── phase7_threading.rs
│   ├── phase8_math.rs
│   └── programs/           Integration test programs (.py, .chsh)
├── DOCS/
│   ├── LANGUAGE_GUIDE.MD
│   ├── ARCHITECTURE.MD
│   ├── TYPE_SYSTEM.MD
│   ├── STDLIB.MD
│   ├── THREADING.MD
│   ├── GC.MD
│   └── CONTRIBUTING.MD
├── Cargo.toml
├── MVP.MD                  full language specification
├── ROADMAP.MD              feature roadmap and sprint backlog
└── CLAUDE.MD               AI implementation instructions
```

---

## Phase Status

| Phase | Name | Status |
|-------|------|--------|
| 1 | Lexer | ✅ Complete |
| 2 | Parser & AST | ✅ Complete |
| 3 | Core Evaluator | ✅ Complete |
| 4 | Type Checker | ✅ Complete |
| 5 | Collections & OOP | ✅ Complete |
| 6 | Exception Machinery | ✅ Complete |
| 7 | Threading | ✅ Complete |
| 8 | GC | ✅ Complete |
| 8 | Stdlib | ✅ Complete (math, io, json, os, sys, time, random, threading) |
| 8 | Raylib | 🔲 Planned |

---

## Building

```bash
cargo build            # debug build
cargo build --release  # optimized build
cargo test             # run all tests
cargo test phase1      # run only phase1 tests
cargo clippy           # lint
```

No external dependencies are required for Phases 1–8 stdlib. Raylib will require `raylib-sys` (C library).

---

## Writing Tests

Tests live in `tests/` as integration tests calling into the library via `run_source()`.

```rust
use cherash::run_source;

#[test]
fn test_fibonacci() {
    run_source(r#"
def fib(n):
    if n <= 1: return n
    return fib(n-1) + fib(n-2)

assert fib(10) == 55
"#, false).unwrap();
}
```

### Conventions

- Use `r#"..."#` raw string literals to avoid escaping.
- Use `assert` inside the Cherash program — clear failure messages.
- `run_source(src, true)` for strict mode (`.chsh`) tests.
- `run_source(src, false)` for compatibility mode (`.py`) tests.

### Testing type errors

```rust
fn run_expect_type_error(src: &str) {
    match run_source(src, true) {
        Err(cherash::CherashError::TypeCheck(_)) => {}
        Ok(()) => panic!("expected TypeError but program succeeded"),
        Err(e) => panic!("unexpected error: {}", e),
    }
}

#[test]
fn test_int_str_mismatch() {
    run_expect_type_error(r#"x: int = "hello""#);
}
```

### Testing runtime errors

```rust
fn run_expect_runtime_error(src: &str, expected_type: &str) {
    match run_source(src, false) {
        Err(cherash::CherashError::Runtime(e)) => {
            assert_eq!(e.type_name, expected_type,
                "expected '{}' but got '{}: {}'", expected_type, e.type_name, e.message);
        }
        Ok(()) => panic!("expected '{}' but program succeeded", expected_type),
        Err(e) => panic!("unexpected error: {}", e),
    }
}
```

---

## Adding a Builtin Function

Built-in functions live in `src/interpreter/evaluator.rs` in `load_builtins()`:

```rust
fn load_builtins(env: &EnvRef) {
    let builtins: &[(&str, BuiltinFnPtr)] = &[
        ("mybuiltin", builtin_mybuiltin),
        // ...
    ];
    for (name, f) in builtins {
        Env::set_local(env, name, Value::BuiltinFunction(Arc::new(BuiltinFn { name, func: *f })));
    }
}

fn builtin_mybuiltin(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let n = match args.first() {
        Some(Value::Int(n)) => *n,
        _ => return Err(RuntimeError::type_error("mybuiltin() requires an int")),
    };
    Ok(Value::Int(n * 2))
}
```

---

## Adding a Collection Method

Collection methods live in `src/interpreter/methods.rs`. They receive `self` as `args[0]` via the `BoundBuiltin` dispatch:

```rust
pub fn list_mymethod(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("missing self"))?;
    let list = match self_val {
        Value::List(l) => l,
        _ => return Err(RuntimeError::type_error("expected list")),
    };
    // ...
}
```

Register it in `list_method()` in `evaluator.rs`:

```rust
"mymethod" => ok(Value::BoundBuiltin {
    receiver: Box::new(obj.clone()),
    name: "mymethod",
    func: crate::interpreter::methods::list_mymethod,
}),
```

---

## Adding a Stdlib Module

1. Create `src/stdlib/mymodule.rs`:

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::runtime::value::*;
use crate::interpreter::evaluator::RuntimeError;

pub fn make_mymodule_module() -> Value {
    let mut attrs = HashMap::new();
    attrs.insert("myfunction".into(), Value::BuiltinFunction(Arc::new(BuiltinFn {
        name: "myfunction",
        func: |args| {
            // implementation
            Ok(Value::None)
        },
    })));
    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "mymodule".into(), attrs })))
}
```

2. Export from `src/stdlib/mod.rs`:
```rust
pub mod mymodule;
```

3. Register in `import_module()` in `evaluator.rs`:
```rust
"mymodule" => ok(crate::stdlib::mymodule::make_mymodule_module()),
```

4. Write tests in `tests/`:
```rust
#[test]
fn test_mymodule() {
    run_source(r#"
import mymodule
assert mymodule.myfunction() is None
"#, false).unwrap();
}
```

---

## Coding Standards

### Rust style

- Follow standard Rust idioms (`cargo clippy` should be clean).
- Use `GcHandle<T>` (via `alloc_list`, `alloc_dict`, `alloc_set`, `alloc_instance`) for all mutable heap values — never raw `Arc<Mutex<T>>` for `List`/`Dict`/`Set`/`Instance`.
- Never use `unsafe` for sharing mutable state across threads.
- Prefer `match` with exhaustive patterns over `if let` chains.
- Use `?` for error propagation in `Result`-returning functions.
- Use `Signal` for control flow (return, break, continue, raise) — not panics.

### Error messages

Every error must include:
- The error type name (`TypeError`, `ValueError`, etc.)
- A specific message describing what went wrong
- Source location (line and column) when available

Avoid: `"operation failed"`, `"invalid value"`.
Prefer: `"expected int, got str at 5:3"`, `"list index 10 out of range (len=5)"`.

### No silent fallbacks

If a builtin receives wrong argument types, raise `RuntimeError::type_error(...)`. Never silently return `Value::None` from a function that should raise.

---

## Raylib Integration Plan

When adding Raylib bindings (`src/stdlib/raylib.rs`):

Add to `Cargo.toml`:
```toml
[dependencies]
raylib = "5"
```

Map Raylib types to `Value`:
- `Color` → `Value::Tuple` of 4 ints (r, g, b, a)
- `Vector2` → `Value::Tuple` of 2 floats (x, y)
- `Rectangle` → `Value::Tuple` of 4 floats (x, y, w, h)
- Window handle → managed via a thread-local singleton (Raylib is not thread-safe)
- Textures/sounds → `Value::Instance` with an opaque ID into a static registry

The game loop pattern:
```python
import raylib

raylib.init_window(800, 600, "My Game")
raylib.set_target_fps(60)

while not raylib.window_should_close():
    raylib.begin_drawing()
    raylib.clear_background((20, 20, 20, 255))
    raylib.draw_text("Hello Cherash!", 10, 10, 20, (255, 255, 255, 255))
    raylib.end_drawing()

raylib.close_window()
```
