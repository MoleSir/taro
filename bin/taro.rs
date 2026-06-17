// Taro — a dynamically-typed scripting language.
//
// Entry point for the CLI interpreter.  Supports:
//   taro                  interactive REPL
//   taro <file>           execute a script file
//   taro -c <code>        execute code from the command line
//   taro -                execute code from stdin
//   taro -h | --help      print help
//   taro -v | --version   print version

use std::env;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use taro_lang::compile::CompileError;
use taro_lang::vm::{ExecuteError, InterpretError, VirtualMachine};

// ── constants ────────────────────────────────────────────────────────────────

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
Taro — a dynamically-typed scripting language

Usage: taro [OPTIONS] [FILE]

Options:
  -c, --command <code>   Execute Taro code string
  -e, --eval <code>      Same as --command
  -h, --help             Show this help message
  -v, --version          Show version information
  -                      Read code from standard input

With no arguments, launches the interactive REPL.
";

// ── ANSI helpers ─────────────────────────────────────────────────────────────

/// Return true when the `NO_COLOR` env-var is set (https://no-color.org/).
fn no_color() -> bool {
    env::var("NO_COLOR").is_ok_and(|v| !v.is_empty())
}

macro_rules! style {
    ($code:literal, $text:expr) => {
        if no_color() {
            format!("{}", $text)
        } else {
            format!("{}{}{}", $code, $text, "\x1b[0m")
        }
    };
}

fn bold(t: &str) -> String  { style!("\x1b[1m", t) }
fn dim(t: &str) -> String   { style!("\x1b[2m", t) }
fn red(t: &str) -> String   { style!("\x1b[31m", t) }
fn cyan(t: &str) -> String  { style!("\x1b[36m", t) }

// ── CLI arguments ────────────────────────────────────────────────────────────

enum Action {
    /// Launch the interactive REPL.
    Repl,
    /// Execute a script file.
    RunFile(String),
    /// Execute code supplied on the command line.
    RunCommand(String),
    /// Execute code read from stdin.
    RunStdin,
    /// Print help and exit.
    Help,
    /// Print version and exit.
    Version,
}

fn parse_args() -> Action {
    let mut args = env::args().skip(1); // skip program name

    let mut flag_command: Option<String> = None;

    while let Some(arg) = args.next() {
        // ---- flags that take a code argument ----
        // Compact short form: -cCODE, -eCODE (no space)
        if (arg.starts_with("-c") || arg.starts_with("-e")) && arg.len() > 2 {
            flag_command = Some(arg[2..].to_string());
            continue;
        }
        // Long form with '=': --command=CODE, --eval=CODE
        if let Some(code) = arg
            .strip_prefix("--command=")
            .or_else(|| arg.strip_prefix("--eval="))
        {
            flag_command = Some(code.to_string());
            continue;
        }

        match arg.as_str() {
            "-h" | "--help" => return Action::Help,
            "-v" | "--version" => return Action::Version,
            "-c" | "--command" | "-e" | "--eval" => {
                let code = args.next().unwrap_or_else(|| {
                    eprintln!("taro: option '{arg}' requires an argument");
                    std::process::exit(64);
                });
                flag_command = Some(code);
            }
            "-" => return Action::RunStdin,
            other => {
                if other.starts_with('-') {
                    eprintln!("taro: unknown option '{other}'");
                    eprintln!("Try 'taro --help' for more information.");
                    std::process::exit(64);
                }
                // First positional argument is the file; ignore extras.
                return Action::RunFile(other.to_string());
            }
        }
    }

    if let Some(code) = flag_command {
        Action::RunCommand(code)
    } else {
        Action::Repl
    }
}

// ── entry point ──────────────────────────────────────────────────────────────

fn main() -> ExitCode {
    match parse_args() {
        Action::Help => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Action::Version => {
            println!("taro {VERSION}");
            ExitCode::SUCCESS
        }
        Action::Repl => match run_repl() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{}: {e}", red(&bold("fatal")));
                ExitCode::FAILURE
            }
        },
        Action::RunFile(path) => match run_file(&path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(ExitError::Interpret(e)) => {
                display_error(Some(&path), &e);
                ExitCode::from(1)
            }
            Err(ExitError::Io(e)) => {
                eprintln!("{}: {e}", red(&bold("error")));
                ExitCode::from(1)
            }
        },
        Action::RunCommand(code) => match run_command(&code) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                display_error(None, &e);
                ExitCode::from(1)
            }
        },
        Action::RunStdin => match run_stdin() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                display_error(None, &e);
                ExitCode::from(1)
            }
        },
    }
}

