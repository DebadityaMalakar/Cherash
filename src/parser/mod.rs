pub mod ast;

use crate::lexer::token::{Token, TokenKind};
use ast::*;

#[derive(Debug)]
pub struct ParseError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ParseError at {}:{}: {}", self.line, self.col, self.msg)
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // ── utilities ─────────────────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| self.tokens.last().unwrap())
    }

    fn peek2(&self) -> &Token {
        self.tokens.get(self.pos + 1).unwrap_or_else(|| self.tokens.last().unwrap())
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() { self.pos += 1; }
        t
    }

    fn span(&self) -> Span {
        Span::new(self.peek().line, self.peek().col)
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<&Token, ParseError> {
        if std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind) {
            Ok(self.advance())
        } else {
            Err(ParseError {
                msg: format!("expected {:?}, got {:?}", kind, self.peek().kind),
                line: self.peek().line,
                col: self.peek().col,
            })
        }
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek().kind, TokenKind::Newline) {
            self.advance();
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    // ── module entry ──────────────────────────────────────────────────────────

    pub fn parse_module(&mut self) -> Result<Module, ParseError> {
        self.skip_newlines();
        let mut body = Vec::new();
        while !self.at_eof() {
            self.skip_newlines();
            if self.at_eof() { break; }
            let stmt = self.parse_stmt()?;
            body.push(stmt);
        }
        Ok(Module { body })
    }

    // ── statements ────────────────────────────────────────────────────────────

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.span();
        match &self.peek().kind {
            TokenKind::If      => self.parse_if(),
            TokenKind::While   => self.parse_while(),
            TokenKind::For     => self.parse_for(false),
            TokenKind::Async   => self.parse_async(),
            TokenKind::With    => self.parse_with(false),
            TokenKind::Try     => self.parse_try(),
            TokenKind::Def     => self.parse_funcdef(vec![], false),
            TokenKind::Class   => self.parse_classdef(vec![]),
            TokenKind::At      => self.parse_decorated(),
            TokenKind::Return  => {
                self.advance();
                let value = if !matches!(self.peek().kind, TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof) {
                    Some(self.parse_expr()?)
                } else { None };
                self.eat_stmt_end();
                Ok(Stmt::Return { value, span })
            }
            TokenKind::Raise => {
                self.advance();
                let exc = if !matches!(self.peek().kind, TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof) {
                    Some(self.parse_expr()?)
                } else { None };
                let cause = if self.eat(&TokenKind::From) {
                    Some(self.parse_expr()?)
                } else { None };
                self.eat_stmt_end();
                Ok(Stmt::Raise { exc, cause, span })
            }
            TokenKind::Del => {
                self.advance();
                let mut targets = vec![self.parse_expr()?];
                while self.eat(&TokenKind::Comma) {
                    if matches!(self.peek().kind, TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof) { break; }
                    targets.push(self.parse_expr()?);
                }
                self.eat_stmt_end();
                Ok(Stmt::Delete { targets, span })
            }
            TokenKind::Pass => { self.advance(); self.eat_stmt_end(); Ok(Stmt::Pass(span)) }
            TokenKind::Break => { self.advance(); self.eat_stmt_end(); Ok(Stmt::Break(span)) }
            TokenKind::Continue => { self.advance(); self.eat_stmt_end(); Ok(Stmt::Continue(span)) }
            TokenKind::Global => {
                self.advance();
                let names = self.parse_name_list()?;
                self.eat_stmt_end();
                Ok(Stmt::Global { names, span })
            }
            TokenKind::Nonlocal => {
                self.advance();
                let names = self.parse_name_list()?;
                self.eat_stmt_end();
                Ok(Stmt::Nonlocal { names, span })
            }
            TokenKind::Assert => {
                self.advance();
                let test = self.parse_expr()?;
                let msg = if self.eat(&TokenKind::Comma) { Some(self.parse_expr()?) } else { None };
                self.eat_stmt_end();
                Ok(Stmt::Assert { test, msg, span })
            }
            TokenKind::Import => {
                self.advance();
                let names = self.parse_aliases()?;
                self.eat_stmt_end();
                Ok(Stmt::Import { names, span })
            }
            TokenKind::From => self.parse_import_from(),
            _ => self.parse_expr_or_assign(),
        }
    }

    fn eat_stmt_end(&mut self) {
        while self.eat(&TokenKind::Semicolon) {}
        self.eat(&TokenKind::Newline);
    }

    fn parse_name_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut names = vec![self.expect_ident()?];
        while self.eat(&TokenKind::Comma) {
            names.push(self.expect_ident()?);
        }
        Ok(names)
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek().kind.clone() {
            TokenKind::Ident(s) => { self.advance(); Ok(s) }
            _ => Err(ParseError {
                msg: format!("expected identifier, got {:?}", self.peek().kind),
                line: self.peek().line, col: self.peek().col,
            })
        }
    }

    // ── if statement ──────────────────────────────────────────────────────────

    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        let span = self.span();
        self.advance(); // eat `if`
        let test = self.parse_expr()?;
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_block()?;
        let orelse = self.parse_elif_or_else()?;
        Ok(Stmt::If { test, body, orelse, span })
    }

    fn parse_elif_or_else(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.skip_newlines();
        match &self.peek().kind {
            TokenKind::Elif => {
                let span = self.span();
                self.advance();
                let test = self.parse_expr()?;
                self.expect(&TokenKind::Colon)?;
                let body = self.parse_block()?;
                let orelse = self.parse_elif_or_else()?;
                Ok(vec![Stmt::If { test, body, orelse, span }])
            }
            TokenKind::Else => {
                self.advance();
                self.expect(&TokenKind::Colon)?;
                self.parse_block()
            }
            _ => Ok(vec![]),
        }
    }

    // ── while ─────────────────────────────────────────────────────────────────

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        let span = self.span();
        self.advance();
        let test = self.parse_expr()?;
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_block()?;
        let orelse = if self.eat_keyword_else() {
            self.expect(&TokenKind::Colon)?;
            self.parse_block()?
        } else { vec![] };
        Ok(Stmt::While { test, body, orelse, span })
    }

    fn eat_keyword_else(&mut self) -> bool {
        self.skip_newlines();
        if matches!(self.peek().kind, TokenKind::Else) {
            self.advance(); true
        } else { false }
    }

    // ── for ───────────────────────────────────────────────────────────────────

    fn parse_for(&mut self, is_async: bool) -> Result<Stmt, ParseError> {
        let span = self.span();
        self.advance(); // eat `for`
        let target = self.parse_target()?;
        self.expect(&TokenKind::In)?;
        let iter = self.parse_expr()?;
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_block()?;
        let orelse = if self.eat_keyword_else() {
            self.expect(&TokenKind::Colon)?;
            self.parse_block()?
        } else { vec![] };
        Ok(Stmt::For { target, iter, body, orelse, is_async, span })
    }

    fn parse_target(&mut self) -> Result<Expr, ParseError> {
        // simplified: parse expr tuple (a, b) or single
        let first = self.parse_primary_or_star()?;
        if self.eat(&TokenKind::Comma) {
            let span = first.span().clone();
            let mut elts = vec![first];
            loop {
                if matches!(self.peek().kind, TokenKind::In | TokenKind::Colon | TokenKind::Newline) { break; }
                elts.push(self.parse_primary_or_star()?);
                if !self.eat(&TokenKind::Comma) { break; }
            }
            Ok(Expr::Tuple(elts, span))
        } else {
            Ok(first)
        }
    }

    fn parse_primary_or_star(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&TokenKind::Star) {
            let span = self.span();
            let e = self.parse_primary()?;
            return Ok(Expr::Starred(Box::new(e), span));
        }
        self.parse_primary()
    }

    // ── async ─────────────────────────────────────────────────────────────────

    fn parse_async(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // eat `async`
        match &self.peek().kind {
            TokenKind::Def => self.parse_funcdef(vec![], true),
            TokenKind::For => self.parse_for(true),
            TokenKind::With => self.parse_with(true),
            _ => Err(ParseError {
                msg: "expected def/for/with after async".into(),
                line: self.peek().line, col: self.peek().col,
            })
        }
    }

    // ── with ──────────────────────────────────────────────────────────────────

    fn parse_with(&mut self, is_async: bool) -> Result<Stmt, ParseError> {
        let span = self.span();
        self.advance();
        let mut items = vec![self.parse_with_item()?];
        while self.eat(&TokenKind::Comma) {
            items.push(self.parse_with_item()?);
        }
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(Stmt::With { items, body, is_async, span })
    }

    fn parse_with_item(&mut self) -> Result<WithItem, ParseError> {
        let context_expr = self.parse_expr()?;
        let optional_vars = if self.eat(&TokenKind::As) {
            Some(self.parse_expr()?)
        } else { None };
        Ok(WithItem { context_expr, optional_vars })
    }

    // ── try ───────────────────────────────────────────────────────────────────

    fn parse_try(&mut self) -> Result<Stmt, ParseError> {
        let span = self.span();
        self.advance();
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_block()?;
        let mut handlers = Vec::new();
        loop {
            self.skip_newlines();
            if !matches!(self.peek().kind, TokenKind::Except) { break; }
            handlers.push(self.parse_except_handler()?);
        }
        let orelse = if self.eat_keyword_else() {
            self.expect(&TokenKind::Colon)?;
            self.parse_block()?
        } else { vec![] };
        let finalbody = if self.eat_keyword(TokenKind::Finally) {
            self.expect(&TokenKind::Colon)?;
            self.parse_block()?
        } else { vec![] };
        Ok(Stmt::Try { body, handlers, orelse, finalbody, span })
    }

    fn eat_keyword(&mut self, k: TokenKind) -> bool {
        self.skip_newlines();
        if std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(&k) {
            self.advance(); true
        } else { false }
    }

    fn parse_except_handler(&mut self) -> Result<ExceptHandler, ParseError> {
        let span = self.span();
        self.advance(); // eat `except`
        let typ = if !matches!(self.peek().kind, TokenKind::Colon) {
            Some(self.parse_expr()?)
        } else { None };
        let name = if self.eat(&TokenKind::As) {
            Some(self.expect_ident()?)
        } else { None };
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(ExceptHandler { typ, name, body, span })
    }

    // ── function def ──────────────────────────────────────────────────────────

    fn parse_funcdef(&mut self, decorators: Vec<Expr>, is_async: bool) -> Result<Stmt, ParseError> {
        let span = self.span();
        self.advance(); // eat `def`
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;
        let return_annotation = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type_annotation()?)
        } else { None };
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(Stmt::FunctionDef { name, params, return_annotation, body, decorators, is_async, span })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        let mut seen_default = false;
        let mut in_keyword_only = false;

        while !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
            if self.eat(&TokenKind::DoubleStar) {
                let name = self.expect_ident()?;
                let ann = if self.eat(&TokenKind::Colon) { Some(self.parse_type_annotation()?) } else { None };
                params.push(Param { name, annotation: ann, default: None, kind: ParamKind::DoubleStarArgs });
                break;
            }
            if self.eat(&TokenKind::Star) {
                if matches!(self.peek().kind, TokenKind::Comma | TokenKind::RParen) {
                    in_keyword_only = true;
                    self.eat(&TokenKind::Comma);
                    continue;
                }
                let name = self.expect_ident()?;
                let ann = if self.eat(&TokenKind::Colon) { Some(self.parse_type_annotation()?) } else { None };
                params.push(Param { name, annotation: ann, default: None, kind: ParamKind::VarArgs });
                in_keyword_only = true;
                if !self.eat(&TokenKind::Comma) { break; }
                continue;
            }
            // positional-only separator /
            if matches!(self.peek().kind, TokenKind::Slash) {
                self.advance();
                for p in params.iter_mut() {
                    if p.kind == ParamKind::Regular { p.kind = ParamKind::PosOnly; }
                }
                if !self.eat(&TokenKind::Comma) { break; }
                continue;
            }

            let name = self.expect_ident()?;
            let ann = if self.eat(&TokenKind::Colon) { Some(self.parse_type_annotation()?) } else { None };
            let default = if self.eat(&TokenKind::Eq) {
                seen_default = true;
                Some(self.parse_expr()?)
            } else {
                if seen_default && !in_keyword_only {
                    return Err(ParseError {
                        msg: "non-default argument follows default argument".into(),
                        line: self.peek().line, col: self.peek().col,
                    });
                }
                None
            };
            let kind = if in_keyword_only { ParamKind::KeywordOnly } else { ParamKind::Regular };
            params.push(Param { name, annotation: ann, default, kind });
            if !self.eat(&TokenKind::Comma) { break; }
        }
        Ok(params)
    }

    fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ParseError> {
        // simplified: name or name[T, ...] or (T, ...) for union
        match self.peek().kind.clone() {
            TokenKind::None => { self.advance(); return Ok(TypeAnnotation::None); }
            TokenKind::LParen => {
                self.advance();
                let mut types = Vec::new();
                while !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
                    types.push(self.parse_type_annotation()?);
                    if !self.eat(&TokenKind::Comma) { break; }
                }
                self.expect(&TokenKind::RParen)?;
                return Ok(TypeAnnotation::Tuple(types));
            }
            TokenKind::Ident(name) => {
                self.advance();
                if self.eat(&TokenKind::LBracket) {
                    let mut params = Vec::new();
                    while !matches!(self.peek().kind, TokenKind::RBracket | TokenKind::Eof) {
                        params.push(self.parse_type_annotation()?);
                        if !self.eat(&TokenKind::Comma) { break; }
                    }
                    self.expect(&TokenKind::RBracket)?;
                    return Ok(TypeAnnotation::Subscript { name, params });
                }
                return Ok(TypeAnnotation::Name(name));
            }
            _ => {
                // fall back: parse as expression and stringify
                let e = self.parse_expr()?;
                let s = format!("{:?}", e);
                return Ok(TypeAnnotation::Name(s));
            }
        }
    }

    // ── class def ─────────────────────────────────────────────────────────────

    fn parse_classdef(&mut self, decorators: Vec<Expr>) -> Result<Stmt, ParseError> {
        let span = self.span();
        self.advance(); // eat `class`
        let name = self.expect_ident()?;
        let mut bases = Vec::new();
        let mut keywords = Vec::new();
        if self.eat(&TokenKind::LParen) {
            while !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
                if let TokenKind::Ident(kw) = self.peek().kind.clone() {
                    if matches!(self.peek2().kind, TokenKind::Eq) {
                        self.advance(); self.advance();
                        let val = self.parse_expr()?;
                        keywords.push((kw, val));
                        if !self.eat(&TokenKind::Comma) { break; }
                        continue;
                    }
                }
                bases.push(self.parse_expr()?);
                if !self.eat(&TokenKind::Comma) { break; }
            }
            self.expect(&TokenKind::RParen)?;
        }
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(Stmt::ClassDef { name, bases, keywords, body, decorators, span })
    }

    // ── decorators ────────────────────────────────────────────────────────────

    fn parse_decorated(&mut self) -> Result<Stmt, ParseError> {
        let mut decorators = Vec::new();
        while self.eat(&TokenKind::At) {
            let expr = self.parse_expr()?;
            decorators.push(expr);
            self.eat(&TokenKind::Newline);
        }
        match &self.peek().kind {
            TokenKind::Def => self.parse_funcdef(decorators, false),
            TokenKind::Async => {
                self.advance();
                self.parse_funcdef(decorators, true)
            }
            TokenKind::Class => self.parse_classdef(decorators),
            _ => Err(ParseError {
                msg: "expected def or class after decorator".into(),
                line: self.peek().line, col: self.peek().col,
            })
        }
    }

    // ── import from ───────────────────────────────────────────────────────────

    fn parse_import_from(&mut self) -> Result<Stmt, ParseError> {
        let span = self.span();
        self.advance(); // eat `from`
        let mut level = 0;
        while self.eat(&TokenKind::Dot) { level += 1; }
        if matches!(self.peek().kind, TokenKind::Ellipsis) { self.advance(); level += 3; }
        let module = if !matches!(self.peek().kind, TokenKind::Import) {
            Some(self.parse_dotted_name()?)
        } else { None };
        self.expect(&TokenKind::Import)?;
        let names = if self.eat(&TokenKind::Star) {
            vec![Alias { name: "*".into(), asname: None }]
        } else if self.eat(&TokenKind::LParen) {
            let aliases = self.parse_aliases()?;
            self.expect(&TokenKind::RParen)?;
            aliases
        } else {
            self.parse_aliases()?
        };
        self.eat_stmt_end();
        Ok(Stmt::ImportFrom { module, names, level, span })
    }

    fn parse_dotted_name(&mut self) -> Result<String, ParseError> {
        let mut name = self.expect_ident()?;
        while self.eat(&TokenKind::Dot) {
            name.push('.');
            name.push_str(&self.expect_ident()?);
        }
        Ok(name)
    }

    fn parse_aliases(&mut self) -> Result<Vec<Alias>, ParseError> {
        let mut aliases = vec![self.parse_alias()?];
        while self.eat(&TokenKind::Comma) {
            if matches!(self.peek().kind, TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof) { break; }
            aliases.push(self.parse_alias()?);
        }
        Ok(aliases)
    }

    fn parse_alias(&mut self) -> Result<Alias, ParseError> {
        let name = self.parse_dotted_name()?;
        let asname = if self.eat(&TokenKind::As) {
            Some(self.expect_ident()?)
        } else { None };
        Ok(Alias { name, asname })
    }

    // ── block (indented suite) ────────────────────────────────────────────────

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        // inline block: if x: pass (no newline+indent)
        if !matches!(self.peek().kind, TokenKind::Newline) {
            let stmt = self.parse_simple_stmt()?;
            return Ok(stmt);
        }
        self.expect(&TokenKind::Newline)?;
        self.skip_newlines();
        self.expect(&TokenKind::Indent)?;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek().kind, TokenKind::Dedent | TokenKind::Eof) { break; }
            stmts.push(self.parse_stmt()?);
        }
        self.eat(&TokenKind::Dedent);
        Ok(stmts)
    }

    fn parse_simple_stmt(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = vec![self.parse_stmt()?];
        while self.eat(&TokenKind::Semicolon) {
            if matches!(self.peek().kind, TokenKind::Newline | TokenKind::Eof) { break; }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    // ── expression-or-assignment statement ────────────────────────────────────

    fn parse_expr_or_assign(&mut self) -> Result<Stmt, ParseError> {
        let span = self.span();
        let expr = self.parse_expr_list()?;

        // augmented assignment: target op= value
        if let Some(op) = self.peek_augmented_op() {
            self.advance();
            let value = self.parse_expr()?;
            self.eat_stmt_end();
            return Ok(Stmt::AugAssign { target: expr, op, value, span });
        }

        // annotated assignment: name: Type = value
        if self.eat(&TokenKind::Colon) {
            let annotation = self.parse_type_annotation()?;
            let value = if self.eat(&TokenKind::Eq) { Some(self.parse_expr()?) } else { None };
            self.eat_stmt_end();
            return Ok(Stmt::AnnAssign { target: expr, annotation, value, span });
        }

        // plain assignment: a = b = expr
        if self.eat(&TokenKind::Eq) {
            let mut targets = vec![expr];
            let mut value = self.parse_expr_list()?;
            while self.eat(&TokenKind::Eq) {
                targets.push(value);
                value = self.parse_expr_list()?;
            }
            self.eat_stmt_end();
            return Ok(Stmt::Assign { targets, value, span });
        }

        self.eat_stmt_end();
        Ok(Stmt::Expr(expr))
    }

    fn peek_augmented_op(&self) -> Option<BinOp> {
        match &self.peek().kind {
            TokenKind::PlusEq          => Some(BinOp::Add),
            TokenKind::MinusEq         => Some(BinOp::Sub),
            TokenKind::StarEq          => Some(BinOp::Mul),
            TokenKind::SlashEq         => Some(BinOp::Div),
            TokenKind::DoubleSlashEq   => Some(BinOp::FloorDiv),
            TokenKind::PercentEq       => Some(BinOp::Mod),
            TokenKind::DoubleStarEq    => Some(BinOp::Pow),
            TokenKind::AtEq            => Some(BinOp::MatMul),
            TokenKind::AmpEq           => Some(BinOp::BitAnd),
            TokenKind::PipeEq          => Some(BinOp::BitOr),
            TokenKind::CaretEq         => Some(BinOp::BitXor),
            TokenKind::LtLtEq          => Some(BinOp::LShift),
            TokenKind::GtGtEq          => Some(BinOp::RShift),
            _ => None,
        }
    }

    // ── expression parsing (Pratt / precedence climbing) ─────────────────────

    fn parse_expr_list(&mut self) -> Result<Expr, ParseError> {
        let span = self.span();
        let first = self.parse_expr()?;
        if self.eat(&TokenKind::Comma) {
            let mut elts = vec![first];
            while !matches!(self.peek().kind,
                TokenKind::Newline | TokenKind::Eof | TokenKind::Colon
                | TokenKind::Semicolon | TokenKind::Eq)
            {
                elts.push(self.parse_expr()?);
                if !self.eat(&TokenKind::Comma) { break; }
            }
            return Ok(Expr::Tuple(elts, span));
        }
        Ok(first)
    }

    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        // yield expressions
        if self.eat(&TokenKind::Yield) {
            let span = self.span();
            if self.eat(&TokenKind::From) {
                let e = self.parse_expr()?;
                return Ok(Expr::YieldFrom(Box::new(e), span));
            }
            let val = if !matches!(self.peek().kind,
                TokenKind::Comma | TokenKind::Newline | TokenKind::RParen
                | TokenKind::RBracket | TokenKind::RBrace | TokenKind::Semicolon | TokenKind::Eof)
            { Some(Box::new(self.parse_expr()?)) } else { None };
            return Ok(Expr::Yield(val, span));
        }
        // lambda
        if self.eat(&TokenKind::Lambda) {
            return self.parse_lambda();
        }
        self.parse_if_expr()
    }

    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        let span = self.span();
        let params = if !matches!(self.peek().kind, TokenKind::Colon) {
            self.parse_params()?
        } else { vec![] };
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_expr()?;
        Ok(Expr::Lambda { params, body: Box::new(body), span })
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        let body = self.parse_or()?;
        if self.eat(&TokenKind::If) {
            let span = body.span().clone();
            let test = self.parse_or()?;
            self.expect(&TokenKind::Else)?;
            let orelse = self.parse_if_expr()?;
            return Ok(Expr::IfExpr { test: Box::new(test), body: Box::new(body), orelse: Box::new(orelse), span });
        }
        Ok(body)
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek().kind, TokenKind::Or) {
            let span = left.span().clone();
            self.advance();
            let right = self.parse_and()?;
            left = match left {
                Expr::BoolOp { op: BoolOpKind::Or, mut values, span: s } => {
                    values.push(right);
                    Expr::BoolOp { op: BoolOpKind::Or, values, span: s }
                }
                _ => Expr::BoolOp { op: BoolOpKind::Or, values: vec![left, right], span }
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not()?;
        while matches!(self.peek().kind, TokenKind::And) {
            let span = left.span().clone();
            self.advance();
            let right = self.parse_not()?;
            left = match left {
                Expr::BoolOp { op: BoolOpKind::And, mut values, span: s } => {
                    values.push(right);
                    Expr::BoolOp { op: BoolOpKind::And, values, span: s }
                }
                _ => Expr::BoolOp { op: BoolOpKind::And, values: vec![left, right], span }
            };
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&TokenKind::Not) {
            let span = self.span();
            let expr = self.parse_not()?;
            return Ok(Expr::Unary { op: UnaryOp::Not, expr: Box::new(expr), span });
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_bitwise_or()?;
        let mut ops = Vec::new();
        let mut comparators = Vec::new();
        loop {
            let op = match &self.peek().kind {
                TokenKind::EqEq  => CmpOp::Eq,
                TokenKind::BangEq => CmpOp::NotEq,
                TokenKind::Lt    => CmpOp::Lt,
                TokenKind::LtEq  => CmpOp::LtEq,
                TokenKind::Gt    => CmpOp::Gt,
                TokenKind::GtEq  => CmpOp::GtEq,
                TokenKind::Is => {
                    self.advance();
                    if self.eat(&TokenKind::Not) { ops.push(CmpOp::IsNot); } else { ops.push(CmpOp::Is); }
                    comparators.push(self.parse_bitwise_or()?);
                    continue;
                }
                TokenKind::In    => CmpOp::In,
                TokenKind::Not => {
                    if matches!(self.peek2().kind, TokenKind::In) {
                        self.advance(); self.advance();
                        ops.push(CmpOp::NotIn);
                        comparators.push(self.parse_bitwise_or()?);
                        continue;
                    }
                    break;
                }
                _ => break,
            };
            self.advance();
            ops.push(op);
            comparators.push(self.parse_bitwise_or()?);
        }
        if ops.is_empty() { return Ok(left); }
        let span = left.span().clone();
        Ok(Expr::Compare { left: Box::new(left), ops, comparators, span })
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bitwise_xor()?;
        while matches!(self.peek().kind, TokenKind::Pipe) {
            let span = left.span().clone();
            self.advance();
            let right = self.parse_bitwise_xor()?;
            left = Expr::Binary { op: BinOp::BitOr, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bitwise_and()?;
        while matches!(self.peek().kind, TokenKind::Caret) {
            let span = left.span().clone();
            self.advance();
            let right = self.parse_bitwise_and()?;
            left = Expr::Binary { op: BinOp::BitXor, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_shift()?;
        while matches!(self.peek().kind, TokenKind::Amp) {
            let span = left.span().clone();
            self.advance();
            let right = self.parse_shift()?;
            left = Expr::Binary { op: BinOp::BitAnd, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_add()?;
        loop {
            let op = match &self.peek().kind {
                TokenKind::LtLt => BinOp::LShift,
                TokenKind::GtGt => BinOp::RShift,
                _ => break,
            };
            let span = left.span().clone();
            self.advance();
            let right = self.parse_add()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match &self.peek().kind {
                TokenKind::Plus  => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            let span = left.span().clone();
            self.advance();
            let right = self.parse_mul()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match &self.peek().kind {
                TokenKind::Star        => BinOp::Mul,
                TokenKind::Slash       => BinOp::Div,
                TokenKind::DoubleSlash => BinOp::FloorDiv,
                TokenKind::Percent     => BinOp::Mod,
                TokenKind::At          => BinOp::MatMul,
                _ => break,
            };
            let span = left.span().clone();
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let span = self.span();
        match &self.peek().kind {
            TokenKind::Plus  => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Unary { op: UnaryOp::Plus,   expr: Box::new(e), span }) }
            TokenKind::Minus => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Unary { op: UnaryOp::Minus,  expr: Box::new(e), span }) }
            TokenKind::Tilde => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Unary { op: UnaryOp::BitNot, expr: Box::new(e), span }) }
            TokenKind::Await => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Await(Box::new(e), span)) }
            _ => self.parse_power(),
        }
    }

    fn parse_power(&mut self) -> Result<Expr, ParseError> {
        let base = self.parse_postfix()?;
        if self.eat(&TokenKind::DoubleStar) {
            let span = base.span().clone();
            let exp = self.parse_unary()?;
            return Ok(Expr::Binary { op: BinOp::Pow, left: Box::new(base), right: Box::new(exp), span });
        }
        Ok(base)
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            match &self.peek().kind {
                TokenKind::Dot => {
                    let span = expr.span().clone();
                    self.advance();
                    let attr = self.expect_ident()?;
                    expr = Expr::Attr { obj: Box::new(expr), attr, span };
                }
                TokenKind::LParen => {
                    let span = expr.span().clone();
                    self.advance();
                    let args = self.parse_call_args()?;
                    self.expect(&TokenKind::RParen)?;
                    expr = Expr::Call { func: Box::new(expr), args, span };
                }
                TokenKind::LBracket => {
                    let span = expr.span().clone();
                    self.advance();
                    let index = self.parse_subscript()?;
                    self.expect(&TokenKind::RBracket)?;
                    expr = index.resolve(expr, span);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self) -> Result<Vec<Arg>, ParseError> {
        let mut args = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
            if self.eat(&TokenKind::DoubleStar) {
                args.push(Arg::DoubleStarArgs(self.parse_expr()?));
            } else if self.eat(&TokenKind::Star) {
                args.push(Arg::StarArgs(self.parse_expr()?));
            } else {
                let e = self.parse_expr()?;
                if self.eat(&TokenKind::Eq) {
                    if let Expr::Ident(name, _) = e {
                        args.push(Arg::Keyword { name, value: self.parse_expr()? });
                    } else {
                        return Err(ParseError {
                            msg: "keyword argument must be an identifier".into(),
                            line: self.peek().line, col: self.peek().col,
                        });
                    }
                } else {
                    args.push(Arg::Pos(e));
                }
            }
            if !self.eat(&TokenKind::Comma) { break; }
        }
        Ok(args)
    }

    fn parse_subscript(&mut self) -> Result<SubscriptHelper, ParseError> {
        // could be a slice or an index
        let lower = if !matches!(self.peek().kind, TokenKind::Colon | TokenKind::RBracket) {
            Some(self.parse_expr()?)
        } else { None };

        if self.eat(&TokenKind::Colon) {
            let upper = if !matches!(self.peek().kind, TokenKind::Colon | TokenKind::RBracket) {
                Some(self.parse_expr()?)
            } else { None };
            let step = if self.eat(&TokenKind::Colon) {
                if !matches!(self.peek().kind, TokenKind::RBracket) {
                    Some(self.parse_expr()?)
                } else { None }
            } else { None };
            return Ok(SubscriptHelper::Slice { lower, upper, step });
        }
        Ok(SubscriptHelper::Index(lower.unwrap()))
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let span = self.span();
        match self.peek().kind.clone() {
            TokenKind::Int(n)    => { self.advance(); Ok(Expr::IntLit(n, span)) }
            TokenKind::Float(f)  => { self.advance(); Ok(Expr::FloatLit(f, span)) }
            TokenKind::Str(s)    => { self.advance(); Ok(Expr::StrLit(s, span)) }
            TokenKind::Bytes(b)  => { self.advance(); Ok(Expr::BytesLit(b, span)) }
            TokenKind::FString(parts) => { self.advance(); Ok(Expr::FStringLit(parts, span)) }
            TokenKind::True      => { self.advance(); Ok(Expr::BoolLit(true,  span)) }
            TokenKind::False     => { self.advance(); Ok(Expr::BoolLit(false, span)) }
            TokenKind::None      => { self.advance(); Ok(Expr::NoneLit(span)) }
            TokenKind::Ellipsis  => { self.advance(); Ok(Expr::EllipsisLit(span)) }

            TokenKind::Ident(name) => {
                self.advance();
                // walrus := inside expression
                if self.eat(&TokenKind::Walrus) {
                    let value = self.parse_expr()?;
                    return Ok(Expr::NamedExpr { target: name, value: Box::new(value), span });
                }
                Ok(Expr::Ident(name, span))
            }

            TokenKind::Star => {
                self.advance();
                let e = self.parse_primary()?;
                Ok(Expr::Starred(Box::new(e), span))
            }

            TokenKind::LParen => {
                self.advance();
                // empty tuple
                if self.eat(&TokenKind::RParen) {
                    return Ok(Expr::Tuple(vec![], span));
                }
                let first = self.parse_expr()?;
                // generator expression
                if matches!(self.peek().kind, TokenKind::For) {
                    let gens = self.parse_comprehension_clauses()?;
                    self.expect(&TokenKind::RParen)?;
                    return Ok(Expr::GeneratorExp { elt: Box::new(first), generators: gens, span });
                }
                // tuple
                if self.eat(&TokenKind::Comma) {
                    let mut elts = vec![first];
                    while !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
                        elts.push(self.parse_expr()?);
                        if !self.eat(&TokenKind::Comma) { break; }
                    }
                    self.expect(&TokenKind::RParen)?;
                    return Ok(Expr::Tuple(elts, span));
                }
                self.expect(&TokenKind::RParen)?;
                Ok(first) // just parenthesised expression
            }

            TokenKind::LBracket => {
                self.advance();
                if self.eat(&TokenKind::RBracket) {
                    return Ok(Expr::List(vec![], span));
                }
                let first = self.parse_expr()?;
                // list comprehension
                if matches!(self.peek().kind, TokenKind::For) {
                    let gens = self.parse_comprehension_clauses()?;
                    self.expect(&TokenKind::RBracket)?;
                    return Ok(Expr::ListComp { elt: Box::new(first), generators: gens, span });
                }
                let mut elts = vec![first];
                while self.eat(&TokenKind::Comma) {
                    if matches!(self.peek().kind, TokenKind::RBracket) { break; }
                    elts.push(self.parse_expr()?);
                }
                self.expect(&TokenKind::RBracket)?;
                Ok(Expr::List(elts, span))
            }

            TokenKind::LBrace => {
                self.advance();
                if self.eat(&TokenKind::RBrace) {
                    return Ok(Expr::Dict { keys: vec![], values: vec![], span });
                }
                // dict or set or comprehension
                if self.eat(&TokenKind::DoubleStar) {
                    // dict unpacking
                    let v = self.parse_expr()?;
                    let mut keys = vec![None];
                    let mut values = vec![v];
                    while self.eat(&TokenKind::Comma) {
                        if matches!(self.peek().kind, TokenKind::RBrace) { break; }
                        if self.eat(&TokenKind::DoubleStar) {
                            keys.push(None);
                            values.push(self.parse_expr()?);
                        } else {
                            let k = self.parse_expr()?;
                            self.expect(&TokenKind::Colon)?;
                            let v = self.parse_expr()?;
                            keys.push(Some(k));
                            values.push(v);
                        }
                    }
                    self.expect(&TokenKind::RBrace)?;
                    return Ok(Expr::Dict { keys, values, span });
                }
                let first = self.parse_expr()?;
                if self.eat(&TokenKind::Colon) {
                    // dict literal or dict comprehension
                    let first_val = self.parse_expr()?;
                    if matches!(self.peek().kind, TokenKind::For) {
                        let gens = self.parse_comprehension_clauses()?;
                        self.expect(&TokenKind::RBrace)?;
                        return Ok(Expr::DictComp { key: Box::new(first), value: Box::new(first_val), generators: gens, span });
                    }
                    let mut keys = vec![Some(first)];
                    let mut values = vec![first_val];
                    while self.eat(&TokenKind::Comma) {
                        if matches!(self.peek().kind, TokenKind::RBrace) { break; }
                        if self.eat(&TokenKind::DoubleStar) {
                            keys.push(None);
                            values.push(self.parse_expr()?);
                        } else {
                            let k = self.parse_expr()?;
                            self.expect(&TokenKind::Colon)?;
                            let v = self.parse_expr()?;
                            keys.push(Some(k));
                            values.push(v);
                        }
                    }
                    self.expect(&TokenKind::RBrace)?;
                    return Ok(Expr::Dict { keys, values, span });
                }
                // set or set comprehension
                if matches!(self.peek().kind, TokenKind::For) {
                    let gens = self.parse_comprehension_clauses()?;
                    self.expect(&TokenKind::RBrace)?;
                    return Ok(Expr::SetComp { elt: Box::new(first), generators: gens, span });
                }
                let mut elts = vec![first];
                while self.eat(&TokenKind::Comma) {
                    if matches!(self.peek().kind, TokenKind::RBrace) { break; }
                    elts.push(self.parse_expr()?);
                }
                self.expect(&TokenKind::RBrace)?;
                Ok(Expr::Set(elts, span))
            }

            _ => Err(ParseError {
                msg: format!("unexpected token in expression: {:?}", self.peek().kind),
                line: self.peek().line, col: self.peek().col,
            })
        }
    }

    fn parse_comprehension_clauses(&mut self) -> Result<Vec<Comprehension>, ParseError> {
        let mut gens = Vec::new();
        while matches!(self.peek().kind, TokenKind::For | TokenKind::Async) {
            let is_async = self.eat(&TokenKind::Async);
            self.expect(&TokenKind::For)?;
            let target = self.parse_target()?;
            self.expect(&TokenKind::In)?;
            let iter = self.parse_or()?;
            let mut ifs = Vec::new();
            while self.eat(&TokenKind::If) {
                ifs.push(self.parse_or()?);
            }
            gens.push(Comprehension { target, iter, ifs, is_async });
        }
        Ok(gens)
    }

    // ── Slash in params ───────────────────────────────────────────────────────
}

/// Helper to defer resolving index vs. slice.
enum SubscriptHelper {
    Index(Expr),
    Slice { lower: Option<Expr>, upper: Option<Expr>, step: Option<Expr> },
}

impl SubscriptHelper {
    fn resolve(self, obj: Expr, span: Span) -> Expr {
        match self {
            SubscriptHelper::Index(idx) => Expr::Index { obj: Box::new(obj), index: Box::new(idx), span },
            SubscriptHelper::Slice { lower, upper, step } => Expr::Slice {
                obj: Box::new(obj),
                lower: lower.map(Box::new),
                upper: upper.map(Box::new),
                step:  step.map(Box::new),
                span,
            },
        }
    }
}

// expose a convenient top-level parse function
pub fn parse(tokens: Vec<Token>) -> Result<Module, ParseError> {
    Parser::new(tokens).parse_module()
}
