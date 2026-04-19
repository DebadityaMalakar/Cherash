pub mod types;

use types::Type;
use crate::parser::ast::*;
use std::collections::HashMap;

#[derive(Debug)]
pub struct TypeCheckError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypeError at {}:{}: {}", self.line, self.col, self.msg)
    }
}

pub enum Mode { Strict, Lenient }

pub struct TypeChecker {
    pub mode: Mode,
    env: Vec<HashMap<String, Type>>,
    // Stack of expected return types for nested function defs
    return_stack: Vec<Option<Type>>,
}

impl TypeChecker {
    pub fn new(mode: Mode) -> Self {
        TypeChecker { mode, env: vec![HashMap::new()], return_stack: vec![] }
    }

    pub fn check_module(&mut self, module: &Module) -> Vec<TypeCheckError> {
        let mut errors = Vec::new();
        if matches!(self.mode, Mode::Lenient) { return errors; }
        for stmt in &module.body {
            self.check_stmt(stmt, &mut errors);
        }
        errors
    }

    fn check_stmt(&mut self, stmt: &Stmt, errors: &mut Vec<TypeCheckError>) {
        match stmt {
            Stmt::AnnAssign { target, annotation, value, .. } => {
                let expected = Type::from_annotation(annotation);
                if let Some(val_expr) = value {
                    let actual = self.infer_expr(val_expr);
                    if !expected.is_assignable_from(&actual) {
                        let span = target.span();
                        errors.push(TypeCheckError {
                            msg: format!("expected {}, got {}", expected.display(), actual.display()),
                            line: span.line, col: span.col,
                        });
                    }
                }
                if let Expr::Ident(name, _) = target {
                    self.set_type(name, expected);
                }
            }
            Stmt::Assign { targets, value, .. } => {
                let val_type = self.infer_expr(value);
                for target in targets {
                    if let Expr::Ident(name, _) = target {
                        // Only set if not already annotated with a specific type
                        if self.get_type(name).map(|t| matches!(t, Type::Unknown)).unwrap_or(true) {
                            self.set_type(name, val_type.clone());
                        } else if let Some(expected) = self.get_type(name) {
                            if !expected.is_assignable_from(&val_type) {
                                let span = target.span();
                                errors.push(TypeCheckError {
                                    msg: format!("cannot assign {} to variable '{}' of type {}", val_type.display(), name, expected.display()),
                                    line: span.line, col: span.col,
                                });
                            }
                        }
                    }
                }
            }
            Stmt::FunctionDef { name, params, return_annotation, body, .. } => {
                self.push_scope();
                // Bind parameter types
                for param in params {
                    let t = param.annotation.as_ref().map(Type::from_annotation).unwrap_or(Type::Any);
                    self.set_type(&param.name, t);
                }
                let ret_type = return_annotation.as_ref().map(Type::from_annotation);
                self.return_stack.push(ret_type.clone());
                for s in body { self.check_stmt(s, errors); }
                self.return_stack.pop();
                self.pop_scope();
                // Store function type
                self.set_type(name, Type::Named(name.clone()));
            }
            Stmt::Return { value, span } => {
                if let Some(expected_ret) = self.return_stack.last().and_then(|r| r.clone()) {
                    let actual = match value {
                        Some(e) => self.infer_expr(e),
                        None    => Type::None,
                    };
                    if !expected_ret.is_assignable_from(&actual) {
                        errors.push(TypeCheckError {
                            msg: format!("return type mismatch: expected {}, got {}", expected_ret.display(), actual.display()),
                            line: span.line, col: span.col,
                        });
                    }
                }
            }
            Stmt::If { test: _, body, orelse, .. } => {
                self.push_scope();
                for s in body { self.check_stmt(s, errors); }
                self.pop_scope();
                self.push_scope();
                for s in orelse { self.check_stmt(s, errors); }
                self.pop_scope();
            }
            Stmt::While { body, orelse, .. } => {
                self.push_scope();
                for s in body { self.check_stmt(s, errors); }
                self.pop_scope();
                for s in orelse { self.check_stmt(s, errors); }
            }
            Stmt::For { target, iter, body, orelse, .. } => {
                // Infer element type from iterator
                let iter_type = self.infer_expr(iter);
                let elem_type = match &iter_type {
                    Type::List(inner) => *inner.clone(),
                    Type::Set(inner)  => *inner.clone(),
                    _ => Type::Any,
                };
                self.push_scope();
                if let Expr::Ident(name, _) = target {
                    self.set_type(name, elem_type);
                }
                for s in body { self.check_stmt(s, errors); }
                self.pop_scope();
                for s in orelse { self.check_stmt(s, errors); }
            }
            Stmt::With { items, body, .. } => {
                self.push_scope();
                for item in items {
                    let ctx_type = self.infer_expr(&item.context_expr);
                    if let Some(Expr::Ident(name, _)) = &item.optional_vars {
                        // For `with open(...) as f:`, f gets the context manager's return type
                        self.set_type(name, ctx_type);
                    }
                }
                for s in body { self.check_stmt(s, errors); }
                self.pop_scope();
            }
            Stmt::Try { body, handlers, orelse, finalbody, .. } => {
                self.push_scope();
                for s in body { self.check_stmt(s, errors); }
                self.pop_scope();
                for handler in handlers {
                    self.push_scope();
                    if let Some(name) = &handler.name {
                        self.set_type(name, Type::Named("Exception".into()));
                    }
                    for s in &handler.body { self.check_stmt(s, errors); }
                    self.pop_scope();
                }
                for s in orelse   { self.check_stmt(s, errors); }
                for s in finalbody{ self.check_stmt(s, errors); }
            }
            Stmt::ClassDef { name, body, .. } => {
                self.push_scope();
                for s in body { self.check_stmt(s, errors); }
                self.pop_scope();
                self.set_type(name, Type::Named(name.clone()));
            }
            _ => {}
        }
    }

