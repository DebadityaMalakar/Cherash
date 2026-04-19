use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::runtime::value::*;
use crate::interpreter::evaluator::RuntimeError;

pub fn make_math_module() -> Value {
    let mut attrs = HashMap::new();

    macro_rules! f {
        ($name:expr, $body:expr) => {
            attrs.insert($name.to_string(), Value::BuiltinFunction(Arc::new(BuiltinFn {
                name: $name,
                func: $body,
            })));
        }
    }

    // ── Constants ─────────────────────────────────────────────────────────────
    attrs.insert("pi".into(),  Value::Float(std::f64::consts::PI));
    attrs.insert("e".into(),   Value::Float(std::f64::consts::E));
    attrs.insert("tau".into(), Value::Float(std::f64::consts::TAU));
    attrs.insert("inf".into(), Value::Float(f64::INFINITY));
    attrs.insert("nan".into(), Value::Float(f64::NAN));

    // ── Rounding — return int (matches CPython) ───────────────────────────────
    f!("ceil",  |args| { Ok(Value::Int(get_float(&args, 0)?.ceil()  as i64)) });
    f!("floor", |args| { Ok(Value::Int(get_float(&args, 0)?.floor() as i64)) });
    f!("trunc", |args| { Ok(Value::Int(get_float(&args, 0)?.trunc() as i64)) });

    // ── Powers / logarithms ───────────────────────────────────────────────────
    f!("sqrt",  |args| flt1(&args, f64::sqrt));
    f!("cbrt",  |args| flt1(&args, f64::cbrt));
    f!("exp",   |args| flt1(&args, f64::exp));
    f!("expm1", |args| flt1(&args, f64::exp_m1));
    f!("exp2",  |args| flt1(&args, f64::exp2));
    f!("log",   |args| {
        let x = get_float(&args, 0)?;
        if x <= 0.0 { return Err(RuntimeError::value_error("math domain error")); }
        let base = if args.len() > 1 { get_float(&args, 1)? } else { std::f64::consts::E };
        Ok(Value::Float(x.log(base)))
    });
    f!("log2",  |args| {
        let x = get_float(&args, 0)?;
        if x <= 0.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x.log2()))
    });
    f!("log10", |args| {
        let x = get_float(&args, 0)?;
        if x <= 0.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x.log10()))
    });
    f!("log1p", |args| {
        let x = get_float(&args, 0)?;
        if x <= -1.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x.ln_1p()))
    });
    f!("pow",   |args| {
        let x = get_float(&args, 0)?;
        let y = get_float(&args, 1)?;
        Ok(Value::Float(x.powf(y)))
    });

    // ── Trigonometry ──────────────────────────────────────────────────────────
    f!("sin",     |args| flt1(&args, f64::sin));
    f!("cos",     |args| flt1(&args, f64::cos));
    f!("tan",     |args| flt1(&args, f64::tan));
    f!("asin",    |args| {
        let x = get_float(&args, 0)?;
        if x < -1.0 || x > 1.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x.asin()))
    });
    f!("acos",    |args| {
        let x = get_float(&args, 0)?;
        if x < -1.0 || x > 1.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x.acos()))
    });
    f!("atan",    |args| flt1(&args, f64::atan));
    f!("atan2",   |args| {
        let y = get_float(&args, 0)?;
        let x = get_float(&args, 1)?;
        Ok(Value::Float(y.atan2(x)))
    });
    f!("hypot",   |args| {
        // supports n-ary hypot (Python 3.8+)
        let sum: f64 = args.iter()
            .map(|v| get_float_val(v).unwrap_or(0.0).powi(2))
            .sum();
        Ok(Value::Float(sum.sqrt()))
    });
    f!("degrees", |args| flt1(&args, f64::to_degrees));
    f!("radians", |args| flt1(&args, f64::to_radians));

    // ── Hyperbolic ────────────────────────────────────────────────────────────
    f!("sinh",  |args| flt1(&args, f64::sinh));
    f!("cosh",  |args| flt1(&args, f64::cosh));
    f!("tanh",  |args| flt1(&args, f64::tanh));
    f!("asinh", |args| flt1(&args, f64::asinh));
    f!("acosh", |args| {
        let x = get_float(&args, 0)?;
        if x < 1.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x.acosh()))
    });
    f!("atanh", |args| {
        let x = get_float(&args, 0)?;
        if x <= -1.0 || x >= 1.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x.atanh()))
    });

    // ── Special functions ─────────────────────────────────────────────────────
    f!("erf",    |args| flt1(&args, libm_erf));
    f!("erfc",   |args| flt1(&args, |x| 1.0 - libm_erf(x)));
    f!("gamma",  |args| flt1(&args, libm_gamma));
    f!("lgamma", |args| flt1(&args, |x| libm_gamma(x).abs().ln()));

    // ── Absolute value ────────────────────────────────────────────────────────
    f!("fabs",   |args| flt1(&args, f64::abs));

    // ── Sign / bits ───────────────────────────────────────────────────────────
    f!("copysign", |args| {
        let x = get_float(&args, 0)?;
        let y = get_float(&args, 1)?;
        Ok(Value::Float(x.copysign(y)))
    });
    f!("fmod", |args| {
        let x = get_float(&args, 0)?;
        let y = get_float(&args, 1)?;
        if y == 0.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x % y))
    });
    f!("remainder", |args| {
        let x = get_float(&args, 0)?;
        let y = get_float(&args, 1)?;
        if y == 0.0 { return Err(RuntimeError::value_error("math domain error")); }
        Ok(Value::Float(x - y * (x / y).round()))
    });
    f!("nextafter", |args| {
        let x = get_float(&args, 0)?;
        let y = get_float(&args, 1)?;
        Ok(Value::Float(next_after(x, y)))
    });
    f!("ulp", |args| {
        let x = get_float(&args, 0)?;
        let bits = x.abs().to_bits();
        let next = f64::from_bits(bits + 1);
        Ok(Value::Float(next - x.abs()))
    });

    // ── Decomposition ─────────────────────────────────────────────────────────
    f!("modf", |args| {
        let x = get_float(&args, 0)?;
        let i = x.trunc();
        let f = x.fract();
        Ok(Value::Tuple(Arc::new(vec![Value::Float(f), Value::Float(i)])))
    });
    f!("frexp", |args| {
        let x = get_float(&args, 0)?;
        if x == 0.0 { return Ok(Value::Tuple(Arc::new(vec![Value::Float(0.0), Value::Int(0)]))); }
        let exp = (x.abs().log2().floor() as i32) + 1;
        let mantissa = x / (2.0f64).powi(exp);
        Ok(Value::Tuple(Arc::new(vec![Value::Float(mantissa), Value::Int(exp as i64)])))
    });
    f!("ldexp", |args| {
        let m = get_float(&args, 0)?;
        let e = match args.get(1) {
            Some(Value::Int(n)) => *n as i32,
            _ => return Err(RuntimeError::type_error("ldexp() second arg must be int")),
        };
        Ok(Value::Float(m * (2.0f64).powi(e)))
    });

    // ── Predicates ───────────────────────────────────────────────────────────
    f!("isnan",    |args| { Ok(Value::Bool(get_float(&args, 0)?.is_nan())) });
    f!("isinf",    |args| { Ok(Value::Bool(get_float(&args, 0)?.is_infinite())) });
    f!("isfinite", |args| { Ok(Value::Bool(get_float(&args, 0)?.is_finite())) });
    f!("isclose",  |args| {
        let a   = get_float(&args, 0)?;
        let b   = get_float(&args, 1)?;
        let rel = if args.len() > 2 { get_float(&args, 2)? } else { 1e-9 };
        let abs = if args.len() > 3 { get_float(&args, 3)? } else { 0.0  };
        let tol = f64::max(rel * f64::max(a.abs(), b.abs()), abs);
        Ok(Value::Bool((a - b).abs() <= tol))
    });

    // ── Integer combinatorics ─────────────────────────────────────────────────
    f!("factorial", |args| {
        let n = get_int(&args, 0)?;
        if n < 0  { return Err(RuntimeError::value_error("factorial() not defined for negative values")); }
        if n > 20 { return Err(RuntimeError::overflow_error("factorial() result too large")); }
        let mut r: i64 = 1;
        for i in 2..=n { r *= i; }
        Ok(Value::Int(r))
    });
    f!("gcd", |args| {
        if args.is_empty() { return Ok(Value::Int(0)); }
        let mut result = get_int(&args, 0)?.abs();
        for i in 1..args.len() {
            let mut b = get_int(&args, i)?.abs();
            let mut a = result;
            while b != 0 { let t = b; b = a % b; a = t; }
            result = a;
        }
        Ok(Value::Int(result))
    });
    f!("lcm", |args| {
        if args.is_empty() { return Ok(Value::Int(0)); }
        let mut result = get_int(&args, 0)?.abs();
        for i in 1..args.len() {
            let b = get_int(&args, i)?.abs();
            if result == 0 || b == 0 {
                result = 0;
            } else {
                let mut ga = result; let mut gb = b;
                while gb != 0 { let t = gb; gb = ga % gb; ga = t; }
                result = result / ga * b;
            }
        }
        Ok(Value::Int(result))
    });
    f!("comb", |args| {
        let n = get_int(&args, 0)?;
        let k = get_int(&args, 1)?;
        if n < 0 || k < 0 { return Err(RuntimeError::value_error("comb() requires non-negative ints")); }
        if k > n { return Ok(Value::Int(0)); }
        let k = k.min(n - k);
        let mut r: i64 = 1;
        for i in 0..k {
            r = r.checked_mul(n - i).and_then(|v| v.checked_div(i + 1))
                .ok_or_else(|| RuntimeError::overflow_error("comb() result too large"))?;
        }
        Ok(Value::Int(r))
    });
    f!("perm", |args| {
        let n = get_int(&args, 0)?;
        let k = if args.len() > 1 { get_int(&args, 1)? } else { n };
        if n < 0 || k < 0 { return Err(RuntimeError::value_error("perm() requires non-negative ints")); }
        if k > n { return Ok(Value::Int(0)); }
        let mut r: i64 = 1;
        for i in 0..k {
            r = r.checked_mul(n - i)
                .ok_or_else(|| RuntimeError::overflow_error("perm() result too large"))?;
        }
        Ok(Value::Int(r))
    });

    // ── Aggregation ───────────────────────────────────────────────────────────
    f!("fsum", |args| {
        // Neumaier compensated summation for accurate float addition
        let items = match args.first() {
            Some(Value::List(l)) => l.lock().unwrap().clone(),
            Some(Value::Tuple(t)) => t.as_ref().clone(),
            _ => args.clone(),
        };
        let mut sum  = 0.0f64;
        let mut comp = 0.0f64;
        for v in &items {
            let x = get_float_val(v).unwrap_or(0.0);
            let t = sum + x;
            comp += if sum.abs() >= x.abs() { (sum - t) + x } else { (x - t) + sum };
            sum = t;
        }
        Ok(Value::Float(sum + comp))
    });
    f!("prod", |args| {
        let items = match args.first() {
            Some(Value::List(l)) => l.lock().unwrap().clone(),
            Some(Value::Tuple(t)) => t.as_ref().clone(),
            _ => args.clone(),
        };
        let start = if args.len() > 1 { get_float(&args, 1)? } else { 1.0 };
        let result = items.iter().fold(start, |acc, v| acc * get_float_val(v).unwrap_or(1.0));
        Ok(Value::Float(result))
    });
    f!("dist", |args| {
        let p = list_floats(args.first())?;
        let q = list_floats(args.get(1))?;
        if p.len() != q.len() {
            return Err(RuntimeError::value_error("dist() requires same-length sequences"));
        }
        let sum: f64 = p.iter().zip(q.iter()).map(|(a, b)| (a - b).powi(2)).sum();
        Ok(Value::Float(sum.sqrt()))
    });

    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "math".into(), attrs })))
}

