use std::collections::HashMap;
use std::sync::{Arc, Mutex, Condvar, OnceLock};
use std::sync::atomic::{AtomicI64, Ordering};
use crate::runtime::value::*;
use crate::interpreter::evaluator::RuntimeError;

// ── LockInner — blocking mutex without holding a guard ──────────────────────

pub struct LockInner {
    condvar: Condvar,
    state:   Mutex<bool>, // false = unlocked
}

impl LockInner {
    pub fn new() -> Self {
        LockInner { condvar: Condvar::new(), state: Mutex::new(false) }
    }
    pub fn acquire(&self) {
        let mut locked = self.state.lock().unwrap();
        while *locked {
            locked = self.condvar.wait(locked).unwrap();
        }
        *locked = true;
    }
    pub fn release(&self) {
        let mut locked = self.state.lock().unwrap();
        *locked = false;
        self.condvar.notify_one();
    }
    pub fn try_acquire(&self) -> bool {
        let mut locked = self.state.lock().unwrap();
        if *locked { false } else { *locked = true; true }
    }
}

// ── ConditionInner ────────────────────────────────────────────────────────────

pub struct ConditionInner {
    condvar: Condvar,
    lock:    Mutex<bool>, // underlying lock state
}

impl ConditionInner {
    pub fn new() -> Self {
        ConditionInner { condvar: Condvar::new(), lock: Mutex::new(false) }
    }
    pub fn acquire(&self) {
        let mut locked = self.lock.lock().unwrap();
        while *locked { locked = self.condvar.wait(locked).unwrap(); }
        *locked = true;
    }
    pub fn release(&self) {
        let mut locked = self.lock.lock().unwrap();
        *locked = false;
        self.condvar.notify_all();
    }
    pub fn wait(&self) {
        // Release lock and wait for notification, then reacquire
        let mut locked = self.lock.lock().unwrap();
        *locked = false;
        self.condvar.notify_all(); // wake any waiters on the lock
        locked = self.condvar.wait(locked).unwrap();
        while *locked { locked = self.condvar.wait(locked).unwrap(); }
        *locked = true;
    }
    pub fn notify(&self) {
        self.condvar.notify_one();
    }
    pub fn notify_all(&self) {
        self.condvar.notify_all();
    }
}

// ── SemaphoreInner ────────────────────────────────────────────────────────────

pub struct SemaphoreInner {
    condvar: Condvar,
    count:   Mutex<i64>,
}

impl SemaphoreInner {
    pub fn new(n: i64) -> Self {
        SemaphoreInner { condvar: Condvar::new(), count: Mutex::new(n) }
    }
    pub fn acquire(&self) {
        let mut count = self.count.lock().unwrap();
        while *count <= 0 { count = self.condvar.wait(count).unwrap(); }
        *count -= 1;
    }
    pub fn release(&self) {
        let mut count = self.count.lock().unwrap();
        *count += 1;
        self.condvar.notify_one();
    }
}

// ── BarrierInner ──────────────────────────────────────────────────────────────

pub struct BarrierInner {
    condvar:    Condvar,
    state:      Mutex<(usize, usize)>, // (arrived, generation)
    parties:    usize,
}

impl BarrierInner {
    pub fn new(n: usize) -> Self {
        BarrierInner { condvar: Condvar::new(), state: Mutex::new((0, 0)), parties: n }
    }
    pub fn wait(&self) {
        let mut guard = self.state.lock().unwrap();
        let gen = guard.1;
        guard.0 += 1;
        if guard.0 == self.parties {
            guard.0 = 0;
            guard.1 += 1;
            self.condvar.notify_all();
        } else {
            while guard.1 == gen {
                guard = self.condvar.wait(guard).unwrap();
            }
        }
    }
}

// ── Global registries ────────────────────────────────────────────────────────

