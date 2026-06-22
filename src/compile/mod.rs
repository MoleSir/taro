use crate::{ObjectHandle, ObjectHeap};
use parse::{ParseError, Parser};
use scan::{ScanError, Scanner};
pub mod parse;
pub mod scan;
pub mod token;

#[cfg(test)]
mod tests;

/// Scan `source` and compile it into a single [`Chunk`].
///
/// This is the main entry point for the compile pipeline.
pub fn compile(source: &str, obj_heap: &mut ObjectHeap) -> Result<ObjectHandle, CompileError> {
    let mut scanner = Scanner::new(source);
    let tokens = scanner.scan_tokens().map_err(|e| CompileError::Scan(e))?;

    let parser = Parser::new(tokens, obj_heap);
    let function = parser.parse().map_err(|e| CompileError::Parse(e))?;

    Ok(function)
}

/// Compile `source` as a module.  In module mode top-level definitions become
/// locals so that nested functions / class methods capture them as upvalues
/// rather than referencing globals (which would not exist after the import
/// completes).  The module function returns a dict containing all top-level
/// definitions (name → value).
pub fn compile_module(source: &str, obj_heap: &mut ObjectHeap) -> Result<ObjectHandle, CompileError> {
    let mut scanner = Scanner::new(source);
    let tokens = scanner.scan_tokens().map_err(|e| CompileError::Scan(e))?;

    let parser = Parser::new_module(tokens, obj_heap);
    let function = parser.parse().map_err(|e| CompileError::Parse(e))?;

    Ok(function)
}

#[derive(Debug)]
pub enum CompileError {
    Scan(ScanError),
    Parse(Vec<ParseError>),
}
