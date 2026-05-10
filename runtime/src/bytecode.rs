//! # OMG Bytecode Format & Codec
//!
//! Defines the **instruction set**, **function metadata**, and a strict
//! **binary parser/writer** for OMG bytecode. The binary layout is
//! little-endian and unchanged from the legacy Python compiler so existing
//! `.omgb` files remain readable.
//!
//! ## Binary layout (v2)
//! ```text
//! +------------------+----------------------------+
//! | Magic "OMGB"     | 4 bytes                    |
//! +------------------+----------------------------+
//! | Version          | u32                        |
//! +------------------+----------------------------+
//! | Source-file cnt  | u32                        |
//! +------------------+----------------------------+
//! | For each file:                                |
//! |   Path           | u32 len + UTF-8 bytes      |
//! +------------------+----------------------------+
//! | Func count       | u32                        |
//! +------------------+----------------------------+
//! | For each func:                                |
//! |   Name           | u32 len + UTF-8 bytes      |
//! |   Param count    | u32                        |
//! |   Params[...]    | (Param count times)        |
//! |                  |   u32 len + UTF-8 bytes    |
//! |   Address        | u32                        |
//! |   Source file ix | u32                        |
//! +------------------+----------------------------+
//! | Code length      | u32                        |
//! +------------------+----------------------------+
//! | For each instr:                               |
//! |   Opcode         | u8                         |
//! |   Operands       | opcode-specific payload    |
//! +------------------+----------------------------+
//! | Src-map length   | u32  (= code length)       |
//! +------------------+----------------------------+
//! | For each entry:                               |
//! |   Source file ix | u32                        |
//! |   Line number    | u32 (1-based; 0 = unknown) |
//! +------------------+----------------------------+
//! ```
//!
//! The source-file table + the per-instruction source map are what give
//! runtime errors their `File "foo.omg", line 12, in bar` framing.
//! Version 0x0001xx (no source map) is rejected with a clear "recompile
//! your .omgb" SyntaxError — there's no fallback path.
//!
//! Errors during decode are returned as
//! [`crate::error::RuntimeError::SyntaxError`] (the closest fit for
//! "this image isn't valid"). The parser used to `assert!` and `unwrap()`;
//! it now propagates instead so the runtime doesn't panic on user input.

use std::collections::HashMap;

use crate::error::{ErrorKind, RuntimeError};

/// Packed bytecode version: `(MAJOR << 16) | (MINOR << 8) | PATCH`.
///
/// Bumped to 0x000200 when the source-file table + per-instruction source
/// map were added. Older .omgb files have no source info and would
/// silently truncate at the new sections; we reject them by version.
pub const BC_VERSION: u32 = (0 << 16) | (2 << 8) | 0;

/// Compiled function metadata.
#[derive(Clone, Debug)]
pub struct Function {
    pub params: Vec<String>,
    pub address: usize,
    /// Index into [`SourceMap::files`]. Used by the VM's traceback
    /// formatter to print `File "<path>", line N, in <fn>`. A value of
    /// `u32::MAX` means "unknown source" (REPL-evaluated chunks).
    pub source_file_idx: u32,
}

/// Per-instruction source location, parallel to the instruction stream.
///
/// `files[i]` is the absolute (or repo-relative) path of the i'th source
/// file referenced by this bytecode image. `lines[pc]` gives the
/// `(file_idx, line)` of the bytecode instruction at offset `pc`.
///
/// Line numbers are 1-based and match what the lexer attaches to tokens.
/// A `(u32::MAX, 0)` entry means "no source info" — used for compiler-
/// synthesised instructions that don't correspond to any source span
/// (the trailing `PushNone+Ret` after a function body, for instance).
#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    pub files: Vec<String>,
    pub lines: Vec<(u32, u32)>,
}

impl SourceMap {
    /// Look up the (file path, line) for a given instruction. Returns
    /// `None` if either the pc is out of range or the entry's file index
    /// is the sentinel "unknown source" value.
    pub fn lookup(&self, pc: usize) -> Option<(&str, u32)> {
        let (fi, line) = *self.lines.get(pc)?;
        if fi == u32::MAX {
            return None;
        }
        let file = self.files.get(fi as usize)?;
        Some((file.as_str(), line))
    }
}

