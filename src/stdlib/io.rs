use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use crate::runtime::value::*;
use crate::interpreter::evaluator::RuntimeError;

pub fn make_io_module() -> Value {
    let mut attrs = HashMap::new();
    attrs.insert("open".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "open", func: builtin_open })));
    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "io".into(), attrs })))
}

/// Global `open()` builtin (also registered directly in the evaluator's builtins).
pub fn builtin_open(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let path = match args.first() {
        Some(Value::Str(s)) => s.as_ref().clone(),
        _ => return Err(RuntimeError::type_error("open() requires a filename string")),
    };
    let mode = match args.get(1) {
        Some(Value::Str(s)) => s.as_ref().clone(),
        None => "r".into(),
        _ => return Err(RuntimeError::type_error("open() mode must be a string")),
    };
    make_file_object(path, mode)
}

// ── File object factory ────────────────────────────────────────────────────────

fn make_file_object(path: String, mode: String) -> Result<Value, RuntimeError> {
    let is_read    = mode.contains('r');
    let is_write   = mode.contains('w');
    let is_append  = mode.contains('a');
    let is_binary  = mode.contains('b');
    let is_create  = mode.contains('x');

    // Read initial content for read mode
    let content = if is_read {
        if is_binary {
            std::fs::read(&path)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .map_err(|e| RuntimeError::new("IOError", e.to_string()))?
        } else {
            std::fs::read_to_string(&path)
                .map_err(|e| RuntimeError::new("IOError", e.to_string()))?
        }
    } else if is_append {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else if is_create {
        if std::path::Path::new(&path).exists() {
            return Err(RuntimeError::new("FileExistsError", format!("File '{}' already exists", path)));
        }
        String::new()
    } else {
        // write mode: truncate
        String::new()
    };

    let append_start = if is_append { content.len() } else { 0 };

    let mut fields = HashMap::new();
    fields.insert("name".into(),     Value::Str(Arc::new(path.clone())));
    fields.insert("mode".into(),     Value::Str(Arc::new(mode.clone())));
    fields.insert("_content".into(), Value::Str(Arc::new(content)));
    fields.insert("_pos".into(),     Value::Int(append_start as i64));
    fields.insert("_closed".into(),  Value::Bool(false));
    fields.insert("_writable".into(),Value::Bool(is_write || is_append || is_create));
    fields.insert("_path".into(),    Value::Str(Arc::new(path)));

    let mut methods: HashMap<String, Value> = HashMap::new();
    macro_rules! bm {
        ($name:expr, $f:expr) => {
            methods.insert($name.into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: $name, func: $f })));
        }
    }
    // Note: these are plain BuiltinFn. We use BoundBuiltin dispatch at call time
    // by registering them as BoundBuiltin in Instance getattr.
    // But since Instance getattr calls bind_method (which only handles Function),
    // we instead store them as BuiltinFn and retrieve self from the instance via
    // a different mechanism. To work around this, we store BuiltinFn functions
    // that receive args as [self_instance, ...] via the `with` / call path.
    //
    // Actually: Instance getattr in the evaluator calls bind_method which only
    // wraps Function, not BuiltinFunction. So when `f.read()` is called, the
    // evaluator gets the BuiltinFn from the class methods and calls it WITHOUT
    // self. We need to change the approach.
    //
    // Solution: store methods as BoundBuiltin in the instance fields directly,
    // pre-bound to the instance handle.

    // We'll create the instance first, then patch in the methods as BoundBuiltin.
    let inst_handle = crate::runtime::gc::alloc_instance(Instance {
        class_name: "TextIOWrapper".into(),
        class: Arc::new(Class {
            name: "TextIOWrapper".into(),
            bases: vec![],
            methods: HashMap::new(),
            class_vars: HashMap::new(),
        }),
        fields,
    });

    // Now pre-bind all methods
    {
        let mut guard = inst_handle.lock().unwrap();
        macro_rules! pre_bind {
            ($name:expr, $f:expr) => {
                guard.fields.insert($name.into(), Value::BoundBuiltin {
                    receiver: Box::new(Value::Instance(inst_handle.clone())),
                    name: $name,
                    func: $f,
                });
            }
        }
        pre_bind!("read",       file_read);
        pre_bind!("readline",   file_readline);
        pre_bind!("readlines",  file_readlines);
        pre_bind!("write",      file_write);
        pre_bind!("writelines", file_writelines);
        pre_bind!("seek",       file_seek);
        pre_bind!("tell",       file_tell);
        pre_bind!("close",      file_close);
        pre_bind!("flush",      file_flush);
        pre_bind!("__enter__",  file_enter);
        pre_bind!("__exit__",   file_exit);
        pre_bind!("readable",   file_readable);
        pre_bind!("writable",   file_writable);
    }

    Ok(Value::Instance(inst_handle))
}

// ── File method helpers ───────────────────────────────────────────────────────

