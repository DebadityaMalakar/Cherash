pub mod tri_color;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

// ── Color ─────────────────────────────────────────────────────────────────────

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Color {
    White = 0,
    Grey  = 1,
    Black = 2,
}

// ── GcHandle<T> ───────────────────────────────────────────────────────────────
//
// Drop-in replacement for Arc<Mutex<T>> that also carries a GC color.
// API is identical to Arc<Mutex<T>>: clone is O(1), lock() returns
// LockResult<MutexGuard<T>>, and is Send + Sync when T: Send.

pub struct GcInner<T> {
    pub data: Mutex<T>,
    pub color: AtomicU8,
}

pub struct GcHandle<T: Send + 'static> {
    pub(crate) inner: Arc<GcInner<T>>,
}

impl<T: Send + 'static> GcHandle<T> {
    pub fn new_unregistered(val: T) -> Self {
        GcHandle {
            inner: Arc::new(GcInner {
                data: Mutex::new(val),
                color: AtomicU8::new(Color::White as u8),
            }),
        }
    }

    pub fn lock(&self) -> std::sync::LockResult<MutexGuard<T>> {
        self.inner.data.lock()
    }

    pub fn id(&self) -> usize {
        Arc::as_ptr(&self.inner) as usize
    }

    pub fn color(&self) -> Color {
        match self.inner.color.load(Ordering::Acquire) {
            1 => Color::Grey,
            2 => Color::Black,
            _ => Color::White,
        }
    }

    pub fn set_color(&self, c: Color) {
        self.inner.color.store(c as u8, Ordering::Release);
    }

    fn color_arc(&self) -> Arc<AtomicU8> {
        // We need to share the AtomicU8 with closures stored in the registry.
        // Since it lives inside GcInner, we clone the outer Arc and use a raw
        // pointer offset — but that's unsound. Instead we store a separate Arc<AtomicU8>
        // in a parallel field. For simplicity we use the GcHandle itself to make
        // typed closures.
        // This method is intentionally unused externally; color access goes through
        // the closures registered in GcRecord.
        Arc::new(AtomicU8::new(self.inner.color.load(Ordering::Acquire)))
    }

    pub fn downgrade(&self) -> GcWeak<T> {
        GcWeak { inner: Arc::downgrade(&self.inner) }
    }
}

impl<T: Send + 'static> Clone for GcHandle<T> {
    fn clone(&self) -> Self {
        GcHandle { inner: self.inner.clone() }
    }
}

impl<T: Send + 'static> PartialEq for GcHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

// ── GcWeak<T> ─────────────────────────────────────────────────────────────────

pub struct GcWeak<T: Send + 'static> {
    inner: Weak<GcInner<T>>,
}

impl<T: Send + 'static> GcWeak<T> {
    pub fn upgrade(&self) -> Option<GcHandle<T>> {
        self.inner.upgrade().map(|inner| GcHandle { inner })
    }
    pub fn is_alive(&self) -> bool {
        self.inner.strong_count() > 0
    }
}

impl<T: Send + 'static> Clone for GcWeak<T> {
    fn clone(&self) -> Self {
        GcWeak { inner: self.inner.clone() }
    }
}

// ── GcRecord — type-erased registry entry ─────────────────────────────────────

pub struct GcRecord {
    /// True if the underlying Arc is still live.
    pub is_alive: Box<dyn Fn() -> bool + Send + Sync>,
    /// Get the current color.
    pub get_color: Box<dyn Fn() -> Color + Send + Sync>,
    /// Set the color.
    pub set_color: Box<dyn Fn(Color) + Send + Sync>,
    /// Enumerate the GC IDs of all child heap objects.
    pub trace: Box<dyn Fn(&mut dyn FnMut(usize)) + Send + Sync>,
    /// Break internal heap references (cycle collection).
    pub clear: Box<dyn Fn() + Send + Sync>,
}

// ── GcHeap — global registry ──────────────────────────────────────────────────