static LOCK_REGISTRY:      OnceLock<Mutex<HashMap<usize, Arc<LockInner>>>>      = OnceLock::new();
static COND_REGISTRY:      OnceLock<Mutex<HashMap<usize, Arc<ConditionInner>>>> = OnceLock::new();
static SEM_REGISTRY:       OnceLock<Mutex<HashMap<usize, Arc<SemaphoreInner>>>> = OnceLock::new();
static BARRIER_REGISTRY:   OnceLock<Mutex<HashMap<usize, Arc<BarrierInner>>>>   = OnceLock::new();
static THREAD_REGISTRY:    OnceLock<Mutex<HashMap<i64, std::thread::JoinHandle<()>>>> = OnceLock::new();
static NEXT_THREAD_ID: AtomicI64 = AtomicI64::new(1);

fn lock_registry() -> &'static Mutex<HashMap<usize, Arc<LockInner>>> {
    LOCK_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cond_registry() -> &'static Mutex<HashMap<usize, Arc<ConditionInner>>> {
    COND_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sem_registry() -> &'static Mutex<HashMap<usize, Arc<SemaphoreInner>>> {
    SEM_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn barrier_registry() -> &'static Mutex<HashMap<usize, Arc<BarrierInner>>> {
    BARRIER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn thread_registry() -> &'static Mutex<HashMap<i64, std::thread::JoinHandle<()>>> {
    THREAD_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn instance_id(v: &Value) -> Option<usize> {
    match v { Value::Instance(h) => Some(h.id()), _ => None }
}

// ── Lock methods ─────────────────────────────────────────────────────────────

fn lock_init(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        lock_registry().lock().unwrap().insert(id, Arc::new(LockInner::new()));
    }
    Ok(Value::None)
}

fn lock_acquire(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = lock_registry().lock().unwrap().get(&id).cloned();
        if let Some(inner) = inner {
            inner.acquire();
        }
    }
    Ok(Value::Bool(true))
}

fn lock_release(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = lock_registry().lock().unwrap().get(&id).cloned();
        if let Some(inner) = inner {
            inner.release();
        }
    }
    Ok(Value::None)
}

fn lock_enter(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().cloned().unwrap_or(Value::None);
    lock_acquire(args)?;
    Ok(self_val)
}

fn lock_exit(args: Vec<Value>) -> Result<Value, RuntimeError> {
    lock_release(args)?;
    Ok(Value::Bool(false))
}

// ── Thread methods ────────────────────────────────────────────────────────────

fn thread_init(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(Value::Instance(inst)) = args.first() {
        let mut guard = inst.lock().unwrap();
        // Thread(target=fn, args=tuple_or_list)
        if let Some(target) = args.get(1) {
            guard.fields.insert("target".into(), target.clone());
        }
        let targs = args.get(2).cloned().unwrap_or_else(|| {
            Value::List(crate::runtime::gc::alloc_list(vec![]))
        });
        guard.fields.insert("args".into(), targs);
        guard.fields.insert("_thread_id".into(), Value::Int(-1));
    }
    Ok(Value::None)
}

fn thread_start(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let inst_handle = match args.first() {
        Some(Value::Instance(h)) => h.clone(),
        _ => return Ok(Value::None),
    };
    let inst_id = inst_handle.id() as i64;

    let (target, targs_vec) = {
        let guard = inst_handle.lock().unwrap();
        let target = guard.fields.get("target").cloned().unwrap_or(Value::None);
        let targs_vec = match guard.fields.get("args") {
            Some(Value::List(l))  => l.lock().unwrap().clone(),
            Some(Value::Tuple(t)) => t.as_ref().clone(),
            _                     => vec![],
        };
        (target, targs_vec)
    };

    // Assign a unique thread id
    let tid = NEXT_THREAD_ID.fetch_add(1, Ordering::Relaxed);
    inst_handle.lock().unwrap().fields.insert("_thread_id".into(), Value::Int(tid));

    // Spawn a real OS thread with its own Evaluator
    let handle = std::thread::Builder::new()
        .name(format!("cherash-thread-{}", tid))
        .spawn(move || {
            let mut ev = crate::interpreter::evaluator::Evaluator::new();
            let _ = ev.call_value(target, targs_vec);
        })
        .map_err(|e| RuntimeError::new("RuntimeError", &format!("thread spawn failed: {}", e)))?;

    thread_registry().lock().unwrap().insert(inst_id, handle);
    Ok(Value::None)
}

