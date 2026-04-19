# Cherash Standard Library Reference

Cherash's stdlib is intentionally minimal. Four modules ship with the interpreter; additional functionality is deferred post-MVP.

---

## math

```python
import math
```

### Constants

| Name | Value |
|------|-------|
| `math.pi` | 3.141592653589793 |
| `math.e` | 2.718281828459045 |
| `math.tau` | 6.283185307179586 |
| `math.inf` | positive infinity |
| `math.nan` | not-a-number |

### Functions

#### Basic

```python
math.sqrt(x)       # square root
math.fabs(x)       # absolute value (float)
math.ceil(x)       # ceiling (smallest int >= x)
math.floor(x)      # floor (largest int <= x)
math.trunc(x)      # truncate toward zero
math.pow(x, y)     # x raised to y
math.copysign(x, y)  # magnitude of x with sign of y
math.fmod(x, y)    # floating-point remainder
```

#### Exponential / Logarithm

```python
math.exp(x)         # e^x
math.log(x)         # natural log
math.log(x, base)   # log base 'base'
math.log2(x)        # log base 2
math.log10(x)       # log base 10
```

#### Trigonometry

```python
math.sin(x)    math.asin(x)
math.cos(x)    math.acos(x)
math.tan(x)    math.atan(x)
math.atan2(y, x)    # atan of y/x, correct quadrant
math.hypot(x, y)    # sqrt(x² + y²)
math.degrees(x)     # radians → degrees
math.radians(x)     # degrees → radians
```

#### Integer Math

```python
math.factorial(n)   # n!  (n must be non-negative int)
math.gcd(a, b)      # greatest common divisor
```

#### Predicates

```python
math.isnan(x)       # True if x is NaN
math.isinf(x)       # True if x is ±infinity
math.isfinite(x)    # True if x is neither NaN nor infinity
```

### Examples

```python
import math

# Hypotenuse
c = math.hypot(3, 4)    # 5.0

# Angle in degrees
angle_deg = math.degrees(math.atan2(1, 1))  # 45.0

# Factorial
print(math.factorial(10))   # 3628800

# GCD
print(math.gcd(48, 18))     # 6
```

---

## io

```python
import io
```

The `io` module exposes file I/O. The `open()` builtin is also available without importing `io`.

### open()

```python
open(path: str, mode: str = 'r') -> file
```

| Mode | Meaning |
|------|---------|
| `'r'` | Read text (default) |
| `'w'` | Write text (truncate) |
| `'a'` | Append text |
| `'x'` | Exclusive create |
| `'rb'` | Read bytes |
| `'wb'` | Write bytes |

### File Object Methods

```python
f.read()            # read entire file → str or bytes
f.readline()        # read one line
f.readlines()       # read all lines → list[str]
f.write(s)          # write str or bytes → bytes written
f.seek(n)           # seek to byte offset n
f.close()           # close the file
```

### Context Manager

Always prefer `with` — it guarantees `close()` even if an exception is raised:

```python
with open('data.txt', 'r') as f:
    for line in f.readlines():
        print(line.strip())
```

### Writing

```python
with open('output.txt', 'w') as f:
    f.write("line one\n")
    f.write("line two\n")
```

### Binary Files

```python
with open('image.png', 'rb') as f:
    header = f.read()[:8]
```

---

## sys

```python
import sys
```

### Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `sys.argv` | `list[str]` | Command-line arguments. `argv[0]` is the script path. |
| `sys.version` | `str` | Interpreter version string |
| `sys.platform` | `str` | OS identifier (`'linux'`, `'windows'`, `'macos'`) |
| `sys.maxsize` | `int` | Maximum value of `int` (i64::MAX) |
| `sys.path` | `list[str]` | Module search path |

### Functions

```python
sys.exit(code: int = 0)    # terminate the process
```

### Examples

```python
import sys

script = sys.argv[0]
args   = sys.argv[1:]

print(f"Running on {sys.platform}")
print(f"Arguments: {args}")

if len(args) == 0:
    sys.exit(1)
```

