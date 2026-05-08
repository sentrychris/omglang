//! # OMGlang Lexer
//!
//! Single-pass scanner for OMG source code. Produces a flat `Vec<Token>`
//! consumed by [`crate::parser::Parser`].
//!
//! Behaviour mirrors `omglang/lexer.py` from the original Python reference:
//! - Strips the required `;;;omg` header off the first non-empty line.
//! - Skips `#`-line comments and `/** ... */` doc blocks.
//! - Emits a `Newline` token (used by the parser to swallow blank lines
//!   between block statements).
//! - Decodes string escapes (`\n`, `\t`, `\r`, `\\`, `\"`, `\0`).
//!
//! Errors are returned as [`crate::error::RuntimeError::SyntaxError`] so the
//! caller doesn't have to translate.

use crate::error::RuntimeError;

/// One lexical token.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokKind,
    pub line: usize,
}

/// Token kinds. String/number tokens carry their decoded values directly.
#[derive(Clone, Debug, PartialEq)]
pub enum TokKind {
    // Literals
    Number(i64),
    Float(f64),
    Str(String),
    True,
    False,

    // Keywords
    If,
    Elif,
    Else,
    Loop,
    Break,
    Emit,
    Import,
    As,
    Facts,
    Func,
    Return,
    And,
    Or,
    Alloc,
    Try,
    Except,

    // Identifier
    Ident(String),

    // Assignment
    Assign,

    // Delimiters
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Colon,

    // Arithmetic
    Plus,
    Minus,
    Star,
    Percent,
    Slash,
    DoubleSlash,

    // Bitwise
    Shl,
    Shr,
    Amp,
    Pipe,
    Caret,
    Tilde,

    // Comparison
    Ge,
    Le,
    Eq,
    Ne,
    Gt,
    Lt,

    // End-of-line / end-of-file
    Newline,
    Eof,
}

impl TokKind {
    /// Human-readable token name (used in parser error messages).
    pub fn describe(&self) -> &'static str {
        match self {
            TokKind::Number(_) => "number",
            TokKind::Float(_) => "float",
            TokKind::Str(_) => "string",
            TokKind::True => "true",
            TokKind::False => "false",
            TokKind::If => "if",
            TokKind::Elif => "elif",
            TokKind::Else => "else",
            TokKind::Loop => "loop",
            TokKind::Break => "break",
            TokKind::Emit => "emit",
            TokKind::Import => "import",
            TokKind::As => "as",
            TokKind::Facts => "facts",
            TokKind::Func => "proc",
            TokKind::Return => "return",
            TokKind::And => "and",
            TokKind::Or => "or",
            TokKind::Alloc => "alloc",
            TokKind::Try => "try",
            TokKind::Except => "except",
            TokKind::Ident(_) => "identifier",
            TokKind::Assign => ":=",
            TokKind::LBrace => "{",
            TokKind::RBrace => "}",
            TokKind::LParen => "(",
            TokKind::RParen => ")",
            TokKind::LBracket => "[",
            TokKind::RBracket => "]",
            TokKind::Comma => ",",
            TokKind::Dot => ".",
            TokKind::Colon => ":",
            TokKind::Plus => "+",
            TokKind::Minus => "-",
            TokKind::Star => "*",
            TokKind::Percent => "%",
            TokKind::Slash => "/",
            TokKind::DoubleSlash => "//",
            TokKind::Shl => "<<",
            TokKind::Shr => ">>",
            TokKind::Amp => "&",
            TokKind::Pipe => "|",
            TokKind::Caret => "^",
            TokKind::Tilde => "~",
            TokKind::Ge => ">=",
            TokKind::Le => "<=",
            TokKind::Eq => "==",
            TokKind::Ne => "!=",
            TokKind::Gt => ">",
            TokKind::Lt => "<",
            TokKind::Newline => "newline",
            TokKind::Eof => "<eof>",
        }
    }
}

