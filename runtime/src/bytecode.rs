use serde_json;
use std::collections::HashMap;

/// Representation of a compiled function.
#[derive(Clone)]
pub struct Function {
    pub params: Vec<String>,
    pub address: usize,
}

/// Instruction set for the OMG stack VM.
#[derive(Clone)]
pub enum Instr {
    PushInt(i64),
    PushStr(String),
    PushBool(bool),
    BuildList(usize),
    BuildDict(usize),
    Load(String),
    Store(String),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BAnd,
    BOr,
    BXor,
    Shl,
    Shr,
    And,
    Or,
    Not,
    Neg,
    Index,
    Slice,
    Jump(usize),
    JumpIfFalse(usize),
    Call(String),
    TailCall(String),
    CallBuiltin(String, usize),
    Pop,
    PushNone,
    Ret,
    Emit,
    Halt,
    StoreIndex,
    Attr(String),
    StoreAttr(String),
    Assert,
    CallValue(usize),
}

/// Parse a textual bytecode file into instructions.
pub fn parse_bytecode(src: &str) -> (Vec<Instr>, HashMap<String, Function>) {
    let mut code = Vec::new();
    let mut funcs: HashMap<String, Function> = HashMap::new();
    for line in src.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("FUNC ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[0].to_string();
                let param_count: usize = parts[1].parse().unwrap_or(0);
                let params = parts[2..2 + param_count]
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect::<Vec<_>>();
                let addr_idx = 2 + param_count;
                let address: usize = parts[addr_idx].parse().unwrap_or(0);
                funcs.insert(name, Function { params, address });
            }
        } else if let Some(rest) = trimmed.strip_prefix("PUSH_INT ") {
            if let Ok(v) = rest.parse::<i64>() {
                code.push(Instr::PushInt(v));
            }
        } else if let Some(rest) = trimmed.strip_prefix("PUSH_STR ") {
            if let Ok(s) = serde_json::from_str::<String>(rest) {
                code.push(Instr::PushStr(s));
            }
        } else if let Some(rest) = trimmed.strip_prefix("PUSH_BOOL ") {
            let b = rest.trim() == "1" || rest.trim().eq_ignore_ascii_case("true");
            code.push(Instr::PushBool(b));
        } else if let Some(rest) = trimmed.strip_prefix("BUILD_LIST ") {
            if let Ok(n) = rest.parse::<usize>() {
                code.push(Instr::BuildList(n));
            }
        } else if let Some(rest) = trimmed.strip_prefix("BUILD_DICT ") {
            if let Ok(n) = rest.parse::<usize>() {
                code.push(Instr::BuildDict(n));
            }
        } else if let Some(rest) = trimmed.strip_prefix("LOAD ") {
            code.push(Instr::Load(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("STORE ") {
            code.push(Instr::Store(rest.to_string()));
        } else if trimmed == "ADD" {
            code.push(Instr::Add);
        } else if trimmed == "SUB" {
            code.push(Instr::Sub);
        } else if trimmed == "MUL" {
            code.push(Instr::Mul);
        } else if trimmed == "DIV" {
            code.push(Instr::Div);
        } else if trimmed == "MOD" {
            code.push(Instr::Mod);
        } else if trimmed == "EQ" {
            code.push(Instr::Eq);
        } else if trimmed == "NE" {
            code.push(Instr::Ne);
        } else if trimmed == "LT" {
            code.push(Instr::Lt);
        } else if trimmed == "LE" {
            code.push(Instr::Le);
        } else if trimmed == "GT" {
            code.push(Instr::Gt);
        } else if trimmed == "GE" {
            code.push(Instr::Ge);
        } else if trimmed == "BAND" {
            code.push(Instr::BAnd);
        } else if trimmed == "BOR" {
            code.push(Instr::BOr);
        } else if trimmed == "BXOR" {
            code.push(Instr::BXor);
        } else if trimmed == "SHL" {
            code.push(Instr::Shl);
        } else if trimmed == "SHR" {
            code.push(Instr::Shr);
        } else if trimmed == "AND" {
            code.push(Instr::And);
        } else if trimmed == "OR" {
            code.push(Instr::Or);
        } else if trimmed == "NOT" {
            code.push(Instr::Not);
        } else if trimmed == "NEG" {
            code.push(Instr::Neg);
        } else if trimmed == "INDEX" {
            code.push(Instr::Index);
        } else if trimmed == "SLICE" {
            code.push(Instr::Slice);
        } else if trimmed == "STORE_INDEX" {
            code.push(Instr::StoreIndex);
        } else if let Some(rest) = trimmed.strip_prefix("ATTR ") {
            code.push(Instr::Attr(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("STORE_ATTR ") {
            code.push(Instr::StoreAttr(rest.to_string()));
        } else if trimmed == "ASSERT" {
            code.push(Instr::Assert);
        } else if let Some(rest) = trimmed.strip_prefix("CALL_VALUE ") {
            if let Ok(n) = rest.parse::<usize>() {
                code.push(Instr::CallValue(n));
            }
        } else if let Some(rest) = trimmed.strip_prefix("JUMP_IF_FALSE ") {
            if let Ok(t) = rest.parse::<usize>() {
                code.push(Instr::JumpIfFalse(t));
            }
        } else if let Some(rest) = trimmed.strip_prefix("JUMP ") {
            if let Ok(t) = rest.parse::<usize>() {
                code.push(Instr::Jump(t));
            }
        } else if let Some(rest) = trimmed.strip_prefix("CALL ") {
            code.push(Instr::Call(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("TCALL ") {
            code.push(Instr::TailCall(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("BUILTIN ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 2 {
                if let Ok(argc) = parts[1].parse::<usize>() {
                    code.push(Instr::CallBuiltin(parts[0].to_string(), argc));
                }
            }
        } else if trimmed == "RET" {
            code.push(Instr::Ret);
        } else if trimmed == "EMIT" {
            code.push(Instr::Emit);
        } else if trimmed == "HALT" {
            code.push(Instr::Halt);
        } else if trimmed == "POP" {
            code.push(Instr::Pop);
        } else if trimmed == "PUSH_NONE" {
            code.push(Instr::PushNone);
        }
    }
    (code, funcs)
}
