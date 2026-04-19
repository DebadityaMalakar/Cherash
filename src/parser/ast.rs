/// Span carries source location for error reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self { Span { line, col } }
}

// ── Expressions ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64, Span),
    FloatLit(f64, Span),
    StrLit(String, Span),
    BytesLit(Vec<u8>, Span),
    FStringLit(Vec<crate::lexer::token::FStringPart>, Span),
    BoolLit(bool, Span),
    NoneLit(Span),
    EllipsisLit(Span),

    Ident(String, Span),

    // unary op
    Unary { op: UnaryOp, expr: Box<Expr>, span: Span },
    // binary op
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr>, span: Span },
    // comparison chain: left op1 mid op2 right  (Python allows `a < b < c`)
    Compare { left: Box<Expr>, ops: Vec<CmpOp>, comparators: Vec<Expr>, span: Span },
    // boolean ops
    BoolOp { op: BoolOpKind, values: Vec<Expr>, span: Span },

    // f(args, *args, **kwargs)
    Call { func: Box<Expr>, args: Vec<Arg>, span: Span },
    // obj.attr
    Attr { obj: Box<Expr>, attr: String, span: Span },
    // obj[index]
    Index { obj: Box<Expr>, index: Box<Expr>, span: Span },
    // obj[lo:hi:step]
    Slice { obj: Box<Expr>, lower: Option<Box<Expr>>, upper: Option<Box<Expr>>, step: Option<Box<Expr>>, span: Span },

    // [expr for target in iter if cond]
    ListComp { elt: Box<Expr>, generators: Vec<Comprehension>, span: Span },
    SetComp  { elt: Box<Expr>, generators: Vec<Comprehension>, span: Span },
    DictComp { key: Box<Expr>, value: Box<Expr>, generators: Vec<Comprehension>, span: Span },
    GeneratorExp { elt: Box<Expr>, generators: Vec<Comprehension>, span: Span },

    // [a, b, c]
    List(Vec<Expr>, Span),
    // (a, b, c)
    Tuple(Vec<Expr>, Span),
    // {a, b, c}
    Set(Vec<Expr>, Span),
    // {k: v, ...}
    Dict { keys: Vec<Option<Expr>>, values: Vec<Expr>, span: Span },

    // lambda args: expr
    Lambda { params: Vec<Param>, body: Box<Expr>, span: Span },
    // expr if test else orelse
    IfExpr { test: Box<Expr>, body: Box<Expr>, orelse: Box<Expr>, span: Span },

    // await expr
    Await(Box<Expr>, Span),
    // yield expr
    Yield(Option<Box<Expr>>, Span),
    // yield from expr
    YieldFrom(Box<Expr>, Span),

    // *expr (starred in assignment target)
    Starred(Box<Expr>, Span),

    // name := expr (walrus)
    NamedExpr { target: String, value: Box<Expr>, span: Span },
}

