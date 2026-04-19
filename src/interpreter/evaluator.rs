use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::interpreter::environment::{Env, EnvRef};
use crate::parser::ast::*;
use crate::runtime::value::*;

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub type_name: String,
    pub message: String,
    pub traceback: Vec<TracebackEntry>,
}

impl RuntimeError {
    pub fn new(type_name: &str, message: impl Into<String>) -> Self {
        RuntimeError { type_name: type_name.into(), message: message.into(), traceback: vec![] }
    }
    pub fn type_error(msg: impl Into<String>) -> Self { Self::new("TypeError", msg) }
    pub fn value_error(msg: impl Into<String>) -> Self { Self::new("ValueError", msg) }
    pub fn name_error(name: &str) -> Self { Self::new("NameError", format!("name '{}' is not defined", name)) }
    pub fn index_error(msg: impl Into<String>) -> Self { Self::new("IndexError", msg) }
    pub fn key_error(key: &str) -> Self { Self::new("KeyError", key.to_string()) }
    pub fn zero_div() -> Self { Self::new("ZeroDivisionError", "division by zero") }
    pub fn attr_error(obj: &str, attr: &str) -> Self {
        Self::new("AttributeError", format!("'{}' object has no attribute '{}'", obj, attr))
    }
    pub fn stop_iteration() -> Self { Self::new("StopIteration", "") }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.type_name, self.message)
    }
}

/// Control flow signals (not true errors).
pub enum Signal {
    Return(Value),
    Break,
    Continue,
    Raise(RuntimeError),
}

pub type EvalResult = Result<Value, Signal>;

fn err(e: RuntimeError) -> EvalResult { Err(Signal::Raise(e)) }
fn ok(v: Value) -> EvalResult { Ok(v) }

pub struct Evaluator {
    pub globals: EnvRef,
}

impl Evaluator {
    pub fn new() -> Self {
        let globals = Env::new_root();
        let ev = Evaluator { globals: globals.clone() };
        ev.load_builtins();
        ev
    }

    fn load_builtins(&self) {
        let builtins: &[(&'static str, BuiltinFnPtr)] = &[
            ("print", builtin_print),
            ("len",   builtin_len),
            ("range", builtin_range),
            ("int",   builtin_int),
            ("float", builtin_float),
            ("str",   builtin_str),
            ("bool",  builtin_bool),
            ("type",  builtin_type),
            ("isinstance", builtin_isinstance),
            ("abs",   builtin_abs),
            ("max",   builtin_max),
            ("min",   builtin_min),
            ("sum",   builtin_sum),
            ("sorted",builtin_sorted),
            ("list",  builtin_list),
            ("dict",  builtin_dict_fn),
            ("tuple", builtin_tuple),
            ("set",   builtin_set),
            ("enumerate", builtin_enumerate),
            ("zip",   builtin_zip),
            ("map",   builtin_map_fn),
            ("filter",builtin_filter_fn),
            ("reversed", builtin_reversed),
            ("ord",   builtin_ord),
            ("chr",   builtin_chr),
            ("hex",   builtin_hex),
            ("oct",   builtin_oct),
            ("bin",   builtin_bin),
            ("round", builtin_round),
            ("divmod",builtin_divmod),
            ("pow",   builtin_pow),
            ("input", builtin_input),
            ("id",    builtin_id),
            ("hash",  builtin_hash),
            ("repr",  builtin_repr),
            ("hasattr", builtin_hasattr),
            ("getattr", builtin_getattr),
            ("setattr", builtin_setattr),
            ("callable", builtin_callable),
            ("vars",  builtin_vars),
            ("dir",   builtin_dir),
            ("iter",  builtin_iter),
            ("next",  builtin_next),
            ("open",  crate::stdlib::io::builtin_open),
        ];
        for (name, func) in builtins {
            Env::set_local(&self.globals, name, Value::BuiltinFunction(Arc::new(BuiltinFn { name, func: *func })));
        }
        // None / True / False are keywords handled in evaluator, but also expose as values
        Env::set_local(&self.globals, "None", Value::None);
        Env::set_local(&self.globals, "True", Value::Bool(true));
        Env::set_local(&self.globals, "False", Value::Bool(false));
        // Exception classes as builtins
        for exc in ["Exception","BaseException","TypeError","ValueError","RuntimeError",
                    "IOError","IndexError","KeyError","ZeroDivisionError","NameError",
                    "AttributeError","StopIteration","NotImplementedError","OSError",
                    "FileNotFoundError","PermissionError","SystemExit","KeyboardInterrupt",
                    "OverflowError","AssertionError","ImportError","ThreadError","ArithmeticError"] {
            Env::set_local(&self.globals, exc, Value::Class(Arc::new(Class {
                name: exc.to_string(), bases: vec![], methods: HashMap::new(), class_vars: HashMap::new(),
            })));
        }
    }

    pub fn exec_module(&mut self, module: &Module) -> Result<(), RuntimeError> {
        match self.exec_stmts(&module.body, &self.globals.clone()) {
            Ok(_) => Ok(()),
            Err(Signal::Raise(e)) => Err(e),
            Err(Signal::Return(_)) => Ok(()),
            Err(Signal::Break) | Err(Signal::Continue) => Ok(()),
        }
    }

    fn exec_stmts(&mut self, stmts: &[Stmt], env: &EnvRef) -> EvalResult {
        let mut last = Value::None;
        for stmt in stmts {
            last = self.exec_stmt(stmt, env)?;
        }
        Ok(last)
    }

    fn exec_stmt(&mut self, stmt: &Stmt, env: &EnvRef) -> EvalResult {
        match stmt {
            Stmt::Expr(e) => self.eval_expr(e, env),

            Stmt::Assign { targets, value, .. } => {
                let val = self.eval_expr(value, env)?;
                for target in targets {
                    self.assign_target(target, val.clone(), env)?;
                }
                ok(Value::None)
            }

            Stmt::AugAssign { target, op, value, .. } => {
                let lhs = self.eval_expr(target, env)?;
                let rhs = self.eval_expr(value, env)?;
                let result = self.apply_binop(op, &lhs, &rhs)?;
                self.assign_target(target, result, env)?;
                ok(Value::None)
            }

            Stmt::AnnAssign { target, value, .. } => {
                if let Some(val_expr) = value {
                    let val = self.eval_expr(val_expr, env)?;
                    self.assign_target(target, val, env)?;
                }
                ok(Value::None)
            }

            Stmt::Return { value, .. } => {
                let val = match value {
                    Some(e) => self.eval_expr(e, env)?,
                    None => Value::None,
                };
                Err(Signal::Return(val))
            }

            Stmt::Raise { exc, cause, .. } => {
                let exc_val = match exc {
                    Some(e) => self.eval_expr(e, env)?,
                    None => return Err(Signal::Raise(RuntimeError::new("RuntimeError", "bare raise outside exception context"))),
                };
                let mut re = self.value_to_runtime_error(exc_val);
                if let Some(cause_expr) = cause {
                    let _ = self.eval_expr(cause_expr, env);
                }
                Err(Signal::Raise(re))
            }

            Stmt::If { test, body, orelse, .. } => {
                let cond = self.eval_expr(test, env)?;
                if cond.is_truthy() {
                    self.exec_stmts(body, env)
                } else {
                    self.exec_stmts(orelse, env)
                }
            }

            Stmt::While { test, body, orelse, .. } => {
                loop {
                    let cond = self.eval_expr(test, env)?;
                    if !cond.is_truthy() { break; }
                    match self.exec_stmts(body, env) {
                        Ok(_) => {}
                        Err(Signal::Break) => return ok(Value::None),
                        Err(Signal::Continue) => continue,
                        Err(e) => return Err(e),
                    }
                }
                self.exec_stmts(orelse, env)
            }

            Stmt::For { target, iter, body, orelse, .. } => {
                let iterable = self.eval_expr(iter, env)?;
                let items = self.collect_iter(iterable)?;
                let mut did_break = false;
                for item in items {
                    self.assign_target(target, item, env)?;
                    match self.exec_stmts(body, env) {
                        Ok(_) => {}
                        Err(Signal::Break) => { did_break = true; break; }
                        Err(Signal::Continue) => continue,
                        Err(e) => return Err(e),
                    }
                }
                if !did_break {
                    self.exec_stmts(orelse, env)?;
                }
                ok(Value::None)
            }

            Stmt::Break(_)    => Err(Signal::Break),
            Stmt::Continue(_) => Err(Signal::Continue),
            Stmt::Pass(_)     => ok(Value::None),

            Stmt::Global { names, .. } => {
                // mark names as global in current env by copying from global scope
                for name in names {
                    if let Some(v) = Env::get(&self.globals, name) {
                        Env::set_local(env, name, v);
                    }
                }
                ok(Value::None)
            }

            Stmt::Nonlocal { .. } => ok(Value::None), // handled at assignment time

            Stmt::FunctionDef { name, params, return_annotation: _, body, decorators, is_async, .. } => {
                let mut func = Value::Function(Arc::new(Function {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                    closure_env: env.clone(),
                    is_async: *is_async,
                }));
                // apply decorators in reverse order
                for dec in decorators.iter().rev() {
                    let dec_val = self.eval_expr(dec, env)?;
                    func = self.call_value(dec_val, vec![func])?;
                }
                Env::set_local(env, name, func);
                ok(Value::None)
            }

            Stmt::ClassDef { name, bases, body, decorators, .. } => {
                let mut base_classes = Vec::new();
                for b in bases {
                    match self.eval_expr(b, env)? {
                        Value::Class(c) => base_classes.push(c),
                        other => return err(RuntimeError::type_error(format!("base must be a class, got {}", other.type_name()))),
                    }
                }
                let class_env = Env::new_child(env.clone());
                self.exec_stmts(body, &class_env)?;
                let vars = class_env.lock().unwrap().vars.clone();
                let mut methods = HashMap::new();
                let mut class_vars = HashMap::new();
                for (k, v) in vars {
                    match &v {
                        Value::Function(_) => { methods.insert(k, v); }
                        _ => { class_vars.insert(k, v); }
                    }
                }
                let mut cls = Value::Class(Arc::new(Class { name: name.clone(), bases: base_classes, methods, class_vars }));
                for dec in decorators.iter().rev() {
                    let dec_val = self.eval_expr(dec, env)?;
                    cls = self.call_value(dec_val, vec![cls])?;
                }
                Env::set_local(env, name, cls);
                ok(Value::None)
            }

            Stmt::Try { body, handlers, orelse, finalbody, .. } => {
                let body_result = self.exec_stmts(body, env);
                let result = match body_result {
                    Err(Signal::Raise(ref re)) => {
                        let mut handled = false;
                        for handler in handlers {
                            if self.matches_handler(handler, re) {
                                if let Some(alias) = &handler.name {
                                    let exc_v = self.runtime_error_to_value(re);
                                    Env::set_local(env, alias, exc_v);
                                }
                                let r = self.exec_stmts(&handler.body, env);
                                handled = true;
                                break;
                            }
                        }
                        if !handled { body_result } else { Ok(Value::None) }
                    }
                    Ok(_) => {
                        let r = self.exec_stmts(orelse, env);
                        r
                    }
                    other => other,
                };
                // always run finally
                self.exec_stmts(finalbody, env)?;
                result
            }

            Stmt::With { items, body, .. } => {
                let mut ctx_managers = Vec::new();
                for item in items {
                    let ctx = self.eval_expr(&item.context_expr, env)?;
                    let enter = self.get_method(&ctx, "__enter__")?;
                    let val = self.call_value(enter, vec![ctx.clone()])?;
                    if let Some(var) = &item.optional_vars {
                        self.assign_target(var, val, env)?;
                    }
                    ctx_managers.push(ctx);
                }
                let result = self.exec_stmts(body, env);
                for ctx in ctx_managers.iter().rev() {
                    let exit = self.get_method(ctx, "__exit__")?;
                    match &result {
                        Err(Signal::Raise(re)) => {
                            let exc_val = self.runtime_error_to_value(re);
                            let suppressed = self.call_value(exit, vec![ctx.clone(), exc_val, Value::None, Value::None])?;
                            if !suppressed.is_truthy() {
                                // re-raise if not suppressed — handled below
                            }
                        }
                        _ => { self.call_value(exit, vec![ctx.clone(), Value::None, Value::None, Value::None])?; }
                    }
                }
                result
            }

            Stmt::Assert { test, msg, .. } => {
                let cond = self.eval_expr(test, env)?;
                if !cond.is_truthy() {
                    let message = match msg {
                        Some(m) => self.eval_expr(m, env)?.str_display(),
                        None => "".into(),
                    };
                    return err(RuntimeError::new("AssertionError", message));
                }
                ok(Value::None)
            }

            Stmt::Delete { targets, .. } => {
                for target in targets {
                    self.delete_target(target, env)?;
                }
                ok(Value::None)
            }

            Stmt::Import { names, .. } => {
                for alias in names {
                    let module = self.import_module(&alias.name)?;
                    let bind_name = alias.asname.as_deref().unwrap_or(&alias.name);
                    Env::set_local(env, bind_name, module);
                }
                ok(Value::None)
            }

            Stmt::ImportFrom { module, names, .. } => {
                let mod_name = module.as_deref().unwrap_or("");
                let mod_val = self.import_module(mod_name)?;
                for alias in names {
                    if alias.name == "*" {
                        if let Value::Module(m) = &mod_val {
                            for (k, v) in m.lock().unwrap().attrs.clone() {
                                Env::set_local(env, &k, v);
                            }
                        }
                    } else {
                        let attr = self.getattr_val(&mod_val, &alias.name)?;
                        let bind = alias.asname.as_deref().unwrap_or(&alias.name);
                        Env::set_local(env, bind, attr);
                    }
                }
                ok(Value::None)
            }

            Stmt::Yield { value, .. } => {
                // Yield as statement — not inside a generator context at this phase
                ok(Value::None)
            }

            Stmt::Nonlocal { .. } => ok(Value::None),
            Stmt::Global { .. } => ok(Value::None),
        }
    }

    // ── Expression evaluation ─────────────────────────────────────────────────

    pub fn eval_expr(&mut self, expr: &Expr, env: &EnvRef) -> EvalResult {
        match expr {
            Expr::IntLit(n, _)   => ok(Value::Int(*n)),
            Expr::FloatLit(f, _) => ok(Value::Float(*f)),
            Expr::StrLit(s, _)   => ok(Value::Str(Arc::new(s.clone()))),
            Expr::BytesLit(b, _) => ok(Value::Bytes(Arc::new(b.clone()))),
            Expr::BoolLit(b, _)  => ok(Value::Bool(*b)),
            Expr::NoneLit(_)     => ok(Value::None),
            Expr::EllipsisLit(_) => ok(Value::Ellipsis),

            Expr::FStringLit(parts, _) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        crate::lexer::token::FStringPart::Literal(s) => result.push_str(s),
                        crate::lexer::token::FStringPart::Expr(code) => {
                            // re-lex and eval the embedded expression
                            let val = self.eval_fstring_expr(code, env)?;
                            result.push_str(&val.str_display());
                        }
                    }
                }
                ok(Value::Str(Arc::new(result)))
            }