/// Tokenize a complete source string.
///
/// Strips the leading `;;;omg` header (if present) before scanning and tracks
/// line numbers from the line *after* the header, matching the Python lexer.
pub fn tokenize(code: &str, file: &str) -> Result<Vec<Token>, RuntimeError> {
    // The Python lexer numbers lines from 2 because the ;;;omg header is
    // line 1 and is stripped.  We keep parity here.
    let (body, start_line) = strip_header(code);
    let mut tokens = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i: usize = 0;
    let mut line = start_line;
    while i < chars.len() {
        let c = chars[i];
        // Whitespace (preserves newline tokens for the parser to skip)
        if c == ' ' || c == '\t' || c == '\r' {
            i += 1;
            continue;
        }
        if c == '\n' {
            tokens.push(Token {
                kind: TokKind::Newline,
                line,
            });
            line += 1;
            i += 1;
            continue;
        }
        // Comments
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // Doc blocks /** ... */ (Python lexer treats /* */ as well; we follow
        // the documented form).  We accept any /* ... */ for simplicity.
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        // Numbers
        if c.is_ascii_digit() {
            // Binary literal
            if c == '0' && i + 1 < chars.len() && (chars[i + 1] == 'b' || chars[i + 1] == 'B') {
                i += 2;
                let start = i;
                while i < chars.len() && (chars[i] == '0' || chars[i] == '1') {
                    i += 1;
                }
                if start == i {
                    return Err(RuntimeError::SyntaxError(format!(
                        "Empty binary literal on line {} in {}",
                        line, file
                    )));
                }
                let s: String = chars[start..i].iter().collect();
                let v = i64::from_str_radix(&s, 2).map_err(|e| {
                    RuntimeError::SyntaxError(format!(
                        "Invalid binary literal '{}' on line {} in {}: {}",
                        s, line, file, e
                    ))
                })?;
                tokens.push(Token {
                    kind: TokKind::Number(v),
                    line,
                });
                continue;
            }
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            // Float literal? Either `.<digit>` or `[eE][+-]?<digit>` after
            // the integer part promotes to float. A bare trailing `.`
            // (e.g. `5.`) does NOT — that lets `5.foo` still parse as
            // attribute access on int 5 and keeps the grammar unambiguous.
            let is_float_dot = i < chars.len()
                && chars[i] == '.'
                && peek(&chars, i + 1).map_or(false, |c| c.is_ascii_digit());
            let is_float_exp = i < chars.len() && (chars[i] == 'e' || chars[i] == 'E');
            if is_float_dot || is_float_exp {
                if is_float_dot {
                    i += 1; // consume '.'
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    i += 1;
                    if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                        i += 1;
                    }
                    let exp_start = i;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    if exp_start == i {
                        let bad: String = chars[start..i].iter().collect();
                        return Err(RuntimeError::SyntaxError(format!(
                            "Float exponent has no digits in '{}' on line {} in {}",
                            bad, line, file
                        )));
                    }
                }
                let s: String = chars[start..i].iter().collect();
                let v: f64 = s.parse().map_err(|e| {
                    RuntimeError::SyntaxError(format!(
                        "Invalid float literal '{}' on line {} in {}: {}",
                        s, line, file, e
                    ))
                })?;
                tokens.push(Token {
                    kind: TokKind::Float(v),
                    line,
                });
                continue;
            }
            let s: String = chars[start..i].iter().collect();
            let v: i64 = s.parse().map_err(|e| {
                RuntimeError::SyntaxError(format!(
                    "Invalid integer literal '{}' on line {} in {}: {}",
                    s, line, file, e
                ))
            })?;
            tokens.push(Token {
                kind: TokKind::Number(v),
                line,
            });
            continue;
        }
        // Strings
        if c == '"' {
            i += 1;
            let mut buf = String::new();
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                    if i >= chars.len() {
                        return Err(RuntimeError::SyntaxError(format!(
                            "Unterminated escape on line {} in {}",
                            line, file
                        )));
                    }
                    match chars[i] {
                        'n' => buf.push('\n'),
                        't' => buf.push('\t'),
                        'r' => buf.push('\r'),
                        '\\' => buf.push('\\'),
                        '"' => buf.push('"'),
                        '0' => buf.push('\0'),
                        '\'' => buf.push('\''),
                        other => {
                            buf.push('\\');
                            buf.push(other);
                        }
                    }
                    i += 1;
                } else {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    buf.push(chars[i]);
                    i += 1;
                }
            }
            if i >= chars.len() {
                return Err(RuntimeError::SyntaxError(format!(
                    "Unterminated string starting on line {} in {}",
                    line, file
                )));
            }
            i += 1; // consume closing "
            tokens.push(Token {
                kind: TokKind::Str(buf),
                line,
            });
            continue;
        }
        // Identifiers / keywords
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let kind = match word.as_str() {
                "if" => TokKind::If,
                "elif" => TokKind::Elif,
                "else" => TokKind::Else,
                "loop" => TokKind::Loop,
                "break" => TokKind::Break,
                "emit" => TokKind::Emit,
                "import" => TokKind::Import,
                "as" => TokKind::As,
                "facts" => TokKind::Facts,
                "proc" => TokKind::Func,
                "return" => TokKind::Return,
                "and" => TokKind::And,
                "or" => TokKind::Or,
                "alloc" => TokKind::Alloc,
                "try" => TokKind::Try,
                "except" => TokKind::Except,
                "true" => TokKind::True,
                "false" => TokKind::False,
                _ => TokKind::Ident(word),
            };
            tokens.push(Token { kind, line });
            continue;
        }
        // Multi-character operators
        if c == ':' && peek(&chars, i + 1) == Some('=') {
            tokens.push(Token {
                kind: TokKind::Assign,
                line,
            });
            i += 2;
            continue;
        }
        if c == '=' && peek(&chars, i + 1) == Some('=') {
            tokens.push(Token {
                kind: TokKind::Eq,
                line,
            });
            i += 2;
            continue;
        }
        if c == '!' && peek(&chars, i + 1) == Some('=') {
            tokens.push(Token {
                kind: TokKind::Ne,
                line,
            });
            i += 2;
            continue;
        }
        if c == '<' && peek(&chars, i + 1) == Some('=') {
            tokens.push(Token {
                kind: TokKind::Le,
                line,
            });
            i += 2;
            continue;
        }
        if c == '>' && peek(&chars, i + 1) == Some('=') {
            tokens.push(Token {
                kind: TokKind::Ge,
                line,
            });
            i += 2;
            continue;
        }
        if c == '<' && peek(&chars, i + 1) == Some('<') {
            tokens.push(Token {
                kind: TokKind::Shl,
                line,
            });
            i += 2;
            continue;
        }
        if c == '>' && peek(&chars, i + 1) == Some('>') {
            tokens.push(Token {
                kind: TokKind::Shr,
                line,
            });
            i += 2;
            continue;
        }
        if c == '/' && peek(&chars, i + 1) == Some('/') {
            tokens.push(Token {
                kind: TokKind::DoubleSlash,
                line,
            });
            i += 2;
            continue;
        }
        // Single-character operators & delimiters
        let single = match c {
            '{' => Some(TokKind::LBrace),
            '}' => Some(TokKind::RBrace),
            '(' => Some(TokKind::LParen),
            ')' => Some(TokKind::RParen),
            '[' => Some(TokKind::LBracket),
            ']' => Some(TokKind::RBracket),
            ',' => Some(TokKind::Comma),
            '.' => Some(TokKind::Dot),
            ':' => Some(TokKind::Colon),
            '+' => Some(TokKind::Plus),
            '-' => Some(TokKind::Minus),
            '*' => Some(TokKind::Star),
            '%' => Some(TokKind::Percent),
            '/' => Some(TokKind::Slash),
            '&' => Some(TokKind::Amp),
            '|' => Some(TokKind::Pipe),
            '^' => Some(TokKind::Caret),
            '~' => Some(TokKind::Tilde),
            '>' => Some(TokKind::Gt),
            '<' => Some(TokKind::Lt),
            _ => None,
        };
        if let Some(kind) = single {
            tokens.push(Token { kind, line });
            i += 1;
            continue;
        }
        return Err(RuntimeError::SyntaxError(format!(
            "Unexpected character '{}' on line {} in {}",
            c, line, file
        )));
    }
    tokens.push(Token {
        kind: TokKind::Eof,
        line,
    });
    Ok(tokens)
}

