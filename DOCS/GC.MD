# Cherash Garbage Collector

Cherash uses a **concurrent tricolor mark-and-sweep** garbage collector inspired by Go's runtime. The design goal is low-latency collection with no stop-the-world pauses.

> **Current status:** Phase 1–6 use Rust's `Arc` reference counting as a stop-gap. The full concurrent GC described here is the Phase 8 target.

---

## Why a GC?

The current `Arc<Mutex<T>>` model gives automatic memory management — values are freed when the last reference is dropped. This is correct for acyclic data but **cannot collect reference cycles**:

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

A cycle-aware GC is needed for any real-world program that builds graphs, doubly-linked lists, or objects with parent references.

---

## Tricolor Algorithm

The GC maintains a color for every heap object:

| Color | Meaning |
|-------|---------|
| **White** | Not yet visited in this cycle. Candidate for collection at cycle end. |
| **Grey** | Reachable from roots, but children not yet traced. |
| **Black** | Fully traced. All reachable children are grey or black. |

### Invariant

The **tricolor invariant** is: *no black object has a direct reference to a white object*. If this invariant holds at the end of a cycle, all remaining white objects are unreachable and can be freed.

### Collection cycle

```
1. Mark all roots grey
   (roots = globals, stack frames, open upvalues)

2. Repeatedly pick a grey object, mark it black,
   and mark all its white children grey

3. When no grey objects remain:
   all white objects are unreachable → free them
   all black objects become white for the next cycle
```

---

## Concurrent Collection

The GC runs on a **dedicated background thread** concurrently with the mutator (the program). This eliminates stop-the-world pauses.

```
┌──────────────────────────────────────┐
│  Mutator thread(s)                   │
│  (executing Cherash program)         │
└────────────────┬─────────────────────┘
                 │ reads/writes Values (Arc<Mutex<T>>)
                 ▼
┌──────────────────────────────────────┐
│  GC thread                           │
│  (mark-and-sweep, background)        │
└──────────────────────────────────────┘
```

### Write barrier

The challenge of concurrent collection: the mutator can write a pointer **while the GC is mid-cycle**. Without precautions, the GC might miss a newly-created reference and free a live object.

Cherash uses a **Dijkstra-style write barrier**: whenever the mutator writes a pointer `p → q`, if `p` is black, `q` is immediately shaded grey. This maintains the tricolor invariant despite concurrent writes.

```rust
fn write_barrier(parent: &GcRef, child: &GcRef) {
    if parent.color() == Color::Black {
        child.shade_grey();
        gc_worklist.push(child.clone());
    }
}
```

Every `Arc<Mutex<T>>` write that crosses an object boundary must call `write_barrier`.

---

## GC Tuning

Environment variables control GC behavior (Phase 8):

```bash
CHERASH_GC_PERCENT=100     # Run GC when heap grows 100% (doubles) since last cycle.
                            # Default: 100. Lower = more frequent, lower peak memory.
                            # Higher = less frequent, higher throughput.

CHERASH_GC_HARD_LIMIT=512M # Force an immediate GC cycle when live heap exceeds this.
                            # Default: unlimited.
```

These mirror Go's `GOGC` and `GOMEMLIMIT` semantics. The 100% default means: collect when live heap doubles, aiming for roughly 50% GC overhead on total memory usage.

---

## Object Graph

The GC traces the following object types as heap nodes:

| Value type | References to trace |
|------------|-------------------|
| `List` | Each element |
| `Dict` | Each key and value |
| `Set` | Each element |
| `Tuple` | Each element |
| `Instance` | Each field value |
| `Function` | Closure environment (`EnvRef`) |
| `Module` | Each attribute |
| `Class` | Methods map, class vars map |

Primitive values (`Int`, `Float`, `Bool`, `Str`, `Bytes`, `None`) contain no references and are leaf nodes.

---

## Integration with Arc

The existing `Arc<Mutex<T>>` wrappers serve double duty:

1. **Thread safety** — `Mutex<T>` serializes concurrent access.
2. **GC handle** — `Arc<T>` gives the GC a stable identity for each heap object.

When the GC frees a white object, it drops the `Arc` — if no other `Arc` clone exists, Rust's allocator frees the memory. This means the GC is layered *on top of* Rust's memory model, not in place of it. The GC handles *reachability*; Rust handles *deallocation*.

---

## Interaction with Threading

Each OS thread has its own stack (local variables, call frames). The GC must trace **all** live threads' stacks to correctly identify roots.

At the start of a mark phase:
1. The GC signals all mutator threads to publish their current stack roots.
2. Threads cooperate by registering their `EnvRef` chains in a shared root set.
3. The GC proceeds with marking from those roots while threads continue executing (with write barriers active).

This is a **cooperative safepoint** model — threads are never forcibly stopped, only asked to register their roots at the next safe opportunity.

---

## Current State (Phases 1–6)

The placeholder GC in `src/runtime/gc/mod.rs` is empty — no collection runs. Memory is managed entirely by `Arc` reference counting. This is sufficient for programs without reference cycles.

Programs with cycles will leak memory until Phase 8. This is a known limitation of the current MVP.

```rust
// src/runtime/gc/mod.rs — Phase 8 placeholder
pub struct Gc;
impl Gc {
    pub fn new() -> Self { Gc }
    // collect(), register_root(), write_barrier() — Phase 8
}
```

---

## Comparison with Other GCs

| Runtime | Algorithm | Stop-the-world? | Concurrent? |
|---------|-----------|-----------------|-------------|
| CPython | Reference counting + cycle detector | Brief (cycle sweep) | No |
| Java (G1) | Generational mark-sweep | Short pauses | Partial |
| Go | Tricolor mark-sweep | Minimal (< 1ms) | Yes |
| Cherash | Tricolor mark-sweep | None (target) | Yes |

Cherash targets Go-level GC latency: sub-millisecond pauses with concurrent collection, no generational complexity in the initial implementation.