            Expr::Ident(name, _) => {
                match Env::get(env, name) {
                    Some(v) => ok(v),
                    None => err(RuntimeError::name_error(name)),
                }
            }

            Expr::Unary { op, expr, .. } => {
                let val = self.eval_expr(expr, env)?;
                self.apply_unary(op, val)
            }

            Expr::Binary { op, left, right, .. } => {
                let l = self.eval_expr(left, env)?;
                let r = self.eval_expr(right, env)?;
                self.apply_binop(op, &l, &r)
            }

            Expr::Compare { left, ops, comparators, .. } => {
                let mut lhs = self.eval_expr(left, env)?;
                for (op, rhs_expr) in ops.iter().zip(comparators.iter()) {
                    let rhs = self.eval_expr(rhs_expr, env)?;
                    let result = self.apply_cmpop(op, &lhs, &rhs)?;
                    if !result { return ok(Value::Bool(false)); }
                    lhs = rhs;
                }
                ok(Value::Bool(true))
            }

            Expr::BoolOp { op, values, .. } => {
                match op {
                    BoolOpKind::And => {
                        let mut last = Value::Bool(true);
                        for v in values {
                            last = self.eval_expr(v, env)?;
                            if !last.is_truthy() { return ok(last); }
                        }
                        ok(last)
                    }
                    BoolOpKind::Or => {
                        let mut last = Value::Bool(false);
                        for v in values {
                            last = self.eval_expr(v, env)?;
                            if last.is_truthy() { return ok(last); }
                        }
                        ok(last)
                    }
                }
            }

            Expr::IfExpr { test, body, orelse, .. } => {
                let cond = self.eval_expr(test, env)?;
                if cond.is_truthy() { self.eval_expr(body, env) }
                else { self.eval_expr(orelse, env) }
            }

