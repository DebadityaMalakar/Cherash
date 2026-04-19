use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::runtime::value::Value;

/// A single scope frame. Wrapped in Arc<Mutex<>> so closures can share it.
pub struct Env {
    pub vars: HashMap<String, Value>,
    parent: Option<EnvRef>,
}

pub type EnvRef = Arc<Mutex<Env>>;

impl Env {
    pub fn new_root() -> EnvRef {
        Arc::new(Mutex::new(Env { vars: HashMap::new(), parent: None }))
    }

    pub fn new_child(parent: EnvRef) -> EnvRef {
        Arc::new(Mutex::new(Env { vars: HashMap::new(), parent: Some(parent) }))
    }

    /// LEGB lookup.
    pub fn get(env: &EnvRef, name: &str) -> Option<Value> {
        let frame = env.lock().unwrap();
        if let Some(v) = frame.vars.get(name) {
            return Some(v.clone());
        }
        if let Some(ref parent) = frame.parent {
            let parent_clone = parent.clone();
            drop(frame);
            return Env::get(&parent_clone, name);
        }
        None
    }

    /// Assign in the local frame.
    pub fn set_local(env: &EnvRef, name: &str, value: Value) {
        env.lock().unwrap().vars.insert(name.to_string(), value);
    }

    /// Walk up to find and update an existing binding (for nonlocal / augmented assign).
    /// Returns true if found.
    pub fn set_existing(env: &EnvRef, name: &str, value: Value) -> bool {
        let mut frame = env.lock().unwrap();
        if frame.vars.contains_key(name) {
            frame.vars.insert(name.to_string(), value);
            return true;
        }
        if let Some(ref parent) = frame.parent {
            let parent_clone = parent.clone();
            drop(frame);
            return Env::set_existing(&parent_clone, name, value);
        }
        false
    }

    /// Assign in the global (root) frame.
    pub fn set_global(env: &EnvRef, name: &str, value: Value) {
        let root = Env::get_root(env);
        root.lock().unwrap().vars.insert(name.to_string(), value);
    }

    fn get_root(env: &EnvRef) -> EnvRef {
        let frame = env.lock().unwrap();
        match &frame.parent {
            None => env.clone(),
            Some(p) => {
                let p_clone = p.clone();
                drop(frame);
                Env::get_root(&p_clone)
            }
        }
    }

    pub fn delete(env: &EnvRef, name: &str) {
        env.lock().unwrap().vars.remove(name);
    }
}
