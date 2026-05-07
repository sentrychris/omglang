//! # OMG Language REPL
//!
//! Resident, in-process Read–Eval–Print loop. Each line is appended to a
//! buffer; once braces (and string literals) are balanced the buffer is
//! compiled and executed against a single persistent VM. State (`globals`,
//! function table, file handles) survives across turns because we never
//! leave the process.
//!
//! Replaces the previous design which spawned a child process per turn and
//! diff'd its stdout to suppress already-seen output — both layers of hack
//! are now unnecessary.

use std::collections::HashMap;
use std::io::{self, Write};

use crate::bytecode::Function;
use crate::compiler::compile_source;
use crate::value::Value;
use crate::vm::run_program;

/// Run the interactive REPL.
pub fn repl_interpret() {
    println!("OMG Language Interpreter - REPL");
    println!("Type `exit` or `quit` to leave.");

    let mut globals: HashMap<String, Value> = HashMap::new();
    let mut funcs: HashMap<String, Function> = HashMap::new();
    crate::vm::seed_program_globals(&mut globals, &[]);

    let mut buffer: Vec<String> = Vec::new();
    let mut brace_depth: i32 = 0;
    let mut in_string: Option<char> = None;

    loop {
        let prompt = if buffer.is_empty() { ">>> " } else { "... " };
        print!("{}", prompt);
        if io::stdout().flush().is_err() {
            return;
        }

        let mut line = String::new();
        let read = io::stdin().read_line(&mut line);
        match read {
            Ok(0) => {
                println!();
                return;
            }
            Ok(_) => {}
            Err(_) => return,
        }

        let trimmed = line.trim();
        if buffer.is_empty() && (trimmed == "exit" || trimmed == "quit") {
            return;
        }

        // Track brace nesting (ignoring braces inside string literals).
        let mut escape = false;
        for ch in line.chars() {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' | '\'' => {
                    if in_string == Some(ch) {
                        in_string = None;
                    } else if in_string.is_none() {
                        in_string = Some(ch);
                    }
                }
                '{' if in_string.is_none() => brace_depth += 1,
                '}' if in_string.is_none() => brace_depth -= 1,
                _ => {}
            }
        }
        buffer.push(line);
        if brace_depth > 0 || in_string.is_some() {
            continue;
        }

        let block: String = buffer.join("");
        let source = format!(";;;omg\n{}", block);
        let path = std::env::temp_dir().join("<repl>.omg");

        // Compile the snippet against a synthetic path so relative imports
        // still resolve against the user's CWD.
        match compile_source(&source, &path) {
            Ok(program) => {
                if let Err(e) =
                    run_program(&program.code, &program.funcs, &mut globals, &mut funcs)
                {
                    eprintln!("{}", e);
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("<eof>") || msg.contains("EOF") {
                    // Allow incremental input — caller is mid-statement.
                    continue;
                }
                eprintln!("{}", msg);
            }
        }

        buffer.clear();
        brace_depth = 0;
        in_string = None;
    }
}