            Expr::Call { func, args, .. } => {
                let func_val = self.eval_expr(func, env)?;
                let mut pos_args: Vec<Value> = Vec::new();
                let mut kw_args: Vec<(String, Value)> = Vec::new();
                for arg in args {
                    match arg {
                        Arg::Pos(e) => pos_args.push(self.eval_expr(e, env)?),
                        Arg::Keyword { name, value: e } => {
                            kw_args.push((name.clone(), self.eval_expr(e, env)?));
                        }
                        Arg::StarArgs(e) => {
                            let v = self.eval_expr(e, env)?;
                            let items = self.collect_iter(v)?;
                            pos_args.extend(items);
                        }
                        Arg::DoubleStarArgs(e) => {
                            let v = self.eval_expr(e, env)?;
                            if let Value::Dict(d) = v {
                                for (k, val) in d.lock().unwrap().iter() {
                                    if let HashableValue::Str(s) = k {
                                        kw_args.push((s.clone(), val.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
                if kw_args.is_empty() {
                    self.call_value(func_val, pos_args)
                } else {
                    self.call_value_kwargs(func_val, pos_args, kw_args)
                }
            }

            Expr::Attr { obj, attr, .. } => {
                let obj_val = self.eval_expr(obj, env)?;
                self.getattr_val(&obj_val, attr)
            }

            Expr::Index { obj, index, .. } => {
                let obj_val = self.eval_expr(obj, env)?;
                let idx_val = self.eval_expr(index, env)?;
                self.apply_index(&obj_val, &idx_val)
            }

            Expr::Slice { obj, lower, upper, step, .. } => {
                let obj_val = self.eval_expr(obj, env)?;
                let lo = match lower { Some(e) => Some(self.eval_expr(e, env)?), None => None };
                let hi = match upper { Some(e) => Some(self.eval_expr(e, env)?), None => None };
                let st = match step  { Some(e) => Some(self.eval_expr(e, env)?), None => None };
                self.apply_slice(&obj_val, lo, hi, st)
            }

            Expr::List(elts, _) => {
                let mut items = Vec::new();
                for e in elts {
                    match e {
                        Expr::Starred(inner, _) => {
                            let v = self.eval_expr(inner, env)?;
                            let items2 = self.collect_iter(v)?;
                            items.extend(items2);
                        }
                        _ => items.push(self.eval_expr(e, env)?),
                    }
                }
                ok(Value::List(crate::runtime::gc::alloc_list(items)))
            }

            Expr::Tuple(elts, _) => {
                let mut items = Vec::new();
                for e in elts {
                    match e {
                        Expr::Starred(inner, _) => {
                            let v = self.eval_expr(inner, env)?;
                            let items2 = self.collect_iter(v)?;
                            items.extend(items2);
                        }
                        _ => items.push(self.eval_expr(e, env)?),
                    }
                }
                ok(Value::Tuple(Arc::new(items)))
            }

            Expr::Set(elts, _) => {
                let mut s = std::collections::HashSet::new();
                for e in elts {
                    let v = self.eval_expr(e, env)?;
                    let h = HashableValue::try_from(&v).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                    s.insert(h);
                }
                ok(Value::Set(crate::runtime::gc::alloc_set(s)))
            }

            Expr::Dict { keys, values, .. } => {
                let mut map = HashMap::new();
                for (k, v) in keys.iter().zip(values.iter()) {
                    let val = self.eval_expr(v, env)?;
                    match k {
                        None => {
                            // **dict unpacking
                            if let Value::Dict(d) = val {
                                for (dk, dv) in d.lock().unwrap().iter() {
                                    map.insert(dk.clone(), dv.clone());
                                }
                            }
                        }
                        Some(ke) => {
                            let kv = self.eval_expr(ke, env)?;
                            let h = HashableValue::try_from(&kv).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                            map.insert(h, val);
                        }
                    }
                }
                ok(Value::Dict(crate::runtime::gc::alloc_dict(map)))
            }

            Expr::ListComp { elt, generators, .. } => {
                let items = self.eval_comprehension(elt, generators, env)?;
                ok(Value::List(crate::runtime::gc::alloc_list(items)))
            }

            Expr::SetComp { elt, generators, .. } => {
                let items = self.eval_comprehension(elt, generators, env)?;
                let mut s = std::collections::HashSet::new();
                for v in items {
                    let h = HashableValue::try_from(&v).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                    s.insert(h);
                }
                ok(Value::Set(crate::runtime::gc::alloc_set(s)))
            }

            Expr::DictComp { key, value, generators, .. } => {
                let comp_env = Env::new_child(env.clone());
                let mut map = HashMap::new();
                self.run_comprehension_generators(generators, 0, &comp_env, &mut |ev: &mut Evaluator, e: &EnvRef| {
                    let k = ev.eval_expr(key, e)?;
                    let v = ev.eval_expr(value, e)?;
                    let h = HashableValue::try_from(&k).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                    map.insert(h, v);
                    ok(Value::None)
                })?;
                ok(Value::Dict(crate::runtime::gc::alloc_dict(map)))
            }

            Expr::GeneratorExp { elt, generators, .. } => {
                let items = self.eval_comprehension(elt, generators, env)?;
                ok(Value::List(crate::runtime::gc::alloc_list(items))) // treat as list for now
            }

            Expr::Lambda { params, body, .. } => {
                ok(Value::Function(Arc::new(Function {
                    name: "<lambda>".into(),
                    params: params.clone(),
                    body: vec![Stmt::Return { value: Some(*body.clone()), span: body.span().clone() }],
                    closure_env: env.clone(),
                    is_async: false,
                })))
            }

            Expr::NamedExpr { target, value, .. } => {
                let val = self.eval_expr(value, env)?;
                if !Env::set_existing(env, target, val.clone()) {
                    Env::set_local(env, target, val.clone());
                }
                ok(val)
            }

            Expr::Await(e, _) => self.eval_expr(e, env),
            Expr::Yield(val, _) => {
                match val {
                    Some(e) => self.eval_expr(e, env),
                    None => ok(Value::None),
                }
            }
            Expr::YieldFrom(e, _) => self.eval_expr(e, env),

            Expr::Starred(e, _) => self.eval_expr(e, env),
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn eval_fstring_expr(&mut self, code: &str, env: &EnvRef) -> EvalResult {
        use crate::lexer::Lexer;
        use crate::parser::Parser;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().map_err(|e| Signal::Raise(RuntimeError::new("SyntaxError", e.msg)))?;
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expr().map_err(|e| Signal::Raise(RuntimeError::new("SyntaxError", e.msg)))?;
        self.eval_expr(&expr, env)
    }

    fn assign_target(&mut self, target: &Expr, value: Value, env: &EnvRef) -> EvalResult {
        match target {
            Expr::Ident(name, _) => {
                if !Env::set_existing(env, name, value.clone()) {
                    Env::set_local(env, name, value);
                }
                ok(Value::None)
            }
            Expr::Tuple(elts, _) | Expr::List(elts, _) => {
                let items = self.collect_iter(value)?;
                // Find starred element if any
                let star_pos = elts.iter().position(|e| matches!(e, Expr::Starred(..)));
                if let Some(star_idx) = star_pos {
                    let n_before = star_idx;
                    let n_after  = elts.len() - star_idx - 1;
                    let total    = n_before + n_after;
                    if items.len() < total {
                        return err(RuntimeError::value_error("not enough values to unpack"));
                    }
                    // Assign before-star
                    for (e, v) in elts[..star_idx].iter().zip(items[..n_before].iter()) {
                        self.assign_target(e, v.clone(), env)?;
                    }
                    // Assign starred
                    let star_items = items[n_before..items.len()-n_after].to_vec();
                    if let Expr::Starred(inner, _) = &elts[star_idx] {
                        self.assign_target(inner, Value::List(crate::runtime::gc::alloc_list(star_items)), env)?;
                    }
                    // Assign after-star
                    for (e, v) in elts[star_idx+1..].iter().zip(items[items.len()-n_after..].iter()) {
                        self.assign_target(e, v.clone(), env)?;
                    }
                } else {
                    if items.len() != elts.len() {
                        return err(RuntimeError::value_error(format!(
                            "too {} values to unpack (expected {})",
                            if items.len() > elts.len() { "many" } else { "few" }, elts.len()
                        )));
                    }
                    for (e, v) in elts.iter().zip(items.into_iter()) {
                        self.assign_target(e, v, env)?;
                    }
                }
                ok(Value::None)
            }
            Expr::Starred(inner, _) => {
                let items = self.collect_iter(value)?;
                self.assign_target(inner, Value::List(crate::runtime::gc::alloc_list(items)), env)
            }
            Expr::Attr { obj, attr, .. } => {
                let obj_val = self.eval_expr(obj, env)?;
                self.setattr_val(&obj_val, attr, value)?;
                ok(Value::None)
            }
            Expr::Index { obj, index, .. } => {
                let obj_val = self.eval_expr(obj, env)?;
                let idx_val = self.eval_expr(index, env)?;
                self.set_index(&obj_val, &idx_val, value)?;
                ok(Value::None)
            }
            _ => err(RuntimeError::new("SyntaxError", "invalid assignment target")),
        }
    }

    fn delete_target(&mut self, target: &Expr, env: &EnvRef) -> EvalResult {
        match target {
            Expr::Ident(name, _) => { Env::delete(env, name); ok(Value::None) }
            Expr::Attr { obj, attr, .. } => {
                let obj_val = self.eval_expr(obj, env)?;
                match obj_val {
                    Value::Instance(h) => { h.lock().unwrap().fields.remove(attr.as_str()); }
                    _ => return err(RuntimeError::new("AttributeError", format!("cannot delete attribute '{}'", attr))),
                }
                ok(Value::None)
            }
            Expr::Index { obj, index, .. } => {
                let obj_val = self.eval_expr(obj, env)?;
                let idx_val = self.eval_expr(index, env)?;
                match &obj_val {
                    Value::List(l) => {
                        let idx = self.int_index(&idx_val, l.lock().unwrap().len())?;
                        l.lock().unwrap().remove(idx);
                    }
                    Value::Dict(d) => {
                        let h = HashableValue::try_from(&idx_val).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                        d.lock().unwrap().remove(&h);
                    }
                    _ => return err(RuntimeError::type_error("'del' on unsupported type")),
                }
                ok(Value::None)
            }
            Expr::Tuple(elts, _) | Expr::List(elts, _) => {
                for e in elts { self.delete_target(e, env)?; }
                ok(Value::None)
            }
            _ => err(RuntimeError::new("SyntaxError", "invalid del target")),
        }
    }

    pub fn call_value(&mut self, func: Value, args: Vec<Value>) -> EvalResult {
        match func {
            Value::BoundMethod { receiver, method } => {
                let mut full_args = vec![*receiver];
                full_args.extend(args);
                self.call_value(Value::Function(method), full_args)
            }
            Value::BoundBuiltin { receiver, func, .. } => {
                let mut full_args = vec![*receiver];
                full_args.extend(args);
                func(full_args).map_err(Signal::Raise)
            }
            Value::BuiltinFunction(b) => {
                (b.func)(args).map_err(Signal::Raise)
            }
            Value::Function(f) => {
                let call_env = Env::new_child(f.closure_env.clone());
                // bind positional params
                let mut i = 0;
                let mut varargs: Option<String> = None;
                for param in &f.params {
                    match param.kind {
                        ParamKind::VarArgs => {
                            let rest: Vec<Value> = args[i..].to_vec();
                            Env::set_local(&call_env, &param.name, Value::List(crate::runtime::gc::alloc_list(rest)));
                            i = args.len();
                            varargs = Some(param.name.clone());
                        }
                        ParamKind::DoubleStarArgs => {
                            Env::set_local(&call_env, &param.name, Value::Dict(crate::runtime::gc::alloc_dict(HashMap::new())));
                        }
                        _ => {
                            if i < args.len() {
                                Env::set_local(&call_env, &param.name, args[i].clone());
                                i += 1;
                            } else if let Some(default) = &param.default {
                                let dv = self.eval_expr(default, &f.closure_env.clone())?;
                                Env::set_local(&call_env, &param.name, dv);
                            } else {
                                return err(RuntimeError::new("TypeError",
                                    format!("{}() missing required argument: '{}'", f.name, param.name)));
                            }
                        }
                    }
                }
                match self.exec_stmts(&f.body, &call_env) {
                    Ok(_) => ok(Value::None),
                    Err(Signal::Return(v)) => ok(v),
                    Err(e) => Err(e),
                }
            }
            Value::Class(cls) => {
                // instantiation
                let instance = crate::runtime::gc::alloc_instance(Instance {
                    class_name: cls.name.clone(),
                    class: cls.clone(),
                    fields: cls.class_vars.clone(),
                });
                let inst_val = Value::Instance(instance);
                // call __init__ if present
                if let Some(init) = cls.methods.get("__init__") {
                    let mut init_args = vec![inst_val.clone()];
                    init_args.extend(args);
                    self.call_value(init.clone(), init_args)?;
                }
                ok(inst_val)
            }
            Value::Instance(inst) => {
                // callable instance via __call__
                let cls = inst.lock().unwrap().class.clone();
                if let Some(call_fn) = cls.methods.get("__call__") {
                    let mut call_args = vec![Value::Instance(inst.clone())];
                    call_args.extend(args);
                    self.call_value(call_fn.clone(), call_args)
                } else {
                    err(RuntimeError::type_error("object is not callable"))
                }
            }
            _ => err(RuntimeError::type_error(format!("'{}' object is not callable", func.type_name()))),
        }
    }

    /// Call a function with positional args and keyword args.
    /// For builtins/bound-builtins, kwargs are appended positionally (same behavior as before).
    /// For user-defined functions, kwargs are matched by parameter name.
    pub fn call_value_kwargs(&mut self, func: Value, pos_args: Vec<Value>, kw_args: Vec<(String, Value)>) -> EvalResult {
        match &func {
            Value::Function(f) => {
                let f = f.clone();
                let call_env = Env::new_child(f.closure_env.clone());
                let mut pos_idx = 0;
                let mut kwargs_map: HashMap<String, Value> = kw_args.into_iter().collect();

                for param in &f.params {
                    match param.kind {
                        ParamKind::VarArgs => {
                            // Consume remaining positional args
                            let rest: Vec<Value> = pos_args[pos_idx..].to_vec();
                            Env::set_local(&call_env, &param.name, Value::List(crate::runtime::gc::alloc_list(rest)));
                            pos_idx = pos_args.len();
                        }
                        ParamKind::DoubleStarArgs => {
                            // Collect remaining kwargs into a dict
                            let mut kw_dict = HashMap::new();
                            for (k, v) in &kwargs_map {
                                kw_dict.insert(HashableValue::Str(k.clone()), v.clone());
                            }
                            kwargs_map.clear();
                            Env::set_local(&call_env, &param.name, Value::Dict(crate::runtime::gc::alloc_dict(kw_dict)));
                        }
                        ParamKind::KeywordOnly => {
                            if let Some(v) = kwargs_map.remove(&param.name) {
                                Env::set_local(&call_env, &param.name, v);
                            } else if let Some(default) = &param.default {
                                let dv = self.eval_expr(default, &f.closure_env.clone())?;
                                Env::set_local(&call_env, &param.name, dv);
                            } else {
                                return err(RuntimeError::new("TypeError",
                                    format!("{}() missing keyword-only argument: '{}'", f.name, param.name)));
                            }
                        }
                        _ => {
                            // Regular or PosOnly: prefer positional, fall back to keyword
                            if pos_idx < pos_args.len() {
                                Env::set_local(&call_env, &param.name, pos_args[pos_idx].clone());
                                pos_idx += 1;
                            } else if let Some(v) = kwargs_map.remove(&param.name) {
                                Env::set_local(&call_env, &param.name, v);
                            } else if let Some(default) = &param.default {
                                let dv = self.eval_expr(default, &f.closure_env.clone())?;
                                Env::set_local(&call_env, &param.name, dv);
                            } else {
                                return err(RuntimeError::new("TypeError",
                                    format!("{}() missing required argument: '{}'", f.name, param.name)));
                            }
                        }
                    }
                }
                match self.exec_stmts(&f.body, &call_env) {
                    Ok(_)              => ok(Value::None),
                    Err(Signal::Return(v)) => ok(v),
                    Err(e)             => Err(e),
                }
            }
            Value::Class(_) => {
                // For class instantiation, merge pos+kw into positional order
                let mut all_args = pos_args;
                all_args.extend(kw_args.into_iter().map(|(_, v)| v));
                self.call_value(func, all_args)
            }
            Value::BoundMethod { receiver, method } => {
                // Merge self into positional, then dispatch as Function kwargs
                let mut full_pos = vec![*receiver.clone()];
                full_pos.extend(pos_args);
                self.call_value_kwargs(Value::Function(method.clone()), full_pos, kw_args)
            }
            _ => {
                // Builtins: merge kwargs positionally (best-effort)
                let mut all_args = pos_args;
                all_args.extend(kw_args.into_iter().map(|(_, v)| v));
                self.call_value(func, all_args)
            }
        }
    }

    fn get_method(&mut self, obj: &Value, name: &str) -> Result<Value, Signal> {
        self.getattr_val(obj, name)
    }

    fn getattr_val(&mut self, obj: &Value, attr: &str) -> EvalResult {
        match obj {
            Value::Instance(inst) => {
                let (cls, fields) = {
                    let i = inst.lock().unwrap();
                    (i.class.clone(), i.fields.clone())
                };
                if let Some(v) = fields.get(attr) {
                    return ok(v.clone());
                }
                if let Some(method) = cls.methods.get(attr) {
                    // Bind method to instance
                    let bound = bind_method(method.clone(), Value::Instance(inst.clone()));
                    return ok(bound);
                }
                // check class vars
                if let Some(v) = cls.class_vars.get(attr) {
                    return ok(v.clone());
                }
                // walk bases
                for base in &cls.bases {
                    if let Some(v) = base.methods.get(attr) {
                        let bound = bind_method(v.clone(), Value::Instance(inst.clone()));
                        return ok(bound);
                    }
                }
                err(RuntimeError::attr_error(&cls.name, attr))
            }
            Value::Class(cls) => {
                if let Some(v) = cls.methods.get(attr).or_else(|| cls.class_vars.get(attr)) {
                    return ok(v.clone());
                }
                err(RuntimeError::attr_error(&cls.name, attr))
            }
            Value::Module(m) => {
                let mo = m.lock().unwrap();
                mo.attrs.get(attr).cloned().map(ok).unwrap_or_else(||
                    err(RuntimeError::attr_error(&mo.name, attr)))
            }
            Value::Str(s) => self.str_method(s.clone(), attr),
            Value::List(l) => self.list_method(l.clone(), attr),
            Value::Dict(d) => self.dict_method(d.clone(), attr),
            Value::Set(s)  => self.set_method(s.clone(), attr),
            Value::Tuple(t) => self.tuple_method(t.clone(), attr),
            Value::Bytes(b) => self.bytes_method(b.clone(), attr),
            _ => err(RuntimeError::attr_error(obj.type_name(), attr)),
        }
    }

    fn setattr_val(&mut self, obj: &Value, attr: &str, value: Value) -> EvalResult {
        match obj {
            Value::Instance(inst) => {
                inst.lock().unwrap().fields.insert(attr.to_string(), value);
                ok(Value::None)
            }
            Value::Module(m) => {
                m.lock().unwrap().attrs.insert(attr.to_string(), value);
                ok(Value::None)
            }
            _ => err(RuntimeError::attr_error(obj.type_name(), attr)),
        }
    }

    fn apply_unary(&self, op: &UnaryOp, val: Value) -> EvalResult {
        match (op, &val) {
            (UnaryOp::Not, _)    => ok(Value::Bool(!val.is_truthy())),
            (UnaryOp::Minus, Value::Int(n))   => ok(Value::Int(n.wrapping_neg())),
            (UnaryOp::Minus, Value::Float(f)) => ok(Value::Float(-f)),
            (UnaryOp::Plus,  Value::Int(n))   => ok(Value::Int(*n)),
            (UnaryOp::Plus,  Value::Float(f)) => ok(Value::Float(*f)),
            (UnaryOp::BitNot, Value::Int(n))  => ok(Value::Int(!n)),
            _ => err(RuntimeError::type_error(format!("unsupported unary op on {}", val.type_name()))),
        }
    }

    pub fn apply_binop(&self, op: &BinOp, l: &Value, r: &Value) -> EvalResult {
        match op {
            BinOp::Add => self.op_add(l, r),
            BinOp::Sub => self.op_sub(l, r),
            BinOp::Mul => self.op_mul(l, r),
            BinOp::Div => self.op_div(l, r),
            BinOp::FloorDiv => self.op_floordiv(l, r),
            BinOp::Mod => self.op_mod(l, r),
            BinOp::Pow => self.op_pow(l, r),
            BinOp::BitAnd => self.op_bitwise(l, r, '&'),
            BinOp::BitOr  => self.op_bitwise(l, r, '|'),
            BinOp::BitXor => self.op_bitwise(l, r, '^'),
            BinOp::LShift => self.op_shift(l, r, false),
            BinOp::RShift => self.op_shift(l, r, true),
            BinOp::MatMul => err(RuntimeError::type_error("@ operator not supported")),
        }
    }

    fn op_add(&self, l: &Value, r: &Value) -> EvalResult {
        match (l, r) {
            (Value::Int(a),   Value::Int(b))   => ok(Value::Int(a.wrapping_add(*b))),
            (Value::Float(a), Value::Float(b)) => ok(Value::Float(a + b)),
            (Value::Int(a),   Value::Float(b)) => ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Int(b))   => ok(Value::Float(a + *b as f64)),
            (Value::Str(a),   Value::Str(b))   => ok(Value::Str(Arc::new(format!("{}{}", a, b)))),
            (Value::Bool(a),  Value::Int(b))   => ok(Value::Int(*a as i64 + b)),
            (Value::Int(a),   Value::Bool(b))  => ok(Value::Int(a + *b as i64)),
            (Value::Bool(a),  Value::Bool(b))  => ok(Value::Int(*a as i64 + *b as i64)),
            (Value::List(a),  Value::List(b))  => {
                let mut v = a.lock().unwrap().clone();
                v.extend(b.lock().unwrap().iter().cloned());
                ok(Value::List(crate::runtime::gc::alloc_list(v)))
            }
            (Value::Tuple(a), Value::Tuple(b)) => {
                let mut v = a.as_ref().clone();
                v.extend(b.as_ref().iter().cloned());
                ok(Value::Tuple(Arc::new(v)))
            }
            _ => err(RuntimeError::type_error(format!("unsupported + between {} and {}", l.type_name(), r.type_name()))),
        }
    }

    fn op_sub(&self, l: &Value, r: &Value) -> EvalResult {
        match (l, r) {
            (Value::Int(a),   Value::Int(b))   => ok(Value::Int(a.wrapping_sub(*b))),
            (Value::Float(a), Value::Float(b)) => ok(Value::Float(a - b)),
            (Value::Int(a),   Value::Float(b)) => ok(Value::Float(*a as f64 - b)),
            (Value::Float(a), Value::Int(b))   => ok(Value::Float(a - *b as f64)),
            (Value::Bool(a),  Value::Int(b))   => ok(Value::Int(*a as i64 - b)),
            (Value::Int(a),   Value::Bool(b))  => ok(Value::Int(a - *b as i64)),
            (Value::Bool(a),  Value::Bool(b))  => ok(Value::Int(*a as i64 - *b as i64)),
            _ => err(RuntimeError::type_error(format!("unsupported - between {} and {}", l.type_name(), r.type_name()))),
        }
    }

    fn op_mul(&self, l: &Value, r: &Value) -> EvalResult {
        match (l, r) {
            (Value::Int(a),   Value::Int(b))   => ok(Value::Int(a.wrapping_mul(*b))),
            (Value::Float(a), Value::Float(b)) => ok(Value::Float(a * b)),
            (Value::Int(a),   Value::Float(b)) => ok(Value::Float(*a as f64 * b)),
            (Value::Float(a), Value::Int(b))   => ok(Value::Float(a * *b as f64)),
            (Value::Bool(a),  Value::Int(b))   => ok(Value::Int(*a as i64 * b)),
            (Value::Int(a),   Value::Bool(b))  => ok(Value::Int(a * *b as i64)),
            (Value::Str(s),   Value::Int(n))   => ok(Value::Str(Arc::new(s.repeat(*n as usize)))),
            (Value::Int(n),   Value::Str(s))   => ok(Value::Str(Arc::new(s.repeat(*n as usize)))),
            (Value::List(l2), Value::Int(n))   => {
                let v: Vec<Value> = l2.lock().unwrap().iter().cloned()
                    .cycle().take(l2.lock().unwrap().len() * (*n as usize)).collect();
                ok(Value::List(crate::runtime::gc::alloc_list(v)))
            }
            _ => err(RuntimeError::type_error(format!("unsupported * between {} and {}", l.type_name(), r.type_name()))),
        }
    }

    fn op_div(&self, l: &Value, r: &Value) -> EvalResult {
        let lf = self.to_float(l)?;
        let rf = self.to_float(r)?;
        if rf == 0.0 { return err(RuntimeError::zero_div()); }
        ok(Value::Float(lf / rf))
    }

    fn op_floordiv(&self, l: &Value, r: &Value) -> EvalResult {
        match (l, r) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 { return err(RuntimeError::zero_div()); }
                ok(Value::Int(a.wrapping_div(*b)))
            }
            _ => {
                let lf = self.to_float(l)?;
                let rf = self.to_float(r)?;
                if rf == 0.0 { return err(RuntimeError::zero_div()); }
                ok(Value::Float((lf / rf).floor()))
            }
        }
    }

    fn op_mod(&self, l: &Value, r: &Value) -> EvalResult {
        match (l, r) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 { return err(RuntimeError::zero_div()); }
                ok(Value::Int(a.wrapping_rem(*b)))
            }
            (Value::Str(fmt), _) => {
                // Basic % formatting - just return the format string for now
                ok(Value::Str(fmt.clone()))
            }
            _ => {
                let lf = self.to_float(l)?;
                let rf = self.to_float(r)?;
                if rf == 0.0 { return err(RuntimeError::zero_div()); }
                ok(Value::Float(lf % rf))
            }
        }
    }

    fn op_pow(&self, l: &Value, r: &Value) -> EvalResult {
        match (l, r) {
            (Value::Int(a), Value::Int(b)) if *b >= 0 => ok(Value::Int(a.wrapping_pow(*b as u32))),
            (Value::Int(a), Value::Int(b)) => ok(Value::Float((*a as f64).powf(*b as f64))),
            _ => {
                let lf = self.to_float(l)?;
                let rf = self.to_float(r)?;
                ok(Value::Float(lf.powf(rf)))
            }
        }
    }

    fn op_bitwise(&self, l: &Value, r: &Value, op: char) -> EvalResult {
        let a = self.to_int(l)?;
        let b = self.to_int(r)?;
        let result = match op {
            '&' => a & b,
            '|' => a | b,
            '^' => a ^ b,
            _ => unreachable!(),
        };
        ok(Value::Int(result))
    }

    fn op_shift(&self, l: &Value, r: &Value, right: bool) -> EvalResult {
        let a = self.to_int(l)?;
        let b = self.to_int(r)?;
        if b < 0 { return err(RuntimeError::value_error("negative shift count")); }
        let result = if right { a >> b } else { a << b };
        ok(Value::Int(result))
    }

    fn apply_cmpop(&self, op: &CmpOp, l: &Value, r: &Value) -> Result<bool, Signal> {
        match op {
            CmpOp::Eq    => Ok(l.eq_val(r)),
            CmpOp::NotEq => Ok(!l.eq_val(r)),
            CmpOp::Lt    => Ok(self.compare_order(l, r)? < 0),
            CmpOp::LtEq  => Ok(self.compare_order(l, r)? <= 0),
            CmpOp::Gt    => Ok(self.compare_order(l, r)? > 0),
            CmpOp::GtEq  => Ok(self.compare_order(l, r)? >= 0),
            CmpOp::Is    => Ok(std::ptr::eq(l as *const _, r as *const _) || l.eq_val(r)),
            CmpOp::IsNot => Ok(!std::ptr::eq(l as *const _, r as *const _) && !l.eq_val(r)),
            CmpOp::In    => self.op_in(l, r),
            CmpOp::NotIn => Ok(!self.op_in(l, r)?),
        }
    }

    fn compare_order(&self, l: &Value, r: &Value) -> Result<i64, Signal> {
        match (l, r) {
            (Value::Int(a),   Value::Int(b))   => Ok(a.cmp(b) as i64),
            (Value::Float(a), Value::Float(b)) => Ok(a.partial_cmp(b).map(|o| o as i64).unwrap_or(0)),
            (Value::Int(a),   Value::Float(b)) => Ok((*a as f64).partial_cmp(b).map(|o| o as i64).unwrap_or(0)),
            (Value::Float(a), Value::Int(b))   => Ok(a.partial_cmp(&(*b as f64)).map(|o| o as i64).unwrap_or(0)),
            (Value::Str(a),   Value::Str(b))   => Ok(a.cmp(b) as i64),
            (Value::Bool(a),  Value::Bool(b))  => Ok((*a as i64).cmp(&(*b as i64)) as i64),
            _ => Err(Signal::Raise(RuntimeError::type_error(format!("'<' not supported between {} and {}", l.type_name(), r.type_name())))),
        }
    }

    fn op_in(&self, item: &Value, container: &Value) -> Result<bool, Signal> {
        match container {
            Value::List(l)  => Ok(l.lock().unwrap().iter().any(|v| v.eq_val(item))),
            Value::Tuple(t) => Ok(t.iter().any(|v| v.eq_val(item))),
            Value::Str(s)   => {
                if let Value::Str(sub) = item {
                    Ok(s.contains(sub.as_ref().as_str()))
                } else { Err(Signal::Raise(RuntimeError::type_error("'in <string>' requires string as left operand"))) }
            }
            Value::Dict(d)  => {
                let h = HashableValue::try_from(item).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                Ok(d.lock().unwrap().contains_key(&h))
            }
            Value::Set(s)   => {
                let h = HashableValue::try_from(item).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                Ok(s.lock().unwrap().contains(&h))
            }
            _ => Err(Signal::Raise(RuntimeError::type_error(format!("argument of type '{}' is not iterable", container.type_name())))),
        }
    }

    fn apply_index(&self, obj: &Value, idx: &Value) -> EvalResult {
        match obj {
            Value::List(l) => {
                let v = l.lock().unwrap();
                let i = self.int_index(idx, v.len())?;
                ok(v[i].clone())
            }
            Value::Tuple(t) => {
                let i = self.int_index(idx, t.len())?;
                ok(t[i].clone())
            }
            Value::Str(s) => {
                let chars: Vec<char> = s.chars().collect();
                let i = self.int_index(idx, chars.len())?;
                ok(Value::Str(Arc::new(chars[i].to_string())))
            }
            Value::Dict(d) => {
                let h = HashableValue::try_from(idx).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                d.lock().unwrap().get(&h).cloned()
                    .map(ok)
                    .unwrap_or_else(|| err(RuntimeError::key_error(&idx.repr())))
            }
            Value::Bytes(b) => {
                let i = self.int_index(idx, b.len())?;
                ok(Value::Int(b[i] as i64))
            }
            _ => err(RuntimeError::type_error(format!("'{}' object is not subscriptable", obj.type_name()))),
        }
    }

    fn apply_slice(&self, obj: &Value, lo: Option<Value>, hi: Option<Value>, step: Option<Value>) -> EvalResult {
        let step_n = match &step { Some(Value::Int(n)) => *n, Some(Value::None) | None => 1, _ => return err(RuntimeError::type_error("slice step must be int")) };
        match obj {
            Value::List(l) => {
                let v = l.lock().unwrap();
                let len = v.len() as i64;
                let (start, stop) = self.slice_bounds(lo, hi, len, step_n);
                let items: Vec<Value> = if step_n > 0 {
                    (start..stop.min(len)).step_by(step_n as usize).map(|i| v[i as usize].clone()).collect()
                } else {
                    (stop.max(-1)+1..=start).rev().step_by((-step_n) as usize).map(|i| v[i as usize].clone()).collect()
                };
                ok(Value::List(crate::runtime::gc::alloc_list(items)))
            }
            Value::Str(s) => {
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                let (start, stop) = self.slice_bounds(lo, hi, len, step_n);
                let result: String = if step_n > 0 {
                    (start..stop.min(len)).step_by(step_n as usize).map(|i| chars[i as usize]).collect()
                } else {
                    (stop.max(-1)+1..=start).rev().step_by((-step_n) as usize).map(|i| chars[i as usize]).collect()
                };
                ok(Value::Str(Arc::new(result)))
            }
            Value::Tuple(t) => {
                let len = t.len() as i64;
                let (start, stop) = self.slice_bounds(lo, hi, len, step_n);
                let items: Vec<Value> = if step_n > 0 {
                    (start..stop.min(len)).step_by(step_n as usize).map(|i| t[i as usize].clone()).collect()
                } else {
                    (stop.max(-1)+1..=start).rev().step_by((-step_n) as usize).map(|i| t[i as usize].clone()).collect()
                };
                ok(Value::Tuple(Arc::new(items)))
            }
            _ => err(RuntimeError::type_error(format!("'{}' object is not sliceable", obj.type_name()))),
        }
    }

    fn slice_bounds(&self, lo: Option<Value>, hi: Option<Value>, len: i64, step: i64) -> (i64, i64) {
        let normalize = |v: i64| if v < 0 { (v + len).max(0) } else { v.min(len) };
        let start = match lo {
            Some(Value::Int(n)) => normalize(n),
            Some(Value::None) | None => if step < 0 { len - 1 } else { 0 },
            _ => 0,
        };
        let stop = match hi {
            Some(Value::Int(n)) => normalize(n),
            Some(Value::None) | None => if step < 0 { -1 } else { len },
            _ => len,
        };
        (start, stop)
    }

    fn set_index(&self, obj: &Value, idx: &Value, value: Value) -> EvalResult {
        match obj {
            Value::List(l) => {
                let mut v = l.lock().unwrap();
                let i = self.int_index(idx, v.len())?;
                v[i] = value;
                ok(Value::None)
            }
            Value::Dict(d) => {
                let h = HashableValue::try_from(idx).map_err(|m| Signal::Raise(RuntimeError::type_error(m)))?;
                d.lock().unwrap().insert(h, value);
                ok(Value::None)
            }
            _ => err(RuntimeError::type_error(format!("'{}' object does not support item assignment", obj.type_name()))),
        }
    }

    fn int_index(&self, idx: &Value, len: usize) -> Result<usize, Signal> {
        let n = match idx {
            Value::Int(n)  => *n,
            Value::Bool(b) => *b as i64,
            _ => return Err(Signal::Raise(RuntimeError::type_error(format!("indices must be integers, not {}", idx.type_name())))),
        };
        let i = if n < 0 { len as i64 + n } else { n };
        if i < 0 || i as usize >= len {
            Err(Signal::Raise(RuntimeError::index_error(format!("index {} out of range", n))))
        } else {
            Ok(i as usize)
        }
    }

    fn to_int(&self, v: &Value) -> Result<i64, Signal> {
        match v {
            Value::Int(n)  => Ok(*n),
            Value::Bool(b) => Ok(*b as i64),
            _ => Err(Signal::Raise(RuntimeError::type_error(format!("expected int, got {}", v.type_name())))),
        }
    }

    fn to_float(&self, v: &Value) -> Result<f64, Signal> {
        match v {
            Value::Float(f) => Ok(*f),
            Value::Int(n)   => Ok(*n as f64),
            Value::Bool(b)  => Ok(*b as u8 as f64),
            _ => Err(Signal::Raise(RuntimeError::type_error(format!("expected numeric, got {}", v.type_name())))),
        }
    }

    pub fn collect_iter(&self, val: Value) -> Result<Vec<Value>, Signal> {
        match val {
            Value::List(l)  => Ok(l.lock().unwrap().clone()),
            Value::Tuple(t) => Ok(t.as_ref().clone()),
            Value::Str(s)   => Ok(s.chars().map(|c| Value::Str(Arc::new(c.to_string()))).collect()),
            Value::Bytes(b) => Ok(b.iter().map(|&x| Value::Int(x as i64)).collect()),
            Value::Set(s)   => Ok(s.lock().unwrap().iter().cloned().map(Value::from).collect()),
            Value::Dict(d)  => Ok(d.lock().unwrap().keys().cloned().map(Value::from).collect()),
            Value::Range(start, stop, step) => {
                let mut v = Vec::new();
                let mut i = start;
                if step > 0 { while i < stop { v.push(Value::Int(i)); i += step; } }
                else if step < 0 { while i > stop { v.push(Value::Int(i)); i += step; } }
                Ok(v)
            }
            Value::Instance(inst) => {
                // try __iter__ then fall back
                let cls = inst.lock().unwrap().class.clone();
                if let Some(iter_fn) = cls.methods.get("__iter__").cloned() {
                    // can't easily do stateful iteration here — collect via __next__
                    // This is a simplified implementation
                    return Err(Signal::Raise(RuntimeError::new("NotImplementedError", "__iter__ not fully supported in collect_iter")));
                }
                Err(Signal::Raise(RuntimeError::type_error(format!("'{}' object is not iterable", "object"))))
            }
            _ => Err(Signal::Raise(RuntimeError::type_error(format!("'{}' object is not iterable", val.type_name())))),
        }
    }

    fn eval_comprehension(&mut self, elt: &Expr, generators: &[Comprehension], env: &EnvRef) -> Result<Vec<Value>, Signal> {
        let comp_env = Env::new_child(env.clone());
        let mut results = Vec::new();
        self.run_comprehension_generators(generators, 0, &comp_env, &mut |ev, e| {
            let v = ev.eval_expr(elt, e)?;
            results.push(v);
            ok(Value::None)
        })?;
        Ok(results)
    }

    fn run_comprehension_generators(
        &mut self,
        generators: &[Comprehension],
        idx: usize,
        env: &EnvRef,
        callback: &mut dyn FnMut(&mut Evaluator, &EnvRef) -> EvalResult,
    ) -> EvalResult {
        if idx >= generators.len() {
            return callback(self, env);
        }
        let gen = &generators[idx];
        let iter_val = self.eval_expr(&gen.iter, env)?;
        let items = self.collect_iter(iter_val)?;
        let target = gen.target.clone();
        let ifs = gen.ifs.clone();
        for item in items {
            self.assign_target(&target, item, env)?;
            let mut pass = true;
            for cond in &ifs {
                let c = self.eval_expr(cond, env)?;
                if !c.is_truthy() { pass = false; break; }
            }
            if pass {
                self.run_comprehension_generators(generators, idx + 1, env, callback)?;
            }
        }
        ok(Value::None)
    }

    fn value_to_runtime_error(&self, v: Value) -> RuntimeError {
        match &v {
            Value::Class(c) => RuntimeError::new(&c.name, ""),
            Value::Instance(i) => {
                let inst = i.lock().unwrap();
                let msg = inst.fields.get("args")
                    .and_then(|a| if let Value::Tuple(t) = a {
                        t.first().map(|v| v.str_display())
                    } else { None })
                    .unwrap_or_default();
                RuntimeError::new(&inst.class_name, msg)
            }
            Value::Str(s) => RuntimeError::new("Exception", s.as_ref().clone()),
            _ => RuntimeError::new("Exception", v.str_display()),
        }
    }

    fn runtime_error_to_value(&self, re: &RuntimeError) -> Value {
        Value::Instance(crate::runtime::gc::alloc_instance(Instance {
            class_name: re.type_name.clone(),
            class: Arc::new(Class { name: re.type_name.clone(), bases: vec![], methods: HashMap::new(), class_vars: HashMap::new() }),
            fields: {
                let mut m = HashMap::new();
                m.insert("args".into(), Value::Tuple(Arc::new(vec![Value::Str(Arc::new(re.message.clone()))])));
                m.insert("message".into(), Value::Str(Arc::new(re.message.clone())));
                m
            },
        }))
    }

    fn matches_handler(&self, handler: &ExceptHandler, re: &RuntimeError) -> bool {
        match &handler.typ {
            None => true,
            Some(Expr::Ident(name, _)) => name == &re.type_name || name == "Exception" || name == "BaseException",
            Some(Expr::Tuple(types, _)) => types.iter().any(|t| {
                if let Expr::Ident(name, _) = t { name == &re.type_name || name == "Exception" } else { false }
            }),
            _ => false,
        }
    }

    fn import_module(&mut self, name: &str) -> EvalResult {
        match name {
            "math"      => ok(crate::stdlib::math::make_math_module()),
            "sys"       => ok(crate::stdlib::sys::make_sys_module()),
            "threading" => ok(crate::stdlib::threading_mod::make_threading_module()),
            "io"        => ok(crate::stdlib::io::make_io_module()),
            "os"        => ok(crate::stdlib::os_mod::make_os_module()),
            "time"      => ok(crate::stdlib::time_mod::make_time_module()),
            "random"    => ok(crate::stdlib::random_mod::make_random_module()),
            "json"      => ok(crate::stdlib::json_mod::make_json_module()),
            _ => err(RuntimeError::new("ImportError", format!("No module named '{}'", name))),
        }
    }

    // ── String methods ────────────────────────────────────────────────────────
    fn str_method(&self, s: Arc<String>, attr: &str) -> EvalResult {
        use crate::interpreter::methods::*;
        let recv = Value::Str(s.clone());
        macro_rules! bm {
            ($f:expr) => { ok(Value::BoundBuiltin { receiver: Box::new(recv), name: attr_to_static(attr), func: $f }) }
        }
        match attr {
            // Zero-arg methods — compute eagerly (no receiver needed)
            "upper"      => ok(Value::Str(Arc::new(s.to_uppercase()))),
            "lower"      => ok(Value::Str(Arc::new(s.to_lowercase()))),
            "title"      => {
                let t: String = s.split_whitespace()
                    .map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().to_string() + c.as_str()).unwrap_or_default() })
                    .collect::<Vec<_>>().join(" ");
                ok(Value::Str(Arc::new(t)))
            }
            "swapcase"   => ok(Value::Str(Arc::new(s.chars().map(|c| if c.is_uppercase() { c.to_lowercase().next().unwrap_or(c) } else { c.to_uppercase().next().unwrap_or(c) }).collect()))),
            "capitalize" => {
                let mut c = s.chars();
                let r = c.next().map(|f| f.to_uppercase().to_string() + &c.as_str().to_lowercase()).unwrap_or_default();
                ok(Value::Str(Arc::new(r)))
            }
            "isdigit"    => ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))),
            "isnumeric"  => ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_numeric()))),
            "isalpha"    => ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_alphabetic()))),
            "isalnum"    => ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_alphanumeric()))),
            "isspace"    => ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_whitespace()))),
            "isupper"    => ok(Value::Bool(!s.is_empty() && s.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()))),
            "islower"    => ok(Value::Bool(!s.is_empty() && s.chars().all(|c| !c.is_alphabetic() || c.is_lowercase()))),
            "isidentifier" => ok(Value::Bool(!s.is_empty() && {
                let mut cs = s.chars();
                cs.next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false)
                && cs.all(|c| c.is_alphanumeric() || c == '_')
            })),
            // Arg-taking methods — use BoundBuiltin
            "split"       => bm!(str_split),
            "rsplit"      => bm!(str_rsplit),
            "splitlines"  => bm!(str_splitlines),
            "join"        => bm!(str_join),
            "replace"     => bm!(str_replace),
            "startswith"  => bm!(str_startswith),
            "endswith"    => bm!(str_endswith),
            "find"        => bm!(str_find),
            "rfind"       => bm!(str_rfind),
            "index"       => bm!(str_index),
            "rindex"      => bm!(str_rfind),
            "count"       => bm!(str_count),
            "strip"       => bm!(str_strip),
            "lstrip"      => bm!(str_lstrip),
            "rstrip"      => bm!(str_rstrip),
            "encode"      => bm!(str_encode),
            "format"      => bm!(str_format),
            "zfill"       => bm!(str_zfill),
            "center"      => bm!(str_center),
            "ljust"       => bm!(str_ljust),
            "rjust"       => bm!(str_rjust),
            "expandtabs"  => bm!(str_expandtabs),
            _ => err(RuntimeError::attr_error("str", attr)),
        }
    }

    fn list_method(&self, l: crate::runtime::gc::GcHandle<Vec<Value>>, attr: &str) -> EvalResult {
        use crate::interpreter::methods::*;
        let recv = Value::List(l);
        macro_rules! bm {
            ($f:expr) => { ok(Value::BoundBuiltin { receiver: Box::new(recv), name: attr_to_static(attr), func: $f }) }
        }
        match attr {
            "append"  => bm!(list_append),
            "pop"     => bm!(list_pop),
            "extend"  => bm!(list_extend),
            "insert"  => bm!(list_insert),
            "remove"  => bm!(list_remove),
            "index"   => bm!(list_index),
            "count"   => bm!(list_count),
            "sort"    => bm!(list_sort),
            "reverse" => bm!(list_reverse),
            "clear"   => bm!(list_clear),
            "copy"    => bm!(list_copy),
            _ => err(RuntimeError::attr_error("list", attr)),
        }
    }

    fn dict_method(&self, d: crate::runtime::gc::GcHandle<HashMap<HashableValue, Value>>, attr: &str) -> EvalResult {
        use crate::interpreter::methods::*;
        let recv = Value::Dict(d);
        macro_rules! bm {
            ($f:expr) => { ok(Value::BoundBuiltin { receiver: Box::new(recv), name: attr_to_static(attr), func: $f }) }
        }
        match attr {
            "get"          => bm!(dict_get_default),
            "keys"         => bm!(dict_keys),
            "values"       => bm!(dict_values),
            "items"        => bm!(dict_items),
            "update"       => bm!(dict_update),
            "pop"          => bm!(dict_pop),
            "setdefault"   => bm!(dict_setdefault),
            "clear"        => bm!(dict_clear),
            "copy"         => bm!(dict_copy),
            "__contains__" => bm!(dict_contains),
            _ => err(RuntimeError::attr_error("dict", attr)),
        }
    }

    fn set_method(&self, s: crate::runtime::gc::GcHandle<std::collections::HashSet<HashableValue>>, attr: &str) -> EvalResult {
        use crate::interpreter::methods::*;
        let recv = Value::Set(s);
        macro_rules! bm {
            ($f:expr) => { ok(Value::BoundBuiltin { receiver: Box::new(recv), name: attr_to_static(attr), func: $f }) }
        }
        match attr {
            "add"                   => bm!(set_add),
            "remove"                => bm!(set_remove),
            "discard"               => bm!(set_discard),
            "pop"                   => bm!(set_pop),
            "clear"                 => bm!(set_clear),
            "copy"                  => bm!(set_copy),
            "union"                 => bm!(set_union),
            "intersection"          => bm!(set_intersection),
            "difference"            => bm!(set_difference),
            "symmetric_difference"  => bm!(set_symmetric_difference),
            "issubset"              => bm!(set_issubset),
            "issuperset"            => bm!(set_issuperset),
            "isdisjoint"            => bm!(set_isdisjoint),
            "update"                => bm!(set_update),
            "intersection_update"   => bm!(set_intersection_update),
            "difference_update"     => bm!(set_difference_update),
            _ => err(RuntimeError::attr_error("set", attr)),
        }
    }

    fn tuple_method(&self, t: Arc<Vec<Value>>, attr: &str) -> EvalResult {
        use crate::interpreter::methods::*;
        let recv = Value::Tuple(t);
        macro_rules! bm {
            ($f:expr) => { ok(Value::BoundBuiltin { receiver: Box::new(recv), name: attr_to_static(attr), func: $f }) }
        }
        match attr {
            "count" => bm!(tuple_count),
            "index" => bm!(tuple_index),
            _ => err(RuntimeError::attr_error("tuple", attr)),
        }
    }

    fn bytes_method(&self, b: Arc<Vec<u8>>, attr: &str) -> EvalResult {
        match attr {
            "decode" => {
                let s = String::from_utf8_lossy(&b).into_owned();
                ok(Value::Str(Arc::new(s)))
            }
            "hex" => {
                let h: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
                ok(Value::Str(Arc::new(h)))
            }
            _ => err(RuntimeError::attr_error("bytes", attr)),
        }
    }
}

