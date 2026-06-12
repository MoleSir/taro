use std::env;
use std::io::{self, Write};
use anyhow::Context;
use taro::compile::CompileError;
use taro::vm::{InterpretError, VirtualMachine};

fn main() {
    if let Err(e) = result_main() {
        eprintln!("{:?}", e);
        std::process::exit(1);
    }
}

fn result_main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => repl()?,
        2 => run_file(&args[1])?,
        _ => {
            eprintln!("Usage: taro [path]");
            std::process::exit(64);
        }
    }
    Ok(())
}

fn repl() -> anyhow::Result<()> {
    println!("Taro REPL — type Ctrl-D to quit.");
    let mut buffer = String::new();
    let mut vm = VirtualMachine::new();

    loop {
        print!("{}", if buffer.is_empty() {"> "} else {"· "});
        io::stdout().flush().context("stdout flush failed")?;

        let mut line = String::new();
        std::io::stdin().read_line(&mut line).context("read line failed")?;
        buffer.push_str(&line);
        if is_incomplete(&buffer) {
            continue;
        }

        if let Err(e) = vm.interpret(&buffer) {
            resport_error(&e);
        }

        buffer.clear();
    }
}

fn run_file(path: &str) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("Error reading file '{path}'"))?;

    let mut vm = VirtualMachine::new();
    if let Err(e) = vm.interpret(&source) {
        resport_error(&e);
        anyhow::bail!("interpret failed");
    }

    Ok(())
}

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

fn resport_error(e: &InterpretError) {
    match e {
        InterpretError::Compile(e) => match e {
            CompileError::Scan(e) => eprintln!("Scan error: {e}"),
            CompileError::Parse(errors) => {
                for err in errors {
                    eprintln!("[line {}] Error at '{}': {}", err.line, err.lexeme, err.reason);
                }
            }
        }
        InterpretError::Runtime(e) => eprintln!("Runtime error: {e}"),
    }
}
