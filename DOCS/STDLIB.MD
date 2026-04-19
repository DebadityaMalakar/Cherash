# Cherash Standard Library Reference

Cherash ships eight stdlib modules with the interpreter.

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

#### Rounding (return `int`)

```python
math.ceil(x)       # smallest int >= x  → int
math.floor(x)      # largest int <= x   → int
math.trunc(x)      # truncate toward 0  → int
```

Note: `ceil`, `floor`, `trunc` return `int`, not `float` — matching CPython behaviour.

#### Basic

```python
math.sqrt(x)          # square root
math.fabs(x)          # absolute value (float)
math.pow(x, y)        # x raised to y (float)
math.copysign(x, y)   # magnitude of x, sign of y
math.fmod(x, y)       # floating-point remainder
math.remainder(x, y)  # IEEE 754 remainder
math.cbrt(x)          # cube root
```

#### Exponential / Logarithm

```python
math.exp(x)           # e^x
math.exp2(x)          # 2^x
math.expm1(x)         # e^x - 1 (accurate near 0)
math.log(x)           # natural log
math.log(x, base)     # log base 'base'
math.log2(x)          # log base 2
math.log10(x)         # log base 10
math.log1p(x)         # log(1 + x) (accurate near 0)
```

#### Trigonometry

```python
math.sin(x)   math.asin(x)
math.cos(x)   math.acos(x)
math.tan(x)   math.atan(x)
math.atan2(y, x)      # atan of y/x, correct quadrant
math.hypot(*coords)   # Euclidean distance
math.dist(p, q)       # Euclidean distance between two points (as tuples/lists)
math.degrees(x)       # radians → degrees
math.radians(x)       # degrees → radians
```

#### Hyperbolic

```python
math.sinh(x)   math.asinh(x)
math.cosh(x)   math.acosh(x)
math.tanh(x)   math.atanh(x)
```

#### Special Functions

```python
math.erf(x)     # error function
math.erfc(x)    # complementary error function (1 - erf(x))
math.gamma(x)   # gamma function (Lanczos approximation, g=7)
math.lgamma(x)  # log of absolute value of gamma(x)
```

#### Integer Math

```python
math.factorial(n)        # n!  (non-negative int only)
math.gcd(a, b)           # greatest common divisor
math.lcm(a, b)           # least common multiple
math.comb(n, k)          # n choose k  (binomial coefficient)
math.perm(n, k=None)     # permutations P(n, k)
```

#### Float Utilities

```python
math.isnan(x)            # True if NaN
math.isinf(x)            # True if ±infinity
math.isfinite(x)         # True if neither NaN nor infinity
math.isclose(a, b, rel_tol=1e-9, abs_tol=0.0)
math.fsum(iterable)      # accurate floating-point sum (Neumaier compensation)
math.prod(iterable)      # product of all elements
math.modf(x)             # → (fractional_part, integer_part) as floats
math.frexp(x)            # → (mantissa, exponent) such that x = m * 2^e
math.ldexp(x, i)         # x * 2^i
math.nextafter(x, y)     # next float toward y
math.ulp(x)              # unit in the last place
```

### Examples

```python
import math

c = math.hypot(3, 4)                          # 5.0
deg = math.degrees(math.atan2(1, 1))          # 45.0
print(math.factorial(10))                     # 3628800
print(math.gcd(48, 18))                       # 6
print(math.isclose(0.1 + 0.2, 0.3))          # True
print(math.fsum([0.1, 0.1, 0.1, 0.1, 0.1,
                 0.1, 0.1, 0.1, 0.1, 0.1]))  # 1.0 (accurate)
print(math.comb(10, 3))                       # 120
```

---

## io

```python
import io
# or just use open() — it's a global builtin
```

### open()

```python
open(path: str, mode: str = 'r') -> TextIOWrapper
```