/// One bytecode instruction. Variants match the on-disk opcode table; see
/// [`OPCODES`] for the byte assignments.
#[derive(Clone, Debug)]
pub enum Instr {
    PushInt(i64),
    PushFloat(f64),
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
    FloorDiv,
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
    SetupExcept(usize),
    PopBlock,
    Raise(ErrorKind),
    /// Bind a procedure as a first-class value. At top level this stores
    /// `Closure { name, captured: ∅ }` into globals; inside a function it
    /// captures the current local environment so nested procs become real
    /// closures.
    MakeFunc(String),
    /// `alloc` declaration. Always creates a binding in the *innermost* scope
    /// (locals when inside a function, globals at top-level), without
    /// regard to whether a same-named global already exists. Distinct from
    /// `Store` so that `alloc args := ...` inside a function doesn't clobber
    /// the runtime-injected `args` global.
    StoreLocal(String),
}

mod opcode {
    pub const PUSH_INT: u8 = 0;
    pub const PUSH_STR: u8 = 1;
    pub const PUSH_BOOL: u8 = 2;
    pub const BUILD_LIST: u8 = 3;
    pub const BUILD_DICT: u8 = 4;
    pub const LOAD: u8 = 5;
    pub const STORE: u8 = 6;
    pub const ADD: u8 = 7;
    pub const SUB: u8 = 8;
    pub const MUL: u8 = 9;
    pub const DIV: u8 = 10;
    pub const MOD: u8 = 11;
    pub const EQ: u8 = 12;
    pub const NE: u8 = 13;
    pub const LT: u8 = 14;
    pub const LE: u8 = 15;
    pub const GT: u8 = 16;
    pub const GE: u8 = 17;
    pub const BAND: u8 = 18;
    pub const BOR: u8 = 19;
    pub const BXOR: u8 = 20;
    pub const SHL: u8 = 21;
    pub const SHR: u8 = 22;
    pub const AND: u8 = 23;
    pub const OR: u8 = 24;
    pub const NOT: u8 = 25;
    pub const NEG: u8 = 26;
    pub const INDEX: u8 = 27;
    pub const SLICE: u8 = 28;
    pub const JUMP: u8 = 29;
    pub const JUMP_IF_FALSE: u8 = 30;
    pub const CALL: u8 = 31;
    pub const TCALL: u8 = 32;
    pub const BUILTIN: u8 = 33;
    pub const POP: u8 = 34;
    pub const PUSH_NONE: u8 = 35;
    pub const RET: u8 = 36;
    pub const EMIT: u8 = 37;
    pub const HALT: u8 = 38;
    pub const STORE_INDEX: u8 = 39;
    pub const ATTR: u8 = 40;
    pub const STORE_ATTR: u8 = 41;
    pub const ASSERT: u8 = 42;
    pub const CALL_VALUE: u8 = 43;
    pub const SETUP_EXCEPT: u8 = 44;
    pub const POP_BLOCK: u8 = 45;
    pub const RAISE: u8 = 46;
    pub const MAKE_FUNC: u8 = 52;
    pub const STORE_LOCAL: u8 = 53;
    pub const PUSH_FLOAT: u8 = 54;
    pub const FLOOR_DIV: u8 = 55;
}

// --- Little-endian readers -------------------------------------------------

fn read_u32(data: &[u8], idx: &mut usize) -> Result<u32, RuntimeError> {
    if *idx + 4 > data.len() {
        return Err(RuntimeError::SyntaxError(
            "truncated bytecode (u32)".to_string(),
        ));
    }
    let bytes: [u8; 4] = data[*idx..*idx + 4].try_into().unwrap();
    *idx += 4;
    Ok(u32::from_le_bytes(bytes))
}

