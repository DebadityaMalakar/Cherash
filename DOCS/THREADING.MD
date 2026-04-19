# Cherash Threading Model

Cherash uses real OS threads with no GIL. This document explains the threading model, the synchronization primitives, and the internal implementation.

---

## Design Principles

| Principle | Decision |
|-----------|----------|
| Threads | Real OS threads via Rust's `std::thread` |
| GIL | None — threads run in parallel |
| Shared memory | Allowed — protected by explicit locks |
| Memory model | Java-style: explicit mutual exclusion |
| Value safety | All mutable heap values wrapped in `Arc<Mutex<T>>` |

The threading model is deliberately close to Java's: powerful and correct when used carefully, and your fault when it isn't.

---

## Thread Creation

```python
import threading

def worker(n: int) -> None:
    print(f"thread {n} running")

t = threading.Thread(target=worker, args=(1,))
t.start()
t.join()
```

Multiple threads:

```python
threads = [threading.Thread(target=worker, args=(i,)) for i in range(10)]

for t in threads:
    t.start()
for t in threads:
    t.join()
```

---

## Synchronization Primitives

### Lock

Mutual exclusion. Only one thread can hold the lock at any time. Any other thread calling `acquire()` will block until the lock is released.

```python
lock = threading.Lock()

# Manual acquire/release
lock.acquire()
try:
    shared_list.append(value)
finally:
    lock.release()

# Preferred — context manager, exception-safe
with lock:
    shared_list.append(value)
```

**Internal implementation:** Backed by Rust's `std::sync::Mutex<()>`. The guard is held for the duration of the `with` block.

### RLock (Reentrant Lock)

A reentrant lock that can be acquired multiple times by the same thread. Useful when a locked function calls another function that also tries to acquire the same lock.

```python
rlock = threading.RLock()

def outer():
    with rlock:
        inner()   # safe — same thread

def inner():
    with rlock:   # would deadlock with Lock, fine with RLock
        do_work()
```

**Internal implementation:** Backed by a `Mutex` with a thread-id counter tracking acquisition depth.

### Condition

A condition variable for complex wait/notify coordination. Used when one thread needs to wait for a state change produced by another.

```python
cond = threading.Condition()
queue = []

def producer():
    with cond:
        queue.append(item)
        cond.notify()        # wake one waiting consumer

def consumer():
    with cond:
        while len(queue) == 0:
            cond.wait()      # release lock and sleep
        item = queue.pop(0)
```

`Condition.wait()` atomically releases the underlying lock and suspends the thread. When notified, it reacquires the lock before returning.

Methods:
- `cond.wait()` — wait indefinitely
- `cond.wait(timeout)` — wait up to `timeout` seconds
- `cond.notify()` — wake one waiting thread
- `cond.notify_all()` — wake all waiting threads

### Semaphore

A counting semaphore that allows at most `n` threads to be in the critical section simultaneously.

```python
sem = threading.Semaphore(3)   # 3 concurrent accesses allowed

with sem:
    access_limited_resource()
```

A semaphore with count 1 behaves like a `Lock` but is not reentrant and is not tied to a specific thread (any thread can release it).

### Barrier

Synchronizes `n` threads at a common point. All threads block at `barrier.wait()` until all `n` have arrived.

```python
barrier = threading.Barrier(4)

def phase_worker():
    do_phase_one()
    barrier.wait()    # all 4 must reach here
    do_phase_two()    # all start phase two together
```

Barriers are useful for parallel algorithms with synchronization points between phases.

---

## Producer-Consumer Example

The canonical Phase 7 exit criterion:

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

This program is deterministic: every integer `i` in `[0, 9]` is squared by exactly one worker, the lock prevents concurrent writes to `results`, and `join()` ensures all results are collected before sorting.

---

## Shared State and Safety

Any mutable value shared between threads must be protected. The three patterns:

### Pattern 1: Lock-protected list

```python
results = []
lock = threading.Lock()

def add(v):
    with lock:
        results.append(v)
```

### Pattern 2: Condition-based queue

