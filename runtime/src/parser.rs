//! # OMGlang Recursive-Descent Parser
//!
//! Hand-written predictive parser that consumes a token stream from
//! [`crate::lexer`] and produces a tree of [`crate::ast::Node`]s.
//!
//! Grammar matches `bootstrap/src/compiler.omg` — same precedence levels,
//! same statement forms, same AST tags (just typed instead of stringly-
//! tagged tuples). The two parsers agree byte-for-byte at the bytecode
//! layer; that's what `--verify-self-hosted` checks.
//!
//! Error messages always include the source file and line number so users
//! can navigate from a syntax error directly to the offending token.

use std::rc::Rc;

use crate::ast::{BinOp, Node, UnaryOp};
use crate::error::RuntimeError;
use crate::lexer::{TokKind, Token};

pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    pub source_file: String,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token], source_file: impl Into<String>) -> Self {
        Self {
            tokens,
            pos: 0,
            source_file: source_file.into(),
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_at(&self, off: usize) -> Option<&Token> {
        self.tokens.get(self.pos + off)
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        self.pos += 1;
        t
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek().kind, TokKind::Newline) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, kind: &TokKind) -> Result<&Token, RuntimeError> {
        if std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind) {
            Ok(self.advance())
        } else {
            let line = self.peek().line;
            Err(RuntimeError::SyntaxError(format!(
                "Expected '{}' but got '{}' on line {} in {}",
                kind.describe(),
                self.peek().kind.describe(),
                line,
                self.source_file
            )))
        }
    }

    fn syntax(&self, msg: impl Into<String>) -> RuntimeError {
        let line = self.peek().line;
        RuntimeError::SyntaxError(format!(
            "{} on line {} in {}",
            msg.into(),
            line,
            self.source_file
        ))
    }

    /// Parse a complete program (sequence of top-level statements).
    pub fn parse_program(&mut self) -> Result<Vec<Node>, RuntimeError> {
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek().kind, TokKind::Eof) {
            stmts.push(self.parse_statement()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    fn parse_statement(&mut self) -> Result<Node, RuntimeError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokKind::Facts => self.parse_facts(),
            TokKind::Emit => self.parse_emit(),
            TokKind::Import => self.parse_import(),
            TokKind::If => self.parse_if(),
            TokKind::Loop => self.parse_loop(),
            TokKind::Break => self.parse_break(),
            TokKind::Func => self.parse_func_def(),
            TokKind::Try => self.parse_try(),
            TokKind::Alloc => self.parse_decl(),
            TokKind::Return => self.parse_return(),
            TokKind::Ident(_) => self.parse_id_lead_statement(),
            _ => Err(self.syntax(format!(
                "Unexpected token '{}'",
                tok.kind.describe()
            ))),
        }
    }

    fn parse_block(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::LBrace)?;
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek().kind, TokKind::RBrace) {
            stmts.push(self.parse_statement()?);
            self.skip_newlines();
        }
        self.expect(&TokKind::RBrace)?;
        Ok(Node::Block(stmts, line))
    }

    fn parse_facts(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Facts)?;
        let expr = self.parse_expr()?;
        Ok(Node::Facts(Box::new(expr), line))
    }

    fn parse_emit(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Emit)?;
        let expr = self.parse_expr()?;
        Ok(Node::Emit(Box::new(expr), line))
    }

    fn parse_import(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Import)?;
        let path_tok = self.advance().clone();
        let path = match path_tok.kind {
            TokKind::Str(s) => s,
            _ => return Err(self.syntax("Expected string literal after 'import'")),
        };
        self.expect(&TokKind::As)?;
        let alias_tok = self.advance().clone();
        let alias = match alias_tok.kind {
            TokKind::Ident(s) => s,
            _ => return Err(self.syntax("Expected identifier after 'as'")),
        };
        Ok(Node::Import(path, alias, line))
    }

    fn parse_if(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::If)?;
        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let mut elif_cases: Vec<(Node, Node)> = Vec::new();
        while matches!(self.peek().kind, TokKind::Elif) {
            self.advance();
            let c = self.parse_expr()?;
            let b = self.parse_block()?;
            elif_cases.push((c, b));
        }
        let mut else_block: Option<Node> = None;
        if matches!(self.peek().kind, TokKind::Else) {
            self.advance();
            else_block = Some(self.parse_block()?);
        }
        // Fold elifs into nested if/elses, just like the Python parser.
        let mut tail = else_block;
        for (c, b) in elif_cases.into_iter().rev() {
            let line_c = c.line();
            tail = Some(Node::If(Box::new(c), Box::new(b), tail.map(Box::new), line_c));
        }
        Ok(Node::If(
            Box::new(cond),
            Box::new(then_block),
            tail.map(Box::new),
            line,
        ))
    }

    fn parse_loop(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Loop)?;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Node::Loop(Box::new(cond), Box::new(body), line))
    }

    fn parse_break(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Break)?;
        Ok(Node::Break(line))
    }

    fn parse_func_def(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Func)?;
        let name_tok = self.advance().clone();
        let name = match name_tok.kind {
            TokKind::Ident(s) => s,
            _ => return Err(self.syntax("Expected function name after 'proc'")),
        };
        self.expect(&TokKind::LParen)?;
        self.skip_newlines();
        let mut params: Vec<String> = Vec::new();
        if !matches!(self.peek().kind, TokKind::RParen) {
            let p = self.advance().clone();
            params.push(match p.kind {
                TokKind::Ident(s) => s,
                _ => return Err(self.syntax("Expected parameter name")),
            });
            self.skip_newlines();
            while matches!(self.peek().kind, TokKind::Comma) {
                self.advance();
                self.skip_newlines();
                let p = self.advance().clone();
                params.push(match p.kind {
                    TokKind::Ident(s) => s,
                    _ => return Err(self.syntax("Expected parameter name")),
                });
                self.skip_newlines();
            }
        }
        self.expect(&TokKind::RParen)?;
        let body = self.parse_block()?;
        Ok(Node::FuncDef(name, Rc::new(params), Box::new(body), line))
    }

    fn parse_return(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Return)?;
        let expr = self.parse_expr()?;
        Ok(Node::Return(Box::new(expr), line))
    }

    fn parse_decl(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Alloc)?;
        let id_tok = self.advance().clone();
        let name = match id_tok.kind {
            TokKind::Ident(s) => s,
            _ => return Err(self.syntax("Expected identifier after 'alloc'")),
        };
        self.expect(&TokKind::Assign)?;
        let expr = self.parse_expr()?;
        Ok(Node::Decl(name, Box::new(expr), line))
    }

    fn parse_try(&mut self) -> Result<Node, RuntimeError> {
        let line = self.peek().line;
        self.expect(&TokKind::Try)?;
        let try_block = self.parse_block()?;
        self.expect(&TokKind::Except)?;
        let exc_name = if let TokKind::Ident(_) = self.peek().kind {
            let t = self.advance().clone();
            match t.kind {
                TokKind::Ident(s) => Some(s),
                _ => unreachable!(),
            }
        } else {
            None
        };
        let except_block = self.parse_block()?;
        Ok(Node::Try(
            Box::new(try_block),
            exc_name,
            Box::new(except_block),
            line,
        ))
    }

    /// Statement that begins with an identifier — could be a plain
    /// reassignment (`x := ...`), an attribute / index assignment, or
    /// (rarely) a bare expression statement.
    fn parse_id_lead_statement(&mut self) -> Result<Node, RuntimeError> {
        // Fast path: <ident> := <expr>
        if let TokKind::Ident(_) = &self.peek().kind {
            if let Some(t) = self.peek_at(1) {
                if matches!(t.kind, TokKind::Assign) {
                    let id_tok = self.advance().clone();
                    let name = match id_tok.kind {
                        TokKind::Ident(s) => s,
                        _ => unreachable!(),
                    };
                    self.expect(&TokKind::Assign)?;
                    let expr = self.parse_expr()?;
                    return Ok(Node::Assign(name, Box::new(expr), id_tok.line));
                }
            }
        }
        // Try lvalue := expr; on failure, fall back to expression statement.
        let saved = self.pos;
        if let Ok(lval) = self.parse_lvalue() {
            if matches!(self.peek().kind, TokKind::Assign) {
                let line = self.peek().line;
                self.advance();
                let value = self.parse_expr()?;
                return Ok(match lval {
                    Node::Dot(target, attr, _) => {
                        Node::AttrAssign(target, attr, Box::new(value), line)
                    }
                    Node::Index(target, idx, _) => {
                        Node::IndexAssign(target, idx, Box::new(value), line)
                    }
                    other => Node::ExprStmt(Box::new(other), line),
                });
            }
        }
        // Rewind and parse as an expression statement.
        self.pos = saved;
        let line = self.peek().line;
        let expr = self.parse_factor()?;
        Ok(Node::ExprStmt(Box::new(expr), line))
    }

    fn parse_lvalue(&mut self) -> Result<Node, RuntimeError> {
        let id_tok = self.advance().clone();
        let name = match id_tok.kind {
            TokKind::Ident(s) => s,
            _ => return Err(self.syntax("Expected identifier")),
        };
        let mut node = Node::Ident(name, id_tok.line);
        loop {
            match &self.peek().kind {
                TokKind::Dot => {
                    self.advance();
                    let attr_tok = self.advance().clone();
                    let attr = match attr_tok.kind {
                        TokKind::Ident(s) => s,
                        _ => return Err(self.syntax("Expected identifier after '.'")),
                    };
                    node = Node::Dot(Box::new(node), attr, attr_tok.line);
                }
                TokKind::LBracket => {
                    let line = self.peek().line;
                    self.advance();
                    let idx = self.parse_expr()?;
                    self.expect(&TokKind::RBracket)?;
                    node = Node::Index(Box::new(node), Box::new(idx), line);
                }
                _ => break,
            }
        }
        Ok(node)
    }

    // ------------------------------------------------------------------
    // Expressions (precedence-climbing)
    // ------------------------------------------------------------------

    fn parse_expr(&mut self) -> Result<Node, RuntimeError> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_logical_and()?;
        while matches!(self.peek().kind, TokKind::Or) {
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_logical_and()?;
            lhs = Node::Binary(BinOp::Or, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_logical_and(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_comparison()?;
        while matches!(self.peek().kind, TokKind::And) {
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_comparison()?;
            lhs = Node::Binary(BinOp::And, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_comparison(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_bitwise_or()?;
        loop {
            let op = match self.peek().kind {
                TokKind::Eq => BinOp::Eq,
                TokKind::Ne => BinOp::Ne,
                TokKind::Lt => BinOp::Lt,
                TokKind::Le => BinOp::Le,
                TokKind::Gt => BinOp::Gt,
                TokKind::Ge => BinOp::Ge,
                _ => break,
            };
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_bitwise_or()?;
            lhs = Node::Binary(op, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_bitwise_or(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_bitwise_xor()?;
        while matches!(self.peek().kind, TokKind::Pipe) {
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_bitwise_xor()?;
            lhs = Node::Binary(BinOp::BOr, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_bitwise_and()?;
        while matches!(self.peek().kind, TokKind::Caret) {
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_bitwise_and()?;
            lhs = Node::Binary(BinOp::BXor, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_bitwise_and(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_shift()?;
        while matches!(self.peek().kind, TokKind::Amp) {
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_shift()?;
            lhs = Node::Binary(BinOp::BAnd, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_shift(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_add_sub()?;
        loop {
            let op = match self.peek().kind {
                TokKind::Shl => BinOp::Shl,
                TokKind::Shr => BinOp::Shr,
                _ => break,
            };
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_add_sub()?;
            lhs = Node::Binary(op, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_add_sub(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_term()?;
        loop {
            let op = match self.peek().kind {
                TokKind::Plus => BinOp::Add,
                TokKind::Minus => BinOp::Sub,
                _ => break,
            };
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_term()?;
            lhs = Node::Binary(op, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_term(&mut self) -> Result<Node, RuntimeError> {
        let mut lhs = self.parse_factor()?;
        loop {
            let op = match self.peek().kind {
                TokKind::Star => BinOp::Mul,
                TokKind::Slash => BinOp::Div,
                TokKind::DoubleSlash => BinOp::FloorDiv,
                TokKind::Percent => BinOp::Mod,
                _ => break,
            };
            let line = self.peek().line;
            self.advance();
            let rhs = self.parse_factor()?;
            lhs = Node::Binary(op, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_factor(&mut self) -> Result<Node, RuntimeError> {
        let tok = self.peek().clone();
        // Unary operators
        match tok.kind {
            TokKind::Tilde => {
                self.advance();
                let inner = self.parse_factor()?;
                return Ok(Node::Unary(UnaryOp::BNot, Box::new(inner), tok.line));
            }
            TokKind::Plus => {
                self.advance();
                let inner = self.parse_factor()?;
                return Ok(Node::Unary(UnaryOp::Plus, Box::new(inner), tok.line));
            }
            TokKind::Minus => {
                self.advance();
                let inner = self.parse_factor()?;
                return Ok(Node::Unary(UnaryOp::Neg, Box::new(inner), tok.line));
            }
            _ => {}
        }
        // Primaries
        let mut node = match tok.kind {
            TokKind::Number(v) => {
                self.advance();
                Node::Number(v, tok.line)
            }
            TokKind::Float(v) => {
                self.advance();
                Node::Float(v, tok.line)
            }
            TokKind::Str(s) => {
                self.advance();
                Node::Str(s, tok.line)
            }
            TokKind::True => {
                self.advance();
                Node::Bool(true, tok.line)
            }
            TokKind::False => {
                self.advance();
                Node::Bool(false, tok.line)
            }
            TokKind::LBracket => {
                self.advance();
                let mut elems = Vec::new();
                self.skip_newlines();
                while !matches!(self.peek().kind, TokKind::RBracket) {
                    elems.push(self.parse_expr()?);
                    self.skip_newlines();
                    if matches!(self.peek().kind, TokKind::Comma) {
                        self.advance();
                        self.skip_newlines();
                    } else {
                        break;
                    }
                }
                self.expect(&TokKind::RBracket)?;
                Node::List(elems, tok.line)
            }
            TokKind::LBrace => {
                self.advance();
                let mut pairs: Vec<(String, Node)> = Vec::new();
                self.skip_newlines();
                while !matches!(self.peek().kind, TokKind::RBrace) {
                    let key_tok = self.advance().clone();
                    let key = match key_tok.kind {
                        TokKind::Str(s) => s,
                        TokKind::Ident(s) => s,
                        _ => return Err(self.syntax("Invalid dict key")),
                    };
                    self.expect(&TokKind::Colon)?;
                    self.skip_newlines();
                    let value = self.parse_expr()?;
                    pairs.push((key, value));
                    self.skip_newlines();
                    if matches!(self.peek().kind, TokKind::Comma) {
                        self.advance();
                        self.skip_newlines();
                    } else {
                        break;
                    }
                }
                self.expect(&TokKind::RBrace)?;
                Node::Dict(pairs, tok.line)
            }
            TokKind::Ident(name) => {
                self.advance();
                Node::Ident(name, tok.line)
            }
            TokKind::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(&TokKind::RParen)?;
                inner
            }
            ref other => {
                return Err(self.syntax(format!(
                    "Unexpected token '{}'",
                    other.describe()
                )));
            }
        };
        // Postfix: function calls, indexing, slicing, attribute access
        loop {
            match self.peek().kind.clone() {
                TokKind::LParen => {
                    let line = self.peek().line;
                    self.advance();
                    self.skip_newlines();
                    let mut args = Vec::new();
                    if !matches!(self.peek().kind, TokKind::RParen) {
                        args.push(self.parse_expr()?);
                        self.skip_newlines();
                        while matches!(self.peek().kind, TokKind::Comma) {
                            self.advance();
                            self.skip_newlines();
                            args.push(self.parse_expr()?);
                            self.skip_newlines();
                        }
                    }
                    self.expect(&TokKind::RParen)?;
                    node = Node::FuncCall(Box::new(node), args, line);
                }
                TokKind::LBracket => {
                    let line = self.peek().line;
                    self.advance();
                    let start = self.parse_expr()?;
                    if matches!(self.peek().kind, TokKind::Colon) {
                        self.advance();
                        let end = if matches!(self.peek().kind, TokKind::RBracket) {
                            None
                        } else {
                            Some(Box::new(self.parse_expr()?))
                        };
                        self.expect(&TokKind::RBracket)?;
                        node = Node::Slice(Box::new(node), Box::new(start), end, line);
                    } else {
                        self.expect(&TokKind::RBracket)?;
                        node = Node::Index(Box::new(node), Box::new(start), line);
                    }
                }
                TokKind::Dot => {
                    self.advance();
                    let attr_tok = self.advance().clone();
                    let attr = match attr_tok.kind {
                        TokKind::Ident(s) => s,
                        _ => return Err(self.syntax("Expected identifier after '.'")),
                    };
                    node = Node::Dot(Box::new(node), attr, attr_tok.line);
                }
                _ => break,
            }
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse(src: &str) -> Result<Vec<Node>, RuntimeError> {
        let toks = tokenize(src, "<t>")?;
        let mut p = Parser::new(&toks, "<t>");
        p.parse_program()
    }

    #[test]
    fn parses_simple_decl_and_emit() {
        let ast = parse(";;;omg\nalloc x := 5\nemit x\n").unwrap();
        assert_eq!(ast.len(), 2);
        assert!(matches!(ast[0], Node::Decl(_, _, _)));
        assert!(matches!(ast[1], Node::Emit(_, _)));
    }

    #[test]
    fn parses_if_with_elif_else() {
        let ast = parse(";;;omg\nif 1 { emit 1 } elif 2 { emit 2 } else { emit 3 }\n").unwrap();
        assert!(matches!(ast[0], Node::If(_, _, Some(_), _)));
    }

    #[test]
    fn parses_function() {
        let ast =
            parse(";;;omg\nproc add(a, b) { return a + b }\nemit add(2, 3)\n").unwrap();
        assert!(matches!(ast[0], Node::FuncDef(_, _, _, _)));
        assert!(matches!(ast[1], Node::Emit(_, _)));
    }
}