// ── Bound method helper ───────────────────────────────────────────────────────

fn bind_method(method: Value, instance: Value) -> Value {
    if let Value::Function(f) = method {
        Value::BoundMethod { receiver: Box::new(instance), method: f }
    } else {
        method
    }
}

// ── Static attr name helper ───────────────────────────────────────────────────

fn attr_to_static(s: &str) -> &'static str {
    // leak once — only called for known method names
    Box::leak(s.to_string().into_boxed_str())
}

// ── Noop builtin placeholder ──────────────────────────────────────────────────

fn builtin_noop(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::None)
}

// ── Range value (used in collect_iter) ────────────────────────────────────────
// We extend Value with a Range variant only in this file via a local impl trick.
// Instead we expose a range helper that returns a List.

// ── Built-in functions ────────────────────────────────────────────────────────

fn builtin_print(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let parts: Vec<String> = args.iter().map(|v| v.str_display()).collect();
    println!("{}", parts.join(" "));
    Ok(Value::None)
}

fn builtin_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::List(l))  => Ok(Value::Int(l.lock().unwrap().len() as i64)),
        Some(Value::Tuple(t)) => Ok(Value::Int(t.len() as i64)),
        Some(Value::Str(s))   => Ok(Value::Int(s.chars().count() as i64)),
        Some(Value::Dict(d))  => Ok(Value::Int(d.lock().unwrap().len() as i64)),
        Some(Value::Set(s))   => Ok(Value::Int(s.lock().unwrap().len() as i64)),
        Some(Value::Bytes(b)) => Ok(Value::Int(b.len() as i64)),
        Some(v) => Err(RuntimeError::type_error(format!("object of type '{}' has no len()", v.type_name()))),
        None    => Err(RuntimeError::type_error("len() takes exactly 1 argument")),
    }
}

