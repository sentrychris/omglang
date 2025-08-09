use std::collections::HashMap;

const BC_VERSION: u32 = (0 << 16) | (1 << 8) | 1;

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

fn read_u32(data: &[u8], idx: &mut usize) -> u32 {
    let bytes: [u8; 4] = data[*idx..*idx + 4].try_into().unwrap();
    *idx += 4;
    u32::from_le_bytes(bytes)
}

fn read_i64(data: &[u8], idx: &mut usize) -> i64 {
    let bytes: [u8; 8] = data[*idx..*idx + 8].try_into().unwrap();
    *idx += 8;
    i64::from_le_bytes(bytes)
}

fn read_string(data: &[u8], idx: &mut usize) -> String {
    let len = read_u32(data, idx) as usize;
    let s = String::from_utf8(data[*idx..*idx + len].to_vec()).unwrap();
    *idx += len;
    s
}

/// Parse binary bytecode into instructions and function table.
pub fn parse_bytecode(data: &[u8]) -> (Vec<Instr>, HashMap<String, Function>) {
    let mut idx = 0;
    assert!(&data[0..4] == b"OMGB");
    idx += 4;
    let version = read_u32(data, &mut idx);
    assert_eq!(version, BC_VERSION, "unsupported version");
    let func_count = read_u32(data, &mut idx) as usize;
    let mut funcs: HashMap<String, Function> = HashMap::new();
    for _ in 0..func_count {
        let name = read_string(data, &mut idx);
        let param_count = read_u32(data, &mut idx) as usize;
        let mut params = Vec::new();
        for _ in 0..param_count {
            params.push(read_string(data, &mut idx));
        }
        let address = read_u32(data, &mut idx) as usize;
        funcs.insert(name.clone(), Function { params, address });
    }

    let code_len = read_u32(data, &mut idx) as usize;
    let mut code = Vec::with_capacity(code_len);
    for _ in 0..code_len {
        let op = data[idx];
        idx += 1;
        match op {
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
            27 => code.push(Instr::Index),
            28 => code.push(Instr::Slice),
            29 => {
                let t = read_u32(data, &mut idx) as usize;
                code.push(Instr::Jump(t));
            }
            30 => {
                let t = read_u32(data, &mut idx) as usize;
                code.push(Instr::JumpIfFalse(t));
            }
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
            34 => code.push(Instr::Pop),
            35 => code.push(Instr::PushNone),
            36 => code.push(Instr::Ret),
            37 => code.push(Instr::Emit),
            38 => code.push(Instr::Halt),
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
            43 => {
                let n = read_u32(data, &mut idx) as usize;
                code.push(Instr::CallValue(n));
            }
            _ => {}
        }
    }
    (code, funcs)
}

