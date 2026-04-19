# Contributing to Cherash

This document describes how the Cherash codebase is developed — phase structure, testing methodology, commit discipline, and coding standards.

---

## Development Philosophy

1. **Never skip phases.** Phase N depends on Phase N-1 being complete and tested.
2. **Tests first.** Write the test that validates the exit criterion before writing the implementation.
3. **One sub-task at a time.** Implement, test, commit. Then move to the next sub-task.
4. **Loud failures over silent bugs.** If something is wrong, produce a detailed error with location. Never swallow errors silently.
5. **No premature optimization.** Tree-walking interpreter is the correct target for the MVP. Bytecode VM and JIT are post-Phase-8.

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
│   ├── interpreter/        Phase 3 (evaluator + environment)
│   ├── runtime/            Phases 7–8 (value, gc, threading)
│   └── stdlib/             Phase 8 (math, io, sys, threading, ...)
├── tests/
│   ├── phase1_lexer.rs
│   ├── phase3_evaluator.rs
│   └── ...
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
└── CLAUDE.MD               AI implementation instructions
```

---

## Phase Status

| Phase | Name | Status |
|-------|------|--------|
| 1 | Lexer | Complete |
| 2 | Parser & AST | Complete |
| 3 | Core Evaluator | Complete |
| 4 | Type Checker | Complete (partial — return types TODO) |
| 5 | Collections & OOP | Complete |
| 6 | Exception Machinery | Complete |
| 7 | Threading | Stub — needs real `std::thread` spawning |
| 8 | GC + Stdlib + Raylib | Not started |

---

## Building

```bash
cargo build          # debug build
cargo build --release  # optimized build
cargo test           # run all tests
cargo test phase1    # run only phase1 tests
```

No external dependencies are required for Phases 1–7. Phase 8 will add `raylib-sys` for the Raylib bindings.

---

## Writing Tests

Tests live in `tests/` as integration tests that call into the library via `run_source()`.

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
- Use `assert` statements inside the Cherash program — they produce clear error messages when wrong.
- Use `run_source(src, true)` for strict mode tests.
- Test error cases with `run_source(...).expect_err(...)` or match on `CherashError`.

### Error case pattern

```rust
fn run_expect_err(src: &str, expected_type: &str) {
    match run_source(src, true) {
        Err(cherash::CherashError::TypeCheck(_)) => {}
        Err(cherash::CherashError::Runtime(e)) => {
            assert_eq!(e.type_name, expected_type);
        }
        Ok(()) => panic!("expected error '{}' but program succeeded", expected_type),
        Err(e) => panic!("unexpected error: {}", e),
    }
}
```

---

## Implementing a New Phase

Follow this process for each phase:

### 1. Create sub-tasks

Break the phase exit criteria from `CLAUDE.MD` into tasks of 15–30 minutes each. Document them (in a comment, a GitHub issue, or a local task list).

### 2. Write the test first

Before writing any implementation code, write the integration test that validates the exit criterion. The test should fail initially.

### 3. Implement the sub-task

Write the minimum code to make the test pass. Do not add features beyond what the test requires.

### 4. Verify

```bash
cargo test phase<N>
```

All tests for the current phase must pass. Tests for earlier phases must continue to pass.

### 5. Commit

```
git add src/ tests/
git commit -m "phase<N>: <what was implemented>"
```

---

## Coding Standards

### Rust style

- Follow standard Rust idioms (`cargo clippy` should be clean).
- Use `Arc<Mutex<T>>` for all shared mutable heap values — never `unsafe` for sharing.
- Prefer `match` with exhaustive patterns over `if let` chains.
- Use `?` for error propagation in `Result`-returning functions.
- Use `Signal` for control flow (return, break, continue, raise) — not panics.

### Error messages

Every error produced by the interpreter must include:
- The error type name (`TypeError`, `ValueError`, etc.)
- A clear, specific message describing what went wrong
- Source location (line and column) when available

Avoid: "operation failed", "invalid value", "error occurred".
Prefer: "expected int, got str in assignment at 5:3", "list index 10 out of range for list of length 5".

### Adding builtins

Built-in functions live in `src/interpreter/evaluator.rs`. Each has the signature:

```rust
fn builtin_name(args: Vec<Value>) -> Result<Value, RuntimeError>
```

Register it in `load_builtins()`:

```rust
("mybuiltin", builtin_mybuiltin),
```

For functions that are logically part of a type (list methods, str methods), add them to the `list_method` / `str_method` dispatch tables in the evaluator.

### Adding a stdlib module

1. Create `src/stdlib/mymodule.rs`.
2. Implement `pub fn make_mymodule_module() -> Value`.
3. Export from `src/stdlib/mod.rs`: `pub mod mymodule;`
4. Register in `import_module()` in the evaluator:
   ```rust
   "mymodule" => ok(crate::stdlib::mymodule::make_mymodule_module()),
   ```

---

## Phase 7 Implementation Notes

### What needs to be done

`threading.Thread.start()` currently calls the target function synchronously. It needs to spawn a real OS thread:

```rust
fn thread_start(args: Vec<Value>) -> Result<Value, RuntimeError> {
    // 1. Extract target function and args from Instance fields
    // 2. Clone them (cheap — Arc inside)
    // 3. Spawn std::thread::spawn with a new Evaluator
    // 4. Store the JoinHandle somewhere the instance can join() later
}
```

The `JoinHandle` can't live in a `Value` directly (it's not `Clone`). Options:
- Store it as a raw pointer in `Instance.fields` as `Value::Int` (unsafe)
- Use a `Arc<Mutex<Option<JoinHandle<()>>>>` field stored as a custom value type
- Use a global thread registry indexed by thread ID

### Value::Send requirement

`Value` must be `Send + Sync` to cross thread boundaries. Currently it is, because:
- `Arc<Mutex<T>>` is `Send + Sync` when `T: Send`
- Primitive variants (`Int`, `Float`, etc.) are `Copy` / `Send`
- `Function` carries `EnvRef` which is `Arc<Mutex<Env>>` — `Send`

Verify with `cargo test` after adding `std::thread::spawn` — the compiler will reject non-`Send` types at the spawn site.

---

## Phase 8 Implementation Notes

### GC integration points

1. Every `Arc<Mutex<T>>` write that stores a `Value` needs a write barrier call.
2. The GC thread needs a way to enumerate all live `Arc` instances (the root set).
3. The evaluator's `EnvRef` chain is the primary root — each `Env` holds a scope.

### Raylib bindings

Add to `Cargo.toml`:

```toml
[dependencies]
raylib = "5"    # or raylib-sys for raw bindings
```

Implement `src/stdlib/raylib.rs` following the same pattern as other stdlib modules: a `make_raylib_module()` function returning a `Value::Module` with `BuiltinFn` entries for each Raylib function.

Map Raylib types:
- `Color` → `Value::Tuple(Arc<vec![r, g, b, a]>)` (4-element int tuple)
- `Vector2` → `Value::Tuple(Arc<vec![x, y]>)` (2-element float tuple)
- Window handles → opaque `Value::Int` (pointer cast)