fn builtin_range(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let to_i = |v: &Value| match v {
        Value::Int(n)  => Ok(*n),
        Value::Bool(b) => Ok(*b as i64),
        _ => Err(RuntimeError::type_error("range() requires integer arguments")),
    };
    let (start, stop, step) = match args.len() {
        1 => (0, to_i(&args[0])?, 1),
        2 => (to_i(&args[0])?, to_i(&args[1])?, 1),
        3 => (to_i(&args[0])?, to_i(&args[1])?, to_i(&args[2])?),
        _ => return Err(RuntimeError::type_error("range() takes 1-3 arguments")),
    };
    if step == 0 { return Err(RuntimeError::value_error("range() step argument must not be zero")); }
    Ok(Value::Range(start, stop, step))
}

fn builtin_int(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Int(n))   => Ok(Value::Int(*n)),
        Some(Value::Float(f)) => Ok(Value::Int(*f as i64)),
        Some(Value::Bool(b))  => Ok(Value::Int(*b as i64)),
        Some(Value::Str(s))   => s.trim().parse::<i64>().map(Value::Int)
            .map_err(|_| RuntimeError::value_error(format!("invalid literal for int(): '{}'", s))),
        Some(Value::None) => Ok(Value::Int(0)),
        _ => Err(RuntimeError::type_error("int() requires a numeric or string argument")),
    }
}

