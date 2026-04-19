use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::runtime::value::*;

pub fn make_sys_module() -> Value {
    let mut attrs = HashMap::new();
    let argv: Vec<Value> = std::env::args().map(|a| Value::Str(Arc::new(a))).collect();
    attrs.insert("argv".into(), Value::List(crate::runtime::gc::alloc_list(argv)));
    attrs.insert("version".into(), Value::Str(Arc::new("Cherash 0.1.0".into())));
    attrs.insert("platform".into(), Value::Str(Arc::new(std::env::consts::OS.into())));
    attrs.insert("maxsize".into(), Value::Int(i64::MAX));
    attrs.insert("exit".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "exit", func: |args| {
        let code = match args.first() { Some(Value::Int(n)) => *n as i32, _ => 0 };
        std::process::exit(code);
    }})));
    attrs.insert("stdout".into(), Value::None);
    attrs.insert("stderr".into(), Value::None);
    attrs.insert("stdin".into(),  Value::None);
    attrs.insert("path".into(), Value::List(crate::runtime::gc::alloc_list(vec![])));

    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "sys".into(), attrs })))
}