fn thread_join(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let inst_handle = match args.first() {
        Some(Value::Instance(h)) => h.clone(),
        _ => return Ok(Value::None),
    };
    let inst_id = inst_handle.id() as i64;

    let handle = thread_registry().lock().unwrap().remove(&inst_id);
    if let Some(jh) = handle {
        jh.join().ok();
    }
    Ok(Value::None)
}

fn thread_is_alive(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let inst_handle = match args.first() {
        Some(Value::Instance(h)) => h.clone(),
        _ => return Ok(Value::Bool(false)),
    };
    let inst_id = inst_handle.id() as i64;
    let alive = thread_registry().lock().unwrap().contains_key(&inst_id);
    Ok(Value::Bool(alive))
}

// ── Condition methods ─────────────────────────────────────────────────────────

fn cond_init(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        cond_registry().lock().unwrap().insert(id, Arc::new(ConditionInner::new()));
    }
    Ok(Value::None)
}

fn cond_acquire(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = cond_registry().lock().unwrap().get(&id).cloned();
        if let Some(c) = inner { c.acquire(); }
    }
    Ok(Value::Bool(true))
}

fn cond_release(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = cond_registry().lock().unwrap().get(&id).cloned();
        if let Some(c) = inner { c.release(); }
    }
    Ok(Value::None)
}

fn cond_wait(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = cond_registry().lock().unwrap().get(&id).cloned();
        if let Some(c) = inner { c.wait(); }
    }
    Ok(Value::None)
}

fn cond_notify(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = cond_registry().lock().unwrap().get(&id).cloned();
        if let Some(c) = inner { c.notify(); }
    }
    Ok(Value::None)
}

fn cond_notify_all(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = cond_registry().lock().unwrap().get(&id).cloned();
        if let Some(c) = inner { c.notify_all(); }
    }
    Ok(Value::None)
}

fn cond_enter(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().cloned().unwrap_or(Value::None);
    cond_acquire(args)?;
    Ok(self_val)
}

fn cond_exit(args: Vec<Value>) -> Result<Value, RuntimeError> {
    cond_release(args)?;
    Ok(Value::Bool(false))
}

// ── Semaphore methods ─────────────────────────────────────────────────────────

fn sem_init(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let n = match args.get(1) { Some(Value::Int(n)) => *n, _ => 1 };
        sem_registry().lock().unwrap().insert(id, Arc::new(SemaphoreInner::new(n)));
    }
    Ok(Value::None)
}

fn sem_acquire(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = sem_registry().lock().unwrap().get(&id).cloned();
        if let Some(s) = inner { s.acquire(); }
    }
    Ok(Value::Bool(true))
}

fn sem_release(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = sem_registry().lock().unwrap().get(&id).cloned();
        if let Some(s) = inner { s.release(); }
    }
    Ok(Value::None)
}

fn sem_enter(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().cloned().unwrap_or(Value::None);
    sem_acquire(args)?;
    Ok(self_val)
}

fn sem_exit(args: Vec<Value>) -> Result<Value, RuntimeError> {
    sem_release(args)?;
    Ok(Value::Bool(false))
}

// ── Barrier methods ───────────────────────────────────────────────────────────

fn barrier_init(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let n = match args.get(1) { Some(Value::Int(n)) => *n as usize, _ => 2 };
        barrier_registry().lock().unwrap().insert(id, Arc::new(BarrierInner::new(n)));
    }
    Ok(Value::None)
}

fn barrier_wait(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(id) = args.first().and_then(instance_id) {
        let inner = barrier_registry().lock().unwrap().get(&id).cloned();
        if let Some(b) = inner { b.wait(); }
    }
    Ok(Value::None)
}

// ── Module factory ────────────────────────────────────────────────────────────

