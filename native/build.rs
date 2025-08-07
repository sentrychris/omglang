use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use serde_json::Value;

#[derive(Clone)]
enum Token {
    Symbol(String),
    Kw(String),
    Ident(String),
    Number(i64),
    Bool(bool),
    Str(String),
}

fn is_digit(ch: char) -> bool {
    ch >= '0' && ch <= '9'
}

fn is_alpha(ch: char) -> bool {
    (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch == '_'
}

fn is_alnum(ch: char) -> bool {
    is_alpha(ch) || is_digit(ch)
}

fn read_number(src: &[char], mut i: usize) -> (i64, usize) {
    let mut num: i64 = 0;
    while i < src.len() && is_digit(src[i]) {
        num = num * 10 + (src[i] as i64 - '0' as i64);
        i += 1;
    }
    (num, i)
}

fn read_binary(src: &[char], mut i: usize) -> (i64, usize) {
    let mut num: i64 = 0;
    while i < src.len() && (src[i] == '0' || src[i] == '1') {
        num = num * 2 + (src[i] as i64 - '0' as i64);
        i += 1;
    }
    (num, i)
}

fn read_ident(src: &[char], mut i: usize) -> (String, usize) {
    let mut s = String::new();
    while i < src.len() && is_alnum(src[i]) {
        s.push(src[i]);
        i += 1;
    }
    (s, i)
}

fn tokenize(source: &str) -> Vec<Token> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let mut src = &chars[..];
    if chars.len() >= 6
        && chars[0] == ';'
        && chars[1] == ';'
        && chars[2] == ';'
        && chars[3] == 'o'
        && chars[4] == 'm'
        && chars[5] == 'g'
    {
        i = 6;
        if i < chars.len() && chars[i] == '\r' {
            i += 1;
        }
        if i < chars.len() && chars[i] == '\n' {
            i += 1;
        }
        src = &chars[i..];
        i = 0;
    }
    let src_len = src.len();
    while i < src_len {
        let c = src[i];
        if c == ' ' || c == '\t' || c == '\r' || c == '\n' {
            i += 1;
        } else if c == '#' {
            while i < src_len && src[i] != '\n' {
                i += 1;
            }
        } else if c == ':' && i + 1 < src_len && src[i + 1] == '=' {
            tokens.push(Token::Symbol(":=".to_string()));
            i += 2;
        } else if c == ':' {
            tokens.push(Token::Symbol(":".to_string()));
            i += 1;
        } else if c == '=' && i + 1 < src_len && src[i + 1] == '=' {
            tokens.push(Token::Symbol("==".to_string()));
            i += 2;
        } else if c == '!' && i + 1 < src_len && src[i + 1] == '=' {
            tokens.push(Token::Symbol("!=".to_string()));
            i += 2;
        } else if c == '<' && i + 1 < src_len && src[i + 1] == '=' {
            tokens.push(Token::Symbol("<=".to_string()));
            i += 2;
        } else if c == '>' && i + 1 < src_len && src[i + 1] == '=' {
            tokens.push(Token::Symbol(">=".to_string()));
            i += 2;
        } else if c == '<' && i + 1 < src_len && src[i + 1] == '<' {
            tokens.push(Token::Symbol("<<".to_string()));
            i += 2;
        } else if c == '>' && i + 1 < src_len && src[i + 1] == '>' {
            tokens.push(Token::Symbol(">>".to_string()));
            i += 2;
        } else if c == '/' && i + 1 < src_len && src[i + 1] == '*' {
            i += 2;
            while i + 1 < src_len && !(src[i] == '*' && src[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if [ '(', ')', '{', '}', ',', '+', '-', '*', '/', '%', '<', '>', '[', ']', '&', '|', '^', '~', '.' ].contains(&c) {
            tokens.push(Token::Symbol(c.to_string()));
            i += 1;
        } else if c == '0' && i + 1 < src_len && (src[i + 1] == 'b' || src[i + 1] == 'B') {
            i += 2;
            let (num, ni) = read_binary(src, i);
            tokens.push(Token::Number(num));
            i = ni;
        } else if is_digit(c) {
            let (num, ni) = read_number(src, i);
            tokens.push(Token::Number(num));
            i = ni;
        } else if c == '"' {
            i += 1;
            let mut s = String::new();
            while i < src_len && src[i] != '"' {
                if src[i] == '\\' && i + 1 < src_len && src[i + 1] == 'n' {
                    s.push('\n');
                    i += 2;
                } else {
                    s.push(src[i]);
                    i += 1;
                }
            }
            i += 1;
            tokens.push(Token::Str(s));
        } else {
            let (word, ni) = read_ident(src, i);
            i = ni;
            match word.as_str() {
                "alloc" | "emit" | "proc" | "return" | "if" | "else" | "elif" | "loop" | "break" | "and" | "or" | "facts" | "import" | "as" => {
                    tokens.push(Token::Kw(word));
                }
                "true" => tokens.push(Token::Bool(true)),
                "false" => tokens.push(Token::Bool(false)),
                _ => tokens.push(Token::Ident(word)),
            }
        }
    }
    tokens
}

fn ast_node(kind: &str, parts: Vec<Value>) -> Value {
    let mut v = Vec::with_capacity(1 + parts.len());
    v.push(Value::String(kind.to_string()));
    v.extend(parts);
    Value::Array(v)
}

fn parse(source: &str) -> Vec<Value> {
    let tokens = tokenize(source);
    parse_program(&tokens, 0).0
}

fn parse_program(tokens: &[Token], mut i: usize) -> (Vec<Value>, usize) {
    let mut stmts = Vec::new();
    while i < tokens.len() {
        let (stmt, ni) = parse_statement(tokens, i);
        stmts.push(stmt);
        i = ni;
    }
    (stmts, i)
}

fn parse_block(tokens: &[Token], i: usize) -> (Value, usize) {
    let mut j = i + 1; // skip '{'
    let mut stmts = Vec::new();
    while j < tokens.len() {
        if let Token::Symbol(s) = &tokens[j] {
            if s == "}" {
                return (ast_node("block", vec![Value::Array(stmts)]), j + 1);
            }
        }
        let (stmt, nj) = parse_statement(tokens, j);
        stmts.push(stmt);
        j = nj;
    }
    (ast_node("block", vec![Value::Array(stmts)]), j)
}

fn parse_statement(tokens: &[Token], i: usize) -> (Value, usize) {
    match &tokens[i] {
        Token::Kw(k) if k == "alloc" => {
            if let Token::Ident(name) = &tokens[i + 1] {
                let (expr, j) = parse_expression(tokens, i + 3);
                (ast_node("decl", vec![Value::String(name.clone()), expr]), j)
            } else {
                panic!("expected identifier after alloc");
            }
        }
        Token::Kw(k) if k == "emit" => {
            let (expr, j) = parse_expression(tokens, i + 1);
            (ast_node("emit", vec![expr]), j)
        }
        Token::Kw(k) if k == "return" => {
            let (expr, j) = parse_expression(tokens, i + 1);
            (ast_node("return", vec![expr]), j)
        }
        Token::Kw(k) if k == "break" => (ast_node("break", vec![]), i + 1),
        Token::Kw(k) if k == "loop" => {
            let (cond, j) = parse_expression(tokens, i + 1);
            let (block, k) = parse_block(tokens, j);
            (ast_node("loop", vec![cond, block]), k)
        }
        Token::Kw(k) if k == "if" => parse_if(tokens, i),
        Token::Kw(k) if k == "proc" => {
            if let Token::Ident(name) = &tokens[i + 1] {
                let mut j = i + 3; // skip name and '('
                let mut params = Vec::new();
                while let Token::Ident(p) = &tokens[j] {
                    params.push(Value::String(p.clone()));
                    j += 1;
                    if let Token::Symbol(s) = &tokens[j] {
                        if s == "," {
                            j += 1;
                            continue;
                        }
                    }
                    break;
                }
                if let Token::Symbol(s) = &tokens[j] {
                    if s != ")" {
                        panic!("expected ')' after parameters");
                    }
                }
                let (body, k) = parse_block(tokens, j + 1);
                (ast_node("func_def", vec![Value::String(name.clone()), Value::Array(params), body]), k)
            } else {
                panic!("expected function name");
            }
        }
        Token::Kw(k) if k == "import" => {
            if let Token::Str(path) = &tokens[i + 1] {
                if let Token::Kw(as_kw) = &tokens[i + 2] {
                    if as_kw != "as" {
                        panic!("expected 'as' in import");
                    }
                } else {
                    panic!("expected 'as' in import");
                }
                if let Token::Ident(alias) = &tokens[i + 3] {
                    (
                        ast_node(
                            "import",
                            vec![Value::String(path.clone()), Value::String(alias.clone())],
                        ),
                        i + 4,
                    )
                } else {
                    panic!("expected alias ident");
                }
            } else {
                panic!("expected string path");
            }
        }
        Token::Kw(k) if k == "facts" => {
            let (expr, j) = parse_expression(tokens, i + 1);
            (ast_node("facts", vec![expr]), j)
        }
        Token::Ident(_) => {
            let (lval, j) = parse_factor(tokens, i);
            if j < tokens.len() {
                if let Token::Symbol(s) = &tokens[j] {
                    if s == ":=" {
                        let (rhs, k) = parse_expression(tokens, j + 1);
                        let arr = lval.as_array().unwrap();
                        let res = match arr[0].as_str().unwrap() {
                            "ident" => ast_node(
                                "assign",
                                vec![arr[1].clone(), rhs],
                            ),
                            "dot" => ast_node(
                                "attr_assign",
                                vec![arr[1].clone(), arr[2].clone(), rhs],
                            ),
                            "index" => ast_node(
                                "index_assign",
                                vec![arr[1].clone(), arr[2].clone(), rhs],
                            ),
                            _ => panic!("invalid assignment target"),
                        };
                        return (res, k);
                    }
                }
            }
            let (expr, k) = parse_expression(tokens, i);
            (ast_node("expr_stmt", vec![expr]), k)
        }
        _ => {
            let (expr, j) = parse_expression(tokens, i);
            (ast_node("expr_stmt", vec![expr]), j)
        }
    }
}

fn parse_if(tokens: &[Token], i: usize) -> (Value, usize) {
    let (cond, j) = parse_expression(tokens, i + 1);
    let (then_block, mut k) = parse_block(tokens, j);
    let mut elifs: Vec<(Value, Value)> = Vec::new();
    let mut else_block: Value = Value::Null;
    while k < tokens.len() {
        match &tokens[k] {
            Token::Kw(s) if s == "elif" => {
                let (c, j2) = parse_expression(tokens, k + 1);
                let (b, j3) = parse_block(tokens, j2);
                elifs.push((c, b));
                k = j3;
            }
            Token::Kw(s) if s == "else" => {
                let (b, j2) = parse_block(tokens, k + 1);
                else_block = b;
                k = j2;
                break;
            }
            _ => break,
        }
    }
    let mut tail = else_block;
    for (c, b) in elifs.into_iter().rev() {
        tail = ast_node("if", vec![c, b, tail]);
    }
    (ast_node("if", vec![cond, then_block, tail]), k)
}

fn parse_expression(tokens: &[Token], i: usize) -> (Value, usize) {
    parse_or(tokens, i)
}

fn parse_or(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_and(tokens, i);
    while j < tokens.len() {
        if let Token::Kw(op) = &tokens[j] {
            if op == "or" {
                let (right, nj) = parse_and(tokens, j + 1);
                left = ast_node("or", vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_and(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_comparison(tokens, i);
    while j < tokens.len() {
        if let Token::Kw(op) = &tokens[j] {
            if op == "and" {
                let (right, nj) = parse_comparison(tokens, j + 1);
                left = ast_node("and", vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_comparison(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_bit_or(tokens, i);
    while j < tokens.len() {
        if let Token::Symbol(op) = &tokens[j] {
            let op_name = match op.as_str() {
                "<" => Some("lt"),
                ">" => Some("gt"),
                "<=" => Some("le"),
                ">=" => Some("ge"),
                "==" => Some("eq"),
                "!=" => Some("ne"),
                _ => None,
            };
            if let Some(name) = op_name {
                let (right, nj) = parse_bit_or(tokens, j + 1);
                left = ast_node(name, vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_bit_or(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_bit_xor(tokens, i);
    while j < tokens.len() {
        if let Token::Symbol(s) = &tokens[j] {
            if s == "|" {
                let (right, nj) = parse_bit_xor(tokens, j + 1);
                left = ast_node("bor", vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_bit_xor(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_bit_and(tokens, i);
    while j < tokens.len() {
        if let Token::Symbol(s) = &tokens[j] {
            if s == "^" {
                let (right, nj) = parse_bit_and(tokens, j + 1);
                left = ast_node("bxor", vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_bit_and(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_shift(tokens, i);
    while j < tokens.len() {
        if let Token::Symbol(s) = &tokens[j] {
            if s == "&" {
                let (right, nj) = parse_shift(tokens, j + 1);
                left = ast_node("band", vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_shift(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_add_sub(tokens, i);
    while j < tokens.len() {
        if let Token::Symbol(s) = &tokens[j] {
            if s == "<<" || s == ">>" {
                let op_name = if s == "<<" { "shl" } else { "shr" };
                let (right, nj) = parse_add_sub(tokens, j + 1);
                left = ast_node(op_name, vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_add_sub(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_term(tokens, i);
    while j < tokens.len() {
        if let Token::Symbol(s) = &tokens[j] {
            if s == "+" || s == "-" {
                let op_name = if s == "+" { "add" } else { "sub" };
                let (right, nj) = parse_term(tokens, j + 1);
                left = ast_node(op_name, vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_term(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut left, mut j) = parse_factor(tokens, i);
    while j < tokens.len() {
        if let Token::Symbol(s) = &tokens[j] {
            if s == "*" || s == "/" || s == "%" {
                let op_name = match s.as_str() {
                    "*" => "mul",
                    "/" => "div",
                    _ => "mod",
                };
                let (right, nj) = parse_factor(tokens, j + 1);
                left = ast_node(op_name, vec![left, right]);
                j = nj;
                continue;
            }
        }
        break;
    }
    (left, j)
}

fn parse_factor(tokens: &[Token], i: usize) -> (Value, usize) {
    let (mut node, mut j) = match &tokens[i] {
        Token::Symbol(s) if s == "-" => {
            let (expr, j) = parse_factor(tokens, i + 1);
            (ast_node("unary", vec![Value::String("sub".to_string()), expr]), j)
        }
        Token::Symbol(s) if s == "~" => {
            let (expr, j) = parse_factor(tokens, i + 1);
            (
                ast_node(
                    "unary",
                    vec![Value::String("not_bits".to_string()), expr],
                ),
                j,
            )
        }
        Token::Number(n) => (ast_node("number", vec![Value::Number((*n).into())]), i + 1),
        Token::Bool(b) => (ast_node("bool", vec![Value::Bool(*b)]), i + 1),
        Token::Str(s) => (ast_node("string", vec![Value::String(s.clone())]), i + 1),
        Token::Ident(name) => (ast_node("ident", vec![Value::String(name.clone())]), i + 1),
        Token::Symbol(s) if s == "[" => {
            let mut elems = Vec::new();
            let mut k = i + 1;
            if let Token::Symbol(sym) = &tokens[k] {
                if sym == "]" {
                    return (ast_node("list", vec![Value::Array(elems)]), k + 1);
                }
            }
            loop {
                let (expr, nk) = parse_expression(tokens, k);
                elems.push(expr);
                k = nk;
                if let Token::Symbol(sym) = &tokens[k] {
                    if sym == "," {
                        k += 1;
                        continue;
                    } else if sym == "]" {
                        break;
                    }
                }
            }
            (ast_node("list", vec![Value::Array(elems)]), k + 1)
        }
        Token::Symbol(s) if s == "{" => {
            let mut pairs = Vec::new();
            let mut k = i + 1;
            if let Token::Symbol(sym) = &tokens[k] {
                if sym == "}" {
                    return (ast_node("dict", vec![Value::Array(pairs)]), k + 1);
                }
            }
            loop {
                let key = match &tokens[k] {
                    Token::Str(s) => {
                        k += 1;
                        s.clone()
                    }
                    Token::Ident(s) => {
                        k += 1;
                        s.clone()
                    }
                    _ => panic!("invalid dict key"),
                };
                k += 1; // skip ':'
                let (value, nk) = parse_expression(tokens, k);
                pairs.push(Value::Array(vec![Value::String(key), value]));
                k = nk;
                if let Token::Symbol(sym) = &tokens[k] {
                    if sym == "," {
                        k += 1;
                        continue;
                    } else if sym == "}" {
                        break;
                    }
                }
            }
            (ast_node("dict", vec![Value::Array(pairs)]), k + 1)
        }
        Token::Symbol(s) if s == "(" => {
            let (expr, k) = parse_expression(tokens, i + 1);
            (expr, k + 1)
        }
        _ => panic!("unexpected token in factor"),
    };

    loop {
        if j >= tokens.len() {
            break;
        }
        match &tokens[j] {
            Token::Symbol(s) if s == "(" => {
                let mut k = j + 1;
                let mut args = Vec::new();
                if let Token::Symbol(sym) = &tokens[k] {
                    if sym == ")" {
                        j = k + 1;
                        node = ast_node("func_call", vec![node.clone(), Value::Array(args)]);
                        continue;
                    }
                }
                loop {
                    let (arg, nk) = parse_expression(tokens, k);
                    args.push(arg);
                    k = nk;
                    if let Token::Symbol(sym) = &tokens[k] {
                        if sym == "," {
                            k += 1;
                            continue;
                        } else if sym == ")" {
                            break;
                        }
                    }
                }
                j = k + 1;
                node = ast_node("func_call", vec![node, Value::Array(args)]);
            }
            Token::Symbol(s) if s == "[" => {
                let (start, mut k) = parse_expression(tokens, j + 1);
                if let Token::Symbol(sym) = &tokens[k] {
                    if sym == ":" {
                        k += 1;
                        let end = if let Token::Symbol(sym2) = &tokens[k] {
                            if sym2 == "]" {
                                Value::Null
                            } else {
                                let (e, nk) = parse_expression(tokens, k);
                                k = nk;
                                e
                            }
                        } else {
                            let (e, nk) = parse_expression(tokens, k);
                            k = nk;
                            e
                        };
                        if let Token::Symbol(sym3) = &tokens[k] {
                            if sym3 != "]" {
                                panic!("expected ']' after slice");
                            }
                        } else {
                            panic!("expected ']' after slice");
                        }
                        j = k + 1;
                        node = ast_node("slice", vec![node, start, end]);
                    } else if sym == "]" {
                        j = k + 1;
                        node = ast_node("index", vec![node, start]);
                    } else {
                        let (idx_expr, nk) = parse_expression(tokens, k);
                        k = nk;
                        if let Token::Symbol(sym2) = &tokens[k] {
                            if sym2 != "]" {
                                panic!("expected ']' after index");
                            }
                        } else {
                            panic!("expected ']' after index");
                        }
                        j = k + 1;
                        node = ast_node("index", vec![node, idx_expr]);
                    }
                } else {
                    panic!("expected ']' or ':' in index/slice");
                }
            }
            Token::Symbol(s) if s == "." => {
                if let Token::Ident(name) = &tokens[j + 1] {
                    node = ast_node("dot", vec![node, Value::String(name.clone())]);
                    j += 2;
                } else {
                    panic!("expected identifier after '.'");
                }
            }
            _ => break,
        }
    }

    (node, j)
}


#[derive(Clone)]
struct Instr {
    op: String,
    arg: Option<Arg>,
}

#[derive(Clone)]
enum Arg {
    Int(i64),
    Str(String),
    Builtin(String, usize),
}

struct FunctionEntry {
    name: String,
    params: Vec<String>,
    address: usize,
}

struct Compiler {
    code: Vec<Instr>,
    pending_funcs: Vec<(String, Vec<String>, Vec<Instr>)>,
    funcs: Vec<FunctionEntry>,
    break_stack: Vec<Vec<usize>>,
    builtins: HashSet<String>,
}

impl Compiler {
    fn new() -> Self {
        let builtins = ["chr", "ascii", "hex", "binary", "length", "read_file"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        Self {
            code: Vec::new(),
            pending_funcs: Vec::new(),
            funcs: Vec::new(),
            break_stack: Vec::new(),
            builtins,
        }
    }

    fn emit(&mut self, op: &str, arg: Option<Arg>) {
        self.code.push(Instr { op: op.to_string(), arg });
    }

    fn emit_placeholder(&mut self, op: &str) -> usize {
        let idx = self.code.len();
        self.code.push(Instr { op: op.to_string(), arg: None });
        idx
    }

    fn patch(&mut self, idx: usize, target: usize) {
        let op = self.code[idx].op.clone();
        self.code[idx] = Instr { op, arg: Some(Arg::Int(target as i64)) };
    }

    fn compile(&mut self, ast: &Vec<Value>) {
        self.compile_block(ast);
        self.emit("HALT", None);

        let mut final_code = self.code.clone();
        for (name, params, body) in self.pending_funcs.drain(..) {
            let addr = final_code.len();
            self.funcs.push(FunctionEntry { name: name.clone(), params: params.clone(), address: addr });
            for instr in body {
                match instr.arg {
                    Some(Arg::Int(i)) if instr.op == "JUMP" || instr.op == "JUMP_IF_FALSE" => {
                        final_code.push(Instr { op: instr.op, arg: Some(Arg::Int(i + addr as i64)) });
                    }
                    _ => final_code.push(instr),
                }
            }
        }
        self.code = final_code;
    }

    fn compile_block(&mut self, block: &Vec<Value>) {
        for stmt in block {
            self.compile_stmt(stmt);
        }
    }

    fn compile_stmt(&mut self, stmt: &Value) {
        let arr = stmt.as_array().expect("stmt array");
        let kind = arr[0].as_str().expect("kind str");
        match kind {
            "emit" => {
                self.compile_expr(&arr[1]);
                self.emit("EMIT", None);
            }
            "decl" | "assign" => {
                let name = arr[1].as_str().unwrap().to_string();
                self.compile_expr(&arr[2]);
                self.emit("STORE", Some(Arg::Str(name)));
            }
            "attr_assign" => {
                self.compile_expr(&arr[1]);
                self.compile_expr(&arr[3]);
                let attr = arr[2].as_str().unwrap().to_string();
                self.emit("STORE_ATTR", Some(Arg::Str(attr)));
            }
            "index_assign" => {
                self.compile_expr(&arr[1]);
                self.compile_expr(&arr[2]);
                self.compile_expr(&arr[3]);
                self.emit("STORE_INDEX", None);
            }
            "expr_stmt" => {
                self.compile_expr(&arr[1]);
                self.emit("POP", None);
            }
            "import" => {
                let path = arr[1].as_str().unwrap().to_string();
                let alias = arr[2].as_str().unwrap().to_string();
                self.emit("PUSH_STR", Some(Arg::Str(path)));
                self.emit("IMPORT", None);
                self.emit("STORE", Some(Arg::Str(alias)));
            }
            "facts" => {
                self.compile_expr(&arr[1]);
                self.emit("ASSERT", None);
            }
            "if" => {
                // Unroll nested if/elif chain
                let mut cond_blocks: Vec<(Value, Vec<Value>)> = Vec::new();
                let mut current = stmt.clone();
                let mut else_block: Option<Vec<Value>> = None;
                loop {
                    let carr = current.as_array().unwrap();
                    let cond = carr[1].clone();
                    let block_node = &carr[2];
                    let block = block_node.as_array().unwrap()[1].as_array().unwrap().clone();
                    cond_blocks.push((cond, block));
                    let tail = &carr[3];
                    if let Some(tarr) = tail.as_array() {
                        if tarr[0].as_str().unwrap() == "if" {
                            current = tail.clone();
                            continue;
                        } else if tarr[0].as_str().unwrap() == "block" {
                            else_block = Some(tarr[1].as_array().unwrap().clone());
                        }
                    }
                    break;
                }
                let mut end_jumps = Vec::new();
                for (cond, block) in cond_blocks {
                    self.compile_expr(&cond);
                    let jf = self.emit_placeholder("JUMP_IF_FALSE");
                    self.compile_block(&block);
                    end_jumps.push(self.emit_placeholder("JUMP"));
                    self.patch(jf, self.code.len());
                }
                if let Some(block) = else_block {
                    self.compile_block(&block);
                }
                for j in end_jumps {
                    self.patch(j, self.code.len());
                }
            }
            "loop" => {
                let start = self.code.len();
                self.compile_expr(&arr[1]);
                let jf = self.emit_placeholder("JUMP_IF_FALSE");
                let body = arr[2].as_array().unwrap()[1].as_array().unwrap().clone();
                self.break_stack.push(Vec::new());
                self.compile_block(&body);
                self.emit("JUMP", Some(Arg::Int(start as i64)));
                self.patch(jf, self.code.len());
                if let Some(brks) = self.break_stack.pop() {
                    for idx in brks {
                        self.patch(idx, self.code.len());
                    }
                }
            }
            "func_def" => {
                let name = arr[1].as_str().unwrap().to_string();
                let params = arr[2]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                let body = arr[3].as_array().unwrap()[1].as_array().unwrap().clone();
                let body_code = self.compile_function_body(&body);
                self.pending_funcs.push((name, params, body_code));
            }
            "return" => {
                let expr = &arr[1];
                if let Some(farr) = expr.as_array() {
                    if farr[0].as_str().unwrap() == "func_call" {
                        if let Some(func_arr) = farr[1].as_array() {
                            if func_arr[0].as_str().unwrap() == "ident" {
                                let name = func_arr[1].as_str().unwrap();
                                let args = farr[2].as_array().unwrap();
                                for a in args {
                                    self.compile_expr(a);
                                }
                                if self.builtins.contains(name) {
                                    self.emit("BUILTIN", Some(Arg::Builtin(name.to_string(), args.len())));
                                    self.emit("RET", None);
                                } else {
                                    self.emit("TCALL", Some(Arg::Str(name.to_string())));
                                }
                                return;
                            }
                        }
                    }
                }
                self.compile_expr(expr);
                self.emit("RET", None);
            }
            "break" => {
                if self.break_stack.is_empty() {
                    panic!("'break' used outside of loop");
                }
                let j = self.emit_placeholder("JUMP");
                self.break_stack.last_mut().unwrap().push(j);
            }
            "block" => {
                let stmts = arr[1].as_array().unwrap();
                self.compile_block(stmts);
            }
            _ => panic!("Unsupported statement: {:?}", stmt),
        }
    }

    fn compile_function_body(&mut self, body: &Vec<Value>) -> Vec<Instr> {
        let saved = std::mem::take(&mut self.code);
        self.compile_block(body);
        self.emit("RET", None);
        let body_code = std::mem::take(&mut self.code);
        self.code = saved;
        body_code
    }

    fn compile_expr(&mut self, node: &Value) {
        let arr = node.as_array().expect("expr array");
        let op = arr[0].as_str().unwrap();
        match op {
            "number" => {
                self.emit("PUSH_INT", Some(Arg::Int(arr[1].as_i64().unwrap())));
            }
            "string" => {
                self.emit("PUSH_STR", Some(Arg::Str(arr[1].as_str().unwrap().to_string())));
            }
            "bool" => {
                let v = if arr[1].as_bool().unwrap() { 1 } else { 0 };
                self.emit("PUSH_BOOL", Some(Arg::Int(v)));
            }
            "ident" => {
                self.emit("LOAD", Some(Arg::Str(arr[1].as_str().unwrap().to_string())));
            }
            "list" => {
                let elems = arr[1].as_array().unwrap();
                for e in elems {
                    self.compile_expr(e);
                }
                self.emit("BUILD_LIST", Some(Arg::Int(elems.len() as i64)));
            }
            "dict" => {
                let pairs = arr[1].as_array().unwrap();
                for p in pairs {
                    let key = p.as_array().unwrap()[0].as_str().unwrap().to_string();
                    let val = &p.as_array().unwrap()[1];
                    self.emit("PUSH_STR", Some(Arg::Str(key)));
                    self.compile_expr(val);
                }
                self.emit("BUILD_DICT", Some(Arg::Int(pairs.len() as i64)));
            }
            "index" => {
                self.compile_expr(&arr[1]);
                self.compile_expr(&arr[2]);
                self.emit("INDEX", None);
            }
            "slice" => {
                self.compile_expr(&arr[1]);
                self.compile_expr(&arr[2]);
                if arr[3].is_null() {
                    self.emit("PUSH_NONE", None);
                } else {
                    self.compile_expr(&arr[3]);
                }
                self.emit("SLICE", None);
            }
            "dot" => {
                self.compile_expr(&arr[1]);
                self.emit("ATTR", Some(Arg::Str(arr[2].as_str().unwrap().to_string())));
            }
            "func_call" => {
                let func_node = &arr[1];
                let args = arr[2].as_array().unwrap();
                if let Some(farr) = func_node.as_array() {
                    if farr[0].as_str().unwrap() == "ident" {
                        let name = farr[1].as_str().unwrap();
                        for a in args {
                            self.compile_expr(a);
                        }
                        if self.builtins.contains(name) {
                            self.emit("BUILTIN", Some(Arg::Builtin(name.to_string(), args.len())));
                        } else {
                            self.emit("CALL", Some(Arg::Str(name.to_string())));
                        }
                        return;
                    }
                }
                self.compile_expr(func_node);
                for a in args {
                    self.compile_expr(a);
                }
                self.emit("CALL_VALUE", Some(Arg::Int(args.len() as i64)));
            }
            "unary" => {
                let unary_op = arr[1].as_str().unwrap();
                self.compile_expr(&arr[2]);
                match unary_op {
                    "sub" => self.emit("NEG", None),
                    "not_bits" => self.emit("NOT", None),
                    "add" => {}
                    _ => panic!("Unknown unary op {}", unary_op),
                }
            }
            _ => {
                // binary operations encoded as op name
                let ops: HashMap<&'static str, &'static str> = [
                    ("add", "ADD"),
                    ("sub", "SUB"),
                    ("mul", "MUL"),
                    ("div", "DIV"),
                    ("mod", "MOD"),
                    ("eq", "EQ"),
                    ("ne", "NE"),
                    ("gt", "GT"),
                    ("lt", "LT"),
                    ("ge", "GE"),
                    ("le", "LE"),
                    ("and", "AND"),
                    ("or", "OR"),
                    ("and_bits", "BAND"),
                    ("or_bits", "BOR"),
                    ("xor_bits", "BXOR"),
                    ("shl", "SHL"),
                    ("shr", "SHR"),
                ].into_iter().collect();
                if let Some(opcode) = ops.get(op) {
                    self.compile_expr(&arr[1]);
                    self.compile_expr(&arr[2]);
                    self.emit(opcode, None);
                } else {
                    panic!("Unsupported expression node: {:?}", node);
                }
            }
        }
    }

    fn to_string(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for f in &self.funcs {
            let params = f.params.join(" ");
            lines.push(format!("FUNC {} {} {} {}", f.name, f.params.len(), params, f.address));
        }
        for instr in &self.code {
            match &instr.arg {
                Some(Arg::Builtin(name, argc)) => {
                    lines.push(format!("BUILTIN {} {}", name, argc));
                }
                Some(Arg::Str(s)) if instr.op == "PUSH_STR" => {
                    lines.push(format!("PUSH_STR {}", serde_json::to_string(s).unwrap()));
                }
                Some(Arg::Str(s)) => {
                    lines.push(format!("{} {}", instr.op, s));
                }
                Some(Arg::Int(i)) => {
                    lines.push(format!("{} {}", instr.op, i));
                }
                None => lines.push(instr.op.clone()),
            }
        }
        lines.join("\n")
    }
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest.parent().expect("no parent");
    let source_path = repo_root.join("bootstrap/interpreter.omg");
    let out_dir = manifest.join("bytecode");
    fs::create_dir_all(&out_dir).expect("create bytecode dir");
    let out_bc = out_dir.join("interpreter.bc");

    let src = fs::read_to_string(&source_path).expect("read source");
    let ast = parse(&src);

    let mut compiler = Compiler::new();
    compiler.compile(&ast);
    let bc_str = compiler.to_string();

    fs::write(&out_bc, bc_str).expect("write bytecode");

    println!("cargo:rerun-if-changed={}", source_path.display());
}
