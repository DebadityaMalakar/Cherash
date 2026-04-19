use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::runtime::value::*;
use crate::interpreter::evaluator::RuntimeError;

pub fn make_time_module() -> Value {
    let mut attrs = HashMap::new();

    macro_rules! bf {
        ($name:expr, $f:expr) => {
            attrs.insert($name.into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: $name, func: $f })));
        }
    }

    bf!("time", |_| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        Ok(Value::Float(t.as_secs_f64()))
    });

    bf!("time_ns", |_| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        Ok(Value::Int(t.as_nanos() as i64))
    });

    bf!("monotonic", |_| {
        use std::time::Instant;
        // We use a process-start baseline stored in a static
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(Instant::now);
        Ok(Value::Float(start.elapsed().as_secs_f64()))
    });

    bf!("perf_counter", |_| {
        use std::time::Instant;
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(Instant::now);
        Ok(Value::Float(start.elapsed().as_secs_f64()))
    });

    bf!("sleep", |args| {
        let secs = match args.first() {
            Some(Value::Float(f)) => *f,
            Some(Value::Int(n))   => *n as f64,
            _ => 0.0,
        };
        if secs < 0.0 {
            return Err(RuntimeError::new("ValueError", "sleep length must be non-negative"));
        }
        std::thread::sleep(std::time::Duration::from_secs_f64(secs));
        Ok(Value::None)
    });

    // gmtime() / localtime() return a struct_time-like tuple:
    // (tm_year, tm_mon, tm_mday, tm_hour, tm_min, tm_sec, tm_wday, tm_yday, tm_isdst)
    bf!("gmtime", |args| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = match args.first() {
            Some(Value::Float(f)) => *f,
            Some(Value::Int(n))   => *n as f64,
            _ => SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64(),
        };
        Ok(Value::Tuple(Arc::new(unix_to_struct_time(ts as i64, false))))
    });

    bf!("localtime", |args| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = match args.first() {
            Some(Value::Float(f)) => *f,
            Some(Value::Int(n))   => *n as f64,
            _ => SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64(),
        };
        Ok(Value::Tuple(Arc::new(unix_to_struct_time(ts as i64, true))))
    });

    bf!("strftime", |args| {
        // Minimal strftime: just return a formatted timestamp string
        let fmt = match args.first() {
            Some(Value::Str(s)) => s.as_ref().clone(),
            _ => "%Y-%m-%d %H:%M:%S".into(),
        };
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = match args.get(1) {
            Some(Value::Tuple(t)) => struct_time_to_unix(t),
            _ => SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
        };
        Ok(Value::Str(Arc::new(format_time(&fmt, ts))))
    });

    attrs.insert("timezone".into(), Value::Int(0)); // UTC offset in seconds
    attrs.insert("altzone".into(),  Value::Int(0));
    attrs.insert("daylight".into(), Value::Int(0));
    attrs.insert("tzname".into(), Value::Tuple(Arc::new(vec![
        Value::Str(Arc::new("UTC".into())),
        Value::Str(Arc::new("UTC".into())),
    ])));

    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "time".into(), attrs })))
}

// Minimal UTC decomposition (no timezone awareness)
fn unix_to_struct_time(ts: i64, _local: bool) -> Vec<Value> {
    // Days since epoch
    let days = ts / 86400;
    let secs_in_day = ts % 86400;
    let hour = secs_in_day / 3600;
    let minute = (secs_in_day % 3600) / 60;
    let second = secs_in_day % 60;

    // Gregorian calendar decomposition (Euclidean algorithm)
    let (year, month, day) = days_to_ymd(days);
    let wday = ((days + 3) % 7) as i64; // Thursday=0 in Unix epoch → Monday=0 in struct_time
    let wday = (wday + 4) % 7; // convert: epoch day 0 was Thursday (4 in Mon=0)
    let yday = day_of_year(year, month, day) as i64;

    vec![
        Value::Int(year as i64),
        Value::Int(month as i64),
        Value::Int(day as i64),
        Value::Int(hour),
        Value::Int(minute),
        Value::Int(second),
        Value::Int(wday),
        Value::Int(yday),
        Value::Int(0), // tm_isdst
    ]
}

fn struct_time_to_unix(t: &[Value]) -> i64 {
    let get = |i: usize| match t.get(i) { Some(Value::Int(n)) => *n, _ => 0 };
    let year = get(0); let month = get(1); let day = get(2);
    let hour = get(3); let minute = get(4); let second = get(5);
    let days = ymd_to_days(year as i32, month as u32, day as u32);
    days * 86400 + hour * 3600 + minute * 60 + second
}

fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn ymd_to_days(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let m = m as i64;
    let d = d as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn day_of_year(year: i32, month: u32, day: u32) -> u32 {
    let days_before = [0u32, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let extra = if leap && month > 2 { 1 } else { 0 };
    days_before[(month - 1) as usize] + day + extra
}

fn format_time(fmt: &str, ts: i64) -> String {
    let (year, month, day) = days_to_ymd(ts / 86400);
    let secs_in_day = ts % 86400;
    let hour = secs_in_day / 3600;
    let minute = (secs_in_day % 3600) / 60;
    let second = secs_in_day % 60;
    let wday_names = ["Mon","Tue","Wed","Thu","Fri","Sat","Sun"];
    let mon_names  = ["","Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];

    let wday = (((ts / 86400) + 3) % 7 + 7) as usize % 7;

    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('Y') => out.push_str(&format!("{:04}", year)),
                Some('m') => out.push_str(&format!("{:02}", month)),
                Some('d') => out.push_str(&format!("{:02}", day)),
                Some('H') => out.push_str(&format!("{:02}", hour)),
                Some('M') => out.push_str(&format!("{:02}", minute)),
                Some('S') => out.push_str(&format!("{:02}", second)),
                Some('A') => out.push_str(wday_names[wday]),
                Some('a') => out.push_str(&wday_names[wday][..3]),
                Some('B') => out.push_str(mon_names[month as usize]),
                Some('b') => out.push_str(&mon_names[month as usize][..3]),
                Some('%') => out.push('%'),
                Some(x)   => { out.push('%'); out.push(x); }
                None      => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
