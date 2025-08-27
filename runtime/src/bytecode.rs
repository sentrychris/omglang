//! # OMG Bytecode Format & Decoder
//!
//! This module defines the **instruction set**, **function metadata**,
//! and a **binary parser** for OMG bytecode. It turns a raw byte slice
//! into an instruction stream (`Vec<Instr>`) plus a function table
//! (`HashMap<String, Function>`), which the runtime VM executes.
//!
//! ## Binary layout (little-endian)
//! ```text
//! +------------------+----------------------------+
//! | Magic "OMGB"     | 4 bytes                    |
//! +------------------+----------------------------+
//! | Version          | u32 (see `BC_VERSION`)     |
//! +------------------+----------------------------+
//! | Func count       | u32                        |
//! +------------------+----------------------------+
//! | For each func:                                |
//! |   Name          | u32 len + UTF-8 bytes       |
//! |   Param count   | u32                         |
//! |   Params[...]   | (Param count times)         |
//! |                 |   u32 len + UTF-8 bytes     |
//! |   Address       | u32 (index into code vec)   |
//! +------------------+----------------------------+
//! | Code length      | u32 (number of instrs)     |
//! +------------------+----------------------------+
//! | For each instr:                               |
//! |   Opcode         | u8                         |
//! |   Operands       | opcode-specific payload    |
//! +------------------+----------------------------+
//! ```
//!
//! The parser is intentionally strict about the header and version and will
//! `assert!` on mismatches. It uses `unwrap()` in a few places because the
//! input is expected to be well-formed compiler output. Feeding arbitrary or
//! corrupted data is undefined behavior.
//!
//! ## Versioning
//! `BC_VERSION` follows a packed `(MAJOR << 16) | (MINOR << 8) | PATCH` layout.
//! The parser requires an exact match for simplicity.
//!
//! ## Functions
//! A `Function` records its parameter list and the address (PC) of its first
//! instruction within the decoded `code` vector. Calls jump to `address`.

use std::collections::HashMap;

use crate::error::ErrorKind;

/// Packed bytecode version: `(MAJOR << 16) | (MINOR << 8) | PATCH`.
const BC_VERSION: u32 = (0 << 16) | (1 << 8) | 1;

/// Representation of a compiled function.
///
/// - `params`: ordered list of parameter names.
/// - `address`: instruction index (PC) of the function entry point within
///   the decoded `code` vector returned by [`parse_bytecode`].
#[derive(Clone)]
pub struct Function {
    pub params: Vec<String>,
    pub address: usize,
}

/// Instruction set for the OMG stack VM.
///
/// Each variant matches a concrete opcode in the on-disk format (see
/// the `match op` table in [`parse_bytecode`]). Payload-bearing
/// instructions carry their decoded operands here.
#[derive(Clone)]
pub enum Instr {
    // ----- Constants / literals -----
    PushInt(i64),
    PushStr(String),
    PushBool(bool),
    // ----- Aggregate construction -----
    /// Build list from the top `n` stack values (in-order as specified by the encoder).
    BuildList(usize),
    /// Build dictionary from the top `n` *pairs* (key, value) on the stack.
    BuildDict(usize),
    // ----- Variables -----
    /// Load variable by name (local first, then global).
    Load(String),
    /// Store to an existing name or define per runtime rules (local/global).
    Store(String),
    // ----- Arithmetic / comparison / bitwise / boolean -----
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
    // ----- Indexing / slicing / attributes -----
    /// `container[index]`
    Index,
    /// `container[start:end]`
    Slice,
    /// Unconditional jump to absolute instruction index.
    Jump(usize),
    /// Pop condition; if falsey, jump to target.
    JumpIfFalse(usize),
    /// Call named function (user-defined).
    Call(String),
    /// Tail-call named function (frame reuse when possible).
    TailCall(String),
    /// Call builtin by name with fixed arity.
    CallBuiltin(String, usize),
    /// Pop/discard the top of stack.
    Pop,
    /// Push `None` sentinel.
    PushNone,
    /// Return from current function.
    Ret,
    /// Print/emit top-of-stack (side effect).
    Emit,
    /// Stop execution (advance PC beyond code).
    Halt,
    /// `container[index] = value`
    StoreIndex,
    /// Read attribute `attr` from an object/dict-like value.
    Attr(String),
    /// Write attribute `attr` on an object/dict-like value.
    StoreAttr(String),
    /// Runtime assertion (error on falsey).
    Assert,
    /// Call using a first-class callable on the stack; arity given here.
    CallValue(usize),
    // ----- Structured exception handling -----
    /// Establish an exception handler targeting instruction `usize`.
    SetupExcept(usize),
    /// Pop the most recent exception handler.
    PopBlock,
    /// Synthesize/raise a runtime error of the given kind.
    Raise(ErrorKind),
}

