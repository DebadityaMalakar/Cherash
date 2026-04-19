use cherash::lexer::{token::TokenKind, Lexer};

fn lex(src: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(src);
    lexer.tokenize().expect("lex error").into_iter().map(|t| t.kind).collect()
}

fn lex_kinds_no_nl(src: &str) -> Vec<TokenKind> {
    lex(src).into_iter().filter(|t| !matches!(t, TokenKind::Newline | TokenKind::Eof)).collect()
}

#[test]
fn test_keywords() {
    let src = "if else elif while for break continue return def class pass import from as";
    let kinds = lex_kinds_no_nl(src);
    assert!(kinds.contains(&TokenKind::If));
    assert!(kinds.contains(&TokenKind::Else));
    assert!(kinds.contains(&TokenKind::While));
    assert!(kinds.contains(&TokenKind::Def));
    assert!(kinds.contains(&TokenKind::Class));
}

#[test]
fn test_all_keywords() {
    let keywords = ["False","None","True","and","as","assert","async","await",
        "break","class","continue","def","del","elif","else","except",
        "finally","for","from","global","if","import","in","is",
        "lambda","nonlocal","not","or","pass","raise","return",
        "try","while","with","yield"];
    for kw in keywords {
        let kinds = lex_kinds_no_nl(kw);
        assert!(!kinds.is_empty(), "keyword '{}' produced no tokens", kw);
        assert!(!matches!(kinds[0], TokenKind::Ident(_)), "keyword '{}' was lexed as ident", kw);
    }
}

#[test]
fn test_integer_literals() {
    assert!(matches!(&lex_kinds_no_nl("42")[0], TokenKind::Int(42)));
    assert!(matches!(&lex_kinds_no_nl("0xFF")[0], TokenKind::Int(255)));
    assert!(matches!(&lex_kinds_no_nl("0o17")[0], TokenKind::Int(15)));
    assert!(matches!(&lex_kinds_no_nl("0b1010")[0], TokenKind::Int(10)));
    assert!(matches!(&lex_kinds_no_nl("1_000_000")[0], TokenKind::Int(1000000)));
}

#[test]
fn test_float_literals() {
    assert!(matches!(&lex_kinds_no_nl("3.14")[0], TokenKind::Float(_)));
    assert!(matches!(&lex_kinds_no_nl("1e10")[0], TokenKind::Float(_)));
    assert!(matches!(&lex_kinds_no_nl("1.5e-3")[0], TokenKind::Float(_)));
}

#[test]
fn test_string_literals() {
    assert!(matches!(&lex_kinds_no_nl("'hello'")[0], TokenKind::Str(s) if s == "hello"));
    assert!(matches!(&lex_kinds_no_nl("\"world\"")[0], TokenKind::Str(s) if s == "world"));
    assert!(matches!(&lex_kinds_no_nl("\"\"\"triple\"\"\"")[0], TokenKind::Str(s) if s == "triple"));
    assert!(matches!(&lex_kinds_no_nl("'''also'''")[0], TokenKind::Str(s) if s == "also"));
}

#[test]
fn test_bytes_literal() {
    assert!(matches!(&lex_kinds_no_nl("b'abc'")[0], TokenKind::Bytes(_)));
}

#[test]
fn test_fstring() {
    assert!(matches!(&lex_kinds_no_nl("f'hello {name}'")[0], TokenKind::FString(_)));
}

#[test]
fn test_operators() {
    let src = "+ - * ** / // % @ & | ^ ~ << >> == != < > <= >= := -> ...";
    let kinds = lex_kinds_no_nl(src);
    assert!(kinds.contains(&TokenKind::DoubleStar));
    assert!(kinds.contains(&TokenKind::DoubleSlash));
    assert!(kinds.contains(&TokenKind::Walrus));
    assert!(kinds.contains(&TokenKind::Arrow));
    assert!(kinds.contains(&TokenKind::Ellipsis));
    assert!(kinds.contains(&TokenKind::EqEq));
    assert!(kinds.contains(&TokenKind::BangEq));
}

#[test]
fn test_augmented_assignment() {
    let src = "+= -= *= /= //= %= **= @= &= |= ^= <<= >>=";
    let kinds = lex_kinds_no_nl(src);
    assert!(kinds.contains(&TokenKind::PlusEq));
    assert!(kinds.contains(&TokenKind::MinusEq));
    assert!(kinds.contains(&TokenKind::DoubleSlashEq));
}

#[test]
fn test_indent_dedent() {
    let src = "if True:\n    x = 1\n    y = 2\nz = 3\n";
    let kinds = lex(src);
    assert!(kinds.contains(&TokenKind::Indent), "expected INDENT in {:?}", kinds);
    assert!(kinds.contains(&TokenKind::Dedent), "expected DEDENT in {:?}", kinds);
}

#[test]
fn test_comment_stripped() {
    let kinds = lex_kinds_no_nl("x = 1  # this is a comment");
    assert!(!kinds.iter().any(|k| matches!(k, TokenKind::Ident(s) if s.starts_with('#'))));
}

#[test]
fn test_line_continuation() {
    let src = "x = 1 + \\\n    2\n";
    // should produce tokens without error
    let kinds = lex(src);
    assert!(kinds.contains(&TokenKind::Int(1)));
    assert!(kinds.contains(&TokenKind::Int(2)));
}

#[test]
fn test_implicit_continuation_in_parens() {
    let src = "x = (1 +\n     2)\n";
    let kinds = lex(src);
    // newline inside parens should be suppressed
    assert!(!kinds.iter().filter(|k| matches!(k, TokenKind::Newline)).count() > 1);
}
