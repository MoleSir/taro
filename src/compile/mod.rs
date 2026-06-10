use parse::{Parser, ParseError};
use scan::{ScanError, Scanner};
use crate::Chunk;
pub mod token;
pub mod scan;
pub mod parse;

#[cfg(test)]
mod tests;

/// Scan `source` and compile it into a single [`Chunk`].
///
/// This is the main entry point for the compile pipeline.
pub fn compile(source: &str) -> Result<Chunk, CompileError> {
    let mut scanner = Scanner::new(source);
    let tokens = scanner
        .scan_tokens()
        .map_err(|e| CompileError::Scan(e))?;

    let parser = Parser::new(tokens);
    let chunk = parser
        .parse()
        .map_err(|e| CompileError::Parse(e))?;
    
    Ok(chunk)
}

#[derive(Debug)]
pub enum CompileError {
    Scan(ScanError),
    Parse(Vec<ParseError>),
}