// ── Helper functions ──────────────────────────────────────────────────────────

fn flt1(args: &[Value], f: impl Fn(f64) -> f64) -> Result<Value, RuntimeError> {
    Ok(Value::Float(f(get_float(args, 0)?)))
}

fn get_float(args: &[Value], idx: usize) -> Result<f64, RuntimeError> {
    match args.get(idx) {
        Some(v) => get_float_val(v),
        None    => Err(RuntimeError::type_error("math function requires numeric argument")),
    }
}

fn get_float_val(v: &Value) -> Result<f64, RuntimeError> {
    match v {
        Value::Float(f) => Ok(*f),
        Value::Int(n)   => Ok(*n as f64),
        Value::Bool(b)  => Ok(*b as u8 as f64),
        _ => Err(RuntimeError::type_error("math function requires numeric argument")),
    }
}

fn get_int(args: &[Value], idx: usize) -> Result<i64, RuntimeError> {
    match args.get(idx) {
        Some(Value::Int(n))  => Ok(*n),
        Some(Value::Bool(b)) => Ok(*b as i64),
        _ => Err(RuntimeError::type_error("math function requires integer argument")),
    }
}

fn list_floats(v: Option<&Value>) -> Result<Vec<f64>, RuntimeError> {
    match v {
        Some(Value::List(l))  => l.lock().unwrap().iter().map(get_float_val).collect(),
        Some(Value::Tuple(t)) => t.iter().map(get_float_val).collect(),
        _ => Err(RuntimeError::type_error("math.dist() requires two sequences")),
    }
}

