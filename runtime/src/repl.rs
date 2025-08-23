//! # OMG Language REPL
//!
//! This module implements an **interactive Read–Eval–Print Loop (REPL)** for
//! the OMG language. It allows users to type OMG code line by line, evaluate
//! it immediately, and see results.
//!
//! ## Design
//! - Provides prompts (`>>>` for fresh input, `...` for continuation).
//! - Tracks **brace depth** so users can enter multi-line blocks (e.g., function
//!   definitions, conditionals) before execution.
//! - Preserves **command history** so new input can build upon previously
//!   defined variables and functions.
//! - Executes code by writing it to a temporary `.omg` file and re-invoking the
//!   current binary with that file. This ensures consistency between REPL and
//!   script execution.
//! - Supports graceful exit with `exit` or `quit`.
//!
//! ## Limitations
//! - Because execution is performed by spawning a new process, performance is
//!   lower than a native in-process interpreter loop.
//! - Output diffing (`last_output`) is used to only print new results between
//!   iterations, preventing repeated display of old output.

use std::fs;
use std::io::{self, Write};
use std::process::Command;

/// Run an interactive REPL for the OMG language.
///
/// The loop:
/// 1. Prints a prompt.
/// 2. Reads a line of user input.
/// 3. If braces are balanced and the user isn’t inside a string, executes the
///    accumulated block.
/// 4. Displays new output while suppressing repeated history.
/// 5. Resets buffers for the next iteration.
///
/// Exits cleanly on EOF (Ctrl+D) or if the user types `exit`/`quit`.
pub fn repl_interpret() {
    println!("OMG Language Interpreter - REPL");
    println!("Type `exit` or `quit` to leave.");

    // Running history of successfully executed code (preserved across turns).
    let mut history = String::new();
    // Tracks the full stdout of the last run so we can diff and only print new lines.
    let mut last_output = String::new();
    // Buffer for building a multi-line input block (if braces are unbalanced).
    let mut buffer: Vec<String> = Vec::new();
    // Current open-brace depth, ignoring those inside string literals.
    let mut brace_depth: i32 = 0;

    loop {
        // Choose primary (>>> ) or continuation (... ) prompt.
        let prompt = if buffer.is_empty() { ">>> " } else { "... " };
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut line = String::new();
        // EOF (Ctrl+D) → exit gracefully.
        if io::stdin().read_line(&mut line).unwrap() == 0 {
            println!();
            break;
        }

        let trimmed = line.trim();
        // Allow "exit" or "quit" as explicit exit commands (only at fresh prompt).
        if buffer.is_empty() && (trimmed == "exit" || trimmed == "quit") {
            break;
        }

        // --- Track braces (for multiline input) ----------------------------
        // We scan the line while respecting string literals and escapes.
        let mut string_char: Option<char> = None; // '"' or '\'', if inside string
        let mut escape = false;
        for ch in line.chars() {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' | '\'' => {
                    if string_char == Some(ch) {
                        string_char = None; // close string
                    } else if string_char.is_none() {
                        string_char = Some(ch); // open string
                    }
                }
                '{' if string_char.is_none() => brace_depth += 1,
                '}' if string_char.is_none() => brace_depth -= 1,
                _ => {}
            }
        }

        buffer.push(line);

        // If braces are still open, wait for more input before executing.
        if brace_depth > 0 {
            continue;
        }

        // --- Execution path ------------------------------------------------
        let block = buffer.join("");
        // Combine prior history with the current block into one program.
        let source = format!(";;;omg\n{}{}", history, block);

        // Write to a temporary `.omg` file.
        let temp_path = std::env::temp_dir().join("omg_repl.omg");
        if fs::write(&temp_path, &source).is_err() {
            println!("failed to write temp file");
            buffer.clear();
            brace_depth = 0;
            continue;
        }

        // Spawn a child process of the current binary, running the temp script.
        let output = Command::new(std::env::current_exe().unwrap())
            .arg(temp_path.to_string_lossy().to_string())
            .output();

        // Clean up the temp file after execution.
        let _ = fs::remove_file(&temp_path);

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if !stderr.is_empty() {
                    // If parse error complains about unexpected EOF, allow more input.
                    if stderr.contains("EOF") {
                        continue;
                    } else {
                        // Otherwise print error and reset buffer.
                        print!("{}", stderr);
                        buffer.clear();
                        brace_depth = 0;
                        continue;
                    }
                }

                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                // Diff new stdout against the previous run, only print new content.
                if stdout.starts_with(&last_output) {
                    print!("{}", &stdout[last_output.len()..]);
                } else {
                    print!("{}", stdout);
                }

                last_output = stdout;
                // Accumulate successful block into history so state persists.
                history.push_str(&block);
                buffer.clear();
                brace_depth = 0;
            }
            Err(_) => {
                println!("failed to run script");
                buffer.clear();
                brace_depth = 0;
            }
        }
    }
}