fn read_i64(data: &[u8], idx: &mut usize) -> Result<i64, RuntimeError> {
    if *idx + 8 > data.len() {
        return Err(RuntimeError::SyntaxError(
            "truncated bytecode (i64)".to_string(),
        ));
    }
    let bytes: [u8; 8] = data[*idx..*idx + 8].try_into().unwrap();
    *idx += 8;
    Ok(i64::from_le_bytes(bytes))
}

fn read_f64(data: &[u8], idx: &mut usize) -> Result<f64, RuntimeError> {
    if *idx + 8 > data.len() {
        return Err(RuntimeError::SyntaxError(
            "truncated bytecode (f64)".to_string(),
        ));
    }
    let bytes: [u8; 8] = data[*idx..*idx + 8].try_into().unwrap();
    *idx += 8;
    Ok(f64::from_le_bytes(bytes))
}

fn read_string(data: &[u8], idx: &mut usize) -> Result<String, RuntimeError> {
    let len = read_u32(data, idx)? as usize;
    if *idx + len > data.len() {
        return Err(RuntimeError::SyntaxError(
            "truncated bytecode (string)".to_string(),
        ));
    }
    let s = std::str::from_utf8(&data[*idx..*idx + len])
        .map_err(|e| RuntimeError::SyntaxError(format!("invalid UTF-8 in bytecode: {}", e)))?
        .to_string();
    *idx += len;
    Ok(s)
}

// --- Parser ---------------------------------------------------------------