```python
queue = []
cond = threading.Condition()

def enqueue(v):
    with cond:
        queue.append(v)
        cond.notify()

def dequeue():
    with cond:
        while not queue:
            cond.wait()
        return queue.pop(0)
```

### Pattern 3: Thread-local data

If each thread only reads/writes its own data, no synchronization is needed:

```python
def worker(n):
    local = []          # each thread has its own list
    for i in range(n):
        local.append(i * i)
    return local        # no sharing, no lock
```

---

## What Requires a Lock

| Shared value | Access type | Lock needed? |
|-------------|-------------|--------------|
| `list` | Any write (`append`, `pop`, assign) | Yes |
| `list` | Read-only when no writers running | No |
| `dict` | Any write | Yes |
| `int` | Read (Python int is immutable) | No, but: |
| `int` (rebinding) | `counter += 1` is read-then-write | Yes |
| `str` | Any — immutable | No |
| `tuple` | Any — immutable | No |

The key rule: **if any thread writes, all threads that access that value need a lock**.

---

## Deadlock Avoidance

Deadlock occurs when thread A holds lock X and waits for lock Y, while thread B holds lock Y and waits for lock X.

Rules to avoid deadlock:

1. **Consistent lock ordering.** If you acquire multiple locks, always acquire them in the same order across all threads.
2. **No lock across blocking calls.** Don't hold a lock while calling `t.join()`, `cond.wait()`, or any I/O.
3. **Prefer `with` over manual acquire/release.** It guarantees release even on exception.
4. **Keep critical sections short.** Do not do complex computation inside a lock.

---

## Internal Implementation (Rust)

### Value thread safety

All mutable heap values in the `Value` enum are wrapped in `Arc<Mutex<T>>`:

```rust
Value::List(Arc<Mutex<Vec<Value>>>)
Value::Dict(Arc<Mutex<HashMap<HashableValue, Value>>>)
Value::Set(Arc<Mutex<HashSet<HashableValue>>>)
Value::Instance(Arc<Mutex<Instance>>)
```

`Arc` enables shared ownership across threads (reference counted, atomic). `Mutex` serializes access. `Value::clone()` is always `O(1)` — just a reference count increment.

### Thread spawning (Phase 7 plan)

Each spawned thread gets:
- Its own `Evaluator` instance (stack, local environment)
- Shared access to global `Value`s via `Arc` clones
- The function to call and its arguments (already cloned before spawn)

```rust
let target_fn = thread_val.target.clone();   // Arc<Function>
let args = thread_val.args.clone();           // Vec<Value> — Arcs inside

std::thread::spawn(move || {
    let mut ev = Evaluator::new();
    ev.call_value(Value::Function(target_fn), args)
        .unwrap_or(Value::None);
});
```

### Lock implementation

`threading.Lock` is backed by `Arc<Mutex<()>>`. The Python-facing `acquire()`/`release()` maps to locking/unlocking the mutex. The guard is stored in the `Instance` fields as a raw pointer during the hold period (or managed via RAII in the Rust layer).

---

## Comparison with Python's threading

| Feature | CPython threading | Cherash threading |
|---------|-------------------|-------------------|
| GIL | Yes — only one thread runs Python at a time | No — true parallelism |
| Thread type | OS thread wrapped in GIL | Raw OS thread |
| Parallelism | CPU-bound: no. IO-bound: yes | CPU-bound: yes |
| Lock | `threading.Lock` | `threading.Lock` (same API) |
| Condition | `threading.Condition` | `threading.Condition` (same API) |
| Semaphore | `threading.Semaphore` | `threading.Semaphore` (same API) |
| Barrier | `threading.Barrier` | `threading.Barrier` (same API) |
| Safety | GIL prevents many races | Explicit locks required |

Because Cherash has no GIL, programs that would run correctly in CPython due to the GIL's implicit serialization **may have data races in Cherash** if they don't use explicit locks. This is intentional — the Cherash design philosophy is that concurrency correctness is the programmer's responsibility.
