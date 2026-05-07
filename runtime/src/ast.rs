//! # OMG Abstract Syntax Tree
//!
//! Tagged-tree representation used by the parser, the compiler, and (for
//! debugging) the disassembler. Mirrors the AST shapes produced by
//! `omglang/parser/*.py` so that the language semantics are unchanged from
//! the Python reference implementation.

use std::rc::Rc;

/// Binary operators recognised by the AST. Comparison and bitwise ops are
/// included; logical `and` / `or` are kept separate so the compiler can emit
/// short-circuit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BAnd,
    BOr,
    BXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// Unary operators. `Plus` is a no-op kept for source-fidelity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Neg,
    BNot,
}

/// Every AST node. Rc'd children let the compiler walk the tree without
/// cloning while still keeping the node owned by the parser's vector.
#[derive(Clone, Debug)]
pub enum Node {
    // Expressions
    Number(i64, usize),
    Str(String, usize),
    Bool(bool, usize),
    List(Vec<Node>, usize),
    Dict(Vec<(String, Node)>, usize),
    Ident(String, usize),
    Binary(BinOp, Box<Node>, Box<Node>, usize),
    Unary(UnaryOp, Box<Node>, usize),
    Index(Box<Node>, Box<Node>, usize),
    Slice(Box<Node>, Box<Node>, Option<Box<Node>>, usize),
    Dot(Box<Node>, String, usize),
    FuncCall(Box<Node>, Vec<Node>, usize),

    // Statements
    Decl(String, Box<Node>, usize),
    Assign(String, Box<Node>, usize),
    AttrAssign(Box<Node>, String, Box<Node>, usize),
    IndexAssign(Box<Node>, Box<Node>, Box<Node>, usize),
    Emit(Box<Node>, usize),
    Facts(Box<Node>, usize),
    Import(String, String, usize),
    If(Box<Node>, Box<Node>, Option<Box<Node>>, usize),
    Loop(Box<Node>, Box<Node>, usize),
    Break(usize),
    FuncDef(String, Rc<Vec<String>>, Box<Node>, usize),
    Return(Box<Node>, usize),
    ExprStmt(Box<Node>, usize),
    Block(Vec<Node>, usize),
    Try(Box<Node>, Option<String>, Box<Node>, usize),
}

impl Node {
    /// Source line where the node begins. Used for error messages.
    pub fn line(&self) -> usize {
        use Node::*;
        match self {
            Number(_, l)
            | Str(_, l)
            | Bool(_, l)
            | List(_, l)
            | Dict(_, l)
            | Ident(_, l)
            | Binary(_, _, _, l)
            | Unary(_, _, l)
            | Index(_, _, l)
            | Slice(_, _, _, l)
            | Dot(_, _, l)
            | FuncCall(_, _, l)
            | Decl(_, _, l)
            | Assign(_, _, l)
            | AttrAssign(_, _, _, l)
            | IndexAssign(_, _, _, l)
            | Emit(_, l)
            | Facts(_, l)
            | Import(_, _, l)
            | If(_, _, _, l)
            | Loop(_, _, l)
            | Break(l)
            | FuncDef(_, _, _, l)
            | Return(_, l)
            | ExprStmt(_, l)
            | Block(_, l)
            | Try(_, _, _, l) => *l,
        }
    }
}
