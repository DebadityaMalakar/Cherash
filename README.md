# Cherash

> *From Nachash (Serpent) + Cherut (Freedom) — Serpent of Freedom*

**Python's face. C's soul. Go's memory. Java's threads.**

Cherash is a general-purpose programming language implemented in Rust. It takes Python's syntax as a base, throws out CPython's historical baggage, and replaces it with principled decisions drawn from Go, Java, and C.

---

Created by **Debaditya Malakar**.

This project was built with **Claude** (Anthropic) as a co-author — the interpreter implementation, architecture, and documentation were developed collaboratively with AI assistance. The language design, specification, and creative direction are entirely human.

Co-author: Claude Sonnet 4.6 — noreply@anthropic.com

---

## What it is

| Dimension | Decision |
|-----------|----------|
| Syntax | Python — `.py` and `.chsh` files, full Python grammar |
| Integers | 64-bit, wraps on overflow (C-style, no BigInt) |
| Threads | Real OS threads, no GIL (Java model) |
| GC | Concurrent tricolor mark-and-sweep (Go-inspired) |
| Types | Strict in `.chsh`, lenient in `.py` |
| Errors | Loud — full tracebacks, no silent failures |

---

## Usage

```bash
cherash run script.py       # run in compatibility mode (lenient typing)
cherash run script.chsh     # run with strict type checking
cherash check script.chsh   # type-check only, no execution
cherash repl                # interactive REPL
```

Add `# cherash:strict` as the first line of any `.py` file to opt into strict mode.

---

## Quick examples

```python
# fib.chsh — strictly typed fibonacci
def fib(n: int) -> int:
    if n <= 1:
        return n
    return fib(n-1) + fib(n-2)

print(fib(10))   # 55
```

```python
# closures.chsh — closures with nonlocal
def counter():
    n = 0
    def inc():
        nonlocal n
        n += 1
        return n
    return inc

c = counter()
print(c(), c(), c())  # 1 2 3
```

```python
# threading.py — real OS threads, no GIL
import threading

lock = threading.Lock()
total = 0

def worker():
    global total
    for _ in range(1000):
        with lock:
            total += 1

threads = [threading.Thread(target=worker) for _ in range(4)]
for t in threads: t.start()
for t in threads: t.join()
print(total)  # 4000
```

```python
# json_io.py — file I/O and JSON
import json

data = {"name": "Cherash", "version": 1, "tags": ["lang", "fast"]}
with open("out.json", "w") as f:
    json.dump(data, f)

with open("out.json", "r") as f:
    loaded = json.load(f)
print(loaded["name"])  # Cherash
```

---

## Building

Requires Rust stable (1.70+). No external C dependencies for the core interpreter.

```bash
git clone <repo>
cd cherash
cargo build --release
./target/release/cherash repl
```

Run the test suite:

```bash
cargo test
```

---

## Status

| Phase | Feature | Status |
|-------|---------|--------|
| 1 | Lexer (full Python grammar) | ✅ Complete |
| 2 | Parser & AST | ✅ Complete |
| 3 | Core evaluator (LEGB scoping, closures) | ✅ Complete |
| 4 | Type checker (strict/lenient modes) | ✅ Complete |
| 5 | Collections & OOP (list, dict, set, classes) | ✅ Complete |
| 6 | Exception machinery | ✅ Complete |
| 7 | Threading (Thread, Lock, RLock, Condition, Semaphore, Barrier) | ✅ Complete |
| 8 | Concurrent GC (tricolor mark-and-sweep) | ✅ Complete |
| 8 | Stdlib (math, io, json, os, sys, time, random, threading) | ✅ Complete |
| 8 | Raylib graphics bindings | 🔲 Planned |

---

## Stdlib modules

| Module | Contents |
|--------|----------|
| `math` | Full CPython-compatible math (trig, gamma, erf, gcd, lcm, ...) |
| `io` | `open()`, TextIOWrapper, context managers |
| `json` | `loads`, `dumps`, `load`, `dump` — pure Rust parser |
| `os` | `getcwd`, `chdir`, `listdir`, `mkdir`, `remove`, `path.*` |
| `sys` | `argv`, `exit`, `path`, `version`, `stdin`/`stdout`/`stderr` |
| `time` | `time`, `sleep`, `monotonic`, `gmtime`, `strftime` |
| `random` | PCG64 PRNG: `random`, `randint`, `choice`, `shuffle`, `gauss` |
| `threading` | `Thread`, `Lock`, `RLock`, `Condition`, `Semaphore`, `Barrier` |

---

## Language features

- **Full Python syntax**: `def`, `class`, `if/elif/else`, `for`, `while`, `with`, `try/except/finally`, `raise`, `import`, comprehensions, f-strings, decorators, `lambda`, `:=`, `*args`, `**kwargs`
- **Keyword arguments**: fully dispatched by parameter name
- **Starred unpacking**: `a, *b, c = [1,2,3,4,5]`
- **Exception chaining**: `raise X from Y`
- **Context managers**: `with open(...) as f:`, `with lock:`
- **Real threading**: `threading.Thread` backed by `std::thread`, no GIL
- **Concurrent GC**: background collector, configurable via `CHERASH_GC_PERCENT` and `CHERASH_GC_HARD_LIMIT`
- **C integer semantics**: `i64` wrapping overflow

---

## GC tuning

```bash
CHERASH_GC_PERCENT=50 cherash run heavy.chsh    # collect when heap grows 50%
CHERASH_GC_HARD_LIMIT=256 cherash run big.chsh  # hard limit 256 MB
CHERASH_GC_INTERVAL_MS=200 cherash run app.chsh # collector interval 200ms
```

---

## Documentation

- [ROADMAP.MD](ROADMAP.MD) — feature roadmap and sprint backlog
- [DOCS/LANGUAGE_GUIDE.MD](DOCS/LANGUAGE_GUIDE.MD) — full language reference
- [DOCS/TYPE_SYSTEM.MD](DOCS/TYPE_SYSTEM.MD) — strict vs lenient typing
- [DOCS/THREADING.MD](DOCS/THREADING.MD) — threading model and primitives
- [DOCS/GC.MD](DOCS/GC.MD) — garbage collector design
- [DOCS/STDLIB.MD](DOCS/STDLIB.MD) — standard library reference
- [DOCS/ARCHITECTURE.MD](DOCS/ARCHITECTURE.MD) — interpreter internals

---

## License

GNU General Public License v3.0 — see [LICENSE](LICENSE).
