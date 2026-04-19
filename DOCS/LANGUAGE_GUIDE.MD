# Cherash Language Guide

> Python's face. C's soul. Go's memory. Java's threads.

Cherash is a general-purpose programming language with Python syntax, C integer semantics, Go's concurrent GC, and Java's threading model. This guide covers everything you need to write Cherash programs.

---

## File Extensions

| Extension | Behavior |
|-----------|----------|
| `.py` | Compatibility mode. Dynamic typing tolerated. Existing Python scripts run as-is. |
| `.chsh` | Full Cherash mode. Strict type checking enforced. |

A `.py` file can opt into strict mode with a header directive:

```python
# cherash:strict
```

---

## Running Programs

```bash
cherash run script.py       # compatibility mode
cherash run script.chsh     # strict mode
cherash check script.chsh   # type-check only, no execution
cherash repl                # interactive REPL
```

---

## Types

### Primitives

| Type | Description | Literal |
|------|-------------|---------|
| `int` | 64-bit signed integer, wraps on overflow | `42`, `0xFF`, `0o17`, `0b1010` |
| `float` | 64-bit IEEE 754 double | `3.14`, `1e-10`, `.5` |
| `bool` | `True` or `False`. Not an alias for int. | `True`, `False` |
| `str` | Immutable UTF-8 string | `'hi'`, `"hi"`, `"""hi"""` |
| `bytes` | Immutable byte array | `b'raw'`, `b"\x00\x01"` |
| `None` | Unit type | `None` |

### Integer Overflow

Overflow wraps silently — C-style. No exceptions, no BigInt.

```python
x: int = 9223372036854775807  # i64::MAX
x = x + 1                     # -9223372036854775808 (i64::MIN)
```

### Collections

| Type | Description |
|------|-------------|
| `list[T]` | Dynamic homogeneous array |
| `dict[K, V]` | Hash map |
| `set[T]` | Unordered unique collection |
| `tuple[T, ...]` | Heterogeneous fixed-size sequence |

```python
nums: list[int]      = [1, 2, 3]
ages: dict[str, int] = {'alice': 30, 'bob': 25}
primes: set[int]     = {2, 3, 5, 7}
point: tuple[int, int] = (10, 20)
```

### Type Annotations

In `.chsh` files, annotations are enforced at check time:

```python
name: str = "cherash"
count: int = 0

def greet(name: str) -> str:
    return "hello " + name
```

In `.py` files, annotations are parsed but ignored.

---

## Control Flow

### if / elif / else

```python
x = 10
if x > 20:
    print("big")
elif x > 5:
    print("medium")
else:
    print("small")
```

### while

```python
i = 0
while i < 10:
    i += 1
```

### for

```python
for item in [1, 2, 3]:
    print(item)

for i in range(10):
    print(i)

for i in range(0, 10, 2):   # step
    print(i)
```

### for / else

The `else` clause runs only if the loop was not exited via `break`:

```python
for x in items:
    if x == target:
        break
else:
    print("not found")
```

### break / continue / pass

```python
for i in range(10):
    if i == 5:
        break
    if i % 2 == 0:
        continue
    pass  # no-op
```

---

## Functions

```python
def add(a: int, b: int) -> int:
    return a + b
```

### Default Arguments

```python
def greet(name: str, prefix: str = "Hello") -> str:
    return prefix + ", " + name
```

### Keyword-only parameters

Parameters after `*` can only be supplied as keyword arguments:

```python
def create(name: str, *, verbose: bool = False) -> None:
    if verbose:
        print(f"Creating {name}")
```

### *args and **kwargs

```python
def total(*nums: int) -> int:
    return sum(nums)

def config(**opts):
    for k, v in opts.items():
        print(k, "=", v)

# Both:
def mixed(a, b, *args, **kwargs):
    pass

mixed(1, 2, 3, 4, x=5, y=6)  # a=1, b=2, args=[3,4], kwargs={x:5, y:6}
```

### Keyword arguments in calls

Arguments can always be passed by name:

```python
def connect(host: str, port: int = 80, ssl: bool = False) -> None:
    pass

connect("example.com", ssl=True, port=443)
```

### Starred unpacking in assignment

```python
first, *rest = [1, 2, 3, 4, 5]
# first=1, rest=[2, 3, 4, 5]

a, *middle, z = [1, 2, 3, 4, 5]
# a=1, middle=[2, 3, 4], z=5

head, *_ = range(10)   # only the first element
```

### Lambda

```python
square = lambda x: x * x
sorted_items = sorted(data, key=lambda x: x.value)
```

---

## Closures and Scoping

Cherash uses Python's LEGB rules: **Local → Enclosing → Global → Builtin**.

```python
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

### global / nonlocal

```python
count = 0

def increment():
    global count
    count += 1
```

---

## Classes

```python
class Animal:
    def __init__(self, name: str):
        self.name = name

    def speak(self) -> str:
        return f"{self.name} makes a sound"


class Dog(Animal):
    def speak(self) -> str:
        return f"{self.name} barks"


d = Dog("Rex")
print(d.speak())  # Rex barks
```

### Dunder Methods

| Method | Triggered by |
|--------|-------------|
| `__init__(self, ...)` | `ClassName(...)` |
| `__str__(self)` | `str(obj)`, `print(obj)` |
| `__repr__(self)` | `repr(obj)` |
| `__len__(self)` | `len(obj)` |
| `__getitem__(self, key)` | `obj[key]` |
| `__setitem__(self, key, val)` | `obj[key] = val` |
| `__iter__(self)` | `for x in obj` |
| `__next__(self)` | iterator protocol |
| `__eq__(self, other)` | `==` |
| `__lt__(self, other)` | `<` |
| `__call__(self, ...)` | `obj(...)` |
| `__enter__` / `__exit__` | `with` statement |

---

## Exception Handling

```python
try:
    x = int("not a number")
