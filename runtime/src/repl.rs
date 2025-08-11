use std::fs;
use std::io::{self, Write};
use std::process::Command;

/// Run an interactive REPL for the OMG language.
pub fn repl_interpret() {
    println!("OMG Language Interpreter - REPL");
    println!("Type `exit` or `quit` to leave.");
    let mut history = String::new();
    let mut last_output = String::new();
    let mut buffer: Vec<String> = Vec::new();
    let mut brace_depth: i32 = 0;
    loop {
        let prompt = if buffer.is_empty() { ">>> " } else { "... " };
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).unwrap() == 0 {
            println!();
            break;
        }

        let trimmed = line.trim();
        if buffer.is_empty() && (trimmed == "exit" || trimmed == "quit") {
            break;
        }

        // Track nested braces to allow multiline input before execution.
        let mut string_char: Option<char> = None;
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
                        string_char = None;
                    } else if string_char.is_none() {
                        string_char = Some(ch);
                    }
                }
                '{' if string_char.is_none() => brace_depth += 1,
                '}' if string_char.is_none() => brace_depth -= 1,
                _ => {}
            }
        }

        buffer.push(line);
        if brace_depth > 0 {
            continue;
        }

        let block = buffer.join("");
        let source = format!(";;;omg\n{}{}", history, block);
        let temp_path = std::env::temp_dir().join("omg_repl.omg");
        if fs::write(&temp_path, &source).is_err() {
            println!("failed to write temp file");
            buffer.clear();
            brace_depth = 0;
            continue;
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .arg(temp_path.to_string_lossy().to_string())
            .output();
        let _ = fs::remove_file(&temp_path);
        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if !stderr.is_empty() {
                    if stderr.contains("EOF") {
                        continue;
                    } else {
                        print!("{}", stderr);
                        buffer.clear();
                        brace_depth = 0;
                        continue;
                    }
                }

                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                if stdout.starts_with(&last_output) {
                    print!("{}", &stdout[last_output.len()..]);
                } else {
                    print!("{}", stdout);
                }

                last_output = stdout;
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