| Mode | Meaning |
|------|---------|
| `'r'` | Read text (default) |
| `'w'` | Write text (truncate) |
| `'a'` | Append text |
| `'x'` | Exclusive create — fails if file exists |
| `'rb'` | Read binary |
| `'wb'` | Write binary |

### File Object Methods

```python
f.read(size=-1)       # read all or up to size chars → str
f.readline()          # read one line including \n → str
f.readlines()         # read all lines → list[str]
f.write(s)            # write string → bytes written (int)
f.writelines(lines)   # write each element of iterable
f.seek(pos)           # seek to byte offset → int
f.tell()              # current position → int
f.flush()             # flush write buffer to disk
f.close()             # close and flush
f.readable()          # True if opened for reading
f.writable()          # True if opened for writing
```

### Context Manager

```python
with open('data.txt', 'r') as f:
    content = f.read()
    # f is automatically closed after the block
```

### Examples

```python
# Read entire file
with open('input.txt', 'r') as f:
    text = f.read()

# Write lines
with open('output.txt', 'w') as f:
    f.write("line one\n")
    f.writelines(["line two\n", "line three\n"])

# Append
with open('log.txt', 'a') as f:
    f.write("new entry\n")

# Seek and tell
with open('data.bin', 'rb') as f:
    header = f.read(8)
    print(f.tell())       # 8
    f.seek(0)             # rewind
    all_data = f.read()
```

---

## json

```python
import json
```

Pure-Rust JSON parser and serializer. No external dependencies.

### Functions

```python
json.dumps(obj) -> str           # serialize to JSON string
json.loads(s: str) -> object     # parse JSON string → Value
json.dump(obj, file)             # serialize and write to file object
json.load(file) -> object        # read from file object and parse
```

### Supported types

| Cherash type | JSON representation |
|-------------|---------------------|
| `None` | `null` |
| `bool` | `true` / `false` |
| `int` | number |
| `float` | number |
| `str` | string |
| `list` / `tuple` | array |
| `dict` (str keys) | object |

`NaN` and `Infinity` raise `ValueError` — they are not valid JSON.

### Examples

```python
import json

# Serialize
data = {"name": "cherash", "version": 1, "features": ["gc", "threads"]}
s = json.dumps(data)
print(s)   # {"name":"cherash","version":1,"features":["gc","threads"]}

# Deserialize
obj = json.loads('{"x": 1, "y": [2, 3]}')
print(obj["x"])      # 1
print(obj["y"][1])   # 3

# File I/O
with open("config.json", "w") as f:
    json.dump(data, f)

with open("config.json", "r") as f:
    loaded = json.load(f)
```

---

## os

```python
import os
```

### Functions

```python
os.getcwd()                      # current working directory → str
os.chdir(path)                   # change directory
os.listdir(path='.')             # directory contents → list[str]
os.mkdir(path)                   # create directory
os.remove(path)                  # delete file
os.path.join(a, b, ...)          # join path components
os.path.exists(path)             # True if path exists
os.path.isfile(path)             # True if regular file
os.path.isdir(path)              # True if directory
```

### Examples

```python
import os

cwd = os.getcwd()
for name in os.listdir(cwd):
    full = os.path.join(cwd, name)
    if os.path.isfile(full):
        print(f"file: {full}")
    elif os.path.isdir(full):
        print(f"dir:  {full}")
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
| `sys.maxsize` | `int` | i64::MAX |
| `sys.path` | `list[str]` | Module search path |
| `sys.stdin` | file-like | Standard input |
| `sys.stdout` | file-like | Standard output |
| `sys.stderr` | file-like | Standard error |

### Functions

```python
sys.exit(code: int = 0)    # terminate the process with exit code
```

### Examples

```python
import sys

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <input>", file=sys.stderr)
    sys.exit(1)

