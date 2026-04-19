# Cherash Language — Development Roadmap

> Python's face. C's soul. Go's memory. Java's threads.

---

## Status Overview

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Lexer | ✅ Complete |
| 2 | Parser & AST | ✅ Complete |
| 3 | Core Evaluator | ✅ Complete |
| 4 | Type Checker | ✅ Complete |
| 5 | Collections & OOP | ✅ Complete |
| 6 | Exception Machinery | ✅ Complete |
| 7 | Threading | ✅ Complete |
| 8 | GC + Stdlib | ✅ Complete |
| 8 | Raylib | 🔲 Planned |

---

## Milestone 1: Language Core (Phases 1–3) — DONE

All core language features are implemented and tested.

- [x] Full Python-grammar lexer with INDENT/DEDENT
- [x] Recursive descent parser with Pratt precedence
- [x] Tree-walking evaluator with LEGB scoping
- [x] Closures, nonlocal, global
- [x] All arithmetic, comparison, logical operators
- [x] For/while loops with break/continue/else
- [x] Functions, return, default args, *args, **kwargs
- [x] i64 wrapping overflow (C semantics)

---

## Milestone 2: Type System (Phase 4) — DONE

- [x] Type annotations parsed from function signatures
- [x] Lenient mode (.py files) — no type checking
- [x] Strict mode (.chsh files) — primitive types, collections, return types
- [x] Generic collection types: `list[T]`, `dict[K,V]`, `set[T]`
- [x] Return type checking in strict mode
- [x] `# cherash:strict` header directive in .py files
- [x] `cherash check` subcommand (type-check without executing)

---

## Milestone 3: Collections & OOP (Phase 5) — DONE

- [x] `list`, `dict`, `set`, `tuple` with full method suites
- [x] Class definitions with inheritance
- [x] Method resolution order (MRO)
- [x] `__init__`, `__str__`, `__repr__`, `__len__`
- [x] `__getitem__`, `__setitem__`, `__iter__`, `__next__`
- [x] `__eq__`, `__lt__`, `__hash__` and other comparison dunders
- [x] `@property` decorator
- [x] `@staticmethod`, `@classmethod`
- [x] List/dict/set comprehensions
- [x] Generator expressions (basic)

---

## Milestone 4: Exception System (Phase 6) — DONE

- [x] Full exception hierarchy (BaseException → Exception → ...)
- [x] `raise`, `try/except/else/finally`
- [x] Exception chaining (`raise X from Y`)
- [x] Bare `raise` (re-raise)
- [x] Tracebacks with line numbers and call stack
- [x] `__cause__`, `__context__` attributes

---

## Milestone 5: Threading (Phase 7) — DONE

- [x] `threading.Thread` backed by `std::thread::spawn`
- [x] `threading.Lock` with `acquire()`/`release()`
- [x] `threading.RLock` (reentrant lock)
- [x] `threading.Condition` — `wait()`, `notify()`, `notify_all()`
- [x] `threading.Semaphore(n)` / `BoundedSemaphore` — counting semaphore
- [x] `threading.Barrier(n)` — N-party rendezvous barrier
- [x] Context manager protocol (`with lock:`, `with cond:`)
- [x] `thread.start()`, `thread.join()`, `thread.is_alive()`
- [x] No GIL — real concurrent OS threads
- [x] Keyword arguments: `Thread(target=fn, args=(...))`
- [ ] `threading.Event` — `set()`, `clear()`, `wait()`
- [ ] Thread-local storage (`threading.local()`)

---

## Milestone 6: GC (Phase 8, Part 1) — DONE

- [x] Tricolor mark-and-sweep (white/grey/black)
- [x] `GcHandle<T>` — drop-in for `Arc<Mutex<T>>` with GC color tracking
- [x] Concurrent collector thread
- [x] GC roots from evaluator stack
- [x] Write barrier on mutation
- [ ] `CHERASH_GC_PERCENT` env var (target heap ratio before collection)
- [ ] `CHERASH_GC_HARD_LIMIT` env var (hard memory cap)
- [ ] Cycle collection stress tests

---

## Milestone 7: Stdlib (Phase 8, Part 2) — PARTIAL

