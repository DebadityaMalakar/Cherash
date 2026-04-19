/// Implementations of built-in methods for list, dict, set, str, tuple, bytes.
/// Each function receives `self` as `args[0]`, matching args as `args[1..]`.

use std::collections::HashMap;
use std::sync::Arc;
use crate::runtime::value::*;
use crate::interpreter::evaluator::RuntimeError;

// ── Helper macros ─────────────────────────────────────────────────────────────

macro_rules! get_self_list {
    ($args:expr) => {
        match $args.first() {
            Some(Value::List(h)) => h.clone(),
            _ => return Err(RuntimeError::type_error("expected list")),
        }
    };
}

macro_rules! get_self_dict {
    ($args:expr) => {
        match $args.first() {
            Some(Value::Dict(h)) => h.clone(),
            _ => return Err(RuntimeError::type_error("expected dict")),
        }
    };
}

macro_rules! get_self_set {
    ($args:expr) => {
        match $args.first() {
            Some(Value::Set(h)) => h.clone(),
            _ => return Err(RuntimeError::type_error("expected set")),
        }
    };
}

macro_rules! get_self_str {
    ($args:expr) => {
        match $args.first() {
            Some(Value::Str(s)) => s.clone(),
            _ => return Err(RuntimeError::type_error("expected str")),
        }
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// LIST METHODS
// ═══════════════════════════════════════════════════════════════════════════════

pub fn list_append(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let item = args.into_iter().nth(1).unwrap_or(Value::None);
    l.lock().unwrap().push(item);
    Ok(Value::None)
}

pub fn list_pop(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let mut v = l.lock().unwrap();
    if v.is_empty() { return Err(RuntimeError::new("IndexError", "pop from empty list")); }
    let idx = match args.get(1) {
        Some(Value::Int(n)) => {
            let len = v.len() as i64;
            let i = if *n < 0 { len + n } else { *n };
            if i < 0 || i >= len { return Err(RuntimeError::new("IndexError", "pop index out of range")); }
            i as usize
        }
        None | Some(Value::None) => v.len() - 1,
        _ => return Err(RuntimeError::type_error("pop() index must be an integer")),
    };
    Ok(v.remove(idx))
}

pub fn list_extend(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let items: Vec<Value> = match args.into_iter().nth(1).unwrap_or(Value::None) {
        Value::List(h)  => h.lock().unwrap().clone(),
        Value::Tuple(t) => t.as_ref().clone(),
        Value::Str(s)   => s.chars().map(|c| Value::Str(Arc::new(c.to_string()))).collect(),
        other => return Err(RuntimeError::type_error(format!("'{}' is not iterable", other.type_name()))),
    };
    l.lock().unwrap().extend(items);
    Ok(Value::None)
}

pub fn list_insert(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let idx = match args.get(1) {
        Some(Value::Int(n)) => *n,
        _ => return Err(RuntimeError::type_error("insert() index must be int")),
    };
    let item = args.into_iter().nth(2).unwrap_or(Value::None);
    let mut v = l.lock().unwrap();
    let len = v.len() as i64;
    let pos = if idx < 0 { (len + idx).max(0) as usize } else { (idx as usize).min(v.len()) };
    v.insert(pos, item);
    Ok(Value::None)
}

pub fn list_remove(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let target = args.into_iter().nth(1).unwrap_or(Value::None);
    let mut v = l.lock().unwrap();
    let pos = v.iter().position(|x| x.eq_val(&target))
        .ok_or_else(|| RuntimeError::value_error("list.remove(): value not found"))?;
    v.remove(pos);
    Ok(Value::None)
}

pub fn list_index(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let target = args.into_iter().nth(1).unwrap_or(Value::None);
    let v = l.lock().unwrap();
    let pos = v.iter().position(|x| x.eq_val(&target))
        .ok_or_else(|| RuntimeError::value_error("value not in list"))?;
    Ok(Value::Int(pos as i64))
}

pub fn list_count(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let target = args.into_iter().nth(1).unwrap_or(Value::None);
    let v = l.lock().unwrap();
    Ok(Value::Int(v.iter().filter(|x| x.eq_val(&target)).count() as i64))
}

pub fn list_sort(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let reverse = args.get(1).map_or(false, |v| v.is_truthy());
    let mut v = l.lock().unwrap();
    v.sort_by(|a, b| {
        let ord = match (a, b) {
            (Value::Int(x),   Value::Int(y))   => x.cmp(y),
            (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Str(x),   Value::Str(y))   => x.cmp(y),
            (Value::Bool(x),  Value::Bool(y))  => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        };
        if reverse { ord.reverse() } else { ord }
    });
    Ok(Value::None)
}

pub fn list_reverse(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    l.lock().unwrap().reverse();
    Ok(Value::None)
}

pub fn list_clear(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    l.lock().unwrap().clear();
    Ok(Value::None)
}

pub fn list_copy(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let l = get_self_list!(args);
    let v = l.lock().unwrap().clone();
    Ok(Value::List(crate::runtime::gc::alloc_list(v)))
}

// ═══════════════════════════════════════════════════════════════════════════════
// DICT METHODS
// ═══════════════════════════════════════════════════════════════════════════════

pub fn dict_get(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let key = match args.into_iter().nth(1) {
        Some(v) => HashableValue::try_from(&v).map_err(RuntimeError::type_error)?,
        None    => return Err(RuntimeError::type_error("get() requires a key argument")),
    };
    let map = d.lock().unwrap();
    Ok(map.get(&key).cloned().unwrap_or(Value::None))
}

pub fn dict_get_default(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let mut it = args.into_iter().skip(1);
    let key_val = it.next().unwrap_or(Value::None);
    let default  = it.next().unwrap_or(Value::None);
    let key = HashableValue::try_from(&key_val).map_err(RuntimeError::type_error)?;
    let map = d.lock().unwrap();
    Ok(map.get(&key).cloned().unwrap_or(default))
}

pub fn dict_keys(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let keys: Vec<Value> = d.lock().unwrap().keys().cloned().map(Value::from).collect();
    Ok(Value::List(crate::runtime::gc::alloc_list(keys)))
}

pub fn dict_values(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let vals: Vec<Value> = d.lock().unwrap().values().cloned().collect();
    Ok(Value::List(crate::runtime::gc::alloc_list(vals)))
}

pub fn dict_items(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let items: Vec<Value> = d.lock().unwrap().iter()
        .map(|(k, v)| Value::Tuple(Arc::new(vec![Value::from(k.clone()), v.clone()])))
        .collect();
    Ok(Value::List(crate::runtime::gc::alloc_list(items)))
}

pub fn dict_update(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let other = args.into_iter().nth(1).unwrap_or(Value::None);
    if let Value::Dict(other_d) = other {
        let mut dst = d.lock().unwrap();
        for (k, v) in other_d.lock().unwrap().iter() {
            dst.insert(k.clone(), v.clone());
        }
    }
    Ok(Value::None)
}

pub fn dict_pop(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let mut it = args.into_iter().skip(1);
    let key_val = it.next().ok_or_else(|| RuntimeError::type_error("pop() requires a key"))?;
    let default  = it.next();
    let key = HashableValue::try_from(&key_val).map_err(RuntimeError::type_error)?;
    let mut map = d.lock().unwrap();
    match (map.remove(&key), default) {
        (Some(v), _)          => Ok(v),
        (None,    Some(d))    => Ok(d),
        (None,    None)       => Err(RuntimeError::new("KeyError", key_val.repr())),
    }
}

pub fn dict_setdefault(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let mut it = args.into_iter().skip(1);
    let key_val = it.next().ok_or_else(|| RuntimeError::type_error("setdefault() requires a key"))?;
    let default  = it.next().unwrap_or(Value::None);
    let key = HashableValue::try_from(&key_val).map_err(RuntimeError::type_error)?;
    let mut map = d.lock().unwrap();
    Ok(map.entry(key).or_insert(default).clone())
}

pub fn dict_clear(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    d.lock().unwrap().clear();
    Ok(Value::None)
}

pub fn dict_copy(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let m = d.lock().unwrap().clone();
    Ok(Value::Dict(crate::runtime::gc::alloc_dict(m)))
}

pub fn dict_contains(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let d = get_self_dict!(args);
    let key_val = args.into_iter().nth(1).unwrap_or(Value::None);
    let key = HashableValue::try_from(&key_val).map_err(RuntimeError::type_error)?;
    let found = d.lock().unwrap().contains_key(&key);
    Ok(Value::Bool(found))
}

// ═══════════════════════════════════════════════════════════════════════════════
// SET METHODS
// ═══════════════════════════════════════════════════════════════════════════════

pub fn set_add(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let item = args.into_iter().nth(1).unwrap_or(Value::None);
    let h = HashableValue::try_from(&item).map_err(RuntimeError::type_error)?;
    s.lock().unwrap().insert(h);
    Ok(Value::None)
}

pub fn set_remove(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let item = args.into_iter().nth(1).unwrap_or(Value::None);
    let h = HashableValue::try_from(&item).map_err(RuntimeError::type_error)?;
    if !s.lock().unwrap().remove(&h) {
        return Err(RuntimeError::new("KeyError", item.repr()));
    }
    Ok(Value::None)
}

pub fn set_discard(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let item = args.into_iter().nth(1).unwrap_or(Value::None);
    if let Ok(h) = HashableValue::try_from(&item) {
        s.lock().unwrap().remove(&h);
    }
    Ok(Value::None)
}

pub fn set_pop(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let mut guard = s.lock().unwrap();
    let item = guard.iter().next().cloned()
        .ok_or_else(|| RuntimeError::new("KeyError", "pop from an empty set"))?;
    guard.remove(&item);
    Ok(Value::from(item))
}

pub fn set_clear(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    s.lock().unwrap().clear();
    Ok(Value::None)
}

pub fn set_copy(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let copy = s.lock().unwrap().clone();
    Ok(Value::Set(crate::runtime::gc::alloc_set(copy)))
}

pub fn set_union(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let mut result = s.lock().unwrap().clone();
    for other in args.into_iter().skip(1) {
        if let Value::Set(o) = other {
            for item in o.lock().unwrap().iter() {
                result.insert(item.clone());
            }
        }
    }
    Ok(Value::Set(crate::runtime::gc::alloc_set(result)))
}

pub fn set_intersection(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let mut result = s.lock().unwrap().clone();
    for other in args.into_iter().skip(1) {
        if let Value::Set(o) = other {
            let other_set = o.lock().unwrap();
            result.retain(|x| other_set.contains(x));
        }
    }
    Ok(Value::Set(crate::runtime::gc::alloc_set(result)))
}

pub fn set_difference(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let mut result = s.lock().unwrap().clone();
    for other in args.into_iter().skip(1) {
        if let Value::Set(o) = other {
            let other_set = o.lock().unwrap();
            result.retain(|x| !other_set.contains(x));
        }
    }
    Ok(Value::Set(crate::runtime::gc::alloc_set(result)))
}

pub fn set_symmetric_difference(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let a = s.lock().unwrap().clone();
    let b = match args.into_iter().nth(1) {
        Some(Value::Set(o)) => o.lock().unwrap().clone(),
        _ => return Err(RuntimeError::type_error("symmetric_difference() requires a set")),
    };
    let result: std::collections::HashSet<HashableValue> =
        a.symmetric_difference(&b).cloned().collect();
    Ok(Value::Set(crate::runtime::gc::alloc_set(result)))
}

pub fn set_issubset(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let a = s.lock().unwrap();
    let b = match args.get(1) {
        Some(Value::Set(o)) => o.lock().unwrap().clone(),
        _ => return Err(RuntimeError::type_error("issubset() requires a set")),
    };
    Ok(Value::Bool(a.iter().all(|x| b.contains(x))))
}

pub fn set_issuperset(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let a = s.lock().unwrap();
    let b = match args.get(1) {
        Some(Value::Set(o)) => o.lock().unwrap().clone(),
        _ => return Err(RuntimeError::type_error("issuperset() requires a set")),
    };
    Ok(Value::Bool(b.iter().all(|x| a.contains(x))))
}

pub fn set_isdisjoint(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let a = s.lock().unwrap();
    let b = match args.get(1) {
        Some(Value::Set(o)) => o.lock().unwrap().clone(),
        _ => return Err(RuntimeError::type_error("isdisjoint() requires a set")),
    };
    Ok(Value::Bool(a.iter().all(|x| !b.contains(x))))
}

pub fn set_update(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    for other in args.into_iter().skip(1) {
        if let Value::Set(o) = other {
            let items: Vec<HashableValue> = o.lock().unwrap().iter().cloned().collect();
            let mut guard = s.lock().unwrap();
            for item in items { guard.insert(item); }
        }
    }
    Ok(Value::None)
}

pub fn set_intersection_update(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let b = match args.get(1) {
        Some(Value::Set(o)) => o.lock().unwrap().clone(),
        _ => return Err(RuntimeError::type_error("intersection_update() requires a set")),
    };
    s.lock().unwrap().retain(|x| b.contains(x));
    Ok(Value::None)
}

pub fn set_difference_update(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let b = match args.get(1) {
        Some(Value::Set(o)) => o.lock().unwrap().clone(),
        _ => return Err(RuntimeError::type_error("difference_update() requires a set")),
    };
    s.lock().unwrap().retain(|x| !b.contains(x));
    Ok(Value::None)
}

pub fn set_contains(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_set!(args);
    let item = args.into_iter().nth(1).unwrap_or(Value::None);
    if let Ok(h) = HashableValue::try_from(&item) {
        return Ok(Value::Bool(s.lock().unwrap().contains(&h)));
    }
    Ok(Value::Bool(false))
}

// ═══════════════════════════════════════════════════════════════════════════════
// STR METHODS
// ═══════════════════════════════════════════════════════════════════════════════

pub fn str_split(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let sep = args.get(1).and_then(|v| if let Value::Str(sep) = v { Some(sep.clone()) } else { None });
    let maxsplit = args.get(2).and_then(|v| if let Value::Int(n) = v { Some(*n) } else { None });

    let parts: Vec<Value> = if let Some(sep) = sep {
        if sep.is_empty() { return Err(RuntimeError::value_error("empty separator")); }
        match maxsplit {
            Some(n) if n >= 0 => s.splitn(n as usize + 1, sep.as_str())
                .map(|p| Value::Str(Arc::new(p.to_string()))).collect(),
            _ => s.split(sep.as_str())
                .map(|p| Value::Str(Arc::new(p.to_string()))).collect(),
        }
    } else {
        match maxsplit {
            Some(n) if n >= 0 => s.splitn(n as usize + 1, |c: char| c.is_whitespace())
                .filter(|p| !p.is_empty())
                .map(|p| Value::Str(Arc::new(p.to_string()))).collect(),
            _ => s.split_whitespace()
                .map(|p| Value::Str(Arc::new(p.to_string()))).collect(),
        }
    };
    Ok(Value::List(crate::runtime::gc::alloc_list(parts)))
}

pub fn str_rsplit(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let sep = args.get(1).and_then(|v| if let Value::Str(sep) = v { Some(sep.clone()) } else { None });
    let maxsplit = args.get(2).and_then(|v| if let Value::Int(n) = v { Some(*n as usize) } else { None });

    let parts: Vec<Value> = if let Some(sep) = sep {
        match maxsplit {
            Some(n) => s.rsplitn(n + 1, sep.as_str())
                .map(|p| Value::Str(Arc::new(p.to_string()))).collect::<Vec<_>>()
                .into_iter().rev().collect(),
            _ => s.split(sep.as_str())
                .map(|p| Value::Str(Arc::new(p.to_string()))).collect(),
        }
    } else {
        s.split_whitespace().map(|p| Value::Str(Arc::new(p.to_string()))).collect()
    };
    Ok(Value::List(crate::runtime::gc::alloc_list(parts)))
}

pub fn str_join(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let sep = get_self_str!(args);
    let iterable = args.into_iter().nth(1).unwrap_or(Value::None);
    let items: Vec<String> = match iterable {
        Value::List(l)  => l.lock().unwrap().iter().map(|v| v.str_display()).collect(),
        Value::Tuple(t) => t.iter().map(|v| v.str_display()).collect(),
        other => return Err(RuntimeError::type_error(format!("'{}' is not iterable", other.type_name()))),
    };
    Ok(Value::Str(Arc::new(items.join(&*sep))))
}

pub fn str_replace(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let old = match args.get(1) { Some(Value::Str(o)) => o.clone(), _ => return Err(RuntimeError::type_error("replace() requires str")) };
    let new = match args.get(2) { Some(Value::Str(n)) => n.clone(), _ => return Err(RuntimeError::type_error("replace() requires str")) };
    let count = args.get(3).and_then(|v| if let Value::Int(n) = v { Some(*n as usize) } else { None });
    let result = match count {
        Some(n) => s.replacen(old.as_str(), new.as_str(), n),
        None    => s.replace(old.as_str(), new.as_str()),
    };
    Ok(Value::Str(Arc::new(result)))
}

pub fn str_startswith(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let prefix = match args.get(1) { Some(Value::Str(p)) => p.clone(), _ => return Ok(Value::Bool(false)) };
    Ok(Value::Bool(s.starts_with(prefix.as_str())))
}

pub fn str_endswith(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let suffix = match args.get(1) { Some(Value::Str(p)) => p.clone(), _ => return Ok(Value::Bool(false)) };
    Ok(Value::Bool(s.ends_with(suffix.as_str())))
}

pub fn str_find(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let sub = match args.get(1) { Some(Value::Str(p)) => p.clone(), _ => return Ok(Value::Int(-1)) };
    Ok(Value::Int(s.find(sub.as_str()).map(|i| i as i64).unwrap_or(-1)))
}

pub fn str_rfind(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let sub = match args.get(1) { Some(Value::Str(p)) => p.clone(), _ => return Ok(Value::Int(-1)) };
    Ok(Value::Int(s.rfind(sub.as_str()).map(|i| i as i64).unwrap_or(-1)))
}

pub fn str_index(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let sub = match args.get(1) { Some(Value::Str(p)) => p.clone(), _ => return Err(RuntimeError::type_error("index() requires str")) };
    s.find(sub.as_str())
        .map(|i| Ok(Value::Int(i as i64)))
        .unwrap_or_else(|| Err(RuntimeError::value_error("substring not found")))
}

pub fn str_count(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let sub = match args.get(1) { Some(Value::Str(p)) => p.clone(), _ => return Err(RuntimeError::type_error("count() requires str")) };
    if sub.is_empty() { return Ok(Value::Int(s.chars().count() as i64 + 1)); }
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = s[start..].find(sub.as_str()) {
        count += 1;
        start += pos + sub.len();
    }
    Ok(Value::Int(count))
}

pub fn str_strip(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let result = match args.get(1) {
        Some(Value::Str(chars)) => {
            let set: std::collections::HashSet<char> = chars.chars().collect();
            s.trim_matches(|c| set.contains(&c)).to_string()
        }
        _ => s.trim().to_string(),
    };
    Ok(Value::Str(Arc::new(result)))
}

pub fn str_lstrip(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let result = match args.get(1) {
        Some(Value::Str(chars)) => {
            let set: std::collections::HashSet<char> = chars.chars().collect();
            s.trim_start_matches(|c| set.contains(&c)).to_string()
        }
        _ => s.trim_start().to_string(),
    };
    Ok(Value::Str(Arc::new(result)))
}

pub fn str_rstrip(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let result = match args.get(1) {
        Some(Value::Str(chars)) => {
            let set: std::collections::HashSet<char> = chars.chars().collect();
            s.trim_end_matches(|c| set.contains(&c)).to_string()
        }
        _ => s.trim_end().to_string(),
    };
    Ok(Value::Str(Arc::new(result)))
}

pub fn str_encode(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    Ok(Value::Bytes(Arc::new(s.as_bytes().to_vec())))
}

pub fn str_format(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let mut result = s.as_ref().clone();
    let mut pos_idx = 0usize;
    let mut out = String::new();
    let mut chars = result.chars().peekable();
    let fmt_args = &args[1..];

    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') { chars.next(); out.push('{'); continue; }
            let mut spec = String::new();
            for ch in chars.by_ref() {
                if ch == '}' { break; }
                spec.push(ch);
            }
            let val = if spec.is_empty() {
                fmt_args.get(pos_idx).cloned().unwrap_or(Value::None)
            } else if let Ok(n) = spec.parse::<usize>() {
                fmt_args.get(n).cloned().unwrap_or(Value::None)
            } else {
                Value::Str(Arc::new(format!("{{{}}}", spec)))
            };
            if spec.is_empty() { pos_idx += 1; }
            out.push_str(&val.str_display());
        } else if c == '}' && chars.peek() == Some(&'}') {
            chars.next(); out.push('}');
        } else {
            out.push(c);
        }
    }
    let _ = result; // suppress unused
    Ok(Value::Str(Arc::new(out)))
}

pub fn str_zfill(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let width = match args.get(1) { Some(Value::Int(n)) => *n as usize, _ => return Ok(Value::Str(s)) };
    if s.len() >= width { return Ok(Value::Str(s)); }
    let pad = width - s.len();
    let result = if s.starts_with('-') || s.starts_with('+') {
        format!("{}{}{}", &s[..1], "0".repeat(pad), &s[1..])
    } else {
        format!("{}{}", "0".repeat(pad), *s)
    };
    Ok(Value::Str(Arc::new(result)))
}

pub fn str_center(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let width = match args.get(1) { Some(Value::Int(n)) => *n as usize, _ => return Ok(Value::Str(s)) };
    let fill = match args.get(2) { Some(Value::Str(c)) => c.chars().next().unwrap_or(' '), _ => ' ' };
    if s.len() >= width { return Ok(Value::Str(s)); }
    let pad = width - s.len();
    let left = pad / 2; let right = pad - left;
    Ok(Value::Str(Arc::new(format!("{}{}{}", fill.to_string().repeat(left), *s, fill.to_string().repeat(right)))))
}

pub fn str_ljust(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let width = match args.get(1) { Some(Value::Int(n)) => *n as usize, _ => return Ok(Value::Str(s)) };
    let fill = match args.get(2) { Some(Value::Str(c)) => c.chars().next().unwrap_or(' '), _ => ' ' };
    if s.len() >= width { return Ok(Value::Str(s)); }
    Ok(Value::Str(Arc::new(format!("{}{}", *s, fill.to_string().repeat(width - s.len())))))
}

pub fn str_rjust(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let width = match args.get(1) { Some(Value::Int(n)) => *n as usize, _ => return Ok(Value::Str(s)) };
    let fill = match args.get(2) { Some(Value::Str(c)) => c.chars().next().unwrap_or(' '), _ => ' ' };
    if s.len() >= width { return Ok(Value::Str(s)); }
    Ok(Value::Str(Arc::new(format!("{}{}", fill.to_string().repeat(width - s.len()), *s))))
}

pub fn str_splitlines(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let lines: Vec<Value> = s.lines().map(|l| Value::Str(Arc::new(l.to_string()))).collect();
    Ok(Value::List(crate::runtime::gc::alloc_list(lines)))
}

pub fn str_expandtabs(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = get_self_str!(args);
    let tabsize = match args.get(1) { Some(Value::Int(n)) => *n as usize, _ => 8 };
    Ok(Value::Str(Arc::new(s.replace('\t', &" ".repeat(tabsize)))))
}

// ═══════════════════════════════════════════════════════════════════════════════
// TUPLE METHODS
// ═══════════════════════════════════════════════════════════════════════════════

pub fn tuple_count(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let t = match args.first() { Some(Value::Tuple(t)) => t.clone(), _ => return Err(RuntimeError::type_error("expected tuple")) };
    let item = args.into_iter().nth(1).unwrap_or(Value::None);
    Ok(Value::Int(t.iter().filter(|x| x.eq_val(&item)).count() as i64))
}

pub fn tuple_index(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let t = match args.first() { Some(Value::Tuple(t)) => t.clone(), _ => return Err(RuntimeError::type_error("expected tuple")) };
    let item = args.into_iter().nth(1).unwrap_or(Value::None);
    t.iter().position(|x| x.eq_val(&item))
        .map(|i| Ok(Value::Int(i as i64)))
        .unwrap_or_else(|| Err(RuntimeError::value_error("tuple.index(): value not found")))
}
