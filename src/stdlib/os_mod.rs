use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::runtime::value::*;

pub fn make_os_module() -> Value {
    let mut attrs = HashMap::new();
    attrs.insert("getcwd".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "getcwd", func: |_| {
        Ok(Value::Str(Arc::new(std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned())))
    }})));
    attrs.insert("listdir".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "listdir", func: |args| {
        let path = match args.first() { Some(Value::Str(s)) => s.as_ref().clone(), _ => ".".into() };
        let entries: Vec<Value> = std::fs::read_dir(&path)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| Value::Str(Arc::new(e.file_name().to_string_lossy().into_owned())))).collect())
            .unwrap_or_default();
        Ok(Value::List(crate::runtime::gc::alloc_list(entries)))
    }})));
    attrs.insert("path".into(), {
        let mut path_attrs = HashMap::new();
        path_attrs.insert("join".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "join", func: |args| {
            let parts: Vec<String> = args.iter().map(|v| v.str_display()).collect();
            Ok(Value::Str(Arc::new(parts.join(std::path::MAIN_SEPARATOR_STR))))
        }})));
        path_attrs.insert("exists".into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: "exists", func: |args| {
            let path = match args.first() { Some(Value::Str(s)) => s.as_ref().clone(), _ => return Ok(Value::Bool(false)) };
            Ok(Value::Bool(std::path::Path::new(&path).exists()))
        }})));
        Value::Module(Arc::new(Mutex::new(ModuleObj { name: "os.path".into(), attrs: path_attrs })))
    });
    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "os".into(), attrs })))
}