pub fn make_threading_module() -> Value {
    let mut attrs = HashMap::new();

    // Thread class
    let thread_class = Arc::new(Class {
        name: "Thread".into(),
        bases: vec![],
        methods: {
            let mut m = HashMap::new();
            m.insert("__init__".into(),  Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__init__",  func: thread_init    })));
            m.insert("start".into(),     Value::BuiltinFunction(Arc::new(BuiltinFn { name: "start",     func: thread_start   })));
            m.insert("join".into(),      Value::BuiltinFunction(Arc::new(BuiltinFn { name: "join",      func: thread_join    })));
            m.insert("is_alive".into(),  Value::BuiltinFunction(Arc::new(BuiltinFn { name: "is_alive",  func: thread_is_alive})));
            m
        },
        class_vars: HashMap::new(),
    });
    attrs.insert("Thread".into(), Value::Class(thread_class));

    // Lock class (also used for RLock — simplified)
    let make_lock_class = |name: &'static str| Arc::new(Class {
        name: name.into(),
        bases: vec![],
        methods: {
            let mut m = HashMap::new();
            m.insert("__init__".into(),  Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__init__",  func: lock_init    })));
            m.insert("acquire".into(),   Value::BuiltinFunction(Arc::new(BuiltinFn { name: "acquire",   func: lock_acquire })));
            m.insert("release".into(),   Value::BuiltinFunction(Arc::new(BuiltinFn { name: "release",   func: lock_release })));
            m.insert("__enter__".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__enter__", func: lock_enter   })));
            m.insert("__exit__".into(),  Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__exit__",  func: lock_exit    })));
            m
        },
        class_vars: HashMap::new(),
    });

    attrs.insert("Lock".into(),  Value::Class(make_lock_class("Lock")));
    attrs.insert("RLock".into(), Value::Class(make_lock_class("RLock")));

    // Condition class
    let condition_class = Arc::new(Class {
        name: "Condition".into(),
        bases: vec![],
        methods: {
            let mut m = HashMap::new();
            m.insert("__init__".into(),   Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__init__",   func: cond_init       })));
            m.insert("acquire".into(),    Value::BuiltinFunction(Arc::new(BuiltinFn { name: "acquire",    func: cond_acquire    })));
            m.insert("release".into(),    Value::BuiltinFunction(Arc::new(BuiltinFn { name: "release",    func: cond_release    })));
            m.insert("wait".into(),       Value::BuiltinFunction(Arc::new(BuiltinFn { name: "wait",       func: cond_wait       })));
            m.insert("notify".into(),     Value::BuiltinFunction(Arc::new(BuiltinFn { name: "notify",     func: cond_notify     })));
            m.insert("notify_all".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "notify_all", func: cond_notify_all })));
            m.insert("__enter__".into(),  Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__enter__",  func: cond_enter      })));
            m.insert("__exit__".into(),   Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__exit__",   func: cond_exit       })));
            m
        },
        class_vars: HashMap::new(),
    });
    attrs.insert("Condition".into(), Value::Class(condition_class));

    // Semaphore class
    let semaphore_class = Arc::new(Class {
        name: "Semaphore".into(),
        bases: vec![],
        methods: {
            let mut m = HashMap::new();
            m.insert("__init__".into(),  Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__init__",  func: sem_init    })));
            m.insert("acquire".into(),   Value::BuiltinFunction(Arc::new(BuiltinFn { name: "acquire",   func: sem_acquire })));
            m.insert("release".into(),   Value::BuiltinFunction(Arc::new(BuiltinFn { name: "release",   func: sem_release })));
            m.insert("__enter__".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__enter__", func: sem_enter   })));
            m.insert("__exit__".into(),  Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__exit__",  func: sem_exit    })));
            m
        },
        class_vars: HashMap::new(),
    });
    attrs.insert("Semaphore".into(), Value::Class(semaphore_class.clone()));
    attrs.insert("BoundedSemaphore".into(), Value::Class(semaphore_class));

    // Barrier class
    let barrier_class = Arc::new(Class {
        name: "Barrier".into(),
        bases: vec![],
        methods: {
            let mut m = HashMap::new();
            m.insert("__init__".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "__init__", func: barrier_init })));
            m.insert("wait".into(),     Value::BuiltinFunction(Arc::new(BuiltinFn { name: "wait",     func: barrier_wait })));
            m
        },
        class_vars: HashMap::new(),
    });
    attrs.insert("Barrier".into(), Value::Class(barrier_class));

    // Convenience: current_thread() stub
    attrs.insert("current_thread".into(), Value::BuiltinFunction(Arc::new(BuiltinFn {
        name: "current_thread",
        func: |_| Ok(Value::None),
    })));

    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "threading".into(), attrs })))
}