fn builtin_float(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Float(f)) => Ok(Value::Float(*f)),
        Some(Value::Int(n))   => Ok(Value::Float(*n as f64)),
        Some(Value::Bool(b))  => Ok(Value::Float(*b as u8 as f64)),
        Some(Value::Str(s))   => s.trim().parse::<f64>().map(Value::Float)
            .map_err(|_| RuntimeError::value_error(format!("invalid literal for float(): '{}'", s))),
        _ => Err(RuntimeError::type_error("float() requires a numeric or string argument")),
    }
}

fn builtin_str(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Str(Arc::new(args.first().map(|v| v.str_display()).unwrap_or_default())))
}

fn builtin_repr(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Str(Arc::new(args.first().map(|v| v.repr()).unwrap_or("None".into()))))
}

fn builtin_bool(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(args.first().map(|v| v.is_truthy()).unwrap_or(false)))
}

fn builtin_type(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Str(Arc::new(args.first().map(|v| v.type_name().to_string()).unwrap_or("NoneType".into()))))
}

fn builtin_isinstance(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() < 2 { return Err(RuntimeError::type_error("isinstance() requires 2 arguments")); }
    let type_name = match &args[1] {
        Value::Class(c) => c.name.clone(),
        Value::Str(s) => s.as_ref().clone(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(args[0].type_name() == type_name))
}

fn builtin_abs(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Int(n))   => Ok(Value::Int(n.wrapping_abs())),
        Some(Value::Float(f)) => Ok(Value::Float(f.abs())),
        _ => Err(RuntimeError::type_error("abs() requires numeric argument")),
    }
}

