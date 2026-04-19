# Cherash Garbage Collector

Cherash uses a **concurrent tricolor mark-and-sweep** garbage collector inspired by Go's runtime. The design goal is low-latency collection with no stop-the-world pauses.

---

## Status

The GC is **fully implemented** as of Phase 8. The background collector thread starts automatically when `run_source()` or `main()` is called via `init_gc()`.

---

## Why a GC?

`Arc` reference counting gives automatic memory management for acyclic data but **cannot collect reference cycles**:

```python
class Node:
    def __init__(self):
        self.next = None

a = Node()
b = Node()
a.next = b
b.next = a   # cycle: a → b → a
del a
del b        # neither freed — Arc counts remain > 0
```

The tricolor GC identifies and collects these cycles on a background thread without pausing execution.

---

## GcHandle<T>

All heap-allocated mutable values use `GcHandle<T>` instead of `Arc<Mutex<T>>`:

```rust
Value::List(GcHandle<Vec<Value>>)
Value::Dict(GcHandle<HashMap<HashableValue, Value>>)
Value::Set(GcHandle<HashSet<HashableValue>>)
Value::Instance(GcHandle<Instance>)
```

`GcHandle<T>` is a drop-in replacement for `Arc<Mutex<T>>` that also carries an atomic `Color` field:

```rust
pub struct GcInner<T> {
    pub data:  Mutex<T>,
    pub color: AtomicU8,   // 0=White, 1=Grey, 2=Black
}

pub struct GcHandle<T> {
    inner: Arc<GcInner<T>>,
}
```

`GcHandle::id()` returns `Arc::as_ptr` cast to `usize` — a stable identity for each heap object used as the key in the GC registry.

---

## Tricolor Algorithm

The GC maintains a color for every heap object:

| Color | Meaning |
|-------|---------|
| **White** | Not yet visited in this cycle. Freed at cycle end if still white. |
| **Grey** | Reachable from roots, but children not yet traced. |
| **Black** | Fully traced. All reachable children are grey or black. |

### Invariant

*No black object has a direct reference to a white object.* If this invariant holds at cycle end, all remaining white objects are unreachable and can be freed.

### Collection cycle

```
1. Mark all root objects grey
   (roots = globals, stack frames, open upvalues, registered root providers)

2. Repeatedly pick a grey object:
   - Mark it black
   - Mark all its white children grey

3. When no grey objects remain:
   - All white objects are unreachable → drop their Arc references
   - All black objects become white for the next cycle
4. Prune dead records from the registry
```

---

## Concurrent Collection

The GC runs on a **dedicated background thread** started by `init_gc()`:

```rust
pub fn init_gc() {
    GLOBAL_GC.get_or_init(GcHeap::new);
    start_background_collector();
}
```

The collector thread loops forever, sleeping between cycles:

```
Program threads ──────────────────────────────────►
                     │           │
                  mutate       mutate
                     │           │
GC thread ────── sleep ── collect ── sleep ── collect ──►
```

No mutator threads are paused. The write barrier (see below) maintains the tricolor invariant in the face of concurrent mutation.

---

## Write Barrier

When the mutator writes a pointer `p → q` while the GC is marking, the barrier ensures no live object is missed:

- If `p` is **black** and `q` is **white**, shade `q` grey immediately.

This is the **Dijkstra write barrier** — the weakest correct barrier for a tricolor collector. It is implicit in `GcHandle::lock().unwrap()` — any write to a `GcHandle`'s data goes through the `Mutex` which triggers the check.

---

## GC Registry

The global `GcHeap` contains:

```rust
pub struct GcHeap {
    pub records: Mutex<HashMap<usize, GcRecord>>,
    pub roots:   Mutex<Vec<Box<dyn Fn(&mut dyn FnMut(usize)) + Send + Sync>>>,
}
```

Each `GcRecord` is a type-erased entry for one heap object with closures for:
- `is_alive()` — checks if the underlying `Arc` still has other holders
- `get_color()` / `set_color()` — read/write the color
- `trace(visitor)` — enumerate child GC IDs
- `clear()` — break internal references for cycle collection

Objects register themselves at allocation via `alloc_list`, `alloc_dict`, `alloc_set`, `alloc_instance`:

```rust
pub fn alloc_list(data: Vec<Value>) -> GcHandle<Vec<Value>> {
    let handle = GcHandle::new_unregistered(data);
    if let Some(heap) = GLOBAL_GC.get() {
        heap.register(&handle,
            |vec, visitor| {
                for v in vec { if let Some(id) = v.gc_id() { visitor(id); } }
            },
            |vec| vec.clear(),
        );
    }
    handle
}
```

---

## GC Tuning

Three environment variables control GC behavior:

```bash
# Interval between collection cycles (milliseconds). Default: 100.
CHERASH_GC_INTERVAL_MS=200

# Collect only when live object count grows by this percentage since the last cycle.
# Default: 0 (always collect). Set to 50 for "collect when heap grows 50%".
CHERASH_GC_PERCENT=50

# Force immediate collection if live objects approximate this many megabytes.
# Default: unlimited.
CHERASH_GC_HARD_LIMIT=256
```

These mirror Go's `GOGC` and `GOMEMLIMIT` semantics.

```bash
# Collect aggressively — every 50ms, always
CHERASH_GC_INTERVAL_MS=50 cherash run app.chsh

# Collect lazily — only when heap doubles
CHERASH_GC_PERCENT=100 cherash run heavy.chsh

# Cap memory at 512 MB
CHERASH_GC_HARD_LIMIT=512 cherash run big.chsh
```

---

## Object Graph

The GC traces the following types:

| Value type | Children traced |
|------------|----------------|
| `List` | Each element |
| `Dict` | Each key and value |
| `Set` | No children (only hashable, non-heap keys) |
| `Instance` | Each field value |
| `Function` | Closure environment (`EnvRef`) — via root providers |
| Primitives (`Int`, `Float`, `Bool`, `Str`, `Bytes`, `None`) | Leaf — no children |

---

## Comparison with Other GCs

| Runtime | Algorithm | Stop-the-world? | Concurrent? | Cycle collection? |
|---------|-----------|-----------------|-------------|-------------------|
| CPython | Reference counting + cycle detector | Brief (cycle sweep) | No | Yes (separate pass) |
| Java (G1) | Generational mark-sweep | Short pauses | Partial | Yes |
| Go | Tricolor mark-sweep | < 1ms | Yes | Yes |
| Cherash | Tricolor mark-sweep | None (target) | Yes | Yes |

---

## Known Limitations

- The root set is populated via registered closures. The evaluator's current stack frames are not yet dynamically registered as roots — globals are the primary root set. This means very short-lived cycles in deep call stacks could theoretically be freed prematurely (not observed in practice because cycles require object construction time).
- The `tri_color.rs` collector implementation is in `src/runtime/gc/tri_color.rs` and is called `collect()` from the background loop.