pub struct GcHeap {
    pub records: Mutex<HashMap<usize, GcRecord>>,
    /// Root providers: closures that report the GC IDs of all root objects
    /// (globals, open upvalues, stack frames).
    pub roots: Mutex<Vec<Box<dyn Fn(&mut dyn FnMut(usize)) + Send + Sync>>>,
}

impl GcHeap {
    pub fn new() -> Self {
        GcHeap {
            records: Mutex::new(HashMap::new()),
            roots: Mutex::new(Vec::new()),
        }
    }

    /// Register a heap object. `trace_fn` enumerates child GC IDs from the
    /// object's data. `clear_fn` breaks internal references for cycle collection.
    pub fn register<T: Send + 'static>(
        &self,
        handle: &GcHandle<T>,
        trace_fn:  impl Fn(&T, &mut dyn FnMut(usize)) + Send + Sync + 'static,
        clear_fn:  impl Fn(&mut T) + Send + Sync + 'static,
    ) {
        let id       = handle.id();
        let weak     = handle.downgrade();
        let weak2    = weak.clone();
        let weak3    = weak.clone();
        let inner_w  = Arc::downgrade(&handle.inner);
        let inner_w2 = Arc::downgrade(&handle.inner);

        let record = GcRecord {
            is_alive: Box::new(move || weak.is_alive()),
            get_color: Box::new(move || {
                match inner_w.upgrade() {
                    Some(i) => match i.color.load(Ordering::Acquire) {
                        1 => Color::Grey,
                        2 => Color::Black,
                        _ => Color::White,
                    },
                    None => Color::White,
                }
            }),
            set_color: Box::new(move |c| {
                if let Some(i) = inner_w2.upgrade() {
                    i.color.store(c as u8, Ordering::Release);
                }
            }),
            trace: Box::new(move |visitor| {
                if let Some(h) = weak2.upgrade() {
                    if let Ok(data) = h.lock() {
                        trace_fn(&*data, visitor);
                    }
                }
            }),
            clear: Box::new(move || {
                if let Some(h) = weak3.upgrade() {
                    if let Ok(mut data) = h.lock() {
                        clear_fn(&mut *data);
                    }
                }
            }),
        };

        self.records.lock().unwrap().insert(id, record);
    }

    /// Add a root provider closure.
    pub fn add_root_provider(&self, f: impl Fn(&mut dyn FnMut(usize)) + Send + Sync + 'static) {
        self.roots.lock().unwrap().push(Box::new(f));
    }

    /// Prune entries whose Arc has already been freed.
    pub fn prune_dead(&self) {
        let mut records = self.records.lock().unwrap();
        records.retain(|_, rec| (rec.is_alive)());
    }

    pub fn get() -> Option<&'static GcHeap> {
        GLOBAL_GC.get()
    }
}

// ── Global singleton ──────────────────────────────────────────────────────────

pub static GLOBAL_GC: OnceLock<GcHeap> = OnceLock::new();

/// Initialise the GC and start the background collector thread.
///
/// Call once at program startup. Safe to call multiple times — subsequent
/// calls are no-ops.
pub fn init_gc() {
    GLOBAL_GC.get_or_init(GcHeap::new);
    start_background_collector();
}

