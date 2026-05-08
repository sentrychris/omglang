//! # OMG Language REPL
//!
//! Resident, in-process Read–Eval–Print loop with cross-turn state
//! preservation.
//!
//! Each line (or balanced block) is compiled as its own bytecode chunk and
//! **appended** to a persistent buffer. Function addresses produced by the
//! compiler are local to the chunk; we rebase them by the chunk's offset
//! before merging them into the persistent function table. The VM then
//! runs only the freshly appended chunk, but `Call` / `CallValue` lookups
//! happen against the full persistent table — so a call made on turn 5 to
//! a `proc` defined on turn 2 jumps to the right address.
//!
//! This is the right semantic model for an interactive REPL: state
//! (globals, defined procs, file handles) survives across turns; bytecode
//! is never re-executed; user-visible output happens exactly once.

use std::collections::HashMap;
use std::io::{self, Write};

use crate::bytecode::{Function, Instr};
use crate::compiler::compile_source_with_globals;
use crate::value::Value;
use crate::vm::{run_program_from, seed_program_globals};

/// Run the interactive REPL.
pub fn repl_interpret() {
    println!("OMG Language Interpreter - REPL");
    println!("Type `exit` or `quit` to leave.");

    let mut accum_code: Vec<Instr> = Vec::new();
    let mut funcs: HashMap<String, Function> = HashMap::new();
    let mut globals: HashMap<String, Value> = HashMap::new();
    seed_program_globals(&mut globals, &[]);

    let mut buffer: Vec<String> = Vec::new();
    let mut brace_depth: i32 = 0;
    let mut paren_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut in_string: Option<char> = None;

    loop {
        let prompt = if buffer.is_empty() { ">>> " } else { "... " };
        print!("{}", prompt);
        if io::stdout().flush().is_err() {
            return;
        }

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
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
                '(' if in_string.is_none() => paren_depth += 1,
                ')' if in_string.is_none() => paren_depth -= 1,
                '[' if in_string.is_none() => bracket_depth += 1,
                ']' if in_string.is_none() => bracket_depth -= 1,
                _ => {}
            }
        }
        buffer.push(line);
        if brace_depth > 0
            || paren_depth > 0
            || bracket_depth > 0
            || in_string.is_some()
        {
            continue;
        }

        let block: String = buffer.join("");
        let source = format!(";;;omg\n{}", block);
        // Anchor relative `import` paths to the user's actual CWD rather
        // than the temp dir; using a synthetic file in CWD makes
        // `dirname(path)` equal to CWD, which is what users expect.
        let path = std::env::current_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("<repl>");

        // Names of globals/procs declared in earlier turns. Telling the
        // compiler about them ensures `name(args)` calls resolve via
        // `Load + CallValue` (closure path) instead of `Call("name")`
        // (direct function-table lookup), which is the right semantics
        // for top-level allocs that hold closures.
        let known_globals: Vec<String> = globals.keys().cloned().collect();

        match compile_source_with_globals(&source, &path, &known_globals) {
            Ok(program) => {
                // Rebase the chunk so its jump targets and function
                // addresses point into the persistent code buffer at the
                // right place after we append.
                let base = accum_code.len();
                for instr in program.code.into_iter() {
                    accum_code.push(rebase(instr, base));
                }
                for (name, f) in program.funcs {
                    funcs.insert(
                        name,
                        Function {
                            params: f.params,
                            address: f.address + base,
                        },
                    );
                }
                if let Err(e) =
                    run_program_from(&accum_code, &funcs, &mut globals, base)
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
        paren_depth = 0;
        bracket_depth = 0;
        in_string = None;
    }
}

/// Add `base` to any instruction that carries an absolute bytecode address.
fn rebase(instr: Instr, base: usize) -> Instr {
    match instr {
        Instr::Jump(t) => Instr::Jump(t + base),
        Instr::JumpIfFalse(t) => Instr::JumpIfFalse(t + base),
        Instr::SetupExcept(t) => Instr::SetupExcept(t + base),
        other => other,
    }
}
