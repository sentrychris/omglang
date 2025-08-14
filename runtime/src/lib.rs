mod bytecode;
mod error;
mod repl;
mod value;
mod vm;

use bytecode::{parse_bytecode, Function, Instr};
use wasm_bindgen::prelude::*;
use vm::run;

/// Embedded `interpreter.omgb` generated at build time.
const INTERP_OMGBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/interpreter.omgb"));

/// Run bytecode with the given arguments and collect any text emitted by the program.
fn exec(
    code: Vec<Instr>,
    funcs: std::collections::HashMap<String, Function>,
    args: &[String],
) -> Result<String, JsValue> {
    let mut output = String::new();
    {
        let mut emit = |s: String| {
            output.push_str(&s);
            output.push('\n');
        };
        run(&code, &funcs, args, &mut emit)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
    }
    Ok(output)
}

/// Execute an OMG source file by path using the embedded interpreter.
#[wasm_bindgen]
pub fn run_file(prog_path: &str) -> Result<String, JsValue> {
    let args = vec![prog_path.to_string()];
    let (code, funcs) = parse_bytecode(INTERP_OMGBC);
    exec(code, funcs, &args)
}

/// Execute an OMG source string using the embedded interpreter.
///
/// This splices the interpreter's initialization code in front of a small
/// program that pushes the provided source and calls its `run` procedure.
#[wasm_bindgen]
pub fn run_source(source: &str) -> Result<String, JsValue> {
    let (mut program, funcs) = parse_bytecode(INTERP_OMGBC);

    // Drop the interpreter's final HALT so we can append our own instructions.
    if matches!(program.last(), Some(Instr::Halt)) {
        program.pop();
    }

    // Invoke `run(source)` and halt.
    program.push(Instr::PushStr(source.to_string()));
    program.push(Instr::Call("run".to_string()));
    program.push(Instr::Halt);

    exec(program, funcs, &[])
}

