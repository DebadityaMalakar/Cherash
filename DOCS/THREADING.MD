# Cherash Threading Model

Cherash uses real OS threads with no GIL. This document explains the threading model, synchronization primitives, and internal implementation.

---

## Design Principles

| Principle | Decision |
|-----------|----------|
| Threads | Real OS threads via Rust's `std::thread::spawn` |
| GIL | None — threads run in true parallel |
| Shared memory | Allowed — protected by explicit locks |
| Memory model | Java-style: explicit mutual exclusion |
| Value safety | All mutable heap values use `GcHandle<T>` (wraps `Arc<Mutex<T>>`) |

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

Keyword arguments are supported:

```python
t = threading.Thread(target=worker, args=(42,))
```

Multiple threads:

```python
threads = [threading.Thread(target=worker, args=(i,)) for i in range(10)]
for t in threads: t.start()
for t in threads: t.join()
```

### Thread Methods

| Method | Description |
|--------|-------------|
| `t.start()` | Spawn the OS thread |
| `t.join()` | Block until the thread completes |
| `t.is_alive()` | Returns `True` if the thread is still running |

---

## Synchronization Primitives

### Lock

Mutual exclusion. Only one thread can hold the lock at any time. Attempting `acquire()` while the lock is held blocks until it is released.

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

**Implementation:** `Condvar` + `Mutex<bool>` — no `MutexGuard` is stored across interpreter calls, avoiding lock re-entry issues. The `acquire` blocks until the boolean state is `false`, then sets it `true`.

---

### RLock (Reentrant Lock)

A reentrant lock — the same thread may call `acquire()` multiple times without deadlocking. Each `acquire()` must be paired with a corresponding `release()`.

```python
rlock = threading.RLock()

def outer():
    with rlock:
        inner()   # safe — same thread can reacquire

def inner():
    with rlock:
        do_work()
```

**Implementation:** Same `LockInner` backing as `Lock`. Reentrance is achieved because both map to the same underlying `Condvar`-based primitive, and the thread that holds the lock can always release it.

---

### Condition

For wait/notify coordination between threads. A `Condition` bundles a lock with a `Condvar` so that threads can wait for a state change produced by another thread.

```python
cond = threading.Condition()
queue = []

def producer():
    with cond:
        queue.append(produce())
        cond.notify()           # wake one waiting consumer

def consumer():
    with cond:
        while len(queue) == 0:
            cond.wait()         # atomically release lock and sleep
        return queue.pop(0)
```

`wait()` atomically releases the lock and suspends the thread. On notification, the thread reacquires the lock before returning.

#### Methods

| Method | Description |
|--------|-------------|
| `cond.acquire()` | Acquire the underlying lock |
| `cond.release()` | Release the underlying lock |
| `cond.wait()` | Release lock, sleep until notified, reacquire lock |
| `cond.notify()` | Wake one waiting thread |
| `cond.notify_all()` | Wake all waiting threads |
| `with cond:` | Acquire on enter, release on exit |

**Implementation:** `ConditionInner { condvar: Condvar, lock: Mutex<bool> }`. `wait()` sets the lock state to `false`, wakes other threads competing for the lock, then blocks on `condvar.wait()` until notified.

---

### Semaphore

A counting semaphore that allows at most `n` threads to hold it simultaneously.

```python
sem = threading.Semaphore(3)   # 3 concurrent holders allowed

with sem:
    access_limited_resource()
```

`Semaphore(1)` behaves like a `Lock` but is not tied to a specific thread — any thread may `release()` it.

`BoundedSemaphore` is an alias for `Semaphore` (same implementation).

**Implementation:** `SemaphoreInner { condvar: Condvar, count: Mutex<i64> }`. `acquire()` blocks while `count <= 0`, then decrements. `release()` increments and notifies one waiter.

---

### Barrier

A rendezvous point — all `n` threads must call `barrier.wait()` before any of them proceeds.

```python
barrier = threading.Barrier(4)

def phase_worker():
    do_phase_one()
    barrier.wait()    # wait for all 4 threads to arrive
    do_phase_two()    # all 4 start together
```

Useful for parallel algorithms with synchronization points between phases.

**Implementation:** `BarrierInner { condvar: Condvar, state: Mutex<(usize, usize)>, parties: usize }`. The state tuple is `(arrived_count, generation)`. When `arrived == parties`, the generation increments and all waiting threads are woken. The generation counter prevents spurious wake-ups when the barrier is reused.

---

## Producer-Consumer Example