fn builtin_max(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let items = if args.len() == 1 { vec![args[0].clone()] } else { args };
    items.into_iter().reduce(|a, b| {
        match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => if x >= y { a.clone() } else { b.clone() },
            _ => a.clone(),
        }
    }).ok_or_else(|| RuntimeError::value_error("max() arg is an empty sequence"))
}

fn builtin_min(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let items = if args.len() == 1 { vec![args[0].clone()] } else { args };
    items.into_iter().reduce(|a, b| {
        match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => if x <= y { a.clone() } else { b.clone() },
            _ => a.clone(),
        }
    }).ok_or_else(|| RuntimeError::value_error("min() arg is an empty sequence"))
}

fn builtin_sum(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let items = match args.first() {
        Some(Value::List(l)) => l.lock().unwrap().clone(),
        Some(Value::Tuple(t)) => t.as_ref().clone(),
        _ => return Err(RuntimeError::type_error("sum() requires iterable")),
    };
    let mut total: i64 = match args.get(1) { Some(Value::Int(n)) => *n, _ => 0 };
    for v in items {
        match v { Value::Int(n) => total += n, Value::Bool(b) => total += b as i64, _ => {} }
    }
    Ok(Value::Int(total))
}

fn builtin_sorted(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let items = match args.first() {
        Some(Value::List(l)) => l.lock().unwrap().clone(),
        Some(Value::Tuple(t)) => t.as_ref().clone(),
        _ => return Err(RuntimeError::type_error("sorted() requires iterable")),
    };
    let mut v = items;
    v.sort_by(|a, b| {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => x.cmp(y),
            (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Str(x), Value::Str(y)) => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        }
    });
    // check reverse flag
    if args.get(1).map_or(false, |a| a.is_truthy()) { v.reverse(); }
    Ok(Value::List(crate::runtime::gc::alloc_list(v)))
}

