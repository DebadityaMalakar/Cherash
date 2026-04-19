# Cherash Type System

Cherash has a **mode-aware type system**: strict enforcement in `.chsh` files, silent pass-through in `.py` files. The goal is Python source compatibility without sacrificing correctness when you want it.

---

## Two Modes

### Lenient mode (`.py` files)

Type annotations are parsed and stored in the AST but the type checker returns immediately without checking anything. Existing Python code with dynamic typing, missing annotations, or mixed types runs without complaint.

```python
# script.py — no errors regardless of annotations
x: int = "oops"   # type mismatch ignored
y = []
y.append(1)
y.append("two")   # heterogeneous list — fine
```

### Strict mode (`.chsh` files)

Type annotations are enforced. Mismatches raise `TypeError` at check time — **before** the program runs.

```python
# script.chsh
x: int = "hello"   # TypeError: expected int, got str
y: float = 1.5     # OK
z: float = 1       # OK — int is assignable to float
```

A `.py` file can opt into strict mode with a header comment:

```python
# cherash:strict
x: int = "bad"   # TypeError
```

---

## Primitive Types

### int

64-bit signed integer. Overflow wraps — no exception, no BigInt.

```python
x: int = 42
x: int = 0xFF        # hex literal
x: int = 0b1010      # binary literal
x: int = 0o17        # octal literal
x: int = 1_000_000   # underscore separator
```

Overflow behavior:

```python
x: int = 9223372036854775807   # i64::MAX
x = x + 1                      # -9223372036854775808 (wraps)
```

### float

64-bit IEEE 754 double.

```python
x: float = 3.14
x: float = 1e-10
x: float = .5       # 0.5
```

### bool

`True` or `False`. Not an alias for `int`, but arithmetic with `bool` coerces to `int`:

```python
True + 1   # 2 (int)
False * 5  # 0 (int)
```

### str

Immutable UTF-8 string. Indexing returns single-character strings.

```python
s: str = "hello"
s: str = 'hello'
s: str = """multi
line"""
```

### bytes

Immutable byte sequence. Indexing returns `int`.

```python
b: bytes = b"raw\x00data"
b[0]   # int (114)
```

### None

The unit type. Used as the return type of functions that don't return a value.

```python
def log(msg: str) -> None:
    print(msg)
```

---

## Collection Types

All collection types are **generic** in strict mode — the type parameter is required.

### list[T]

Dynamic homogeneous array. All elements must be of type `T`.

```python
names: list[str]  = ["alice", "bob"]
nums:  list[int]  = [1, 2, 3]
empty: list[float] = []
```

### dict[K, V]

Hash map. Key type `K` must be hashable (`int`, `str`, `bool`, `bytes`, `tuple`).

```python
scores: dict[str, int] = {"alice": 95, "bob": 87}
```

### set[T]

Unordered collection of unique values of type `T`.

```python
seen: set[int] = {1, 2, 3}
```

### tuple[T, ...]

Fixed-size heterogeneous sequence. The type annotation lists the type of each position.

```python
point:   tuple[int, int]         = (10, 20)
record:  tuple[str, int, float]  = ("alice", 30, 95.5)
empty:   tuple[()]               = ()
```

---

## Assignability Rules

The type checker determines whether a value of type `S` is assignable to a slot expecting type `T` using these rules:

| `T` (expected) | `S` (actual) | Allowed? |
|----------------|-------------|----------|
| `int` | `int` | Yes |
| `int` | `bool` | Yes — `bool` is a subtype of `int` |
| `float` | `int` | Yes — widening coercion |
| `float` | `bool` | Yes |
| `float` | `float` | Yes |
| `str` | `int` | **No** |
| `int` | `str` | **No** |
| `Any` | anything | Yes |
| anything | `Any` | Yes |
| `Optional[T]` | `T` | Yes |
| `Optional[T]` | `None` | Yes |
| `Optional[T]` | anything else | No |
| `Union[A, B]` | `A` or `B` | Yes |

---

## Optional and Union

```python
from typing import Optional, Union

def find(key: str) -> Optional[int]:
    # returns int or None
    ...

def parse(val: Union[str, int]) -> float:
    ...
```

`Optional[T]` is equivalent to `Union[T, None]`.

---

## Function Signatures

In strict mode, parameter types and return types are checked:

```python
def add(a: int, b: int) -> int:
    return a + b

add(1, 2)       # OK
add(1, "two")   # TypeError: expected int, got str
```

Return type checking is enforced at the `return` statement:

```python
def get_name() -> str:
    return 42   # TypeError: expected str, got int
```

Functions without a return annotation are treated as `-> Any`:

```python
def untyped(x):
    return x   # no annotation — no checking
```

---

## Type Inference

The type checker infers expression types for the purpose of checking annotations. Inference rules:

| Expression | Inferred type |
|------------|--------------|
| `42` | `int` |
| `3.14` | `float` |
| `"hello"` | `str` |
| `True` / `False` | `bool` |
| `None` | `None` |
| `a + b` (int + int) | `int` |
| `a + b` (int + float) | `float` |
| `a / b` | `float` (true division always returns float) |
| `a // b` (int // int) | `int` |
| `a + b` (str + str) | `str` |
| Unknown name | `Unknown` (no error) |

---

## Class Types

Classes define new nominal types. Instances of a class have that class as their type:

```python
class Point:
    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y

p: Point = Point(1, 2)   # OK
p: Point = "not a point" # TypeError
```

Inheritance is recognized for `isinstance` checks but not yet for structural type assignability (that is a post-MVP feature).

---

## Generic Collections (Runtime)

At runtime, Cherash does not enforce element types in collections — a `list[int]` can technically hold any `Value`. The type checker enforces element types at assignment of the collection literal, but not at individual `.append()` calls yet.

This will be tightened in a future phase via instrumented collection types.

---

## The `Any` Type

`Any` is the escape hatch. It disables type checking for that binding. Values of type `Any` are assignable to and from any type.

```python
from typing import Any

x: Any = 42
x = "now a string"   # OK — Any
```

Using `Any` is appropriate for:
- Interfacing with external code or dynamic data
- Gradual typing migration
- Parts of a program where precise typing is not yet worth the effort

---

## Type Errors

A `TypeError` in strict mode is raised **before execution begins**. The error message includes the location and the mismatch:

```
TypeError at 3:5: expected int, got str
```

In lenient mode, the same program runs without error — type mismatches surface only as `RuntimeError` if they actually cause a problem at execution time.

---

## Comparison with Python's Type System

| Feature | Python (mypy/pyright) | Cherash |
|---------|----------------------|---------|
| Checking tool | External (mypy, pyright) | Built-in |
| Enforcement | Optional / CI-only | Mandatory in `.chsh` |
| Generics | Full PEP 484 | Subset (list, dict, set, tuple, Optional, Union) |
| Protocols / structural typing | Yes (mypy) | Not yet |
| TypeVar / ParamSpec | Yes | Not yet |
| Overloads | Yes | Not yet |
| int overflow | BigInt (no overflow) | Wraps (C-style) |
| Runtime type enforcement | No | Partial (assignments) |