// ── error types ──────────────────────────────────────────────────────────────

enum ExitError {
    Interpret(InterpretError),
    Io(anyhow::Error),
}

impl From<InterpretError> for ExitError {
    fn from(e: InterpretError) -> Self {
        ExitError::Interpret(e)
    }
}

impl From<anyhow::Error> for ExitError {
    fn from(e: anyhow::Error) -> Self {
        ExitError::Io(e)
    }
}

// ── error display ────────────────────────────────────────────────────────────

/// Pretty-print an interpretation error.  When a file path is provided and the
/// error carries line information we show a source-code snippet.
fn display_error(path: Option<&str>, error: &InterpretError) {
    match error {
        InterpretError::Compile(CompileError::Scan(e)) => {
            let label = path.unwrap_or("<source>");
            eprintln!("{} [{label}]: {e}", red(&bold("scan error")));
        }
        InterpretError::Compile(CompileError::Parse(errors)) => {
            for err in errors {
                let label = path.unwrap_or("<source>");
                eprintln!(
                    "{} [{label}:{}] at '{}': {}",
                    red(&bold("parse error")),
                    err.line,
                    dim(&err.lexeme),
                    red(&err.reason.to_string()),
                );
                // If a file path is available, show the offending line.
                if let Some(p) = path {
                    show_source_line(p, err.line);
                }
            }
        }
        InterpretError::Runtime(e) => {
            let label = path.unwrap_or("<source>");
            eprintln!("{} [{label}]: {e}", red(&bold("runtime error")));
        }
    }
}

/// Print one line of source with a gutter, dimmed so the user can see context.
fn show_source_line(path: &str, line: usize) {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Some(source_line) = content.lines().nth(line.saturating_sub(1)) {
            eprintln!(" {}", dim(&format!("{line:>4} │ {source_line}")));
        }
    }
}

// ── file execution ───────────────────────────────────────────────────────────

fn run_file(path: &str) -> Result<(), ExitError> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read '{path}': {e}"))?;

    let mut vm = VirtualMachine::new();
    vm.interpret(&source)?;
    Ok(())
}

// ── command execution ────────────────────────────────────────────────────────

fn run_command(code: &str) -> Result<(), InterpretError> {
    let mut vm = VirtualMachine::new();
    vm.interpret(code)
}

// ── stdin execution ──────────────────────────────────────────────────────────

fn run_stdin() -> Result<(), InterpretError> {
    let source = io::read_to_string(io::stdin()).map_err(|e| {
        InterpretError::Runtime(ExecuteError::IoError(format!(
            "failed to read stdin: {e}"
        )))
    })?;

    let mut vm = VirtualMachine::new();
    vm.interpret(&source)
}

// ── REPL ─────────────────────────────────────────────────────────────────────

/// Return the default history-file path.
fn history_path() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".taro_history")
    } else if let Ok(home) = env::var("USERPROFILE") {
        PathBuf::from(home).join(".taro_history")
    } else {
        PathBuf::from(".taro_history")
    }
}

