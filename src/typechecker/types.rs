/// Type representation for the Cherash type checker.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    Str,
    Bytes,
    None,
    List(Box<Type>),
    Dict(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Set(Box<Type>),
    Optional(Box<Type>),
    Union(Vec<Type>),
    Any,
    Unknown,
    Named(String),
}

impl Type {
    pub fn display(&self) -> String {
        match self {
            Type::Int     => "int".into(),
            Type::Float   => "float".into(),
            Type::Bool    => "bool".into(),
            Type::Str     => "str".into(),
            Type::Bytes   => "bytes".into(),
            Type::None    => "None".into(),
            Type::Any     => "Any".into(),
            Type::Unknown => "unknown".into(),
            Type::Named(n) => n.clone(),
            Type::List(t)  => format!("list[{}]", t.display()),
            Type::Set(t)   => format!("set[{}]", t.display()),
            Type::Dict(k, v) => format!("dict[{}, {}]", k.display(), v.display()),
            Type::Tuple(ts)  => format!("tuple[{}]", ts.iter().map(|t| t.display()).collect::<Vec<_>>().join(", ")),
            Type::Optional(t) => format!("Optional[{}]", t.display()),
            Type::Union(ts)   => ts.iter().map(|t| t.display()).collect::<Vec<_>>().join(" | "),
        }
    }

    pub fn is_assignable_from(&self, other: &Type) -> bool {
        if self == other { return true; }
        match (self, other) {
            (Type::Any, _) | (_, Type::Any) | (_, Type::Unknown) => true,
            (Type::Float, Type::Int) | (Type::Float, Type::Bool) => true,
            (Type::Int, Type::Bool) => true,
            (Type::Optional(inner), t) => inner.as_ref() == t || *t == Type::None,
            (Type::Union(types), t) => types.iter().any(|u| u.is_assignable_from(t)),
            _ => false,
        }
    }

    pub fn from_annotation(ann: &crate::parser::ast::TypeAnnotation) -> Self {
        use crate::parser::ast::TypeAnnotation;
        match ann {
            TypeAnnotation::None => Type::None,
            TypeAnnotation::Name(s) => match s.as_str() {
                "int"   => Type::Int,
                "float" => Type::Float,
                "bool"  => Type::Bool,
                "str"   => Type::Str,
                "bytes" => Type::Bytes,
                "None"  => Type::None,
                "Any"   => Type::Any,
                s       => Type::Named(s.to_string()),
            },
            TypeAnnotation::Subscript { name, params } => match name.as_str() {
                "list" => Type::List(Box::new(params.first().map(Type::from_annotation).unwrap_or(Type::Any))),
                "dict" => Type::Dict(
                    Box::new(params.first().map(Type::from_annotation).unwrap_or(Type::Any)),
                    Box::new(params.get(1).map(Type::from_annotation).unwrap_or(Type::Any)),
                ),
                "set"  => Type::Set(Box::new(params.first().map(Type::from_annotation).unwrap_or(Type::Any))),
                "Optional" => Type::Optional(Box::new(params.first().map(Type::from_annotation).unwrap_or(Type::Any))),
                "Union" => Type::Union(params.iter().map(Type::from_annotation).collect()),
                "tuple" => Type::Tuple(params.iter().map(Type::from_annotation).collect()),
                n => Type::Named(n.to_string()),
            },
            TypeAnnotation::Tuple(parts) => Type::Tuple(parts.iter().map(Type::from_annotation).collect()),
            TypeAnnotation::Optional(inner) => Type::Optional(Box::new(Type::from_annotation(inner))),
            TypeAnnotation::Union(parts) => Type::Union(parts.iter().map(Type::from_annotation).collect()),
        }
    }
}