    fn infer_expr(&self, expr: &Expr) -> Type {
        match expr {
            Expr::IntLit(..)   => Type::Int,
            Expr::FloatLit(..) => Type::Float,
            Expr::StrLit(..)   => Type::Str,
            Expr::FStringLit(..) => Type::Str,
            Expr::BoolLit(..)  => Type::Bool,
            Expr::NoneLit(..)  => Type::None,
            Expr::BytesLit(..) => Type::Bytes,
            Expr::Ident(name, _) => self.get_type(name).unwrap_or(Type::Unknown),
            Expr::Binary { op, left, right, .. } => {
                let lt = self.infer_expr(left);
                let rt = self.infer_expr(right);
                infer_binop(op, &lt, &rt)
            }
            Expr::BoolOp { .. } => Type::Bool,
            Expr::Compare { .. } => Type::Bool,
            Expr::Unary { op, expr, .. } => match op {
                UnaryOp::Not => Type::Bool,
                _ => self.infer_expr(expr),
            },
            Expr::List(elts, _) => {
                let elem = elts.first().map(|e| self.infer_expr(e)).unwrap_or(Type::Any);
                Type::List(Box::new(elem))
            }
            Expr::Tuple(elts, _) => Type::Tuple(elts.iter().map(|e| self.infer_expr(e)).collect()),
            Expr::Dict { keys, values, .. } => {
                let k = keys.iter().flat_map(|k| k.as_ref()).next()
                    .map(|e| self.infer_expr(e)).unwrap_or(Type::Any);
                let v = values.first().map(|e| self.infer_expr(e)).unwrap_or(Type::Any);
                Type::Dict(Box::new(k), Box::new(v))
            }
            Expr::Set(elts, _) => {
                let elem = elts.first().map(|e| self.infer_expr(e)).unwrap_or(Type::Any);
                Type::Set(Box::new(elem))
            }
            Expr::IfExpr { body, orelse, .. } => {
                let t1 = self.infer_expr(body);
                let t2 = self.infer_expr(orelse);
                if t1 == t2 { t1 } else { Type::Union(vec![t1, t2]) }
            }
            Expr::Call { func, .. } => {
                // Infer return type from known functions
                match func.as_ref() {
                    Expr::Ident(name, _) => match name.as_str() {
                        "int"   => Type::Int,
                        "float" => Type::Float,
                        "str"   => Type::Str,
                        "bool"  => Type::Bool,
                        "list"  => Type::List(Box::new(Type::Any)),
                        "dict"  => Type::Dict(Box::new(Type::Any), Box::new(Type::Any)),
                        "set"   => Type::Set(Box::new(Type::Any)),
                        "len"   => Type::Int,
                        "range" => Type::List(Box::new(Type::Int)),
                        _       => Type::Any,
                    },
                    _ => Type::Any,
                }
            }
            Expr::ListComp { .. }   => Type::List(Box::new(Type::Any)),
            Expr::SetComp { .. }    => Type::Set(Box::new(Type::Any)),
            Expr::DictComp { .. }   => Type::Dict(Box::new(Type::Any), Box::new(Type::Any)),
            Expr::Lambda { .. }     => Type::Any,
            Expr::Attr { obj, attr, .. } => {
                // A few well-known attribute accesses
                let obj_type = self.infer_expr(obj);
                match (&obj_type, attr.as_str()) {
                    (Type::Str, "upper" | "lower" | "strip" | "replace" | "join") => Type::Str,
                    (Type::List(_), "pop") => Type::Any,
                    _ => Type::Any,
                }
            }
            _ => Type::Any,
        }
    }

    fn push_scope(&mut self) { self.env.push(HashMap::new()); }
    fn pop_scope(&mut self) { self.env.pop(); }
    fn set_type(&mut self, name: &str, t: Type) {
        if let Some(scope) = self.env.last_mut() { scope.insert(name.to_string(), t); }
    }
    fn get_type(&self, name: &str) -> Option<Type> {
        for scope in self.env.iter().rev() {
            if let Some(t) = scope.get(name) { return Some(t.clone()); }
        }
        None
    }
}

fn infer_binop(op: &BinOp, l: &Type, r: &Type) -> Type {
    match (l, r) {
        (Type::Int, Type::Int) => match op {
            BinOp::Div => Type::Float,
            _ => Type::Int,
        },
        (Type::Float, _) | (_, Type::Float) => Type::Float,
        (Type::Str, Type::Str) => match op {
            BinOp::Add => Type::Str,
            BinOp::Mul => Type::Str,
            _ => Type::Any,
        },
        (Type::List(a), Type::List(b)) if a == b => Type::List(a.clone()),
        _ => Type::Any,
    }
}
