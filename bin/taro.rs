use std::env;
use std::fs;
use std::io::{self, Write};

use taro::compile::{compile, CompileError};
use taro::execute::VirtualMachine;
use taro::Chunk;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => repl(),
        2 => run_file(&args[1]),
        _ => {
            eprintln!("Usage: taro [path]");
            std::process::exit(64);
        }
    }
}

// ========================================================================== //
//  REPL
// ========================================================================== //

fn repl() {
    println!("Taro REPL — type Ctrl-D to quit.");
    let mut buffer = String::new();

    // Keep a single VM so that global variables survive across lines.
    let mut vm = VirtualMachine::new(Chunk::new());

    loop {
        // Prompt
        if buffer.is_empty() {
            print!("> ");
        } else {
            print!("· ");
        }
        io::stdout().flush().unwrap();

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => {
                // EOF
                println!();
                break;
            }
            Ok(_) => {
                buffer.push_str(&line);

                // Keep reading if braces / parens / strings are unbalanced.
                if is_incomplete(&buffer) {
                    continue;
                }

                match compile(&buffer) {
                    Ok(chunk) => {
                        // Swap in the new chunk and reset execution state
                        // (but keep globals from previous lines).
                        vm.chunk = chunk;
                        vm.ip = 0;
                        vm.stack.clear();
                        if let Err(e) = vm.run() {
                            eprintln!("Runtime error: {e}");
                        }
                    }
                    Err(e) => report_compile_error(&e),
                }

                buffer.clear();
            }
            Err(e) => {
                eprintln!("Error reading input: {e}");
                break;
            }
        }
    }
}

// ========================================================================== //
//  File runner
// ========================================================================== //

fn run_file(path: &str) {
    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading file '{path}': {e}");
        std::process::exit(74);
    });

    let chunk = compile(&source).unwrap_or_else(|e| {
        report_compile_error(&e);
        std::process::exit(65);
    });

    let mut vm = VirtualMachine::new(chunk);
    if let Err(e) = vm.run() {
        eprintln!("Runtime error: {e}");
        std::process::exit(70);
    }
}

// ========================================================================== //
//  Helpers
// ========================================================================== //

/// Heuristic to detect incomplete multi-line input.
///
/// Returns `true` when there are unclosed `(`, `{`, or an unterminated
/// string literal, suggesting the user hasn't finished typing yet.
fn is_incomplete(source: &str) -> bool {
    let mut parens: i32 = 0;
    let mut braces: i32 = 0;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut chars = source.chars().peekable();

    while let Some(c) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
            }
            continue;
        }

        if in_string {
            if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => in_string = true,
            '/' => {
                if chars.peek() == Some(&'/') {
                    in_line_comment = true;
                    chars.next();
                }
            }
            '(' => parens += 1,
            ')' => parens = (parens - 1).max(0),
            '{' => braces += 1,
            '}' => braces = (braces - 1).max(0),
            _ => {}
        }
    }

    in_string || parens > 0 || braces > 0
}

fn report_compile_error(e: &CompileError) {
    match e {
        CompileError::Scan(e) => eprintln!("Scan error: {e}"),
        CompileError::Parse(errors) => {
            for err in errors {
                eprintln!("[line {}] Error at '{}': {}", err.line, err.lexeme, err.reason);
            }
        }
    }
}
