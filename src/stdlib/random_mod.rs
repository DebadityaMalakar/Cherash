use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::runtime::value::*;
use crate::interpreter::evaluator::RuntimeError;

// ── PCG64 PRNG ────────────────────────────────────────────────────────────────
// Simple PCG-XSH-RR 64-bit generator — fast, decent quality, no deps.

struct Pcg64 {
    state: u64,
    inc:   u64,
}

impl Pcg64 {
    fn new(seed: u64) -> Self {
        let mut rng = Pcg64 { state: 0, inc: (seed << 1) | 1 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }
    fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(6364136223846793005).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }
    fn next_f64(&mut self) -> f64 {
        let hi = self.next_u32() as u64;
        let lo = self.next_u32() as u64;
        let bits = (hi << 32 | lo) >> 11; // 53 bits
        bits as f64 / (1u64 << 53) as f64
    }
    fn next_range(&mut self, a: i64, b: i64) -> i64 {
        if a >= b { return a; }
        let range = (b - a + 1) as u64;
        let r = ((self.next_u32() as u64) << 32 | self.next_u32() as u64) % range;
        a + r as i64
    }
}

// Global PRNG state
static RNG: OnceLock<Mutex<Pcg64>> = OnceLock::new();

fn rng() -> &'static Mutex<Pcg64> {
    RNG.get_or_init(|| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Mutex::new(Pcg64::new(seed))
    })
}

pub fn make_random_module() -> Value {
    let mut attrs = HashMap::new();

    macro_rules! bf {
        ($name:expr, $f:expr) => {
            attrs.insert($name.into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: $name, func: $f })));
        }
    }

    bf!("seed", |args| {
        let seed = match args.first() {
            Some(Value::Int(n))   => *n as u64,
            Some(Value::Float(f)) => f.to_bits(),
            _ => {
                use std::time::{SystemTime, UNIX_EPOCH};
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64
            }
        };
        *rng().lock().unwrap() = Pcg64::new(seed);
        Ok(Value::None)
    });

    bf!("random", |_| {
        Ok(Value::Float(rng().lock().unwrap().next_f64()))
    });

    bf!("uniform", |args| {
        let a = match args.first() { Some(Value::Float(f)) => *f, Some(Value::Int(n)) => *n as f64, _ => 0.0 };
        let b = match args.get(1)  { Some(Value::Float(f)) => *f, Some(Value::Int(n)) => *n as f64, _ => 1.0 };
        let r = rng().lock().unwrap().next_f64();
        Ok(Value::Float(a + r * (b - a)))
    });

    bf!("randint", |args| {
        let a = match args.first() { Some(Value::Int(n)) => *n, _ => 0 };
        let b = match args.get(1)  { Some(Value::Int(n)) => *n, _ => 1 };
        if a > b { return Err(RuntimeError::new("ValueError", "randint: a > b")); }
        Ok(Value::Int(rng().lock().unwrap().next_range(a, b)))
    });

    bf!("randrange", |args| {
        let start = match args.first() { Some(Value::Int(n)) => *n, _ => 0 };
        let stop  = match args.get(1)  { Some(Value::Int(n)) => *n, _ => return Err(RuntimeError::type_error("randrange requires stop")) };
        let step  = match args.get(2)  { Some(Value::Int(n)) => *n, _ => 1 };
        if step == 0 { return Err(RuntimeError::new("ValueError", "randrange step cannot be zero")); }
        let n = ((stop - start + step - step.signum()) / step).max(0);
        if n == 0 { return Err(RuntimeError::new("ValueError", "empty range")); }
        let k = rng().lock().unwrap().next_range(0, n - 1);
        Ok(Value::Int(start + k * step))
    });

    bf!("choice", |args| {
        let seq = match args.first() {
            Some(Value::List(l))  => l.lock().unwrap().clone(),
            Some(Value::Tuple(t)) => t.as_ref().clone(),
            _ => return Err(RuntimeError::type_error("choice requires a sequence")),
        };
        if seq.is_empty() { return Err(RuntimeError::new("IndexError", "cannot choose from empty sequence")); }
        let i = rng().lock().unwrap().next_range(0, seq.len() as i64 - 1) as usize;
        Ok(seq[i].clone())
    });

    bf!("choices", |args| {
        let seq = match args.first() {
            Some(Value::List(l))  => l.lock().unwrap().clone(),
            Some(Value::Tuple(t)) => t.as_ref().clone(),
            _ => return Err(RuntimeError::type_error("choices requires a sequence")),
        };
        let k = match args.get(1) { Some(Value::Int(n)) => *n as usize, _ => 1 };
        let mut result = Vec::with_capacity(k);
        let n = seq.len() as i64;
        if n == 0 { return Err(RuntimeError::new("IndexError", "cannot choose from empty sequence")); }
        for _ in 0..k {
            let i = rng().lock().unwrap().next_range(0, n - 1) as usize;
            result.push(seq[i].clone());
        }
        Ok(Value::List(crate::runtime::gc::alloc_list(result)))
    });

    bf!("shuffle", |args| {
        match args.first() {
            Some(Value::List(l)) => {
                let mut v = l.lock().unwrap();
                let n = v.len();
                let mut rng = rng().lock().unwrap();
                for i in (1..n).rev() {
                    let j = rng.next_range(0, i as i64) as usize;
                    v.swap(i, j);
                }
            }
            _ => return Err(RuntimeError::type_error("shuffle requires a list")),
        }
        Ok(Value::None)
    });

    bf!("sample", |args| {
        let seq = match args.first() {
            Some(Value::List(l))  => l.lock().unwrap().clone(),
            Some(Value::Tuple(t)) => t.as_ref().clone(),
            _ => return Err(RuntimeError::type_error("sample requires a sequence")),
        };
        let k = match args.get(1) { Some(Value::Int(n)) => *n as usize, _ => 0 };
        if k > seq.len() { return Err(RuntimeError::new("ValueError", "sample larger than population")); }
        let mut indices: Vec<usize> = (0..seq.len()).collect();
        let mut rng = rng().lock().unwrap();
        for i in 0..k {
            let j = rng.next_range(i as i64, seq.len() as i64 - 1) as usize;
            indices.swap(i, j);
        }
        let result: Vec<Value> = indices[..k].iter().map(|&i| seq[i].clone()).collect();
        Ok(Value::List(crate::runtime::gc::alloc_list(result)))
    });

    bf!("gauss", |args| {
        let mu    = match args.first() { Some(Value::Float(f)) => *f, Some(Value::Int(n)) => *n as f64, _ => 0.0 };
        let sigma = match args.get(1)  { Some(Value::Float(f)) => *f, Some(Value::Int(n)) => *n as f64, _ => 1.0 };
        // Box-Muller transform
        let mut rng = rng().lock().unwrap();
        let u1 = rng.next_f64().max(f64::EPSILON);
        let u2 = rng.next_f64();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        Ok(Value::Float(mu + sigma * z))
    });

    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "random".into(), attrs })))
}