print(f"Running Cherash {sys.version} on {sys.platform}")
```

---

## time

```python
import time
```

### Functions

```python
time.time()          # seconds since Unix epoch → float
time.time_ns()       # nanoseconds since Unix epoch → int
time.monotonic()     # monotonic clock → float (seconds since process start)
time.perf_counter()  # high-resolution performance counter → float
time.sleep(secs)     # pause execution for secs seconds (float or int)
time.gmtime(ts=None) # UTC struct_time tuple from timestamp
time.localtime(ts=None) # local struct_time tuple from timestamp
time.strftime(fmt, t=None) # format struct_time or current time
```

### struct_time tuple

`gmtime()` and `localtime()` return a 9-element tuple:
`(tm_year, tm_mon, tm_mday, tm_hour, tm_min, tm_sec, tm_wday, tm_yday, tm_isdst)`

### strftime directives

| Directive | Meaning |
|-----------|---------|
| `%Y` | 4-digit year |
| `%m` | Month (01–12) |
| `%d` | Day (01–31) |
| `%H` | Hour (00–23) |
| `%M` | Minute (00–59) |
| `%S` | Second (00–59) |
| `%A` | Weekday name (Mon, Tue, ...) |
| `%B` | Month name (Jan, Feb, ...) |

### Examples

```python
import time

start = time.monotonic()
time.sleep(0.1)
elapsed = time.monotonic() - start
print(f"Slept for {elapsed:.3f}s")

ts = time.time()
t = time.gmtime(ts)
print(time.strftime("%Y-%m-%d %H:%M:%S", t))
```

---

## random

```python
import random
```

Uses a **PCG64** pseudo-random number generator. State is global and automatically seeded from the system clock at import time.

### Functions

```python
random.seed(a=None)            # seed the PRNG (int, float, or None for clock)
random.random()                # uniform float in [0.0, 1.0)
random.uniform(a, b)           # uniform float in [a, b]
random.randint(a, b)           # random int in [a, b] inclusive
random.randrange(start, stop, step=1)  # random int in range(start, stop, step)
random.choice(seq)             # random element from sequence
random.choices(seq, k=1)       # k random elements (with replacement)
random.shuffle(lst)            # shuffle list in-place
random.sample(seq, k)          # k unique elements (without replacement)
random.gauss(mu=0.0, sigma=1.0) # Gaussian (normal) distribution
```

### Examples

```python
import random

random.seed(42)
print(random.random())       # repeatable: 0.something
print(random.randint(1, 6))  # dice roll

deck = list(range(52))
random.shuffle(deck)

items = ["a", "b", "c", "d"]
print(random.choice(items))         # random element
print(random.sample(items, 2))      # 2 unique elements
print(random.choices(items, k=3))   # 3 with replacement

# Monte Carlo pi approximation
import math
hits = sum(1 for _ in range(100000)
           if math.hypot(random.random(), random.random()) < 1.0)
print(4 * hits / 100000)   # ≈ 3.14
```

---

## threading

```python
import threading
```

Full documentation: [THREADING.MD](THREADING.MD)

### Quick Reference

```python
# Thread
t = threading.Thread(target=fn, args=(a, b))
t.start(); t.join(); t.is_alive()

# Lock
lock = threading.Lock()
with lock: ...

# RLock (reentrant)
rlock = threading.RLock()

# Condition
cond = threading.Condition()
with cond: cond.wait(); cond.notify(); cond.notify_all()

# Semaphore
sem = threading.Semaphore(n)
with sem: ...

# Barrier
barrier = threading.Barrier(n)
barrier.wait()   # blocks until n threads have called wait()
```

---

## Future Stdlib (Post-MVP)

| Module | Contents |
|--------|----------|
| `re` | Regular expressions (Rust `regex` crate) |
| `collections` | `deque`, `Counter`, `OrderedDict`, `defaultdict` |
| `itertools` | `chain`, `product`, `permutations`, `combinations` |
| `functools` | `reduce`, `partial`, `lru_cache` |
| `pathlib` | Object-oriented path manipulation |
| `datetime` | Date and time types |
| `struct` | Binary packing/unpacking |
| `net` | TCP/UDP sockets |
| `raylib` | Graphics/game bindings |