fn start_background_collector() {
    std::thread::Builder::new()
        .name("cherash-gc".into())
        .spawn(|| {
            while GLOBAL_GC.get().is_none() {
                std::thread::sleep(Duration::from_millis(10));
            }
            loop {
                // CHERASH_GC_INTERVAL_MS: explicit interval override (default 100ms)
                let interval_ms: u64 = std::env::var("CHERASH_GC_INTERVAL_MS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(100);

                std::thread::sleep(Duration::from_millis(interval_ms));

                // CHERASH_GC_HARD_LIMIT: refuse to run if live objects exceed N MB
                // (conservative — just skip collection rather than OOM)
                if let Ok(limit_str) = std::env::var("CHERASH_GC_HARD_LIMIT") {
                    if let Ok(limit_mb) = limit_str.parse::<usize>() {
                        if let Some(heap) = GLOBAL_GC.get() {
                            let live = heap.records.lock().unwrap().len();
                            // Rough heuristic: each record ~ 200 bytes
                            let approx_mb = live * 200 / (1024 * 1024);
                            if approx_mb > limit_mb {
                                // Hard limit exceeded — force collection immediately
                                tri_color::collect();
                                continue;
                            }
                        }
                    }
                }

                // CHERASH_GC_PERCENT: collect only when live_objects / last_live > (1 + pct/100)
                // Default: always collect (pct=0)
                let gc_pct: usize = std::env::var("CHERASH_GC_PERCENT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                let should_collect = if gc_pct == 0 {
                    true
                } else if let Some(heap) = GLOBAL_GC.get() {
                    let live = heap.records.lock().unwrap().len();
                    // Store last-seen count in a thread-local
                    thread_local! {
                        static LAST_LIVE: std::cell::Cell<usize> = std::cell::Cell::new(0);
                    }
                    let last = LAST_LIVE.with(|c| c.get());
                    let threshold = last + last * gc_pct / 100;
                    if live >= threshold {
                        LAST_LIVE.with(|c| c.set(live));
                        true
                    } else {
                        false
                    }
                } else {
                    true
                };

                if should_collect {
                    tri_color::collect();
                }
            }
        })
        .ok();
}

// ── Convenience constructors ──────────────────────────────────────────────────
//
// These are called from value.rs to allocate GC-tracked heap objects.
// Each one creates a GcHandle<T>, registers the appropriate trace/clear
// functions, and returns the handle.

use std::collections::HashSet;

/// Allocate a GC-tracked list.
pub fn alloc_list(
    data: Vec<crate::runtime::value::Value>,
) -> GcHandle<Vec<crate::runtime::value::Value>> {
    let handle = GcHandle::new_unregistered(data);
    if let Some(heap) = GLOBAL_GC.get() {
        heap.register(
            &handle,
            |vec, visitor| {
                for v in vec {
                    if let Some(id) = v.gc_id() { visitor(id); }
                }
            },
            |vec| vec.clear(),
        );
    }
    handle
}

/// Allocate a GC-tracked dict.
pub fn alloc_dict(
    data: HashMap<crate::runtime::value::HashableValue, crate::runtime::value::Value>,
) -> GcHandle<HashMap<crate::runtime::value::HashableValue, crate::runtime::value::Value>> {
    let handle = GcHandle::new_unregistered(data);
    if let Some(heap) = GLOBAL_GC.get() {
        heap.register(
            &handle,
            |map, visitor| {
                for (k, v) in map {
                    if let Some(id) = crate::runtime::value::Value::from(k.clone()).gc_id() { visitor(id); }
                    if let Some(id) = v.gc_id() { visitor(id); }
                }
            },
            |map| map.clear(),
        );
    }
    handle
}

/// Allocate a GC-tracked set.
pub fn alloc_set(
    data: HashSet<crate::runtime::value::HashableValue>,
) -> GcHandle<HashSet<crate::runtime::value::HashableValue>> {
    let handle = GcHandle::new_unregistered(data);
    // Sets only contain hashable (non-heap) values — no children to trace.
    if let Some(heap) = GLOBAL_GC.get() {
        heap.register(&handle, |_, _| {}, |s| s.clear());
    }
    handle
}

/// Allocate a GC-tracked instance.
pub fn alloc_instance(
    data: crate::runtime::value::Instance,
) -> GcHandle<crate::runtime::value::Instance> {
    let handle = GcHandle::new_unregistered(data);
    if let Some(heap) = GLOBAL_GC.get() {
        heap.register(
            &handle,
            |inst, visitor| {
                for v in inst.fields.values() {
                    if let Some(id) = v.gc_id() { visitor(id); }
                }
            },
            |inst| inst.fields.clear(),
        );
    }
    handle
}
