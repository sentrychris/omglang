mod bytecode;
mod error;
mod repl;
mod value;
mod vm;

use bytecode::{parse_bytecode, Instr};
use vm::run;
use wasm_bindgen::prelude::*;

/// Embedded interpreter.omgb generated at build time.
const INTERP_OMGBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/interpreter.omgb"));

/// Execute an OMG source file using the embedded interpreter.
#[wasm_bindgen]
pub fn run_file(prog_path: &str) -> Result<String, JsValue> {
    let args = vec![prog_path.to_string()];
    let (code, funcs) = parse_bytecode(INTERP_OMGBC);
    let mut output = String::new();
    {
        let mut emit = |s: String| {
            output.push_str(&s);
            output.push('\n');
        };
        run(&code, &funcs, &args, &mut emit)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
    }
    Ok(output)
}

/// Execute an OMG source string using the embedded interpreter.
#[wasm_bindgen]
pub fn run_source(source: &str) -> Result<String, JsValue> {
    let (_, funcs) = parse_bytecode(INTERP_OMGBC);
    let code = vec![
        Instr::PushStr(source.to_string()),
        Instr::Call("run".to_string()),
        Instr::Halt,
    ];
    let mut output = String::new();
    {
        let mut emit = |s: String| {
            output.push_str(&s);
            output.push('\n');
        };
        run(&code, &funcs, &[], &mut emit)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
    }
    Ok(output)
}