fn builtin_list(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        None => Ok(Value::List(crate::runtime::gc::alloc_list(vec![]))),
        Some(Value::List(l)) => Ok(Value::List(crate::runtime::gc::alloc_list(l.lock().unwrap().clone()))),
        Some(Value::Tuple(t)) => Ok(Value::List(crate::runtime::gc::alloc_list(t.as_ref().clone()))),
        Some(Value::Str(s)) => Ok(Value::List(crate::runtime::gc::alloc_list(
            s.chars().map(|c| Value::Str(Arc::new(c.to_string()))).collect()
        ))),
        Some(Value::Set(s)) => Ok(Value::List(crate::runtime::gc::alloc_list(
            s.lock().unwrap().iter().cloned().map(Value::from).collect()
        ))),
        Some(Value::Dict(d)) => Ok(Value::List(crate::runtime::gc::alloc_list(
            d.lock().unwrap().keys().cloned().map(Value::from).collect()
        ))),
        Some(v) => Err(RuntimeError::type_error(format!("list() argument '{}' is not iterable", v.type_name()))),
    }
}

fn builtin_dict_fn(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Dict(crate::runtime::gc::alloc_dict(HashMap::new())))
}

fn builtin_tuple(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        None => Ok(Value::Tuple(Arc::new(vec![]))),
        Some(Value::List(l)) => Ok(Value::Tuple(Arc::new(l.lock().unwrap().clone()))),
        Some(Value::Tuple(t)) => Ok(Value::Tuple(t.clone())),
        Some(v) => Err(RuntimeError::type_error(format!("tuple() argument '{}' is not iterable", v.type_name()))),
    }
}

fn builtin_set(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let mut s = std::collections::HashSet::new();
    if let Some(v) = args.first() {
        let items = match v {
            Value::List(l) => l.lock().unwrap().clone(),
            Value::Tuple(t) => t.as_ref().clone(),
            _ => return Err(RuntimeError::type_error("set() requires iterable")),
        };
        for item in items {
            let h = HashableValue::try_from(&item).map_err(|m| RuntimeError::type_error(m))?;
            s.insert(h);
        }
    }
    Ok(Value::Set(crate::runtime::gc::alloc_set(s)))
}

fn builtin_enumerate(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let items = match args.first() {
        Some(Value::List(l)) => l.lock().unwrap().clone(),
        Some(Value::Tuple(t)) => t.as_ref().clone(),
        _ => return Err(RuntimeError::type_error("enumerate() requires iterable")),
    };
    let start: i64 = match args.get(1) { Some(Value::Int(n)) => *n, _ => 0 };
    let result: Vec<Value> = items.into_iter().enumerate()
        .map(|(i, v)| Value::Tuple(Arc::new(vec![Value::Int(i as i64 + start), v])))
        .collect();
    Ok(Value::List(crate::runtime::gc::alloc_list(result)))
}

fn builtin_zip(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.is_empty() { return Ok(Value::List(crate::runtime::gc::alloc_list(vec![]))); }
    let vecs: Vec<Vec<Value>> = args.iter().map(|a| match a {
        Value::List(l) => l.lock().unwrap().clone(),
        Value::Tuple(t) => t.as_ref().clone(),
        _ => vec![],
    }).collect();
    let min_len = vecs.iter().map(|v| v.len()).min().unwrap_or(0);
    let result: Vec<Value> = (0..min_len)
        .map(|i| Value::Tuple(Arc::new(vecs.iter().map(|v| v[i].clone()).collect())))
        .collect();
    Ok(Value::List(crate::runtime::gc::alloc_list(result)))
}

fn builtin_map_fn(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::List(crate::runtime::gc::alloc_list(vec![])))
}

fn builtin_filter_fn(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::List(crate::runtime::gc::alloc_list(vec![])))
}

fn builtin_reversed(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::List(l)) => {
            let mut v = l.lock().unwrap().clone();
            v.reverse();
            Ok(Value::List(crate::runtime::gc::alloc_list(v)))
        }
        _ => Err(RuntimeError::type_error("reversed() requires a sequence")),
    }
}

fn builtin_ord(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Str(s)) if s.chars().count() == 1 => Ok(Value::Int(s.chars().next().unwrap() as i64)),
        _ => Err(RuntimeError::type_error("ord() expected a character")),
    }
}

fn builtin_chr(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Int(n)) => char::from_u32(*n as u32)
            .map(|c| Value::Str(Arc::new(c.to_string())))
            .ok_or_else(|| RuntimeError::value_error("chr() arg not in range(0x110000)")),
        _ => Err(RuntimeError::type_error("chr() requires int")),
    }
}

fn builtin_hex(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Str(Arc::new(format!("{:#x}", n)))),
        _ => Err(RuntimeError::type_error("hex() requires int")),
    }
}

fn builtin_oct(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Str(Arc::new(format!("{:#o}", n)))),
        _ => Err(RuntimeError::type_error("oct() requires int")),
    }
}

fn builtin_bin(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Str(Arc::new(format!("{:#b}", n)))),
        _ => Err(RuntimeError::type_error("bin() requires int")),
    }
}

fn builtin_round(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::Float(f)), Some(Value::Int(n))) => {
            let factor = 10f64.powi(*n as i32);
            Ok(Value::Float((f * factor).round() / factor))
        }
        (Some(Value::Float(f)), _) => Ok(Value::Int(f.round() as i64)),
        (Some(Value::Int(n)), _) => Ok(Value::Int(*n)),
        _ => Err(RuntimeError::type_error("round() requires numeric")),
    }
}

fn builtin_divmod(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => {
            if *b == 0 { return Err(RuntimeError::zero_div()); }
            Ok(Value::Tuple(Arc::new(vec![Value::Int(a / b), Value::Int(a % b)])))
        }
        _ => Err(RuntimeError::type_error("divmod() requires two numbers")),
    }
}

fn builtin_pow(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) if *b >= 0 => Ok(Value::Int(a.wrapping_pow(*b as u32))),
        (Some(Value::Float(a)), Some(Value::Float(b))) => Ok(Value::Float(a.powf(*b))),
        (Some(Value::Int(a)), Some(Value::Float(b))) => Ok(Value::Float((*a as f64).powf(*b))),
        (Some(Value::Float(a)), Some(Value::Int(b))) => Ok(Value::Float(a.powf(*b as f64))),
        _ => Err(RuntimeError::type_error("pow() requires numeric arguments")),
    }
}

fn builtin_input(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(prompt) = args.first() { print!("{}", prompt.str_display()); }
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map_err(|e| RuntimeError::new("IOError", e.to_string()))?;
    Ok(Value::Str(Arc::new(line.trim_end_matches('\n').trim_end_matches('\r').to_string())))
}

fn builtin_id(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Int(0)) // placeholder
}

fn builtin_hash(args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    match args.first() {
        Some(v) => {
            let h = HashableValue::try_from(v).map_err(|m| RuntimeError::type_error(m))?;
            let mut hasher = DefaultHasher::new();
            h.hash(&mut hasher);
            Ok(Value::Int(hasher.finish() as i64))
        }
        None => Err(RuntimeError::type_error("hash() requires 1 argument")),
    }
}

fn builtin_hasattr(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(false)) // simplified
}

fn builtin_getattr(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() < 2 { return Err(RuntimeError::type_error("getattr() requires at least 2 arguments")); }
    Ok(args.get(2).cloned().unwrap_or(Value::None))
}

fn builtin_setattr(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::None)
}

fn builtin_callable(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Function(_)) | Some(Value::BuiltinFunction(_)) | Some(Value::Class(_)))))
}

fn builtin_vars(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Dict(crate::runtime::gc::alloc_dict(HashMap::new())))
}

fn builtin_dir(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::List(crate::runtime::gc::alloc_list(vec![])))
}

fn builtin_iter(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(args.into_iter().next().unwrap_or(Value::None))
}

fn builtin_next(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    Err(RuntimeError::stop_iteration())
}
