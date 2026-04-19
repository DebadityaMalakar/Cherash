/// All token types in the Cherash/Python grammar.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    FString(Vec<FStringPart>),

    // Identifiers
    Ident(String),

    // Keywords
    False,
    None,
    True,
    And,
    As,
    Assert,
    Async,
    Await,
    Break,
    Class,
    Continue,
    Def,
    Del,
    Elif,
    Else,
    Except,
    Finally,
    For,
    From,
    Global,
    If,
    Import,
    In,
    Is,
    Lambda,
    Nonlocal,
    Not,
    Or,
    Pass,
    Raise,
    Return,
    Try,
    While,
    With,
    Yield,

    // Operators
    Plus,        // +
    Minus,       // -
    Star,        // *
    DoubleStar,  // **
    Slash,       // /
    DoubleSlash, // //
    Percent,     // %
    At,          // @  (matrix multiply / decorator)
    Amp,         // &
    Pipe,        // |
    Caret,       // ^
    Tilde,       // ~
    LtLt,        // <<
    GtGt,        // >>
    Lt,          // <
    Gt,          // >
    LtEq,        // <=
    GtEq,        // >=
    EqEq,        // ==
    BangEq,      // !=

    // Assignment
    Eq,          // =
    PlusEq,      // +=
    MinusEq,     // -=
    StarEq,      // *=
    DoubleStarEq, // **=
    SlashEq,     // /=
    DoubleSlashEq, // //=
    PercentEq,   // %=
    AtEq,        // @=
    AmpEq,       // &=
    PipeEq,      // |=
    CaretEq,     // ^=
    LtLtEq,      // <<=
    GtGtEq,      // >>=
    Walrus,      // :=

    // Delimiters
    LParen,      // (
    RParen,      // )
    LBracket,    // [
    RBracket,    // ]
    LBrace,      // {
    RBrace,      // }
    Comma,       // ,
    Colon,       // :
    Semicolon,   // ;
    Dot,         // .
    Ellipsis,    // ...
    Arrow,       // ->
    Backtick,    // ` (not Python, reserved)

    // Structural
    Newline,
    Indent,
    Dedent,

    // End of file
    Eof,
}

/// A piece of an f-string literal.
#[derive(Debug, Clone, PartialEq)]
pub enum FStringPart {
    Literal(String),
    Expr(String),
}

/// A token with its source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Token { kind, line, col }
    }
}