/// Decode a `.omgb` byte image into instruction stream + function table
/// + source map.
pub fn parse_bytecode(
    data: &[u8],
) -> Result<(Vec<Instr>, HashMap<String, Function>, SourceMap), RuntimeError> {
    let mut idx = 0;
    if data.len() < 8 {
        return Err(RuntimeError::SyntaxError(
            "bytecode image too short".to_string(),
        ));
    }
    if &data[0..4] != b"OMGB" {
        return Err(RuntimeError::SyntaxError(
            "bad magic in bytecode header".to_string(),
        ));
    }
    idx += 4;

    let version = read_u32(data, &mut idx)?;
    if version != BC_VERSION {
        // Guide v1 users — a major .omgb format change (added source
        // map) means old files need recompiling. The error message is
        // user-facing, so call out the action they need to take rather
        // than just dumping the hex versions.
        if version & 0xFFFF_FF00 == 0x0001_0000 {
            return Err(RuntimeError::SyntaxError(
                "bytecode pre-dates source-map support (v1). Recompile \
                 your .omgb with the current toolchain."
                    .to_string(),
            ));
        }
        return Err(RuntimeError::SyntaxError(format!(
            "unsupported bytecode version 0x{:x} (expected 0x{:x})",
            version, BC_VERSION
        )));
    }

    // Source-file table.
    let file_count = read_u32(data, &mut idx)? as usize;
    let mut files: Vec<String> = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        files.push(read_string(data, &mut idx)?);
    }

    let func_count = read_u32(data, &mut idx)? as usize;
    let mut funcs: HashMap<String, Function> = HashMap::new();
    for _ in 0..func_count {
        let name = read_string(data, &mut idx)?;
        let param_count = read_u32(data, &mut idx)? as usize;
        let mut params = Vec::new();
        for _ in 0..param_count {
            params.push(read_string(data, &mut idx)?);
        }
        let address = read_u32(data, &mut idx)? as usize;
        let source_file_idx = read_u32(data, &mut idx)?;
        funcs.insert(
            name,
            Function {
                params,
                address,
                source_file_idx,
            },
        );
    }

    let code_len = read_u32(data, &mut idx)? as usize;
    let mut code = Vec::with_capacity(code_len);
    for _ in 0..code_len {
        if idx >= data.len() {
            return Err(RuntimeError::SyntaxError(
                "truncated bytecode (instruction stream)".to_string(),
            ));
        }
        let op = data[idx];
        idx += 1;
        use opcode::*;
        match op {
            PUSH_INT => code.push(Instr::PushInt(read_i64(data, &mut idx)?)),
            PUSH_STR => code.push(Instr::PushStr(read_string(data, &mut idx)?)),
            PUSH_BOOL => {
                if idx >= data.len() {
                    return Err(RuntimeError::SyntaxError(
                        "truncated bytecode (bool)".to_string(),
                    ));
                }
                let b = data[idx] != 0;
                idx += 1;
                code.push(Instr::PushBool(b));
            }
            BUILD_LIST => code.push(Instr::BuildList(read_u32(data, &mut idx)? as usize)),
            BUILD_DICT => code.push(Instr::BuildDict(read_u32(data, &mut idx)? as usize)),
            LOAD => code.push(Instr::Load(read_string(data, &mut idx)?)),
            STORE => code.push(Instr::Store(read_string(data, &mut idx)?)),
            ADD => code.push(Instr::Add),
            SUB => code.push(Instr::Sub),
            MUL => code.push(Instr::Mul),
            DIV => code.push(Instr::Div),
            MOD => code.push(Instr::Mod),
            EQ => code.push(Instr::Eq),
            NE => code.push(Instr::Ne),
            LT => code.push(Instr::Lt),
            LE => code.push(Instr::Le),
            GT => code.push(Instr::Gt),
            GE => code.push(Instr::Ge),
            BAND => code.push(Instr::BAnd),
            BOR => code.push(Instr::BOr),
            BXOR => code.push(Instr::BXor),
            SHL => code.push(Instr::Shl),
            SHR => code.push(Instr::Shr),
            AND => code.push(Instr::And),
            OR => code.push(Instr::Or),
            NOT => code.push(Instr::Not),
            NEG => code.push(Instr::Neg),
            INDEX => code.push(Instr::Index),
            SLICE => code.push(Instr::Slice),
            JUMP => code.push(Instr::Jump(read_u32(data, &mut idx)? as usize)),
            JUMP_IF_FALSE => {
                code.push(Instr::JumpIfFalse(read_u32(data, &mut idx)? as usize))
            }
            CALL => code.push(Instr::Call(read_string(data, &mut idx)?)),
            TCALL => code.push(Instr::TailCall(read_string(data, &mut idx)?)),
            BUILTIN => {
                let name = read_string(data, &mut idx)?;
                let argc = read_u32(data, &mut idx)? as usize;
                code.push(Instr::CallBuiltin(name, argc));
            }
            POP => code.push(Instr::Pop),
            PUSH_NONE => code.push(Instr::PushNone),
            RET => code.push(Instr::Ret),
            EMIT => code.push(Instr::Emit),
            HALT => code.push(Instr::Halt),
            STORE_INDEX => code.push(Instr::StoreIndex),
            ATTR => code.push(Instr::Attr(read_string(data, &mut idx)?)),
            STORE_ATTR => code.push(Instr::StoreAttr(read_string(data, &mut idx)?)),
            ASSERT => code.push(Instr::Assert),
            CALL_VALUE => code.push(Instr::CallValue(read_u32(data, &mut idx)? as usize)),
            SETUP_EXCEPT => {
                code.push(Instr::SetupExcept(read_u32(data, &mut idx)? as usize))
            }
            POP_BLOCK => code.push(Instr::PopBlock),
            RAISE => {
                if idx >= data.len() {
                    return Err(RuntimeError::SyntaxError(
                        "truncated bytecode (raise)".to_string(),
                    ));
                }
                let kind_b = data[idx];
                idx += 1;
                let kind = ErrorKind::try_from(kind_b).map_err(|_| {
                    RuntimeError::SyntaxError(format!("unknown error kind {}", kind_b))
                })?;
                code.push(Instr::Raise(kind));
            }
            MAKE_FUNC => code.push(Instr::MakeFunc(read_string(data, &mut idx)?)),
            STORE_LOCAL => code.push(Instr::StoreLocal(read_string(data, &mut idx)?)),
            PUSH_FLOAT => code.push(Instr::PushFloat(read_f64(data, &mut idx)?)),
            FLOOR_DIV => code.push(Instr::FloorDiv),
            other => {
                return Err(RuntimeError::SyntaxError(format!(
                    "unknown opcode 0x{:02x}",
                    other
                )));
            }
        }
    }

    // Source map — parallel to the instruction stream.
    let map_len = read_u32(data, &mut idx)? as usize;
    if map_len != code_len {
        return Err(RuntimeError::SyntaxError(format!(
            "source map length ({}) does not match code length ({})",
            map_len, code_len
        )));
    }
    let mut lines: Vec<(u32, u32)> = Vec::with_capacity(map_len);
    for _ in 0..map_len {
        let fi = read_u32(data, &mut idx)?;
        let ln = read_u32(data, &mut idx)?;
        lines.push((fi, ln));
    }

    Ok((code, funcs, SourceMap { files, lines }))
}

