use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::fmt;

use crate::parser::ast::{Param, Stmt};
use crate::runtime::gc::GcHandle;

/// The central value type for the Cherash runtime.
/// All heap-allocated mutable containers are wrapped in Arc<Mutex<>> for thread safety.
#[derive(Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Arc<String>),
    Bytes(Arc<Vec<u8>>),
    None,
    Ellipsis,

    List(GcHandle<Vec<Value>>),
    Tuple(Arc<Vec<Value>>),
    Set(GcHandle<std::collections::HashSet<HashableValue>>),
    Dict(GcHandle<HashMap<HashableValue, Value>>),

    Function(Arc<Function>),
    BuiltinFunction(Arc<BuiltinFn>),
    Class(Arc<Class>),
    Instance(GcHandle<Instance>),
    Module(Arc<Mutex<ModuleObj>>),

    // Raised exception value
    Exception(Arc<Mutex<ExceptionObj>>),

    // Range (used internally by range() builtin)
    Range(i64, i64, i64),

    // Bound method: receiver + user-defined function
    BoundMethod { receiver: Box<Value>, method: Arc<Function> },
    // Bound builtin: receiver prepended to args when called
    BoundBuiltin { receiver: Box<Value>, name: &'static str, func: BuiltinFnPtr },
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.repr())
    }
}

impl Value {
    pub fn repr(&self) -> String {
        match self {
            Value::Int(n)   => n.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 && f.is_finite() { format!("{:.1}", f) }
                else { f.to_string() }
            }
            Value::Bool(b)  => if *b { "True".into() } else { "False".into() }
            Value::Str(s)   => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
            Value::Bytes(b) => format!("b'{}'", String::from_utf8_lossy(b)),
            Value::None     => "None".into(),
            Value::Ellipsis => "...".into(),
            Value::List(l)  => {
                let v = l.lock().unwrap();
                format!("[{}]", v.iter().map(|x| x.repr()).collect::<Vec<_>>().join(", "))
            }
            Value::Tuple(t) => {
                if t.len() == 1 { format!("({},)", t[0].repr()) }
                else { format!("({})", t.iter().map(|x| x.repr()).collect::<Vec<_>>().join(", ")) }
            }
            Value::Set(s)   => {
                let v = s.lock().unwrap();
                let mut items: Vec<_> = v.iter().map(|x| Value::from(x.clone()).repr()).collect();
                items.sort();
                format!("{{{}}}", items.join(", "))
            }
            Value::Dict(d)  => {
                let m = d.lock().unwrap();
                let items: Vec<_> = m.iter()
                    .map(|(k, v)| format!("{}: {}", Value::from(k.clone()).repr(), v.repr()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            Value::Function(f)  => format!("<function {} at {:p}>", f.name, Arc::as_ptr(f)),
            Value::BuiltinFunction(f) => format!("<built-in function {}>", f.name),
            Value::Class(c) => format!("<class '{}'>", c.name),
            Value::Instance(i) => {
                let inst = i.lock().unwrap();
                format!("<{} object>", inst.class_name)
            }
            Value::Module(m) => {
                let mo = m.lock().unwrap();
                format!("<module '{}'>", mo.name)
            }
            Value::Exception(e) => {
                let ex = e.lock().unwrap();
                format!("{}({})", ex.type_name, ex.message)
            }
            Value::BoundMethod { method, .. }    => format!("<bound method {}>", method.name),
            Value::BoundBuiltin { name, .. }     => format!("<built-in method {}>", name),
            Value::Range(start, stop, step) => {
                if *step == 1 { format!("range({}, {})", start, stop) }
                else { format!("range({}, {}, {})", start, stop, step) }
            }
        }
    }

    pub fn str_display(&self) -> String {
        match self {
            Value::Str(s) => s.as_ref().clone(),
            Value::Bool(b) => if *b { "True".into() } else { "False".into() },
            _ => self.repr(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_)             => "int",
            Value::Float(_)           => "float",
            Value::Bool(_)            => "bool",
            Value::Str(_)             => "str",
            Value::Bytes(_)           => "bytes",
            Value::None               => "NoneType",
            Value::Ellipsis           => "ellipsis",
            Value::List(_)            => "list",
            Value::Tuple(_)           => "tuple",
            Value::Set(_)             => "set",
            Value::Dict(_)            => "dict",
            Value::Function(_)        => "function",
            Value::BuiltinFunction(_) => "builtin_function_or_method",
            Value::Class(_)           => "type",
            Value::Instance(_)        => "object",
            Value::Module(_)          => "module",
            Value::Exception(_)       => "Exception",
            Value::Range(..)          => "range",
            Value::BoundMethod { .. }  => "method",
            Value::BoundBuiltin { .. } => "builtin_method",
        }
    }

    pub fn gc_id(&self) -> Option<usize> {
        match self {
            Value::List(h)     => Some(h.id()),
            Value::Dict(h)     => Some(h.id()),
            Value::Set(h)      => Some(h.id()),
            Value::Instance(h) => Some(h.id()),
            _                  => None,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b)  => *b,
            Value::Int(n)   => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::None     => false,
            Value::Str(s)   => !s.is_empty(),
            Value::Bytes(b) => !b.is_empty(),
            Value::List(l)  => !l.lock().unwrap().is_empty(),
            Value::Tuple(t) => !t.is_empty(),
            Value::Set(s)   => !s.lock().unwrap().is_empty(),
            Value::Dict(d)  => !d.lock().unwrap().is_empty(),
            Value::Range(s, e, step) => {
                if *step > 0 { s < e } else { s > e }
            }
            _               => true,
        }
    }

    /// Integer equality (for use in `==` comparisons, including Bool == Int)
    pub fn eq_val(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Int(a),   Value::Int(b))   => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a),   Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b))   => *a == (*b as f64),
            (Value::Bool(a),  Value::Bool(b))  => a == b,
            (Value::Bool(a),  Value::Int(b))   => (*a as i64) == *b,
            (Value::Int(a),   Value::Bool(b))  => *a == (*b as i64),
            (Value::Str(a),   Value::Str(b))   => a == b,
            (Value::None,     Value::None)     => true,
            (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.eq_val(y))
            }
            (Value::List(a), Value::List(b)) => {
                let a = a.lock().unwrap();
                let b = b.lock().unwrap();
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.eq_val(y))
            }
            _ => false,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool { self.eq_val(other) }
}