impl Expr {
    pub fn span(&self) -> &Span {
        match self {
            Expr::IntLit(_, s) | Expr::FloatLit(_, s) | Expr::StrLit(_, s)
            | Expr::BytesLit(_, s) | Expr::FStringLit(_, s) | Expr::BoolLit(_, s)
            | Expr::NoneLit(s) | Expr::EllipsisLit(s) | Expr::Ident(_, s) => s,
            Expr::Unary { span, .. } | Expr::Binary { span, .. } | Expr::Compare { span, .. }
            | Expr::BoolOp { span, .. } | Expr::Call { span, .. } | Expr::Attr { span, .. }
            | Expr::Index { span, .. } | Expr::Slice { span, .. } | Expr::ListComp { span, .. }
            | Expr::SetComp { span, .. } | Expr::DictComp { span, .. }
            | Expr::GeneratorExp { span, .. } | Expr::List(_, span) | Expr::Tuple(_, span)
            | Expr::Set(_, span) | Expr::Dict { span, .. } | Expr::Lambda { span, .. }
            | Expr::IfExpr { span, .. } | Expr::Await(_, span) | Expr::Yield(_, span)
            | Expr::YieldFrom(_, span) | Expr::Starred(_, span) | Expr::NamedExpr { span, .. } => span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp { Plus, Minus, Not, BitNot }

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, FloorDiv, Mod, Pow,
    BitAnd, BitOr, BitXor, LShift, RShift,
    MatMul,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CmpOp { Eq, NotEq, Lt, LtEq, Gt, GtEq, Is, IsNot, In, NotIn }

#[derive(Debug, Clone, PartialEq)]
pub enum BoolOpKind { And, Or }

/// A single argument in a function call.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Pos(Expr),
    Keyword { name: String, value: Expr },
    StarArgs(Expr),
    DoubleStarArgs(Expr),
}

/// A comprehension clause: `for target in iter if cond`.
#[derive(Debug, Clone, PartialEq)]
pub struct Comprehension {
    pub target: Expr,
    pub iter: Expr,
    pub ifs: Vec<Expr>,
    pub is_async: bool,
}

// ── Type annotations ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TypeAnnotation {
    Name(String),
    Subscript { name: String, params: Vec<TypeAnnotation> },
    Tuple(Vec<TypeAnnotation>),
    None,
    Optional(Box<TypeAnnotation>),
    Union(Vec<TypeAnnotation>),
}

// ── Function parameters ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub annotation: Option<TypeAnnotation>,
    pub default: Option<Expr>,
    pub kind: ParamKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    Regular,
    VarArgs,   // *args
    KeywordOnly,
    DoubleStarArgs, // **kwargs
    PosOnly,   // before /
}

// ── Statements ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Expr(Expr),

    // assignments
    Assign { targets: Vec<Expr>, value: Expr, span: Span },
    AugAssign { target: Expr, op: BinOp, value: Expr, span: Span },
    AnnAssign { target: Expr, annotation: TypeAnnotation, value: Option<Expr>, span: Span },

    // deletion
    Delete { targets: Vec<Expr>, span: Span },

    // control flow
    Return { value: Option<Expr>, span: Span },
    Raise  { exc: Option<Expr>, cause: Option<Expr>, span: Span },
    Break(Span),
    Continue(Span),
    Pass(Span),

    // global / nonlocal
    Global   { names: Vec<String>, span: Span },
    Nonlocal { names: Vec<String>, span: Span },

    // if / elif / else
    If { test: Expr, body: Vec<Stmt>, orelse: Vec<Stmt>, span: Span },

    // while / for
    While { test: Expr, body: Vec<Stmt>, orelse: Vec<Stmt>, span: Span },
    For { target: Expr, iter: Expr, body: Vec<Stmt>, orelse: Vec<Stmt>, is_async: bool, span: Span },

    // with
    With { items: Vec<WithItem>, body: Vec<Stmt>, is_async: bool, span: Span },

    // try
    Try {
        body: Vec<Stmt>,
        handlers: Vec<ExceptHandler>,
        orelse: Vec<Stmt>,
        finalbody: Vec<Stmt>,
        span: Span,
    },

    // assert
    Assert { test: Expr, msg: Option<Expr>, span: Span },

    // import
    Import { names: Vec<Alias>, span: Span },
    ImportFrom { module: Option<String>, names: Vec<Alias>, level: usize, span: Span },

    // function def
    FunctionDef {
        name: String,
        params: Vec<Param>,
        return_annotation: Option<TypeAnnotation>,
        body: Vec<Stmt>,
        decorators: Vec<Expr>,
        is_async: bool,
        span: Span,
    },

    // class def
    ClassDef {
        name: String,
        bases: Vec<Expr>,
        keywords: Vec<(String, Expr)>,
        body: Vec<Stmt>,
        decorators: Vec<Expr>,
        span: Span,
    },

    // yield as statement (wrapped in Expr normally, but also standalone)
    Yield { value: Option<Expr>, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithItem {
    pub context_expr: Expr,
    pub optional_vars: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExceptHandler {
    pub typ: Option<Expr>,
    pub name: Option<String>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alias {
    pub name: String,
    pub asname: Option<String>,
}

/// The full program is a module.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub body: Vec<Stmt>,
}