fn peek(chars: &[char], i: usize) -> Option<char> {
    chars.get(i).copied()
}

/// Strip the `;;;omg` header. Returns the body and the line number where the
/// body begins (1-based). When no header is present we still start at line 1.
fn strip_header(code: &str) -> (String, usize) {
    let mut header_seen = false;
    let mut body_start = 0usize;
    for (idx, line) in code.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ";;;omg" {
            header_seen = true;
            body_start = idx + 1;
        }
        break;
    }
    if header_seen {
        let mut iter = code.split_inclusive('\n');
        for _ in 0..body_start {
            iter.next();
        }
        let body: String = iter.collect();
        (body, body_start + 1)
    } else {
        (code.to_string(), 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_simple_program() {
        let src = ";;;omg\nalloc x := 5\nemit x\n";
        let toks = tokenize(src, "<t>").unwrap();
        let kinds: Vec<TokKind> = toks.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokKind::Alloc,
                TokKind::Ident("x".into()),
                TokKind::Assign,
                TokKind::Number(5),
                TokKind::Newline,
                TokKind::Emit,
                TokKind::Ident("x".into()),
                TokKind::Newline,
                TokKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_string_escapes() {
        let src = ";;;omg\nemit \"a\\nb\\\\c\"\n";
        let toks = tokenize(src, "<t>").unwrap();
        match &toks[1].kind {
            TokKind::Str(s) => assert_eq!(s, "a\nb\\c"),
            other => panic!("expected string, got {:?}", other),
        }
    }

    #[test]
    fn lexes_binary_literal() {
        let toks = tokenize(";;;omg\nemit 0b1010\n", "<t>").unwrap();
        assert!(matches!(toks[1].kind, TokKind::Number(10)));
    }

    #[test]
    fn rejects_unknown_char() {
        let err = tokenize(";;;omg\nemit @\n", "<t>").unwrap_err();
        assert!(matches!(err, RuntimeError::SyntaxError(_)));
    }
}