// ── Pure-Rust special function approximations ─────────────────────────────────

fn libm_erf(x: f64) -> f64 {
    if x == 0.0 { return 0.0; }
    // Abramowitz & Stegun 7.1.26 — max error < 1.5e-7
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let poly = t * (0.254829592
        + t * (-0.284496736
        + t * (1.421413741
        + t * (-1.453152027
        + t * 1.061405429))));
    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
    sign * (1.0 - poly * (-x * x).exp())
}

fn libm_gamma(x: f64) -> f64 {
    // Lanczos approximation (g=7, n=9) — accurate to ~15 digits
    if x < 0.5 {
        std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * libm_gamma(1.0 - x))
    } else {
        let x = x - 1.0;
        let coeffs = [
            0.99999999999980993,
            676.5203681218851,
            -1259.1392167224028,
            771.32342877765313,
            -176.61502916214059,
            12.507343278686905,
            -0.13857109526572012,
            9.9843695780195716e-6,
            1.5056327351493116e-7,
        ];
        let mut t = coeffs[0];
        for (i, &c) in coeffs[1..].iter().enumerate() {
            t += c / (x + i as f64 + 1.0);
        }
        let g = 7.0;
        let z = x + g + 0.5;
        (2.0 * std::f64::consts::PI).sqrt() * z.powf(x + 0.5) * (-z).exp() * t
    }
}

fn next_after(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() { return f64::NAN; }
    if x == y { return y; }
    let bits = x.to_bits() as i64;
    let next_bits = if x < y { bits + 1 } else { bits - 1 };
    f64::from_bits(next_bits as u64)
}

// ── Additional error constructors used above ──────────────────────────────────

impl RuntimeError {
    pub fn overflow_error(msg: impl Into<String>) -> Self {
        RuntimeError::new("OverflowError", msg.into())
    }
}