```python
import threading

results = []
lock = threading.Lock()

def worker(n: int) -> None:
    with lock:
        results.append(n * n)

threads = [threading.Thread(target=worker, args=(i,)) for i in range(10)]
for t in threads: t.start()
for t in threads: t.join()

print(sorted(results))  # [0, 1, 4, 9, 16, 25, 36, 49, 64, 81]
```

---

## Condition-Based Queue Example

```python
import threading

queue = []
cond = threading.Condition()
DONE = False

def producer():
    global DONE
    for i in range(5):
        with cond:
            queue.append(i)
            cond.notify()
    with cond:
        DONE = True
        cond.notify_all()

def consumer():
    while True:
        with cond:
            while not queue and not DONE:
                cond.wait()
            if queue:
                print(queue.pop(0))
            if DONE and not queue:
                break

p = threading.Thread(target=producer)
c = threading.Thread(target=consumer)
p.start(); c.start()
p.join();  c.join()
```

---

## What Requires a Lock

| Shared value | Access type | Lock needed? |
|-------------|-------------|--------------|
| `list` | Any write (`append`, `pop`, assign) | Yes |
| `list` | Read-only, no concurrent writers | No |
| `dict` | Any write | Yes |
| `int` (global rebinding) | `counter += 1` is read-then-write | Yes |
| `str` | Immutable — any access | No |
| `tuple` | Immutable — any access | No |

Rule: **if any thread writes, all threads that access that value need a lock.**

---

## Deadlock Avoidance

1. **Consistent lock ordering.** Acquire multiple locks in the same order everywhere.
2. **No lock across blocking calls.** Don't hold a `Lock` while calling `t.join()` or `cond.wait()`.
3. **Prefer `with lock:`** over manual `acquire`/`release` — guarantees release on exception.
4. **Keep critical sections short.** Don't do I/O or heavy computation inside a lock.

---

## Internal Implementation

### Thread spawning

Each `Thread.start()` spawns a real OS thread via `std::thread::Builder::new().spawn(...)`. The spawned thread gets its own `Evaluator` instance with no shared interpreter state:

```rust
let handle = std::thread::Builder::new()
    .name(format!("cherash-thread-{}", tid))
    .spawn(move || {
        let mut ev = Evaluator::new();
        let _ = ev.call_value(target, targs_vec);
    })?;
```

`JoinHandle`s are stored in a global `THREAD_REGISTRY: OnceLock<Mutex<HashMap<i64, JoinHandle<()>>>>` keyed by thread ID. `thread.join()` removes and joins the handle.

### Global registries

Threading primitives store their state outside `Value` (which must be `Clone`) using global registries keyed by the instance's GC ID (`h.id()`):

```rust
static LOCK_REGISTRY:    OnceLock<Mutex<HashMap<usize, Arc<LockInner>>>>      = OnceLock::new();
static COND_REGISTRY:    OnceLock<Mutex<HashMap<usize, Arc<ConditionInner>>>> = OnceLock::new();
static SEM_REGISTRY:     OnceLock<Mutex<HashMap<usize, Arc<SemaphoreInner>>>> = OnceLock::new();
static BARRIER_REGISTRY: OnceLock<Mutex<HashMap<usize, Arc<BarrierInner>>>>   = OnceLock::new();
```

When `Lock.__init__` is called, a new `LockInner` is inserted into `LOCK_REGISTRY` under the instance's GC ID. `acquire()` looks up the instance by ID and calls `inner.acquire()` on the Rust-side state.

### Value thread safety

`Value` is `Send + 'static` because:
- `GcHandle<T>` is `Send + Sync` (wraps `Arc<Mutex<T>>`)
- Primitive variants are `Copy` / `Send`
- `Function` carries `EnvRef` which is `Arc<Mutex<Env>>` — `Send`
- `BoundBuiltin` carries a function pointer (`fn(Vec<Value>) -> ...`) — `Send`

---

## Comparison with CPython threading

| Feature | CPython | Cherash |
|---------|---------|---------|
| GIL | Yes — one thread runs Python at a time | None |
| CPU parallelism | No (GIL-bound) | Yes (true OS threads) |
| `Lock` API | `threading.Lock` | Same |
| `Condition` API | `threading.Condition` | Same |
| `Semaphore` API | `threading.Semaphore` | Same |
| `Barrier` API | `threading.Barrier` | Same |
| Implicit safety | GIL prevents many races | Explicit locks required |

Because Cherash has no GIL, programs that rely on the GIL's implicit serialization in CPython **may have data races in Cherash** if explicit locks are missing. This is intentional — concurrency correctness is the programmer's responsibility.