//
// --- Little-endian readers --------------------------------------------------
//

/// Read a `u32` (little-endian) and advance `idx`.
fn read_u32(data: &[u8], idx: &mut usize) -> u32 {
    let bytes: [u8; 4] = data[*idx..*idx + 4].try_into().unwrap();
    *idx += 4;
    u32::from_le_bytes(bytes)
}

/// Read an `i64` (little-endian) and advance `idx`.
fn read_i64(data: &[u8], idx: &mut usize) -> i64 {
    let bytes: [u8; 8] = data[*idx..*idx + 8].try_into().unwrap();
    *idx += 8;
    i64::from_le_bytes(bytes)
}

/// Read a length-prefixed UTF-8 `String` and advance `idx`.
///
/// Layout: `u32 len` followed by `len` raw bytes (UTF-8).
fn read_string(data: &[u8], idx: &mut usize) -> String {
    let len = read_u32(data, idx) as usize;
    let s = String::from_utf8(data[*idx..*idx + len].to_vec()).unwrap();
    *idx += len;
    s
}

//
// --- Parser ---------------------------------------------------------------
//

/// Parse binary bytecode into a linear instruction stream and a function table.
///
/// This performs a single forward pass, verifying the magic header and exact
/// version (`BC_VERSION`). The returned tuple is:
///
/// - `code`: `Vec<Instr>` that the VM executes with `pc` as an index
/// - `funcs`: mapping from function name → [`Function`] metadata
///
/// ## Panics
/// - If `data` is malformed: bad magic/version, truncated payloads, invalid
///   UTF-8, or unknown `ErrorKind` discriminants used by `Raise`
///
/// ## Notes
/// - The opcode-to-variant mapping is defined inline in the `match op` table.
/// - Op payloads are read *immediately* after the opcode in the specified order.
/// - For forward compatibility, unknown opcodes currently get ignored (no push),
///   but the index still advances past the opcode itself. In practice the encoder
///   should never emit unknown opcodes for a matching version.
pub fn parse_bytecode(data: &[u8]) -> (Vec<Instr>, HashMap<String, Function>) {
    let mut idx = 0;

    // ---- Header ----
    assert!(&data[0..4] == b"OMGB");
    idx += 4;

    // Version check; reject incompatible bytecode.
    let version = read_u32(data, &mut idx);
    assert_eq!(version, BC_VERSION, "unsupported version");

    // ---- Function table ----
    let func_count = read_u32(data, &mut idx) as usize;
    let mut funcs: HashMap<String, Function> = HashMap::new();

    for _ in 0..func_count {
        // Function name
        let name = read_string(data, &mut idx);
        // Formal parameters
        let param_count = read_u32(data, &mut idx) as usize;
        let mut params = Vec::new();
        for _ in 0..param_count {
            params.push(read_string(data, &mut idx));
        }

        // Entry-point address into the forthcoming code vector
        let address = read_u32(data, &mut idx) as usize;
        funcs.insert(name.clone(), Function { params, address });
    }

    // ---- Code stream ----
    let code_len = read_u32(data, &mut idx) as usize;
    let mut code = Vec::with_capacity(code_len);
    for _ in 0..code_len {
        // Single-byte opcode selector
        let op = data[idx];
        idx += 1;
        // Decode one instruction based on opcode; consume any operands.
        match op {
            // 0..6: constants / variables
            0 => {
                let v = read_i64(data, &mut idx);
                code.push(Instr::PushInt(v));
            }
            1 => {
                let s = read_string(data, &mut idx);
                code.push(Instr::PushStr(s));
            }
            2 => {
                let b = data[idx] != 0;
                idx += 1;
                code.push(Instr::PushBool(b));
            }
            3 => {
                let n = read_u32(data, &mut idx) as usize;
                code.push(Instr::BuildList(n));
            }
            4 => {
                let n = read_u32(data, &mut idx) as usize;
                code.push(Instr::BuildDict(n));
            }
            5 => {
                let s = read_string(data, &mut idx);
                code.push(Instr::Load(s));
            }
            6 => {
                let s = read_string(data, &mut idx);
                code.push(Instr::Store(s));
            }
            // 7..26: arithmetic / comparison / bitwise / boolean / unary
            7 => code.push(Instr::Add),
            8 => code.push(Instr::Sub),
            9 => code.push(Instr::Mul),
            10 => code.push(Instr::Div),
            11 => code.push(Instr::Mod),
            12 => code.push(Instr::Eq),
            13 => code.push(Instr::Ne),
            14 => code.push(Instr::Lt),
            15 => code.push(Instr::Le),
            16 => code.push(Instr::Gt),
            17 => code.push(Instr::Ge),
            18 => code.push(Instr::BAnd),
            19 => code.push(Instr::BOr),
            20 => code.push(Instr::BXor),
            21 => code.push(Instr::Shl),
            22 => code.push(Instr::Shr),
            23 => code.push(Instr::And),
            24 => code.push(Instr::Or),
            25 => code.push(Instr::Not),
            26 => code.push(Instr::Neg),
            // 27..28: indexing / slicing
            27 => code.push(Instr::Index),
            28 => code.push(Instr::Slice),
            // 29..30: branches
            29 => {
                let t = read_u32(data, &mut idx) as usize;
                code.push(Instr::Jump(t));
            }
            30 => {
                let t = read_u32(data, &mut idx) as usize;
                code.push(Instr::JumpIfFalse(t));
            }
            // 31..33: calls (named, tail, builtin)
            31 => {
                let s = read_string(data, &mut idx);
                code.push(Instr::Call(s));
            }
            32 => {
                let s = read_string(data, &mut idx);
                code.push(Instr::TailCall(s));
            }
            33 => {
                let name = read_string(data, &mut idx);
                let argc = read_u32(data, &mut idx) as usize;
                code.push(Instr::CallBuiltin(name, argc));
            }
            // 34..38: misc control
            34 => code.push(Instr::Pop),
            35 => code.push(Instr::PushNone),
            36 => code.push(Instr::Ret),
            37 => code.push(Instr::Emit),
            38 => code.push(Instr::Halt),
            // 39..42: stores/attrs/assert
            39 => code.push(Instr::StoreIndex),
            40 => {
                let s = read_string(data, &mut idx);
                code.push(Instr::Attr(s));
            }
            41 => {
                let s = read_string(data, &mut idx);
                code.push(Instr::StoreAttr(s));
            }
            42 => code.push(Instr::Assert),
            // 43: first-class callable invoke (argc inline)
            43 => {
                let n = read_u32(data, &mut idx) as usize;
                code.push(Instr::CallValue(n));
            }
            // 44..46: exception scaffolding and dynamic raise
            44 => {
                let t = read_u32(data, &mut idx) as usize;
                code.push(Instr::SetupExcept(t));
            }
            45 => code.push(Instr::PopBlock),
            46 => {
                let kind_b = data[idx];
                idx += 1;
                let kind = ErrorKind::try_from(kind_b).unwrap();
                code.push(Instr::Raise(kind));
            }
            // 47..51: short opcodes for specific error kinds
            47 => code.push(Instr::Raise(ErrorKind::Syntax)),
            48 => code.push(Instr::Raise(ErrorKind::Type)),
            49 => code.push(Instr::Raise(ErrorKind::UndefinedIdent)),
            50 => code.push(Instr::Raise(ErrorKind::Value)),
            51 => code.push(Instr::Raise(ErrorKind::ModuleImport)),
            // Unknown opcode: no-op decode (advance already consumed 1 byte).
            _ => {}
        }
    }
    (code, funcs)
}