fn run_repl() -> anyhow::Result<()> {
    let mut rl = rustyline::DefaultEditor::new()?;
    let hpath = history_path();
    let _ = rl.load_history(&hpath);

    // Print a friendly banner — but only when stdout is a terminal.
    if io::stdout().is_terminal() {
        println!(
            "Taro {VERSION} — type {} for hints, Ctrl-D to quit.",
            cyan(".help")
        );
    }

    let mut vm = VirtualMachine::new();
    let mut buffer = String::new();

    loop {
        let prompt = if buffer.is_empty() { ">> " } else { ".. " };
        match rl.readline(prompt) {
            Ok(line) => {
                buffer.push_str(&line);
                buffer.push('\n');

                // If the input looks incomplete, continue reading on the
                // next line (the prompt changes to ".. ").
                if is_incomplete(&buffer) {
                    continue;
                }

                // Add the complete multi-line input as a single history entry.
                let trimmed = buffer.trim_end().to_string();
                if !trimmed.is_empty() {
                    rl.add_history_entry(&trimmed)?;
                }

                // Save history after each successful entry.
                let _ = rl.save_history(&hpath);

                match vm.interpret(&buffer) {
                    Ok(()) => {
                        // Print the result value when non-nil so the user
                        // can inspect the last expression.
                        if let Ok(result) = vm.pop_stack() {
                            if !result.is_nil() {
                                match vm.__str__(result) {
                                    Ok(s) => println!("=> {}", s.as_str()),
                                    Err(_) => { /* best-effort */ }
                                }
                            }
                        }
                    }
                    Err(e) => display_error(None, &e),
                }

                buffer.clear();
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                // Ctrl‑C — discard the current multi-line buffer.
                println!("^C");
                buffer.clear();
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                // Ctrl‑D — quit the REPL.
                println!();
                break;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    let _ = rl.save_history(&hpath);
    Ok(())
}

// ── incomplete-input detection ───────────────────────────────────────────────

/// Heuristic that decides whether `source` looks like the user hasn't finished
/// typing yet.  Used by the REPL to decide whether to read another line instead
/// of attempting to compile.
///
/// We track:
/// * double-quoted strings (with `\\` escape handling)
/// * line comments (`//`)
/// * paired delimiters: `()`, `{}`, `[]`
fn is_incomplete(source: &str) -> bool {
    let mut parens: i32 = 0;
    let mut braces: i32 = 0;
    let mut brackets: i32 = 0;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut chars = source.chars();
    let mut prev = '\0';

    while let Some(c) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
            }
            continue;
        }

        if in_string {
            if c == '"' && prev != '\\' {
                in_string = false;
            }
            prev = c;
            continue;
        }

        match c {
            '"' => in_string = true,
            '/' => {
                // Peek ahead: "//" starts a line comment.
                if chars.as_str().starts_with('/') {
                    in_line_comment = true;
                    let _ = chars.next(); // consume the second '/'
                }
            }
            '(' => parens += 1,
            ')' => parens = (parens - 1).max(0),
            '{' => braces += 1,
            '}' => braces = (braces - 1).max(0),
            '[' => brackets += 1,
            ']' => brackets = (brackets - 1).max(0),
            _ => {}
        }
        prev = c;
    }

    in_string || parens > 0 || braces > 0 || brackets > 0
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_incomplete_empty() {
        assert!(!is_incomplete(""));
    }

    #[test]
    fn test_is_incomplete_complete_statement() {
        assert!(!is_incomplete("var x = 1;\n"));
        assert!(!is_incomplete("print(\"hello\");\n"));
    }

    #[test]
    fn test_is_incomplete_open_paren() {
        assert!(is_incomplete("print("));
        assert!(is_incomplete("print(1,"));
    }

    #[test]
    fn test_is_incomplete_closed_paren() {
        assert!(!is_incomplete("print(1)"));
    }

    #[test]
    fn test_is_incomplete_open_brace() {
        assert!(is_incomplete("if (true) {"));
    }

    #[test]
    fn test_is_incomplete_closed_brace() {
        assert!(!is_incomplete("if (true) { x = 1; }"));
    }

    #[test]
    fn test_is_incomplete_open_bracket() {
        assert!(is_incomplete("var x = [1, 2"));
    }

    #[test]
    fn test_is_incomplete_closed_bracket() {
        assert!(!is_incomplete("var x = [1, 2];"));
    }

    #[test]
    fn test_is_incomplete_unclosed_string() {
        assert!(is_incomplete("\"hello"));
    }

    #[test]
    fn test_is_incomplete_closed_string() {
        assert!(!is_incomplete("\"hello\""));
    }

    #[test]
    fn test_is_incomplete_escaped_quote_in_string() {
        // \" inside a string should NOT close the string.
        assert!(is_incomplete("\"hello \\\" world"));
        // Properly closed after escaped quote.
        assert!(!is_incomplete("\"hello \\\" world\""));
    }

    #[test]
    fn test_is_incomplete_line_comment() {
        // Line comment with trailing newline is a complete statement.
        assert!(!is_incomplete("// this is a comment\n"));
        // Line comment at EOF (no trailing newline) is also complete —
        // the scanner handles this case gracefully.
        assert!(!is_incomplete("// comment without newline"));
    }

    #[test]
    fn test_is_incomplete_mixed() {
        assert!(is_incomplete("fn { ( [ \"x"));
    }
}
