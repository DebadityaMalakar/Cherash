use cherash::run_source;

fn run(src: &str) -> String {
    match run_source(src, false) {
        Ok(_)  => "ok".into(),
        Err(e) => format!("ERROR: {}", e),
    }
}

fn run_val(src: &str) -> f64 {
    // Wrap expression in a print, capture nothing — just verify no error.
    // For actual value assertions, embed the check in the source.
    match run_source(src, false) {
        Ok(_)  => 0.0,
        Err(e) => panic!("{}", e),
    }
}

// ── Constants ────────────────────────────────────────────────────────────────

#[test]
fn test_constants() {
    run_val(r#"
import math
assert math.pi  > 3.14159 and math.pi  < 3.14160
assert math.e   > 2.71828 and math.e   < 2.71829
assert math.tau > 6.28318 and math.tau < 6.28319
assert math.inf == math.inf
assert math.nan != math.nan
"#);
}

// ── Rounding — must return int ────────────────────────────────────────────────

#[test]
fn test_ceil_returns_int() {
    run_val(r#"
import math
assert math.ceil(1.2)  == 2
assert math.ceil(-1.2) == -1
assert math.ceil(2.0)  == 2
"#);
}

#[test]
fn test_floor_returns_int() {
    run_val(r#"
import math
assert math.floor(1.9)  == 1
assert math.floor(-1.1) == -2
assert math.floor(3.0)  == 3
"#);
}

#[test]
fn test_trunc_returns_int() {
    run_val(r#"
import math
assert math.trunc(1.9)  == 1
assert math.trunc(-1.9) == -1
"#);
}

// ── Powers & logs ─────────────────────────────────────────────────────────────

#[test]
fn test_sqrt() {
    run_val(r#"
import math
assert math.sqrt(4.0)  == 2.0
assert math.sqrt(9.0)  == 3.0
assert math.sqrt(2.0)  > 1.414 and math.sqrt(2.0) < 1.415
"#);
}

#[test]
fn test_cbrt() {
    run_val(r#"
import math
assert math.cbrt(8.0)  == 2.0
assert math.cbrt(27.0) == 3.0
"#);
}

#[test]
fn test_exp_log() {
    run_val(r#"
import math
assert math.exp(0.0) == 1.0
assert math.exp(1.0) > 2.718 and math.exp(1.0) < 2.719
assert math.log(1.0) == 0.0
assert math.log(math.e) > 0.9999 and math.log(math.e) < 1.0001
assert math.log2(8.0) == 3.0
assert math.log10(1000.0) == 3.0
"#);
}

#[test]
fn test_log1p_expm1() {
    run_val(r#"
import math
assert math.isclose(math.log1p(0.0),  0.0)
assert math.isclose(math.expm1(0.0),  0.0)
assert math.isclose(math.log1p(1.0),  math.log(2.0))
"#);
}

#[test]
fn test_pow_and_hypot() {
    run_val(r#"
import math
assert math.pow(2.0, 10.0) == 1024.0
assert math.hypot(3.0, 4.0) == 5.0
assert math.isclose(math.hypot(1.0, 1.0, 1.0), math.sqrt(3.0))
"#);
}

// ── Trig ──────────────────────────────────────────────────────────────────────

#[test]
fn test_trig_basic() {
    run_val(r#"
import math
assert math.isclose(math.sin(0.0), 0.0)
assert math.isclose(math.cos(0.0), 1.0)
assert math.isclose(math.tan(0.0), 0.0)
assert math.isclose(math.sin(math.pi / 2), 1.0)
assert math.isclose(math.cos(math.pi),    -1.0)
"#);
}

#[test]
fn test_trig_inverse() {
    run_val(r#"
import math
assert math.isclose(math.asin(1.0), math.pi / 2)
assert math.isclose(math.acos(1.0), 0.0)
assert math.isclose(math.atan(1.0), math.pi / 4)
assert math.isclose(math.atan2(1.0, 1.0), math.pi / 4)
"#);
}

#[test]
fn test_degrees_radians() {
    run_val(r#"
import math
assert math.isclose(math.degrees(math.pi), 180.0)
assert math.isclose(math.radians(180.0), math.pi)
"#);
}

// ── Hyperbolic ────────────────────────────────────────────────────────────────

#[test]
fn test_hyperbolic() {
    run_val(r#"
import math
assert math.isclose(math.sinh(0.0),  0.0)
assert math.isclose(math.cosh(0.0),  1.0)
assert math.isclose(math.tanh(0.0),  0.0)
assert math.isclose(math.asinh(0.0), 0.0)
assert math.isclose(math.acosh(1.0), 0.0)
assert math.isclose(math.atanh(0.0), 0.0)
"#);
}

// ── Special functions ─────────────────────────────────────────────────────────

#[test]
fn test_erf() {
    run_val(r#"
import math
assert math.erf(0.0)  == 0.0
assert math.erfc(0.0) == 1.0
assert math.erf(1.0)  > 0.8
assert math.erfc(1.0) < 0.2
"#);
}

#[test]
fn test_gamma() {
    run_val(r#"
import math
assert math.isclose(math.gamma(1.0), 1.0)
assert math.isclose(math.gamma(2.0), 1.0)
assert math.isclose(math.gamma(3.0), 2.0)
assert math.isclose(math.gamma(4.0), 6.0)
assert math.isclose(math.gamma(5.0), 24.0)
"#);
}

// ── Predicates ───────────────────────────────────────────────────────────────

#[test]
fn test_predicates() {
    run_val(r#"
import math
assert math.isnan(math.nan) == True
assert math.isinf(math.inf) == True
assert math.isfinite(1.0)   == True
assert math.isfinite(math.inf) == False
assert math.isclose(1.0, 1.0 + 1e-10)
"#);
}

// ── Integer combinatorics ─────────────────────────────────────────────────────

#[test]
fn test_factorial() {
    run_val(r#"
import math
assert math.factorial(0)  == 1
assert math.factorial(1)  == 1
assert math.factorial(5)  == 120
assert math.factorial(10) == 3628800
"#);
}

#[test]
fn test_gcd_lcm() {
    run_val(r#"
import math
assert math.gcd(12, 8)   == 4
assert math.gcd(0, 5)    == 5
assert math.gcd(100, 75) == 25
assert math.lcm(4, 6)    == 12
assert math.lcm(3, 5)    == 15
"#);
}

#[test]
fn test_comb_perm() {
    run_val(r#"
import math
assert math.comb(5, 2) == 10
assert math.comb(10, 3) == 120
assert math.comb(5, 0)  == 1
assert math.perm(5, 2)  == 20
assert math.perm(4, 4)  == 24
"#);
}

// ── Decomposition ─────────────────────────────────────────────────────────────

#[test]
fn test_modf() {
    run_val(r#"
import math
frac, intg = math.modf(3.75)
assert math.isclose(frac, 0.75)
assert math.isclose(intg, 3.0)
"#);
}

#[test]
fn test_frexp_ldexp() {
    run_val(r#"
import math
m, e = math.frexp(8.0)
assert math.isclose(math.ldexp(m, e), 8.0)
"#);
}

// ── Aggregation ───────────────────────────────────────────────────────────────

#[test]
fn test_fsum() {
    run_val(r#"
import math
assert math.fsum([1.0, 2.0, 3.0]) == 6.0
assert math.isclose(math.fsum([0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1]), 1.0)
"#);
}

#[test]
fn test_dist() {
    run_val(r#"
import math
assert math.isclose(math.dist([0.0, 0.0], [3.0, 4.0]), 5.0)
assert math.isclose(math.dist([0.0], [1.0]), 1.0)
"#);
}

// ── Sign / bits ───────────────────────────────────────────────────────────────

#[test]
fn test_copysign_fmod() {
    run_val(r#"
import math
assert math.copysign(3.0, -1.0) == -3.0
assert math.copysign(-3.0, 1.0) == 3.0
assert math.isclose(math.fmod(10.0, 3.0), 1.0)
"#);
}

#[test]
fn test_fabs() {
    run_val(r#"
import math
assert math.fabs(-3.14) == 3.14
assert math.fabs(2.0)   == 2.0
"#);
}
