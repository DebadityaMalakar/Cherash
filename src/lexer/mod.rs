pub mod token;

use token::{FStringPart, Token, TokenKind};

#[derive(Debug)]
pub struct LexError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LexError at {}:{}: {}", self.line, self.col, self.msg)
    }
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    // indent stack: each entry is the number of spaces for that level
    indent_stack: Vec<usize>,
    // tracks bracket nesting depth to suppress NEWLINE/INDENT/DEDENT inside brackets
    paren_depth: usize,
    // pending tokens (dedents can produce multiple)
    pending: Vec<Token>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            indent_stack: vec![0],
            paren_depth: 0,
            pending: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            // drain pending queue first
            if !self.pending.is_empty() {
                tokens.extend(self.pending.drain(..));
            }
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if let Some(ch) = c {
            self.pos += 1;
            if ch == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        c
    }

    fn at_line_start(&self) -> bool {
        self.col == 1
    }

    fn skip_logical_line_continuation(&mut self) -> bool {
        // backslash at end of line: skip the \ and the \n
        if self.peek() == Some('\\') && self.peek2() == Some('\n') {
            self.advance();
            self.advance();
            return true;
        }
        false
    }

    fn handle_indent(&mut self, indent: usize, line: usize, col: usize) -> Vec<Token> {
        let mut toks = Vec::new();
        let top = *self.indent_stack.last().unwrap();
        if indent > top {
            self.indent_stack.push(indent);
            toks.push(Token::new(TokenKind::Indent, line, col));
        } else if indent < top {
            while *self.indent_stack.last().unwrap() > indent {
                self.indent_stack.pop();
                toks.push(Token::new(TokenKind::Dedent, line, col));
            }
        }
        toks
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        // if we're at the start of a logical line, measure indentation
        if self.paren_depth == 0 && self.at_line_start() {
            return self.lex_line_start();
        }
        self.lex_token()
    }

    fn lex_line_start(&mut self) -> Result<Token, LexError> {
        let line = self.line;
        let col = self.col;
        // count leading spaces/tabs
        let mut indent = 0usize;
        loop {
            match self.peek() {
                Some(' ') => {
                    indent += 1;
                    self.advance();
                }
                Some('\t') => {
                    // tabs expand to next multiple of 8 as per CPython
                    indent = (indent / 8 + 1) * 8;
                    self.advance();
                }
                Some('\\') if self.peek2() == Some('\n') => {
                    self.advance();
                    self.advance();
                    indent = 0; // continuation resets indent counting
                }
                _ => break,
            }
        }

        // blank line or comment-only line: skip it entirely (produce nothing)
        match self.peek() {
            Some('\n') => {
                self.advance();
                return self.next_token();
            }
            Some('\r') => {
                self.advance();
                if self.peek() == Some('\n') {
                    self.advance();
                }
                return self.next_token();
            }
            Some('#') => {
                // skip comment
                while self.peek().is_some() && self.peek() != Some('\n') {
                    self.advance();
                }
                if self.peek() == Some('\n') {
                    self.advance();
                }
                return self.next_token();
            }
            None => {
                // EOF: emit remaining DEDENTs
                let indent_toks = self.handle_indent(0, line, col);
                if !indent_toks.is_empty() {
                    let mut iter = indent_toks.into_iter();
                    let first = iter.next().unwrap();
                    self.pending.extend(iter);
                    self.pending.push(Token::new(TokenKind::Eof, line, col));
                    return Ok(first);
                }
                return Ok(Token::new(TokenKind::Eof, line, col));
            }
            _ => {}
        }

        // emit indent/dedent tokens if needed
        let indent_toks = self.handle_indent(indent, line, col);
        if !indent_toks.is_empty() {
            let mut iter = indent_toks.into_iter();
            let first = iter.next().unwrap();
            self.pending.extend(iter);
            // after indent/dedent tokens, lex the actual token next
            return Ok(first);
        }

        self.lex_token()
    }

    fn lex_token(&mut self) -> Result<Token, LexError> {
        // skip whitespace (but not newlines at top-level)
        loop {
            match self.peek() {
                Some(' ') | Some('\t') => {
                    self.advance();
                }
                Some('\\') => {
                    if self.skip_logical_line_continuation() {
                        continue;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        let line = self.line;
        let col = self.col;

        let c = match self.peek() {
            None => return Ok(Token::new(TokenKind::Eof, line, col)),
            Some(c) => c,
        };

        // newlines
        if c == '\n' || c == '\r' {
            self.advance();
            if c == '\r' && self.peek() == Some('\n') {
                self.advance();
            }
            if self.paren_depth == 0 {
                return Ok(Token::new(TokenKind::Newline, line, col));
            }
            // inside brackets, newlines are ignored
            return self.lex_token();
        }

        // comments
        if c == '#' {
            while self.peek().is_some() && self.peek() != Some('\n') {
                self.advance();
            }
            return self.lex_token();
        }

        // string / bytes / f-string prefixes
        if self.is_string_start() {
            return self.lex_string_or_bytes(line, col);
        }

        // numbers
        if c.is_ascii_digit() || (c == '.' && self.peek2().map_or(false, |d| d.is_ascii_digit())) {
            return self.lex_number(line, col);
        }

        // identifiers and keywords
        if c.is_alphabetic() || c == '_' {
            return self.lex_ident_or_keyword(line, col);
        }

        // operators and delimiters
        self.advance();
        let kind = match c {
            '(' => { self.paren_depth += 1; TokenKind::LParen }
            ')' => { self.paren_depth = self.paren_depth.saturating_sub(1); TokenKind::RParen }
            '[' => { self.paren_depth += 1; TokenKind::LBracket }
            ']' => { self.paren_depth = self.paren_depth.saturating_sub(1); TokenKind::RBracket }
            '{' => { self.paren_depth += 1; TokenKind::LBrace }
            '}' => { self.paren_depth = self.paren_depth.saturating_sub(1); TokenKind::RBrace }
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            '~' => TokenKind::Tilde,
            '+' => if self.peek() == Some('=') { self.advance(); TokenKind::PlusEq } else { TokenKind::Plus }
            '-' => match self.peek() {
                Some('=') => { self.advance(); TokenKind::MinusEq }
                Some('>') => { self.advance(); TokenKind::Arrow }
                _ => TokenKind::Minus,
            }
            '*' => match self.peek() {
                Some('*') => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); TokenKind::DoubleStarEq }
                    else { TokenKind::DoubleStar }
                }
                Some('=') => { self.advance(); TokenKind::StarEq }
                _ => TokenKind::Star,
            }
            '/' => match self.peek() {
                Some('/') => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); TokenKind::DoubleSlashEq }
                    else { TokenKind::DoubleSlash }
                }
                Some('=') => { self.advance(); TokenKind::SlashEq }
                _ => TokenKind::Slash,
            }
            '%' => if self.peek() == Some('=') { self.advance(); TokenKind::PercentEq } else { TokenKind::Percent }
            '@' => if self.peek() == Some('=') { self.advance(); TokenKind::AtEq } else { TokenKind::At }
            '&' => if self.peek() == Some('=') { self.advance(); TokenKind::AmpEq } else { TokenKind::Amp }
            '|' => if self.peek() == Some('=') { self.advance(); TokenKind::PipeEq } else { TokenKind::Pipe }
            '^' => if self.peek() == Some('=') { self.advance(); TokenKind::CaretEq } else { TokenKind::Caret }
            '<' => match self.peek() {
                Some('<') => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); TokenKind::LtLtEq }
                    else { TokenKind::LtLt }
                }
                Some('=') => { self.advance(); TokenKind::LtEq }
                _ => TokenKind::Lt,
            }
            '>' => match self.peek() {
                Some('>') => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); TokenKind::GtGtEq }
                    else { TokenKind::GtGt }
                }
                Some('=') => { self.advance(); TokenKind::GtEq }
                _ => TokenKind::Gt,
            }
            '=' => match self.peek() {
                Some('=') => { self.advance(); TokenKind::EqEq }
                _ => TokenKind::Eq,
            }
            '!' => match self.peek() {
                Some('=') => { self.advance(); TokenKind::BangEq }
                _ => return Err(LexError { msg: "unexpected '!'".into(), line, col })
            }
            ':' => if self.peek() == Some('=') { self.advance(); TokenKind::Walrus } else { TokenKind::Colon }
            '.' => {
                if self.peek() == Some('.') && self.peek2() == Some('.') {
                    self.advance(); self.advance();
                    TokenKind::Ellipsis
                } else {
                    TokenKind::Dot
                }
            }
            _ => return Err(LexError { msg: format!("unexpected character '{}'", c), line, col })
        };

        Ok(Token::new(kind, line, col))
    }

    fn is_string_start(&self) -> bool {
        let c = match self.peek() { Some(c) => c, None => return false };
        match c {
            '\'' | '"' => true,
            'r' | 'R' | 'b' | 'B' | 'f' | 'F' | 'u' | 'U' => {
                let next = self.peek2();
                match next {
                    Some('\'') | Some('"') => true,
                    Some('r') | Some('R') | Some('b') | Some('B') | Some('f') | Some('F') => {
                        self.chars.get(self.pos + 2).map_or(false, |&c| c == '\'' || c == '"')
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn lex_string_or_bytes(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let mut is_raw = false;
        let mut is_bytes = false;
        let mut is_fstring = false;

        // collect prefix characters
        loop {
            match self.peek() {
                Some('r') | Some('R') => { is_raw = true; self.advance(); }
                Some('b') | Some('B') => { is_bytes = true; self.advance(); }
                Some('f') | Some('F') => { is_fstring = true; self.advance(); }
                Some('u') | Some('U') => { self.advance(); } // unicode prefix, ignored
                _ => break,
            }
        }

        let quote = self.advance().unwrap();
        let triple = self.peek() == Some(quote) && self.peek2() == Some(quote);
        if triple {
            self.advance(); self.advance();
        }

        let mut content = String::new();
        loop {
            if triple {
                if self.peek() == Some(quote)
                    && self.peek2() == Some(quote)
                    && self.chars.get(self.pos + 2) == Some(&quote)
                {
                    self.advance(); self.advance(); self.advance();
                    break;
                }
            } else if self.peek() == Some(quote) {
                self.advance();
                break;
            }
            match self.peek() {
                None | Some('\n') if !triple => {
                    return Err(LexError { msg: "unterminated string literal".into(), line, col });
                }
                None => {
                    return Err(LexError { msg: "unterminated triple-quoted string".into(), line, col });
                }
                Some('\\') if !is_raw => {
                    self.advance();
                    let escaped = match self.advance() {
                        Some('n') => '\n',
                        Some('t') => '\t',
                        Some('r') => '\r',
                        Some('\\') => '\\',
                        Some('\'') => '\'',
                        Some('"') => '"',
                        Some('0') => '\0',
                        Some('a') => '\x07',
                        Some('b') => '\x08',
                        Some('f') => '\x0C',
                        Some('v') => '\x0B',
                        Some('\n') => continue, // line continuation inside string
                        Some(c) => { content.push('\\'); c }
                        None => break,
                    };
                    content.push(escaped);
                }
                Some(c) => {
                    content.push(c);
                    self.advance();
                }
            }
        }

        if is_bytes {
            return Ok(Token::new(TokenKind::Bytes(content.bytes().collect()), line, col));
        }
        if is_fstring {
            let parts = parse_fstring_parts(&content);
            return Ok(Token::new(TokenKind::FString(parts), line, col));
        }
        Ok(Token::new(TokenKind::Str(content), line, col))
    }

    fn lex_number(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let start = self.pos;
        let mut is_float = false;

        // hex, octal, binary literals
        if self.peek() == Some('0') {
            let next = self.peek2();
            match next {
                Some('x') | Some('X') => {
                    self.advance(); self.advance();
                    while self.peek().map_or(false, |c| c.is_ascii_hexdigit() || c == '_') {
                        self.advance();
                    }
                    let s: String = self.chars[start..self.pos].iter().filter(|&&c| c != '_').collect();
                    let val = i64::from_str_radix(&s[2..], 16)
                        .unwrap_or_else(|_| i64::MAX);
                    return Ok(Token::new(TokenKind::Int(val), line, col));
                }
                Some('o') | Some('O') => {
                    self.advance(); self.advance();
                    while self.peek().map_or(false, |c| c.is_ascii_digit() || c == '_') {
                        self.advance();
                    }
                    let s: String = self.chars[start..self.pos].iter().filter(|&&c| c != '_').collect();
                    let val = i64::from_str_radix(&s[2..], 8).unwrap_or(0);
                    return Ok(Token::new(TokenKind::Int(val), line, col));
                }
                Some('b') | Some('B') => {
                    self.advance(); self.advance();
                    while self.peek().map_or(false, |c| c == '0' || c == '1' || c == '_') {
                        self.advance();
                    }
                    let s: String = self.chars[start..self.pos].iter().filter(|&&c| c != '_').collect();
                    let val = i64::from_str_radix(&s[2..], 2).unwrap_or(0);
                    return Ok(Token::new(TokenKind::Int(val), line, col));
                }
                _ => {}
            }
        }

        while self.peek().map_or(false, |c| c.is_ascii_digit() || c == '_') {
            self.advance();
        }
        if self.peek() == Some('.') && self.peek2().map_or(false, |c| c.is_ascii_digit() || c == 'e' || c == 'E') {
            is_float = true;
            self.advance();
            while self.peek().map_or(false, |c| c.is_ascii_digit() || c == '_') {
                self.advance();
            }
        } else if self.peek() == Some('.') && !matches!(self.peek2(), Some('.')) {
            is_float = true;
            self.advance();
            while self.peek().map_or(false, |c| c.is_ascii_digit() || c == '_') {
                self.advance();
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            is_float = true;
            self.advance();
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.advance();
            }
            while self.peek().map_or(false, |c| c.is_ascii_digit() || c == '_') {
                self.advance();
            }
        }
        // skip complex suffix 'j' (treated as float for now)
        if matches!(self.peek(), Some('j') | Some('J')) {
            self.advance();
            is_float = true;
        }

        let s: String = self.chars[start..self.pos].iter().filter(|&&c| c != '_').collect();
        // strip trailing j for complex
        let s = s.trim_end_matches('j').trim_end_matches('J');

        if is_float {
            let val: f64 = s.parse().unwrap_or(0.0);
            Ok(Token::new(TokenKind::Float(val), line, col))
        } else {
            let val: i64 = s.parse().unwrap_or_else(|_| {
                // overflow wraps
                s.parse::<u64>().unwrap_or(0) as i64
            });
            Ok(Token::new(TokenKind::Int(val), line, col))
        }
    }

    fn lex_ident_or_keyword(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let start = self.pos;
        while self.peek().map_or(false, |c| c.is_alphanumeric() || c == '_') {
            self.advance();
        }
        let word: String = self.chars[start..self.pos].iter().collect();

        // check for string prefix before a quote
        if matches!(self.peek(), Some('\'') | Some('"')) ||
           (matches!(self.peek(), Some('r') | Some('R') | Some('b') | Some('B') | Some('f') | Some('F'))
            && matches!(self.peek2(), Some('\'') | Some('"')))
        {
            // rewind and re-lex as string (the word was a prefix like r, b, f, rb, etc.)
            if word.chars().all(|c| matches!(c, 'r'|'R'|'b'|'B'|'f'|'F'|'u'|'U')) {
                self.pos = start;
                // reset col accordingly
                self.col = col;
                return self.lex_string_or_bytes(line, col);
            }
        }

        let kind = keyword_or_ident(word);
        Ok(Token::new(kind, line, col))
    }
}

fn keyword_or_ident(word: String) -> TokenKind {
    match word.as_str() {
        "False"    => TokenKind::False,
        "None"     => TokenKind::None,
        "True"     => TokenKind::True,
        "and"      => TokenKind::And,
        "as"       => TokenKind::As,
        "assert"   => TokenKind::Assert,
        "async"    => TokenKind::Async,
        "await"    => TokenKind::Await,
        "break"    => TokenKind::Break,
        "class"    => TokenKind::Class,
        "continue" => TokenKind::Continue,
        "def"      => TokenKind::Def,
        "del"      => TokenKind::Del,
        "elif"     => TokenKind::Elif,
        "else"     => TokenKind::Else,
        "except"   => TokenKind::Except,
        "finally"  => TokenKind::Finally,
        "for"      => TokenKind::For,
        "from"     => TokenKind::From,
        "global"   => TokenKind::Global,
        "if"       => TokenKind::If,
        "import"   => TokenKind::Import,
        "in"       => TokenKind::In,
        "is"       => TokenKind::Is,
        "lambda"   => TokenKind::Lambda,
        "nonlocal" => TokenKind::Nonlocal,
        "not"      => TokenKind::Not,
        "or"       => TokenKind::Or,
        "pass"     => TokenKind::Pass,
        "raise"    => TokenKind::Raise,
        "return"   => TokenKind::Return,
        "try"      => TokenKind::Try,
        "while"    => TokenKind::While,
        "with"     => TokenKind::With,
        "yield"    => TokenKind::Yield,
        _ => TokenKind::Ident(word),
    }
}

/// Parse f-string content into literal + expression parts.
fn parse_fstring_parts(s: &str) -> Vec<FStringPart> {
    let mut parts = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut literal = String::new();
    while i < chars.len() {
        if chars[i] == '{' {
            if i + 1 < chars.len() && chars[i + 1] == '{' {
                literal.push('{');
                i += 2;
                continue;
            }
            if !literal.is_empty() {
                parts.push(FStringPart::Literal(std::mem::take(&mut literal)));
            }
            i += 1;
            let mut expr = String::new();
            let mut depth = 1;
            while i < chars.len() {
                match chars[i] {
                    '{' => { depth += 1; expr.push('{'); i += 1; }
                    '}' => {
                        depth -= 1;
                        if depth == 0 { i += 1; break; }
                        expr.push('}'); i += 1;
                    }
                    c => { expr.push(c); i += 1; }
                }
            }
            parts.push(FStringPart::Expr(expr));
        } else if chars[i] == '}' && i + 1 < chars.len() && chars[i + 1] == '}' {
            literal.push('}');
            i += 2;
        } else {
            literal.push(chars[i]);
            i += 1;
        }
    }
    if !literal.is_empty() {
        parts.push(FStringPart::Literal(literal));
    }
    parts
}