### Done
- [x] `math` module — full CPython-compatible function set
  - Trig: sin, cos, tan, asin, acos, atan, atan2
  - Hyperbolic: sinh, cosh, tanh, asinh, acosh, atanh
  - Exponential: exp, exp2, expm1, log, log2, log10, log1p
  - Rounding: ceil (→int), floor (→int), trunc (→int), round
  - Special: erf, erfc, gamma, lgamma
  - Integer: gcd, lcm, factorial, comb, perm
  - Float utils: isnan, isinf, isfinite, isclose, fsum, prod
  - Geometry: dist, hypot
  - Bit/float: modf, frexp, ldexp, remainder, nextafter, ulp, copysign
  - Constants: pi, e, tau, inf, nan
- [x] `io` module — file I/O with context managers
  - `open(path, mode)` → TextIOWrapper
  - Modes: r, w, a, rb, wb, x
  - Methods: read, readline, readlines, write, writelines, seek, tell, close, flush
  - Context manager: `with open(...) as f:`
  - `readable()`, `writable()` predicates
- [x] `sys` module — argv, exit, path, version, platform, stdin/stdout/stderr
- [x] `os` module — getcwd, chdir, listdir, mkdir, remove, path.join/exists/isfile/isdir

### Done
- [x] `json` module — loads, dumps, load, dump (pure-Rust parser)
- [x] `time` module — time, time_ns, sleep, monotonic, gmtime, localtime, strftime
- [x] `random` module — PCG64 PRNG: random, uniform, randint, randrange, choice, choices, shuffle, sample, gauss, seed

### Remaining
- [ ] `re` module — search, match, findall, sub, compile (delegate to Rust regex crate)
- [ ] `pathlib` module — Path class
- [ ] `collections` module — OrderedDict, defaultdict, Counter, deque
- [ ] `itertools` module — chain, cycle, product, permutations, combinations
- [ ] `functools` module — reduce, partial, lru_cache
- [ ] `string` module — ascii_letters, digits, punctuation, etc.

---

## Milestone 8: Raylib Bindings (Phase 8, Part 3) — NOT STARTED

Full graphical runtime via [raylib-rs](https://github.com/deltaphc/raylib-rs).

- [ ] Add `raylib` dependency to Cargo.toml
- [ ] `raylib` module: window management
  - `init_window(w, h, title)`, `close_window()`, `window_should_close()`
  - `set_target_fps(fps)`, `get_frame_time()`
- [ ] Drawing API
  - `begin_drawing()`, `end_drawing()`, `clear_background(color)`
  - `draw_text(text, x, y, size, color)`
  - `draw_rectangle(x, y, w, h, color)`, `draw_circle(x, y, r, color)`
  - `draw_line(x1, y1, x2, y2, color)`
- [ ] Input
  - `is_key_down(key)`, `is_key_pressed(key)`
  - `is_mouse_button_down(btn)`, `get_mouse_position()`
- [ ] Color constants: RED, GREEN, BLUE, WHITE, BLACK, RAYWHITE, etc.
- [ ] Texture loading: `load_texture(path)`, `draw_texture(tex, x, y)`
- [ ] Audio: `init_audio_device()`, `load_sound(path)`, `play_sound(s)`

---

## Milestone 9: Tooling & Polish — FUTURE

- [ ] `cherash fmt` — code formatter (Black-compatible)
- [ ] `cherash lint` — static analysis
- [ ] `cherash check` — standalone type checker
- [ ] LSP server for IDE integration
- [ ] Package manager (`cherash add <pkg>`)
- [ ] REPL improvements: multi-line, history, tab-completion
- [ ] Bytecode compilation (optional optimization layer)
- [ ] Profile-guided optimizations for hot paths

---

## Immediate Next Steps (Sprint)

Priority order for the next work session:

1. **`threading.Event`** — `set()`, `clear()`, `wait()` — completes the threading suite
2. **`re` module** — regex support (delegate to Rust `regex` crate)
3. **`collections` module** — `deque`, `defaultdict`, `Counter`, `OrderedDict`
4. **`itertools` / `functools`** — higher-order functions, combinatorics
5. **Generator protocol** — real `yield`/`next()` / `StopIteration` (currently generators are eager)
6. **`async/await` runtime** — basic async/await execution on a simple event loop
7. **Raylib bindings** — Phase 8 final deliverable

---

## Design Principles (Immutable)

| Dimension | Decision |
|-----------|----------|
| Integer overflow | Wrapping i64 — silent, C-style |
| Typing | Strict in .chsh, lenient in .py |
| Threading | Real OS threads, no GIL |
| GC | Concurrent tricolor — no stop-the-world |
| Errors | Loud, detailed tracebacks — no silent failures |
| Collections | GC-tracked via `GcHandle<T>` |
| Philosophy | Sharp edges are your fault |

---

*Last updated: 2026-04-19*