except ValueError as e:
    print(f"Caught: {e}")
except (TypeError, RuntimeError) as e:
    raise RuntimeError("wrapped") from e
else:
    print("no exception")
finally:
    print("always runs")
```

### Raising Exceptions

```python
raise ValueError("bad input")
raise RuntimeError("failed") from original_exc
raise   # bare raise — re-raises current exception
```

### Exception Hierarchy

```
BaseException
└── Exception
    ├── TypeError
    ├── ValueError
    ├── RuntimeError
    ├── IOError
    ├── IndexError
    ├── KeyError
    ├── ZeroDivisionError
    ├── NameError
    ├── AttributeError
    ├── AssertionError
    ├── NotImplementedError
    ├── OverflowError
    ├── StopIteration
    ├── ImportError
    └── ThreadError
```

---

## Comprehensions

```python
squares     = [x * x for x in range(10)]
even_sq     = [x * x for x in range(10) if x % 2 == 0]
word_len    = {w: len(w) for w in ["hello", "world"]}
unique_vals = {x % 3 for x in range(9)}
```

---

## Context Managers

```python
with open("file.txt", "r") as f:
    content = f.read()
```

Custom context managers implement `__enter__` and `__exit__`:

```python
class Timer:
    def __enter__(self):
        self.start = time.time()
        return self
    def __exit__(self, *args):
        self.elapsed = time.time() - self.start
        return False
```

---

## Assertions

```python
assert x > 0, "x must be positive"
assert isinstance(name, str)
```

---

## String Formatting

```python
name = "world"
print(f"Hello, {name}!")
print(f"Result: {1 + 2 * 3}")
print(f"Pi is approximately {3.14159:.2f}")
```

---

## Operators

### Arithmetic

| Op | Description |
|----|-------------|
| `+` | Add / string concat |
| `-` | Subtract |
| `*` | Multiply / string repeat |
| `/` | True division (returns float) |
| `//` | Floor division |
| `%` | Modulo |
| `**` | Power |

### Comparison

`==`, `!=`, `<`, `>`, `<=`, `>=`, `is`, `is not`, `in`, `not in`

Comparisons can be chained: `1 < x < 10`

### Bitwise

`&`, `|`, `^`, `~`, `<<`, `>>`

### Augmented Assignment

`+=`, `-=`, `*=`, `/=`, `//=`, `%=`, `**=`, `&=`, `|=`, `^=`, `<<=`, `>>=`

### Walrus Operator

```python
if (n := len(data)) > 10:
    print(f"list is too long ({n} elements)")
```

---

## Imports

```python
import math
import sys
import json
import threading
import time
import random

from math import sqrt, pi
from os.path import join, exists

import math as m
from sys import argv as args

from os import *   # wildcard import
```

### Available modules

| Module | Description |
|--------|-------------|
| `math` | Mathematical functions and constants |
| `io` | File I/O (also via global `open()`) |
| `json` | JSON encode/decode |
| `os` | Operating system interface |
| `sys` | Interpreter state and process control |
| `time` | Time functions |
| `random` | Pseudo-random number generation |
| `threading` | Real OS threads and synchronization |

---

## Built-in Functions

| Function | Description |
|----------|-------------|
| `print(*args, sep=' ', end='\n')` | Print to stdout |
| `len(obj)` | Length of sequence |
| `range(stop)` / `range(start, stop, step)` | Integer range |
| `int(x)` / `float(x)` / `str(x)` / `bool(x)` | Type conversion |
| `type(obj)` | Type name as string |
| `isinstance(obj, cls)` | Type check |
| `abs(x)` | Absolute value |
| `max(...)` / `min(...)` | Maximum / minimum |
| `sum(iterable)` | Sum of elements |
| `sorted(iterable, key=None, reverse=False)` | Sorted copy |
| `reversed(seq)` | Reversed iterator |
| `enumerate(iterable, start=0)` | Index + value pairs |
| `zip(*iterables)` | Zip multiple iterables |
| `map(fn, iterable)` | Apply fn to each element |
| `filter(fn, iterable)` | Keep elements where fn is True |
| `list(iterable)` / `tuple(...)` / `set(...)` / `dict(...)` | Collection constructors |
| `ord(c)` / `chr(n)` | Character ↔ code point |
| `hex(n)` / `oct(n)` / `bin(n)` | Number base conversions |
| `round(x, n=0)` | Round to n decimal places |
| `divmod(a, b)` | `(a // b, a % b)` |
| `pow(x, y)` | `x ** y` |
| `input(prompt='')` | Read line from stdin |
| `repr(obj)` | Developer-readable string |
| `hash(obj)` | Hash value |
| `callable(obj)` | Is object callable |
| `id(obj)` | Object identity (GC handle address) |
| `open(path, mode='r')` | Open a file → TextIOWrapper |
| `iter(obj)` / `next(it)` | Iterator protocol |
| `hasattr(obj, name)` / `getattr(obj, name)` / `setattr(obj, name, val)` | Attribute access |
| `vars(obj)` | Object's `__dict__` |
| `dir(obj)` | List attribute names |
| `issubclass(cls, parent)` | Subclass check |
| `all(iterable)` / `any(iterable)` | Boolean aggregation |
| `format(val, spec)` | Format string |
| `bytes(...)` / `bytearray(...)` | Bytes construction |
| `memoryview(...)` | Memory view |