---

## threading

```python
import threading
```

Cherash uses real OS threads — no GIL. All shared mutable state must be protected with synchronization primitives.

### Thread

```python
threading.Thread(target=fn, args=(arg1, arg2, ...))
```

```python
t = threading.Thread(target=worker, args=(1,))
t.start()   # spawn the thread
t.join()    # wait for completion
```

Full example:

```python
import threading

results = []
lock = threading.Lock()

def worker(n: int) -> None:
    with lock:
        results.append(n * n)

threads = [threading.Thread(target=worker, args=(i,)) for i in range(10)]

for t in threads:
    t.start()
for t in threads:
    t.join()

print(sorted(results))  # [0, 1, 4, 9, 16, 25, 36, 49, 64, 81]
```

### Lock

Mutual exclusion. Only one thread holds the lock at a time.

```python
lock = threading.Lock()

lock.acquire()
# critical section
lock.release()

# preferred — automatic release on exception:
with lock:
    shared_data.append(value)
```

### RLock

Reentrant lock. The same thread may call `acquire()` multiple times without deadlocking.

```python
rlock = threading.RLock()
with rlock:
    with rlock:   # same thread — OK
        pass
```

### Condition

For wait/notify patterns:

```python
cond = threading.Condition()

# Consumer
with cond:
    cond.wait()        # releases lock and waits
    process(item)

# Producer
with cond:
    queue.append(item)
    cond.notify()      # wake one waiting thread
    # cond.notify_all() to wake all
```

### Semaphore

Counting semaphore — allows up to N concurrent holders:

```python
sem = threading.Semaphore(3)  # at most 3 threads at once

with sem:
    access_resource()
```

### Barrier

Synchronize N threads at a rendezvous point:

```python
barrier = threading.Barrier(4)  # wait for 4 threads

def worker():
    do_phase1()
    barrier.wait()   # all 4 must reach here before any continues
    do_phase2()
```

### Safety Rules

1. **Always protect shared mutable state** with a `Lock` or `RLock`.
2. **Never hold a lock across a blocking call** — risk of deadlock.
3. **Prefer `with lock:`** over manual `acquire`/`release` — exception-safe.
4. **Immutable values** (`int`, `str`, `tuple`) are inherently safe to share without locks.

---

## os

```python
import os
```

### Functions

```python
os.getcwd()             # current working directory → str
os.listdir(path='.')    # directory listing → list[str]
```

### os.path

```python
os.path.join(a, b, ...)    # join path components
os.path.exists(path)       # True if path exists
```

### Examples

```python
import os

cwd = os.getcwd()
files = os.listdir(cwd)

for f in files:
    full = os.path.join(cwd, f)
    if os.path.exists(full):
        print(full)
```

---

## time

```python
import time
```

```python
time.time()        # seconds since Unix epoch → float
time.sleep(secs)   # pause execution for secs seconds
```

```python
import time

start = time.time()
time.sleep(0.1)
elapsed = time.time() - start
print(f"Slept for {elapsed:.3f}s")
```

---

## random

```python
import random
```

```python
random.random()          # uniform float in [0.0, 1.0)
random.randint(a, b)     # random int in [a, b] inclusive
```

```python
import random

x = random.random()
n = random.randint(1, 6)   # dice roll
```

---

## Future Stdlib (Post-MVP)

The following modules are planned but deferred until after Phase 8:

| Module | Contents |
|--------|----------|
| `re` | Regular expressions |
| `json` | JSON encode/decode |
| `net` | TCP/UDP sockets |
| `collections` | deque, Counter, OrderedDict, defaultdict |
| `itertools` | chain, product, permutations, combinations |
| `functools` | reduce, partial, lru_cache |
| `pathlib` | Object-oriented path manipulation |
| `datetime` | Date and time types |
| `struct` | Binary packing/unpacking |
