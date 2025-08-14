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

/// Execute interpreter bytecode with extra instructions appended at the end.
fn exec_with<F>(args: &[String], build: F) -> Result<String, JsValue>
where
    F: FnOnce(&mut Vec<Instr>),
{
    let (mut code, funcs) = parse_bytecode(INTERP_OMGBC);
    if let Some(Instr::Halt) = code.last() {
        code.pop();
    }
    build(&mut code);
    code.push(Instr::Halt);

    let mut output = String::new();
    {
        let mut emit = |s: String| {
            output.push_str(&s);
            output.push('\n');
        };
        run(&code, &funcs, args, &mut emit)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    }
    Ok(output)
}

/// Execute an OMG source file using the embedded interpreter.
#[wasm_bindgen]
pub fn run_file(prog_path: &str) -> Result<String, JsValue> {
    let path = prog_path.to_string();
    let args = vec![path.clone()];
    exec_with(&args, move |code| {
        code.push(Instr::PushStr(path));
        code.push(Instr::Call("run_file".to_string()));
    })
}

/// Execute an OMG source string using the embedded interpreter.
#[wasm_bindgen]
pub fn run_source(source: &str) -> Result<String, JsValue> {
    let src = source.to_string();
    exec_with(&[], move |code| {
        code.push(Instr::PushStr(src));
        code.push(Instr::Call("run".to_string()));
    })
}