/// Hashable subset of Value (for use as dict keys / set elements).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HashableValue {
    Int(i64),
    Float(ordered_float::NotNan),
    Bool(bool),
    Str(String),
    Bytes(Vec<u8>),
    Tuple(Vec<HashableValue>),
    None,
}

// We use a tiny wrapper for f64 hashing. Rather than add a dep, we'll restrict
// float keys to non-NaN at insertion time.
mod ordered_float {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct NotNan(pub u64); // bit-cast bits
    impl NotNan {
        pub fn new(f: f64) -> Self { NotNan(f.to_bits()) }
    }
    impl std::fmt::Display for NotNan {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", f64::from_bits(self.0))
        }
    }
}

impl TryFrom<&Value> for HashableValue {
    type Error = String;
    fn try_from(v: &Value) -> Result<Self, Self::Error> {
        match v {
            Value::Int(n)   => Ok(HashableValue::Int(*n)),
            Value::Bool(b)  => Ok(HashableValue::Bool(*b)),
            Value::Float(f) => {
                if f.is_nan() { Err("unhashable: NaN float".into()) }
                else { Ok(HashableValue::Float(ordered_float::NotNan::new(*f))) }
            }
            Value::Str(s)   => Ok(HashableValue::Str(s.as_ref().clone())),
            Value::Bytes(b) => Ok(HashableValue::Bytes(b.as_ref().clone())),
            Value::None     => Ok(HashableValue::None),
            Value::Tuple(t) => {
                let items: Result<Vec<_>, _> = t.iter().map(|x| HashableValue::try_from(x)).collect();
                Ok(HashableValue::Tuple(items?))
            }
            _ => Err(format!("unhashable type: '{}'", v.type_name())),
        }
    }
}

impl From<HashableValue> for Value {
    fn from(h: HashableValue) -> Self {
        match h {
            HashableValue::Int(n)   => Value::Int(n),
            HashableValue::Bool(b)  => Value::Bool(b),
            HashableValue::Float(f) => Value::Float(f64::from_bits(f.0)),
            HashableValue::Str(s)   => Value::Str(Arc::new(s)),
            HashableValue::Bytes(b) => Value::Bytes(Arc::new(b)),
            HashableValue::None     => Value::None,
            HashableValue::Tuple(t) => Value::Tuple(Arc::new(t.into_iter().map(Value::from).collect())),
        }
    }
}

// ── Function ─────────────────────────────────────────────────────────────────

pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub closure_env: crate::interpreter::environment::EnvRef,
    pub is_async: bool,
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function {}>", self.name)
    }
}

// ── Builtin function ──────────────────────────────────────────────────────────

pub type BuiltinFnPtr = fn(Vec<Value>) -> Result<Value, crate::interpreter::evaluator::RuntimeError>;

pub struct BuiltinFn {
    pub name: &'static str,
    pub func: BuiltinFnPtr,
}

impl fmt::Debug for BuiltinFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<built-in {}>", self.name)
    }
}

// ── Class ────────────────────────────────────────────────────────────────────

pub struct Class {
    pub name: String,
    pub bases: Vec<Arc<Class>>,
    pub methods: HashMap<String, Value>,
    pub class_vars: HashMap<String, Value>,
}

impl fmt::Debug for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<class '{}'>", self.name)
    }
}

// ── Instance ─────────────────────────────────────────────────────────────────

pub struct Instance {
    pub class_name: String,
    pub class: Arc<Class>,
    pub fields: HashMap<String, Value>,
}

// ── Module object ────────────────────────────────────────────────────────────

pub struct ModuleObj {
    pub name: String,
    pub attrs: HashMap<String, Value>,
}

// ── Exception ────────────────────────────────────────────────────────────────

pub struct ExceptionObj {
    pub type_name: String,
    pub message: String,
    pub traceback: Vec<TracebackEntry>,
    pub cause: Option<Arc<Mutex<ExceptionObj>>>,
    pub context: Option<Arc<Mutex<ExceptionObj>>>,
}

#[derive(Clone, Debug)]
pub struct TracebackEntry {
    pub file: String,
    pub line: usize,
    pub name: String,
}
