use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("run") => {
            let path = match args.get(2) {
                Some(p) => p,
                None => { eprintln!("Usage: cherash run <file>"); std::process::exit(1); }
            };
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading '{}': {}", path, e); std::process::exit(1);
            });
            let strict = path.ends_with(".chsh") || source.trim_start().starts_with("# cherash:strict");
            if let Err(e) = cherash::run_source(&source, strict) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some("check") => {
            let path = match args.get(2) {
                Some(p) => p,
                None => { eprintln!("Usage: cherash check <file>"); std::process::exit(1); }
            };
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error: {}", e); std::process::exit(1);
            });
            use cherash::lexer::Lexer;
            use cherash::parser::parse;
            use cherash::typechecker::{Mode, TypeChecker};
            let mut lexer = Lexer::new(&source);
            let tokens = lexer.tokenize().unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(1); });
            let module = parse(tokens).unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(1); });
            let mut checker = TypeChecker::new(Mode::Strict);
            let errors = checker.check_module(&module);
            if errors.is_empty() {
                println!("OK");
            } else {
                for e in &errors { eprintln!("{}", e); }
                std::process::exit(1);
            }
        }
        Some("repl") | None => {
            run_repl();
        }
        Some(cmd) => {
            eprintln!("Unknown command '{}'. Use: run <file> | check <file> | repl", cmd);
            std::process::exit(1);
        }
    }
}

fn run_repl() {
    use cherash::lexer::Lexer;
    use cherash::parser::Parser;
    use cherash::interpreter::evaluator::Evaluator;

    cherash::runtime::gc::init_gc();
    println!("Cherash 0.1.0 REPL — Ctrl+C to exit");
    let mut ev = Evaluator::new();

    loop {
        print!(">>> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        if trimmed == "exit" || trimmed == "quit" { break; }

        let source = line.clone();
        let mut lexer = Lexer::new(&source);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(e) => { eprintln!("{}", e); continue; }
        };
        let mut parser = Parser::new(tokens);
        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(e) => { eprintln!("{}", e); continue; }
        };
        match ev.exec_module(&module) {
            Ok(()) => {}
            Err(e) => eprintln!("{}: {}", e.type_name, e.message),
        }
    }
}