fn get_field_str(inst: &Value, key: &str) -> String {
    match inst {
        Value::Instance(h) => {
            let guard = h.lock().unwrap();
            match guard.fields.get(key) {
                Some(Value::Str(s)) => s.as_ref().clone(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}

fn get_field_int(inst: &Value, key: &str) -> i64 {
    match inst {
        Value::Instance(h) => {
            let guard = h.lock().unwrap();
            match guard.fields.get(key) { Some(Value::Int(n)) => *n, _ => 0 }
        }
        _ => 0,
    }
}

fn set_field(inst: &Value, key: &str, val: Value) {
    if let Value::Instance(h) = inst {
        h.lock().unwrap().fields.insert(key.to_string(), val);
    }
}

fn check_open(inst: &Value) -> Result<(), RuntimeError> {
    match inst {
        Value::Instance(h) => {
            let guard = h.lock().unwrap();
            if matches!(guard.fields.get("_closed"), Some(Value::Bool(true))) {
                return Err(RuntimeError::new("ValueError", "I/O operation on closed file"));
            }
        }
        _ => {}
    }
    Ok(())
}

fn flush_writes(inst: &Value) -> Result<(), RuntimeError> {
    let writable = match inst {
        Value::Instance(h) => matches!(h.lock().unwrap().fields.get("_writable"), Some(Value::Bool(true))),
        _ => false,
    };
    if !writable { return Ok(()); }
    let path    = get_field_str(inst, "_path");
    let content = get_field_str(inst, "_content");
    std::fs::write(&path, &content)
        .map_err(|e| RuntimeError::new("IOError", e.to_string()))
}

// ── File methods ──────────────────────────────────────────────────────────────

fn file_read(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("read() missing self"))?;
    check_open(self_val)?;
    let content = get_field_str(self_val, "_content");
    let pos = get_field_int(self_val, "_pos") as usize;
    let size = match args.get(1) {
        Some(Value::Int(n)) if *n >= 0 => Some(*n as usize),
        _ => None,
    };
    let slice = &content[pos.min(content.len())..];
    let result = match size {
        Some(n) => &slice[..n.min(slice.len())],
        None    => slice,
    };
    let new_pos = pos + result.len();
    set_field(self_val, "_pos", Value::Int(new_pos as i64));
    Ok(Value::Str(Arc::new(result.to_string())))
}

fn file_readline(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("readline() missing self"))?;
    check_open(self_val)?;
    let content = get_field_str(self_val, "_content");
    let pos = get_field_int(self_val, "_pos") as usize;
    let slice = &content[pos.min(content.len())..];
    let end = slice.find('\n').map(|i| i + 1).unwrap_or(slice.len());
    let line = &slice[..end];
    set_field(self_val, "_pos", Value::Int((pos + end) as i64));
    Ok(Value::Str(Arc::new(line.to_string())))
}

fn file_readlines(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("readlines() missing self"))?;
    check_open(self_val)?;
    let content = get_field_str(self_val, "_content");
    let pos = get_field_int(self_val, "_pos") as usize;
    let slice = &content[pos.min(content.len())..];
    let lines: Vec<Value> = slice.lines()
        .map(|l| Value::Str(Arc::new(format!("{}\n", l))))
        .collect();
    set_field(self_val, "_pos", Value::Int(content.len() as i64));
    Ok(Value::List(crate::runtime::gc::alloc_list(lines)))
}

fn file_write(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("write() missing self"))?;
    check_open(self_val)?;
    let writable = match self_val {
        Value::Instance(h) => matches!(h.lock().unwrap().fields.get("_writable"), Some(Value::Bool(true))),
        _ => false,
    };
    if !writable { return Err(RuntimeError::new("IOError", "File not open for writing")); }
    let data = match args.get(1) {
        Some(Value::Str(s)) => s.as_ref().clone(),
        Some(v) => v.str_display(),
        None => return Err(RuntimeError::type_error("write() requires a string argument")),
    };
    let n = data.len();
    let mut content = get_field_str(self_val, "_content");
    content.push_str(&data);
    set_field(self_val, "_content", Value::Str(Arc::new(content)));
    Ok(Value::Int(n as i64))
}

fn file_writelines(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("writelines() missing self"))?;
    let lines = match args.get(1) {
        Some(Value::List(l)) => l.lock().unwrap().clone(),
        Some(Value::Tuple(t)) => t.as_ref().clone(),
        _ => return Err(RuntimeError::type_error("writelines() requires iterable")),
    };
    for line in lines {
        file_write(vec![self_val.clone(), line])?;
    }
    Ok(Value::None)
}

fn file_seek(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("seek() missing self"))?;
    check_open(self_val)?;
    let pos = match args.get(1) { Some(Value::Int(n)) => *n, _ => 0 };
    set_field(self_val, "_pos", Value::Int(pos));
    Ok(Value::Int(pos))
}

fn file_tell(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("tell() missing self"))?;
    check_open(self_val)?;
    Ok(Value::Int(get_field_int(self_val, "_pos")))
}

fn file_close(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("close() missing self"))?;
    flush_writes(self_val)?;
    set_field(self_val, "_closed", Value::Bool(true));
    Ok(Value::None)
}

fn file_flush(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("flush() missing self"))?;
    flush_writes(self_val)
        .map(|_| Value::None)
}

fn file_enter(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(args.into_iter().next().unwrap_or(Value::None))
}

fn file_exit(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(self_val) = args.first() {
        let _ = flush_writes(self_val);
        set_field(self_val, "_closed", Value::Bool(true));
    }
    Ok(Value::Bool(false))
}

fn file_readable(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("readable() missing self"))?;
    let mode = get_field_str(self_val, "mode");
    Ok(Value::Bool(mode.contains('r')))
}

fn file_writable(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let self_val = args.first().ok_or_else(|| RuntimeError::type_error("writable() missing self"))?;
    match self_val {
        Value::Instance(h) => Ok(Value::Bool(matches!(h.lock().unwrap().fields.get("_writable"), Some(Value::Bool(true))))),
        _ => Ok(Value::Bool(false)),
    }
}