// --- Writer ---------------------------------------------------------------

fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn write_i64(out: &mut Vec<u8>, v: i64) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn write_f64(out: &mut Vec<u8>, v: f64) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn write_str(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    write_u32(out, b.len() as u32);
    out.extend_from_slice(b);
}

/// Encode a fully-compiled program back into the on-disk `.omgb` format.
///
/// Functions are emitted in **sorted name order** so the output is
/// deterministic across runs — essential for the self-hosted fixed-point
/// check (`bootstrap-fixed-point`).
pub fn write_bytecode(
    code: &[Instr],
    funcs: &HashMap<String, Function>,
    src_map: &SourceMap,
) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(64 + code.len() * 4);
    out.extend_from_slice(b"OMGB");
    write_u32(&mut out, BC_VERSION);
    // Source-file table.
    write_u32(&mut out, src_map.files.len() as u32);
    for f in &src_map.files {
        write_str(&mut out, f);
    }
    write_u32(&mut out, funcs.len() as u32);
    let mut names: Vec<&String> = funcs.keys().collect();
    names.sort();
    for name in names {
        let f = &funcs[name];
        write_str(&mut out, name);
        write_u32(&mut out, f.params.len() as u32);
        for p in &f.params {
            write_str(&mut out, p);
        }
        write_u32(&mut out, f.address as u32);
        write_u32(&mut out, f.source_file_idx);
    }
    write_u32(&mut out, code.len() as u32);
    use opcode::*;
    for instr in code {
        match instr {
            Instr::PushInt(v) => {
                out.push(PUSH_INT);
                write_i64(&mut out, *v);
            }
            Instr::PushFloat(v) => {
                out.push(PUSH_FLOAT);
                write_f64(&mut out, *v);
            }
            Instr::PushStr(s) => {
                out.push(PUSH_STR);
                write_str(&mut out, s);
            }
            Instr::PushBool(b) => {
                out.push(PUSH_BOOL);
                out.push(if *b { 1 } else { 0 });
            }
            Instr::BuildList(n) => {
                out.push(BUILD_LIST);
                write_u32(&mut out, *n as u32);
            }
            Instr::BuildDict(n) => {
                out.push(BUILD_DICT);
                write_u32(&mut out, *n as u32);
            }
            Instr::Load(s) => {
                out.push(LOAD);
                write_str(&mut out, s);
            }
            Instr::Store(s) => {
                out.push(STORE);
                write_str(&mut out, s);
            }
            Instr::Add => out.push(ADD),
            Instr::Sub => out.push(SUB),
            Instr::Mul => out.push(MUL),
            Instr::Div => out.push(DIV),
            Instr::FloorDiv => out.push(FLOOR_DIV),
            Instr::Mod => out.push(MOD),
            Instr::Eq => out.push(EQ),
            Instr::Ne => out.push(NE),
            Instr::Lt => out.push(LT),
            Instr::Le => out.push(LE),
            Instr::Gt => out.push(GT),
            Instr::Ge => out.push(GE),
            Instr::BAnd => out.push(BAND),
            Instr::BOr => out.push(BOR),
            Instr::BXor => out.push(BXOR),
            Instr::Shl => out.push(SHL),
            Instr::Shr => out.push(SHR),
            Instr::And => out.push(AND),
            Instr::Or => out.push(OR),
            Instr::Not => out.push(NOT),
            Instr::Neg => out.push(NEG),
            Instr::Index => out.push(INDEX),
            Instr::Slice => out.push(SLICE),
            Instr::Jump(t) => {
                out.push(JUMP);
                write_u32(&mut out, *t as u32);
            }
            Instr::JumpIfFalse(t) => {
                out.push(JUMP_IF_FALSE);
                write_u32(&mut out, *t as u32);
            }
            Instr::Call(s) => {
                out.push(CALL);
                write_str(&mut out, s);
            }
            Instr::TailCall(s) => {
                out.push(TCALL);
                write_str(&mut out, s);
            }
            Instr::CallBuiltin(name, argc) => {
                out.push(BUILTIN);
                write_str(&mut out, name);
                write_u32(&mut out, *argc as u32);
            }
            Instr::Pop => out.push(POP),
            Instr::PushNone => out.push(PUSH_NONE),
            Instr::Ret => out.push(RET),
            Instr::Emit => out.push(EMIT),
            Instr::Halt => out.push(HALT),
            Instr::StoreIndex => out.push(STORE_INDEX),
            Instr::Attr(s) => {
                out.push(ATTR);
                write_str(&mut out, s);
            }
            Instr::StoreAttr(s) => {
                out.push(STORE_ATTR);
                write_str(&mut out, s);
            }
            Instr::Assert => out.push(ASSERT),
            Instr::CallValue(n) => {
                out.push(CALL_VALUE);
                write_u32(&mut out, *n as u32);
            }
            Instr::SetupExcept(t) => {
                out.push(SETUP_EXCEPT);
                write_u32(&mut out, *t as u32);
            }
            Instr::PopBlock => out.push(POP_BLOCK),
            Instr::Raise(kind) => {
                out.push(RAISE);
                out.push(*kind as u8);
            }
            Instr::MakeFunc(name) => {
                out.push(MAKE_FUNC);
                write_str(&mut out, name);
            }
            Instr::StoreLocal(name) => {
                out.push(STORE_LOCAL);
                write_str(&mut out, name);
            }
        }
    }
    // Source map — parallel to the instruction stream. We emit even
    // when the map is empty or shorter than the code (e.g. a hand-
    // built program), padding with `(u32::MAX, 0)` so readers always
    // see a length matching the code.
    write_u32(&mut out, code.len() as u32);
    for i in 0..code.len() {
        let (fi, ln) = src_map.lines.get(i).copied().unwrap_or((u32::MAX, 0));
        write_u32(&mut out, fi);
        write_u32(&mut out, ln);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_minimal_program() {
        let code = vec![Instr::PushInt(7), Instr::Emit, Instr::Halt];
        let funcs = HashMap::new();
        let src_map = SourceMap::default();
        let bytes = write_bytecode(&code, &funcs, &src_map);
        let (decoded, _, _) = parse_bytecode(&bytes).unwrap();
        assert_eq!(decoded.len(), 3);
        assert!(matches!(decoded[0], Instr::PushInt(7)));
        assert!(matches!(decoded[1], Instr::Emit));
        assert!(matches!(decoded[2], Instr::Halt));
    }

    #[test]
    fn round_trips_a_source_map() {
        let code = vec![Instr::PushInt(7), Instr::Emit, Instr::Halt];
        let funcs = HashMap::new();
        let src_map = SourceMap {
            files: vec!["test.omg".to_string()],
            lines: vec![(0, 1), (0, 1), (0, 2)],
        };
        let bytes = write_bytecode(&code, &funcs, &src_map);
        let (_, _, decoded_map) = parse_bytecode(&bytes).unwrap();
        assert_eq!(decoded_map.files, vec!["test.omg".to_string()]);
        assert_eq!(decoded_map.lines, vec![(0, 1), (0, 1), (0, 2)]);
        assert_eq!(decoded_map.lookup(0), Some(("test.omg", 1)));
        assert_eq!(decoded_map.lookup(2), Some(("test.omg", 2)));
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(parse_bytecode(b"NOPE\x01\x00\x00\x00").is_err());
    }
}
