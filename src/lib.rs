pub mod lexer;
pub mod parser;
pub mod typechecker;
pub mod interpreter;
pub mod runtime;
pub mod stdlib;

use interpreter::evaluator::Evaluator;
use lexer::Lexer;
use parser::parse;
use typechecker::{Mode, TypeChecker};

#[derive(Debug)]
pub enum CherashError {
    Lex(lexer::LexError),
    Parse(parser::ParseError),
    TypeCheck(Vec<typechecker::TypeCheckError>),
    Runtime(interpreter::evaluator::RuntimeError),
}

impl std::fmt::Display for CherashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CherashError::Lex(e)  => write!(f, "SyntaxError: {}", e),
            CherashError::Parse(e) => write!(f, "SyntaxError: {}", e),
            CherashError::TypeCheck(errs) => {
                for e in errs { writeln!(f, "{}", e)?; }
                Ok(())
            }
            CherashError::Runtime(e) => write!(f, "{}: {}", e.type_name, e.message),
        }
    }
}

pub fn run_source(source: &str, strict: bool) -> Result<(), CherashError> {
    runtime::gc::init_gc();
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().map_err(CherashError::Lex)?;

    let module = parse(tokens).map_err(CherashError::Parse)?;

    // Allow .py files to opt in with # cherash:strict header
    let is_strict = strict || source.trim_start().starts_with("# cherash:strict");
    let mode = if is_strict { Mode::Strict } else { Mode::Lenient };
    let mut checker = TypeChecker::new(mode);
    let type_errors = checker.check_module(&module);
    if !type_errors.is_empty() {
        return Err(CherashError::TypeCheck(type_errors));
    }

    let mut ev = Evaluator::new();
    ev.exec_module(&module).map_err(CherashError::Runtime)
}
