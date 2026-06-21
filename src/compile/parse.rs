use std::collections::HashSet;

use super::token::{Token, TokenKind};
use crate::{Chunk, Instruction, ObjectHandle, ObjectHeap, ShrString, UpvalueDesc};

// ========================================================================== //
//                    Precedence
// ========================================================================== //

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
    Call,
    Primary,
}

impl Prec {
    fn next(self) -> Self {
        match self {
            Prec::None       => Prec::Assignment,
            Prec::Assignment => Prec::Or,
            Prec::Or         => Prec::And,
            Prec::And        => Prec::Equality,
            Prec::Equality   => Prec::Comparison,
            Prec::Comparison => Prec::Term,
            Prec::Term       => Prec::Factor,
            Prec::Factor     => Prec::Unary,
            Prec::Unary      => Prec::Call,
            Prec::Call       => Prec::Primary,
            Prec::Primary    => Prec::Primary,
        }
    }
}

// ========================================================================== //
//                    Parse Rule table
// ========================================================================== //

type ParseFn = fn(&mut Parser<'_>, bool) -> ParseResult<()>;

#[derive(Clone, Copy)]
struct ParseRule {
    prefix:     Option<ParseFn>,
    infix:      Option<ParseFn>,
    precedence: Prec,
}

impl ParseRule {
    pub const fn new(prefix: Option<ParseFn>, infix: Option<ParseFn>, precedence: Prec) -> ParseRule {
        Self { prefix, infix, precedence }
    }

    const NONE: Self = Self {
        prefix:     None,
        infix:      None,
        precedence: Prec::None,
    };
}

/// Return the [`ParseRule`] for the given token kind.
fn get_rule(kind: TokenKind) -> ParseRule {
    match kind {
        // Single-character tokens -------------------------------------------
        TokenKind::LeftParen    => ParseRule::new(Some(Parser::grouping), Some(Parser::call), Prec::Call),
        TokenKind::RightParen   => ParseRule::NONE,
        TokenKind::LeftBrace    => ParseRule::new(Some(Parser::dict_literal), None, Prec::Call),
        TokenKind::RightBrace   => ParseRule::NONE,
        TokenKind::LeftBracket  => ParseRule::new(Some(Parser::list_literal), Some(Parser::index), Prec::Call),
        TokenKind::RightBracket => ParseRule::NONE,
        TokenKind::Comma        => ParseRule::NONE,
        TokenKind::Colon        => ParseRule::NONE,
        TokenKind::Dot          => ParseRule::new(None, Some(Parser::dot), Prec::Call),
        TokenKind::Minus        => ParseRule::new(Some(Parser::unary), Some(Parser::binary), Prec::Term),
        TokenKind::Plus         => ParseRule::new(None, Some(Parser::binary), Prec::Term),
        TokenKind::Semicolon    => ParseRule::NONE,
        TokenKind::Slash        => ParseRule::new(None, Some(Parser::binary), Prec::Factor),
        TokenKind::Star         => ParseRule::new(None, Some(Parser::binary), Prec::Factor),
        TokenKind::Percent      => ParseRule::new(None, Some(Parser::binary), Prec::Factor),
        TokenKind::TildeSlash   => ParseRule::new(None, Some(Parser::binary), Prec::Factor),

        // One- or two-character tokens --------------------------------------
        TokenKind::Bang         => ParseRule::new(Some(Parser::unary), None, Prec::None),
        TokenKind::BangEqual    => ParseRule::new(None, Some(Parser::binary), Prec::Equality),
        TokenKind::Equal        => ParseRule::new(None, None, Prec::None),
        TokenKind::EqualEqual   => ParseRule::new(None, Some(Parser::binary), Prec::Equality),
        TokenKind::Greater      => ParseRule::new(None, Some(Parser::binary), Prec::Comparison),
        TokenKind::GreaterEqual => ParseRule::new(None, Some(Parser::binary), Prec::Comparison),
        TokenKind::Less         => ParseRule::new(None, Some(Parser::binary), Prec::Comparison),
        TokenKind::LessEqual    => ParseRule::new(None, Some(Parser::binary), Prec::Comparison),

        // Literals ----------------------------------------------------------
        TokenKind::Identifier   => ParseRule::new(Some(Parser::variable), None, Prec::None),
        TokenKind::String       => ParseRule::new(Some(Parser::string), None, Prec::None),
        TokenKind::Number       => ParseRule::new(Some(Parser::number), None, Prec::None),

        // Keywords ----------------------------------------------------------
        TokenKind::And          => ParseRule::new(None, Some(Parser::and), Prec::And),
        TokenKind::As           => ParseRule::NONE,
        TokenKind::Break        => ParseRule::NONE,
        TokenKind::Class        => ParseRule::NONE,
        TokenKind::Continue     => ParseRule::NONE,
        TokenKind::Else         => ParseRule::NONE,
        TokenKind::Extends      => ParseRule::NONE,
        TokenKind::False        => ParseRule::new(Some(Parser::literal), None, Prec::None),
        TokenKind::For          => ParseRule::NONE,
        TokenKind::Fun          => ParseRule::NONE,
        TokenKind::If           => ParseRule::NONE,
        TokenKind::In           => ParseRule::NONE,
        TokenKind::Import       => ParseRule::new(Some(Parser::import_expr), None, Prec::Call),
        TokenKind::Nil          => ParseRule::new(Some(Parser::literal), None, Prec::None),
        TokenKind::Or           => ParseRule::new(None, Some(Parser::or), Prec::Or),
        TokenKind::Return       => ParseRule::NONE,
        TokenKind::Super        => ParseRule::new(Some(Parser::super_), None, Prec::None),
        TokenKind::True         => ParseRule::new(Some(Parser::literal), None, Prec::None),
        TokenKind::Var          => ParseRule::NONE,
        TokenKind::While        => ParseRule::NONE,

        // Special -----------------------------------------------------------
        TokenKind::Error        => ParseRule::NONE,
        TokenKind::Eof          => ParseRule::NONE,
    }
}

// ========================================================================== //
//                    Parser
// ========================================================================== //

#[derive(Clone)]
pub struct Local {
    name: ShrString,
    /// Stack slot depth. `-1` means the variable is declared but not yet
    /// initialized (sentinel — prevents referencing in its own initializer).
    depth: isize,
    /// True when an inner function captures this local as an upvalue.
    is_captured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionKind {
    /// Regular function — the closure lives at stack slot 0 (placed by
    /// `OP_CALL`), so we reserve slot 0 with a dummy local.
    Function,
    /// Method — the receiver (`self`) lives at stack slot 0 (placed by
    /// `OP_INVOKE` or bound-method call).  No dummy needed because the
    /// user-declared `self` parameter naturally occupies slot 0.
    Method,
    /// Top-level script — same layout as `Function`.
    Script,
    /// Module — like Function but top-level definitions use locals so nested
    /// functions capture them as upvalues.  The function returns a fields
    /// instance containing all top-level definitions.
    Module,
}

struct CompilationUnit {
    name:        ShrString,
    arity:       usize,
    /// Number of parameters *without* a default value.
    required_arity: usize,
    /// Names of all parameters in declaration order.
    param_names: Vec<ShrString>,
    /// Default values for the last `arity - required_arity` parameters.
    defaults:    Vec<ObjectHandle>,
    chunk:       Chunk,
    kind:        FunctionKind,
    locals:      Vec<Local>,
    scope_depth: isize,
    upvalues:    Vec<UpvalueDesc>,
    /// Index of the enclosing unit in `Parser::units`, or `self` for the
    /// root (so we can use `enclosing == current_unit` as a sentinel).
    enclosing:   usize,
}

pub struct Parser<'a> {
    obj_heap:     &'a mut ObjectHeap,
    tokens:       Vec<Token<'a>>,
    current:      usize,
    errors:       Vec<ParseError>,
    units:        Vec<CompilationUnit>,
    /// Index into `units` for the innermost function being compiled.
    current_unit: usize,
    /// Stack of active loop contexts.  Non-empty when parsing the body of a
    /// `while` or `for` loop — `break` / `continue` consult the top-most entry.
    loop_stack:   Vec<LoopContext>,
    /// Names of global variables that have been explicitly declared with
    /// `var`, `fun`, `class`, or `import` at the top level.  Used to reject
    /// assignments to undeclared names.
    declared_globals: HashSet<ShrString>,
}

/// Result of resolving a variable name to a local slot or upvalue.
enum LocalAccess {
    Local(usize),
    Upvalue(usize),
}

/// Tracks state for the innermost enclosing loop, so that `break` and
/// `continue` statements inside the loop body know where to jump.
struct LoopContext {
    /// Bytecode position to jump to on `continue`.
    /// - `while`: position of the condition expression.
    /// - `for` with increment: position of the increment expression.
    /// - `for` without increment: position of the condition (or body start).
    continue_target: usize,
    /// Bytecode addresses of `Jump(0)` placeholder operands emitted for
    /// `break` statements.  Patched after the loop body is fully compiled.
    break_patches: Vec<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseErrorKind {
    #[error("invalid float literal")]
    InvalidFloat(#[from] std::num::ParseFloatError),

    #[error("invalid integer literal")]
    InvalidInteger(#[from] std::num::ParseIntError),

    #[error("expected expression")]
    ExpectedExpression,

    #[error("expected token '{0}'")]
    ExpectedToken(&'static str),

    #[error("invalid assignment target")]
    InvalidAssignmentTarget,

    #[error("Already a variable {0} in this scope.")]
    VariableRedefine(String),

    #[error("Too much code {0} to jump over.")]
    TooMuchCodeToJumpOver(usize),

    #[error("Can't have more than 255 parameters.")]
    TooMuchParameter,

    #[error("Can't have more than 255 arguments.")]
    TooMuchArgument,

    #[error("Can't have more than 255 items.")]
    TooMuchItems,

    #[error("Can't return from top-level code.")]
    ReturnInTop,

    #[error("'break' outside of loop")]
    BreakOutsideLoop,

    #[error("'continue' outside of loop")]
    ContinueOutsideLoop,

    #[error("invalid escape sequence '\\{0}'")]
    InvalidEscape(char),

    #[error("required parameter after optional parameter")]
    RequiredAfterOptional,

    #[error("positional argument after keyword argument")]
    PositionalAfterKeyword,

    #[error("duplicate keyword argument '{0}'")]
    DuplicateKeywordArg(String),

    #[error("invalid default value — only constant literals (numbers, strings, bools, nil) are supported")]
    InvalidDefaultValue,

    #[error("undefined variable '{0}' — use 'var' to declare it first")]
    UndefinedVariable(String),
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub lexeme: String,
    pub kind: ParseErrorKind,
}

pub type ParseResult<T> = std::result::Result<T, ParseError>;

macro_rules! error_at_current {
    ($p:ident, $kind:expr) => {{
        let token = $p.peek();
        ParseError { line: token.line, column: token.column, lexeme: token.lexeme.to_string(), kind: $kind }
    }};
}

macro_rules! bail_error_at_current {
    ($p:ident, $reason:expr) => {{
        Err(error_at_current!($p, $reason))?
    }};
}

macro_rules! record_error_at_current {
    ($p:ident, $kind:expr) => {{
        $p.errors.push(error_at_current!($p, $kind));
    }};
}

#[allow(unused)]
macro_rules! error_at_previous {
    ($p:ident, $kind:expr) => {{
        let token = $p.previous();
        ParseError { line: token.line, column: token.column, lexeme: token.lexeme.to_string(), kind: $kind }
    }};
}

macro_rules! bail_error_at_previous {
    ($p:ident, $reason:expr) => {{
        Err(error_at_previous!($p, $reason))?
    }};
}

#[allow(unused)]
macro_rules! record_error_at_previous {
    ($p:ident, $reason:expr) => {{
        $p.errors.push(error_at_previous!($p, $reason));
    }};
}

impl CompilationUnit {
    fn new(_obj_heap: &mut ObjectHeap, name: impl Into<ShrString>, kind: FunctionKind, enclosing: usize) -> Self {
        let name: ShrString = name.into();
        // For regular functions and the top-level script, stack slot 0 holds
        // the closure (placed by `OP_CALL`).  We reserve it with a dummy
        // entry so the first user-declared local / parameter starts at slot 1.
        // For methods, slot 0 holds the receiver (`self`), which is the first
        // explicit parameter — no dummy needed.
        let locals = if kind == FunctionKind::Method {
            vec![]
        } else {
            vec![Local { depth: 0, name: "".into(), is_captured: false }]
        };
        // Modules start at scope depth 1 so top-level definitions use locals
        // (capturable by nested functions as upvalues) instead of globals.
        let scope_depth = if kind == FunctionKind::Module { 1 } else { 0 };
        Self {
            name: name.clone(),
            arity: 0,
            required_arity: 0,
            param_names: vec![],
            defaults: vec![],
            chunk: Chunk::new(),
            kind,
            locals,
            scope_depth,
            upvalues: vec![],
            enclosing,
        }
    }

    /// Finish compilation and create the function object in the heap.
    fn finish(self, obj_heap: &mut ObjectHeap) -> ObjectHandle {
        obj_heap.alloc_function(
            self.name,
            self.arity,
            self.required_arity,
            self.param_names,
            self.defaults,
            self.chunk,
        )
    }
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>, obj_heap: &'a mut ObjectHeap) -> Self {
        let unit = CompilationUnit::new(obj_heap, "", FunctionKind::Script, 0);
        Self {
            obj_heap,
            tokens,
            current: 0,
            errors: vec![],
            units: vec![unit],
            current_unit: 0,
            loop_stack: vec![],
            declared_globals: HashSet::new(),
        }
    }

    pub fn new_module(tokens: Vec<Token<'a>>, obj_heap: &'a mut ObjectHeap) -> Self {
        let unit = CompilationUnit::new(obj_heap, "__module__", FunctionKind::Module, 0);
        Self {
            obj_heap,
            tokens,
            current: 0,
            errors: vec![],
            units: vec![unit],
            current_unit: 0,
            loop_stack: vec![],
            declared_globals: HashSet::new(),
        }
    }

    // ------------------------------------------------------------------------
    //  Helpers for the current compilation unit
    // ------------------------------------------------------------------------

    #[inline]
    fn cur_unit(&self) -> &CompilationUnit {
        &self.units[self.current_unit]
    }

    #[inline]
    fn cur_unit_mut(&mut self) -> &mut CompilationUnit {
        &mut self.units[self.current_unit]
    }

    // ------------------------------------------------------------------------
    //  Public entry point
    // ------------------------------------------------------------------------

    pub(crate) fn parse(mut self) -> Result<ObjectHandle, Vec<ParseError>> {
        while !self.at_end() {
            if let Err(e) = self.parse_declaration() {
                self.synchronize(e);
            }
        }

        if !self.errors.is_empty() {
            Err(self.errors)
        } else {
            Ok(self.finish_compilation_unit())
        }
    }

    /// Finish the current compilation unit: emit an implicit `return nil`,
    /// pop the unit from the stack, and restore the enclosing unit.
    fn finish_compilation_unit(&mut self) -> ObjectHandle {
        let is_init = self.cur_unit().name.as_str() == "__init__";
        let is_module = self.cur_unit().kind == FunctionKind::Module;

        if is_init {
            // __init__() should return the receiver (self), not nil.
            self.emit(Instruction::GetLocal(0));
            self.emit(Instruction::Return);
        } else if is_module {
            // Build an exports dict from the module's top-level locals.
            // The Return instruction that follows will close upvalues for any
            // locals captured by nested functions / class methods.
            let num_locals = self.cur_unit().locals.len();
            let mut export_count: usize = 0;
            for i in 1..num_locals {
                let local = &self.cur_unit().locals[i];
                if local.name.as_str().is_empty() {
                    continue; // skip dummy closure slot
                }
                let name_handle = self.obj_heap.alloc_string_instance(local.name.clone());
                self.emit(Instruction::Constant(name_handle));
                self.emit(Instruction::GetLocal(i));
                export_count += 1;
            }
            self.emit(Instruction::BuildDict(export_count));
            self.emit(Instruction::Return);
        } else {
            self.emit(Instruction::Nil);
            self.emit(Instruction::Return);
        }
        let unit = self.units.pop().expect("at least the root unit");
        self.current_unit = unit.enclosing;
        let function = unit.finish(self.obj_heap);
        function
    }

    fn parse_declaration(&mut self) -> ParseResult<()> {
        if self.match_token(TokenKind::Var) {
            self.parse_var_declaration()
        } else if self.match_token(TokenKind::Fun) {
            self.parse_fun_declaration()
        } else if self.match_token(TokenKind::Class) {
            self.parse_class_declaration()
        } else if self.match_token(TokenKind::Import) {
            self.parse_import_declaration()
        } else {
            self.parse_statement()
        }
    }

    fn parse_class_declaration(&mut self) -> ParseResult<()> {
        self.consume(TokenKind::Identifier, "Expect class name.")?;
        let class_name = ShrString::new_string(self.previous().lexeme);
        self.add_variable_to_scope(class_name.clone())?;

        self.emit(Instruction::Class(class_name.clone()));
        self.finalize_variable(Some(class_name.clone()))?;

        self.resolve_and_emit_variable(class_name, false)?;

        // Inheritance: class Derived extends Base { ... }
        if self.match_token(TokenKind::Extends) {
            self.consume(TokenKind::Identifier, "Expect superclass name.")?;
            let super_name = ShrString::new_string(self.previous().lexeme);
            self.resolve_and_emit_variable(super_name, false)?; // push superclass onto stack
            self.emit(Instruction::Inherit);          // pop superclass, copy methods
        }

        self.consume(TokenKind::LeftBrace, "Expect '{' before class body.")?;

        while !self.check(TokenKind::RightBrace) && !self.check(TokenKind::Eof) {
            self.parse_method()?;
        }

        self.consume(TokenKind::RightBrace, "Expect '}' after class body.")?;
        self.emit(Instruction::Pop);
        Ok(())
    }

    fn parse_import_declaration(&mut self) -> ParseResult<()> {
        self.consume(TokenKind::String, "Expect string literal after 'import'.")?;
        let path = self.previous().lexeme;
        let inner = &path[1..path.len() - 1]; // strip quotes

        // Determine the module name: either from `as <name>` or derived from
        // the file path (basename without extension).
        let module_name = if self.match_token(TokenKind::As) {
            self.consume(TokenKind::Identifier, "Expect module name after 'as'.")?;
            self.previous().lexeme.to_string()
        } else {
            Self::derive_module_name(inner)
        };

        // Emit import instruction (pushes module object onto stack).
        self.emit(Instruction::Import(ShrString::new_string(inner)));

        // In module/function scope, store in a local so nested functions and
        // class methods can capture the import as an upvalue.  Otherwise
        // define a global variable (script scope).
        if self.cur_unit().scope_depth > 0 {
            self.add_variable_to_scope(ShrString::new_string(module_name.clone()))?;
            self.mark_local_initialized();
        } else {
            let name = ShrString::new_string(module_name.clone());
            self.declared_globals.insert(name.clone());
            self.emit(Instruction::DefineGlobal(name));
        }

        self.consume(TokenKind::Semicolon, "Expect ';' after import.")?;
        Ok(())
    }

    /// Derive a module name from a file path.
    /// e.g. "path/to/math.taro" → "math", "utils" → "utils"
    fn derive_module_name(path: &str) -> String {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string()
    }

    fn parse_fun_declaration(&mut self) -> ParseResult<()> {
        let var_name = self.declare_variable_name("Expect variable name.")?;
        self.mark_local_initialized();
        self.parse_function_body(FunctionKind::Function)?;
        self.finalize_variable(var_name)?;
        Ok(())
    }

    fn parse_var_declaration(&mut self) -> ParseResult<()> {
        let var_name = self.declare_variable_name("Expect variable name.")?;

        if self.match_token(TokenKind::Equal) {
            self.parse_expression()?;
        } else {
            self.emit(Instruction::Nil);
        }

        self.consume(TokenKind::Semicolon, "Expect ';' after variable declaration.")?;
        self.finalize_variable(var_name)?;

        Ok(())
    }

    fn finalize_variable(&mut self, var_name: Option<ShrString>) -> ParseResult<()> {
        if self.cur_unit().scope_depth > 0 {
            self.mark_local_initialized();
        } else {
            let name = var_name.unwrap();
            self.declared_globals.insert(name.clone());
            self.emit(Instruction::DefineGlobal(name));
        }
        Ok(())
    }

    fn parse_statement(&mut self) -> ParseResult<()> {
        if self.match_token(TokenKind::If) {
            self.parse_if_statement()
        } else if self.match_token(TokenKind::While) {
            self.parse_while_statement()
        } else if self.match_token(TokenKind::For) {
            self.parse_for_statement()
        } else if self.match_token(TokenKind::Return) {
            self.parse_return_statement()
        } else if self.match_token(TokenKind::LeftBrace) {
            self.begin_scope();
            self.parse_block()?;
            self.end_scope();
            Ok(())
        } else if self.match_token(TokenKind::Break) {
            self.parse_break_statement()
        } else if self.match_token(TokenKind::Continue) {
            self.parse_continue_statement()
        } else {
            self.parse_expression_statement()
        }
    }

    fn parse_method(&mut self) -> ParseResult<()> {
        self.consume(TokenKind::Fun, "Expect fun in method")?;
        self.consume(TokenKind::Identifier, "Expect method name.")?;

        let method_name = ShrString::new_string(self.previous().lexeme);

        self.parse_function_body(FunctionKind::Method)?;

        self.emit(Instruction::Method(method_name));

        Ok(())
    }

    fn parse_function_body(&mut self, kind: FunctionKind) -> ParseResult<()> {
        let name = if kind != FunctionKind::Script {
            self.previous().lexeme.to_string()
        } else {
            String::new()
        };

        // Save the current loop stack — functions cannot see enclosing loops.
        // break/continue inside a nested function should always error.
        let saved_loop_stack = std::mem::take(&mut self.loop_stack);

        // Push a new compilation unit for the nested function.
        let enclosing = self.current_unit;
        self.current_unit = self.units.len();
        self.units.push(CompilationUnit::new(
            &mut self.obj_heap, name, kind, enclosing,
        ));

        self.begin_scope();

        self.consume(TokenKind::LeftParen, "Expect '(' after function name.")?;
        if !self.check(TokenKind::RightParen) {
            loop {
                self.cur_unit_mut().arity += 1;
                if self.cur_unit().arity > 255 {
                    record_error_at_current!(self, ParseErrorKind::TooMuchParameter);
                }

                // Parse parameter name.
                self.consume(TokenKind::Identifier, "Expect parameter name.")?;
                let param_name_str = ShrString::new_string(self.previous().lexeme);
                self.add_variable_to_scope(param_name_str.clone())?;
                self.cur_unit_mut().param_names.push(param_name_str);

                // Check for default value: `param = literal`.
                if self.match_token(TokenKind::Equal) {
                    let default_handle = self.parse_default_value()?;
                    self.cur_unit_mut().defaults.push(default_handle);
                    // required_arity stays as-is — this parameter is optional.
                } else {
                    // No default — required parameter.
                    if !self.cur_unit().defaults.is_empty() {
                        record_error_at_current!(self, ParseErrorKind::RequiredAfterOptional);
                    }
                    self.cur_unit_mut().required_arity += 1;
                }

                // Finalize: mark local initialized or define global.
                if self.cur_unit().scope_depth > 0 {
                    self.mark_local_initialized();
                } else {
                    let n = ShrString::new_string(self.previous().lexeme);
                    self.emit(Instruction::DefineGlobal(n));
                }

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }

        self.consume(TokenKind::RightParen, "Expect ')' after parameters.")?;
        self.consume(TokenKind::LeftBrace, "Expect '{' before function body.")?;
        self.parse_block()?;

        // Finish the nested function and pop its unit.
        let upvalues = self.cur_unit().upvalues.clone();
        let inner_function = self.finish_compilation_unit();
        self.emit(Instruction::Closure { function: inner_function, upvalues });

        // Restore the enclosing function's loop stack.
        self.loop_stack = saved_loop_stack;

        Ok(())
    }

    fn parse_block(&mut self) -> ParseResult<()> {
        while !self.check(TokenKind::RightBrace) && !self.check(TokenKind::Eof) {
            self.parse_declaration()?;
        }
        self.consume(TokenKind::RightBrace, "Expect '}' after block.")?;
        Ok(())
    }

    fn begin_scope(&mut self) {
        self.cur_unit_mut().scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.cur_unit_mut().scope_depth -= 1;
        let scope_depth = self.cur_unit().scope_depth;
        while self.cur_unit().locals.len() > 0
            && self.cur_unit().locals.last().unwrap().depth > scope_depth
        {
            if self.cur_unit().locals.last().unwrap().is_captured {
                self.emit(Instruction::CloseUpvalue);
            } else {
                self.emit(Instruction::Pop);
            }
            self.cur_unit_mut().locals.pop();
        }
    }

    fn parse_return_statement(&mut self) -> ParseResult<()> {
        if self.cur_unit().kind == FunctionKind::Script {
            record_error_at_current!(self, ParseErrorKind::ReturnInTop);
        }

        if self.match_token(TokenKind::Semicolon) {
            self.emit_return_nil();
        } else {
            self.parse_expression()?;
            self.consume(TokenKind::Semicolon, "Expect ';' after return value.")?;
            self.emit(Instruction::Return);

        }
        Ok(())
    }

    fn parse_break_statement(&mut self) -> ParseResult<()> {
        if self.loop_stack.is_empty() {
            bail_error_at_previous!(self, ParseErrorKind::BreakOutsideLoop);
        }

        // Emit a forward Jump(0) placeholder.  The offset will be patched
        // once the loop body is fully compiled and we know the exit position.
        let jump_addr = self.emit_jump(false); // Jump(0)
        self.loop_stack.last_mut().unwrap().break_patches.push(jump_addr);

        self.consume(TokenKind::Semicolon, "Expect ';' after 'break'.")?;
        Ok(())
    }

    fn parse_continue_statement(&mut self) -> ParseResult<()> {
        if self.loop_stack.is_empty() {
            bail_error_at_previous!(self, ParseErrorKind::ContinueOutsideLoop);
        }

        // Jump back to the loop's continue target (condition or increment).
        let target = self.loop_stack.last().unwrap().continue_target;
        let offset = self.cur_unit().chunk.codes.len() - target + 3;
        if offset > u16::MAX as usize {
            record_error_at_current!(self, ParseErrorKind::TooMuchCodeToJumpOver(offset));
        }
        self.emit(Instruction::Loop(offset));

        self.consume(TokenKind::Semicolon, "Expect ';' after 'continue'.")?;
        Ok(())
    }

    fn parse_for_statement(&mut self) -> ParseResult<()> {
        // Distinguish C-style `for (...)` from `for x in iterable`.
        if !self.check(TokenKind::LeftParen) {
            return self.parse_for_in();
        }

        self.begin_scope();

        self.consume(TokenKind::LeftParen, "Expect '(' after 'for'.")?;
        // initializer
        if self.match_token(TokenKind::Semicolon) {
            // No initializer.
        } else if self.match_token(TokenKind::Var) {
            self.parse_var_declaration()?;
        } else {
            self.parse_expression_statement()?;
        }

        let mut loop_start = self.cur_unit().chunk.codes.len();

        // loop condition
        let mut exit_jump_opt = None;
        if !self.match_token(TokenKind::Semicolon) {
            self.parse_expression()?;
            self.consume(TokenKind::Semicolon, "Expect ';' after loop condition.")?;

            // Jump out of the loop if the condition is false.
            exit_jump_opt = Some(self.emit_jump(true));
            self.emit(Instruction::Pop);
        }

        // increment clause
        if !self.match_token(TokenKind::RightParen) {
            // exit increment clause, we must jump here, each time body done
            let body_jump = self.emit_jump(false);
            let increment_start = self.cur_unit().chunk.codes.len();
            self.parse_expression()?;
            self.emit(Instruction::Pop);
            self.consume(TokenKind::RightParen, "Expect ')' after for clauses.")?;

            self.emit_loop(loop_start)?;
            loop_start = increment_start;
            self.patch_jump(body_jump)?;
        }

        // Push loop context so break/continue inside the body know where to jump.
        // At this point `loop_start` already equals `increment_start` when an
        // increment clause exists (see the `loop_start = increment_start` above),
        // or the condition / body-start position otherwise.
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            break_patches: Vec::new(),
        });

        // Require braces for the body.
        self.consume(TokenKind::LeftBrace, "Expect '{' after for clauses.")?;
        self.begin_scope();
        self.parse_block()?;
        self.end_scope();

        let ctx = self.loop_stack.pop().unwrap();

        self.emit_loop(loop_start)?;

        // exit a jump condition, fill it jump out addr
        if let Some(exit_jump) = exit_jump_opt {
            self.patch_jump(exit_jump)?;
            self.emit(Instruction::Pop);
        }

        self.end_scope();

        // Patch all break-jump placeholders to jump to the current position
        // (just past the end_scope cleanup and exit Pop).
        for &jump_addr in &ctx.break_patches {
            self.patch_jump(jump_addr)?;
        }

        Ok(())
    }

    /// `for <identifier> in <expression> <statement>`
    fn parse_for_in(&mut self) -> ParseResult<()> {
        // Loop variable name.
        self.consume(TokenKind::Identifier, "Expect loop variable name.")?;
        let var_name = ShrString::new_string(self.previous().lexeme);

        self.consume(TokenKind::In, "Expect 'in' after loop variable.")?;

        // Evaluate the iterable and call __iter__.  The iterator object
        // pushed by ForInIter becomes the first new local.
        self.parse_expression()?;                // stack: [iterable]
        self.emit(Instruction::ForInIter);       // stack: [iterator]

        // Open a scope for the two locals.  Slot 0 is reserved for the
        // closure (`CompilationUnit::new`), so the iterator gets the next
        // available index.
        self.begin_scope();

        let iterator_slot = self.cur_unit().locals.len();
        self.add_local("__iter__".into())?;
        self.mark_local_initialized();

        // Loop variable.  Push Nil as initial value.
        self.emit(Instruction::Nil);
        let var_slot = self.cur_unit().locals.len();
        self.add_local(var_name)?;
        self.mark_local_initialized();

        // ---- loop header ----
        let loop_start = self.cur_unit().chunk.codes.len();

        // Push iterator copy, call __next__.
        self.emit(Instruction::GetLocal(iterator_slot));
        let exit_jump = self.emit_for_in_next(); // ForInNext(0) placeholder

        // Store element into the loop variable, then discard the two
        // temporary copies (ForInNext push + GetLocal copy).
        self.emit(Instruction::SetLocal(var_slot));
        self.emit(Instruction::Pop);             // discard ForInNext push
        self.emit(Instruction::Pop);             // discard GetLocal copy

        // ---- loop body ----
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            break_patches: Vec::new(),
        });

        // Require braces for the body.
        self.consume(TokenKind::LeftBrace, "Expect '{' after 'in' expression.")?;
        self.begin_scope();
        self.parse_block()?;
        self.end_scope();

        let ctx = self.loop_stack.pop().unwrap();

        self.emit_loop(loop_start)?;

        // ---- exit (IterEnd path) ----
        self.patch_jump(exit_jump)?;
        // The GetLocal copy is still on the stack.
        self.emit(Instruction::Pop);

        self.end_scope();                        // pops both locals

        // Patch all break-jump placeholders.
        for &jump_addr in &ctx.break_patches {
            self.patch_jump(jump_addr)?;
        }

        Ok(())
    }

    /// Emit `ForInNext(0)` and return the address of the jump-offset field
    /// that must be patched later.
    fn emit_for_in_next(&mut self) -> usize {
        self.emit(Instruction::ForInNext(0));
        self.cur_unit().chunk.codes.len() - 2
    }

    fn parse_while_statement(&mut self) -> ParseResult<()> {
        let loop_start = self.cur_unit().chunk.codes.len();

        // Parse condition expression (no parentheses — Rust‑style).
        self.parse_expression()?;

        // Require braces for the body.
        self.consume(TokenKind::LeftBrace, "Expect '{' after while condition.")?;

        let exit_jump = self.emit_jump(true);
        self.emit(Instruction::Pop);

        // Push loop context so break/continue inside the body know where to jump.
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            break_patches: Vec::new(),
        });

        // Parse body block.
        self.begin_scope();
        self.parse_block()?;
        self.end_scope();

        let ctx = self.loop_stack.pop().unwrap();

        self.emit_loop(loop_start)?;

        self.patch_jump(exit_jump)?;
        self.emit(Instruction::Pop);

        // Patch all break-jump placeholders to jump to just after the loop's
        // exit Pop — i.e. right past the entire loop.
        for &jump_addr in &ctx.break_patches {
            self.patch_jump(jump_addr)?;
        }

        Ok(())
    }

    fn parse_if_statement(&mut self) -> ParseResult<()> {
        // Parse condition expression (no parentheses — Rust‑style).
        self.parse_expression()?;

        // Require braces for the then-branch.
        self.consume(TokenKind::LeftBrace, "Expect '{' after if condition.")?;

        // Jump to else / end if the condition is false.
        let then_jump = self.emit_jump(true);
        self.emit(Instruction::Pop);

        // Parse then-block.
        self.begin_scope();
        self.parse_block()?;
        self.end_scope();

        // Jump past the else-branch after executing the then-block.
        let else_jump = self.emit_jump(false);

        // Patch the then-jump: when the condition is false, jump here.
        self.patch_jump(then_jump)?;
        self.emit(Instruction::Pop);

        // else / else-if chain.
        if self.match_token(TokenKind::Else) {
            if self.match_token(TokenKind::If) {
                // `else if` — recurse without requiring extra braces.
                self.parse_if_statement()?;
            } else {
                self.consume(TokenKind::LeftBrace, "Expect '{' after else.")?;
                self.begin_scope();
                self.parse_block()?;
                self.end_scope();
            }
        }
        self.patch_jump(else_jump)?;

        Ok(())
    }

    fn emit_return_nil(&mut self) {
        self.emit(Instruction::Nil);
        self.emit(Instruction::Return);
    }

    fn emit_loop(&mut self, loop_start: usize) -> ParseResult<()> {
        let offset = self.cur_unit().chunk.codes.len() - loop_start + 3;
        if offset > u16::MAX as usize {
            record_error_at_current!(self, ParseErrorKind::TooMuchCodeToJumpOver(offset));
        }

        self.emit(Instruction::Loop(offset));
        Ok(())
    }

    fn emit_jump(&mut self, if_false: bool) -> usize {
        if if_false {
            self.emit(Instruction::JumpIfFalse(0));
        } else {
            self.emit(Instruction::Jump(0));
        }
        self.cur_unit().chunk.codes.len() - 2
    }

    fn patch_jump(&mut self, jump_addr: usize) -> ParseResult<()> {
        // distance of if and cur
        let offset = self.cur_unit().chunk.codes.len() - jump_addr - 2;
        if offset > u16::MAX as usize {
            record_error_at_current!(self, ParseErrorKind::TooMuchCodeToJumpOver(offset));
        }

        let bytes = (offset as u16).to_le_bytes();
        assert!(jump_addr + 1 < self.cur_unit().chunk.codes.len());
        self.cur_unit_mut().chunk.codes[jump_addr] = bytes[0];
        self.cur_unit_mut().chunk.codes[jump_addr+1] = bytes[1];
        Ok(())
    }

    fn parse_expression_statement(&mut self) -> ParseResult<()> {
        self.parse_expression()?;
        self.consume(TokenKind::Semicolon, "expect ';' after expression.")?;
        self.emit(Instruction::Pop);
        Ok(())
    }

    pub(crate) fn parse_expression(&mut self) -> ParseResult<()> {
        self.parse_precedence(Prec::Assignment)
    }

    fn parse_precedence(&mut self, precedence: Prec) -> ParseResult<()> {
        self.advance();
        let prefix_rule = get_rule(self.previous().kind).prefix;

        let Some(prefix_fn) = prefix_rule else {
            bail_error_at_previous!(self, ParseErrorKind::ExpectedExpression)
        };

        let can_assign = precedence <= Prec::Assignment;
        prefix_fn(self, can_assign)?;

        loop {
            let next_rule = get_rule(self.peek().kind);
            if precedence > next_rule.precedence || next_rule.infix.is_none() {
                break;
            }
            self.advance();

            // SAFETY: we just checked infix.is_some() above
            if let Some(infix_fn) = next_rule.infix {
                infix_fn(self, can_assign)?;
            }
        }

        if can_assign && self.check(TokenKind::Equal) {
            bail_error_at_current!(self, ParseErrorKind::InvalidAssignmentTarget);
        }

        Ok(())
    }

    fn declare_variable_name(&mut self, msg: &'static str) -> ParseResult<Option<ShrString>> {
        self.consume(TokenKind::Identifier, msg)?;
        let var_name = ShrString::new_string(self.previous().lexeme);

        self.add_variable_to_scope(var_name.clone())?;

        if self.cur_unit().scope_depth > 0 {
            Ok(None)
        } else {
            Ok(Some(var_name))
        }
    }

    fn add_variable_to_scope(&mut self, var_name: ShrString) -> ParseResult<()> {
        if self.cur_unit().scope_depth == 0 {
            return Ok(());
        }

        let scope_depth = self.cur_unit().scope_depth;
        for local in self.cur_unit().locals.iter().rev() {
            if local.depth != -1 && local.depth < scope_depth {
                break;
            }
            if var_name == local.name {
                bail_error_at_previous!(self, ParseErrorKind::VariableRedefine(var_name.to_string()));
            }
        }
        self.add_local(var_name)?;
        Ok(())
    }

    fn add_local(&mut self, name: ShrString) -> ParseResult<()> {
        let local = Local { name, depth: -1, is_captured: false };
        self.cur_unit_mut().locals.push(local);
        Ok(())
    }

    /// Mark the most recently declared local as ready for use.
    fn mark_local_initialized(&mut self) {
        let scope_depth = self.cur_unit().scope_depth;
        if scope_depth == 0 {
            return;
        }
        if let Some(last) = self.cur_unit_mut().locals.last_mut() {
            last.depth = scope_depth;
        }
    }

    // ========================================================================== //
    //                    Parse functions
    // ========================================================================== //

    fn and(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        let then_jump = parser.emit_jump(true);

        parser.emit(Instruction::Pop);
        parser.parse_precedence(Prec::And)?;

        parser.patch_jump(then_jump)?;

        Ok(())
    }

    fn or(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        let else_jump = parser.emit_jump(true);
        let end_jump = parser.emit_jump(false);

        parser.patch_jump(else_jump)?;
        parser.emit(Instruction::Pop);

        parser.parse_precedence(Prec::Or)?;
        parser.patch_jump(end_jump)?;

        Ok(())
    }

    /// `number` — prefix parser for numeric literals.
    fn number(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        let lexeme = parser.previous().lexeme;
        if lexeme.contains('.') {
            let value: f64 = lexeme
                .parse()
                .map_err(|e|
                    error_at_previous!(parser, ParseErrorKind::InvalidFloat(e))
                )?;
            let handle = parser.obj_heap.alloc_float_instance(value);
            parser.emit(Instruction::Constant(handle));
        } else {
            let value: i64 = lexeme
                .parse()
                .map_err(|e|
                    error_at_previous!(parser, ParseErrorKind::InvalidInteger(e))
                )?;
            let handle = parser.obj_heap.alloc_integer_instance(value);
            parser.emit(Instruction::Constant(handle));
        }
        Ok(())
    }

    /// `string` — prefix parser for string literals.
    fn string(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        // The lexeme includes the surrounding quotes — strip them.
        let prev = parser.previous();
        let lexeme = prev.lexeme;
        let inner = &lexeme[1..lexeme.len() - 1];
        let unescaped = unescape_string(inner)
            .map_err(|c| ParseError {
                line: prev.line,
                column: prev.column,
                lexeme: lexeme.to_string(),
                kind: ParseErrorKind::InvalidEscape(c),
            })?;
        let handle = parser
            .obj_heap
            .alloc_string_instance(unescaped.into());
        parser.emit(Instruction::Constant(handle));
        Ok(())
    }

    /// `literal` — prefix parser for `true`, `false`, `nil`.
    fn literal(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        match parser.previous().kind {
            TokenKind::True  => parser.emit(Instruction::True),
            TokenKind::False => parser.emit(Instruction::False),
            TokenKind::Nil   => parser.emit(Instruction::Nil),
            _ => unreachable!("literal() called for non-literal token"),
        }
        Ok(())
    }

    /// `grouping` — prefix parser for `(` ... `)`.
    fn grouping(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        parser.parse_expression()?;
        parser.consume(TokenKind::RightParen, "expect ')' after expression.")?;
        Ok(())
    }

    /// `unary` — prefix parser for `-` (negate) and `!` (not).
    fn unary(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        let op_kind = parser.previous().kind;
        parser.parse_precedence(Prec::Unary)?;
        match op_kind {
            TokenKind::Minus => parser.emit(Instruction::Negate),
            TokenKind::Bang  => parser.emit(Instruction::Not),
            _ => unreachable!("unary() called for non‑unary token {op_kind:?}"),
        }
        Ok(())
    }

    /// `binary` — infix parser for all binary operators.
    fn binary(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        let op_kind = parser.previous().kind;
        let rule = get_rule(op_kind);
        // Parse the right operand at strictly higher precedence.
        parser.parse_precedence(rule.precedence.next())?;
        match op_kind {
            TokenKind::Plus         => parser.emit(Instruction::Add),
            TokenKind::Minus        => parser.emit(Instruction::Sub),
            TokenKind::Star         => parser.emit(Instruction::Mul),
            TokenKind::Slash        => parser.emit(Instruction::Div),
            TokenKind::Percent      => parser.emit(Instruction::Mod),
            TokenKind::TildeSlash   => parser.emit(Instruction::FloorDiv),
            TokenKind::EqualEqual   => parser.emit(Instruction::Equal),
            TokenKind::BangEqual    => parser.emit(Instruction::NotEqual),
            TokenKind::Less         => parser.emit(Instruction::Less),
            TokenKind::Greater      => parser.emit(Instruction::Greater),
            TokenKind::LessEqual    => parser.emit(Instruction::LessEqual),
            TokenKind::GreaterEqual => parser.emit(Instruction::GreaterEqual),
            _ => unreachable!("binary() called for non-binary token {op_kind:?}"),
        }
        Ok(())
    }

    /// `variable` — prefix parser for identifiers.
    fn variable(parser: &mut Parser<'_>, can_assign: bool) -> ParseResult<()> {
        let name = ShrString::new_string(parser.previous().lexeme.to_string());
        parser.resolve_and_emit_variable(name, can_assign)
    }

    fn call(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        let (pos_count, kw_count, kw_names) = parser.parse_argument_list()?;
        if kw_count > 0 {
            parser.emit(Instruction::CallKw { pos_count, kw_count, kw_names });
        } else {
            parser.emit(Instruction::Call(pos_count));
        }
        Ok(())
    }

    fn dot(parser: &mut Parser<'_>, can_assign: bool) -> ParseResult<()> {
        parser.consume(TokenKind::Identifier, "Expect property name after '.'.")?;
        let field_name = ShrString::new_string(parser.previous().lexeme.to_string());

        if parser.match_token(TokenKind::LeftParen) {
            // Method invocation — optimized OP_INVOKE.
            let (pos_count, kw_count, _kw_names) = parser.parse_argument_list()?;
            let arg_count = pos_count + kw_count;
            if kw_count > 0 {
                // TODO: support keyword args in Invoke
                record_error_at_current!(parser, ParseErrorKind::ExpectedExpression);
            }
            parser.emit(Instruction::Invoke(field_name, arg_count));
        } else if can_assign && parser.match_token(TokenKind::Equal) {
            parser.parse_expression()?;
            parser.emit(Instruction::SetProperty(field_name));
        } else {
            parser.emit(Instruction::GetProperty(field_name));
        }

        Ok(())
    }

    /// `import_expr` — prefix parser for `import "filename"` expression.
    /// Pushes a module object onto the stack.
    fn import_expr(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        parser.consume(TokenKind::String, "Expect string literal after 'import'.")?;
        let path = parser.previous().lexeme;
        let inner = &path[1..path.len() - 1]; // strip quotes
        parser.emit(Instruction::Import(ShrString::new_string(inner)));
        Ok(())
    }

    /// `super_` — prefix parser for `super.method(args)` syntax.
    fn super_(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        parser.consume(TokenKind::Dot, "Expect '.' after 'super'.")?;
        parser.consume(
            TokenKind::Identifier,
            "Expect superclass method name after 'super.'.",
        )?;
        let method_name = ShrString::new_string(parser.previous().lexeme.to_string());

        // Push `self` (slot 0 in every method frame) as the receiver.
        parser.emit(Instruction::GetLocal(0));

        if parser.match_token(TokenKind::LeftParen) {
            let (pos_count, kw_count, _kw_names) = parser.parse_argument_list()?;
            let arg_count = pos_count + kw_count;
            if kw_count > 0 {
                record_error_at_current!(parser, ParseErrorKind::ExpectedExpression);
            }
            parser.emit(Instruction::SuperInvoke(method_name, arg_count));
        } else {
            bail_error_at_current!(parser, ParseErrorKind::ExpectedToken("Expect '(' after super method name."));
        }

        Ok(())
    }

    /// Parse `{k: v, ...}` (dict) or `{a, b, ...}` (set).
    /// `{}` is always an empty dict (Python convention).
    fn dict_literal(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        // Empty braces → empty dict.
        if parser.check(TokenKind::RightBrace) {
            parser.consume(TokenKind::RightBrace, "Expect '}' after dict literal.")?;
            parser.emit(Instruction::BuildDict(0));
            return Ok(());
        }

        // Parse the first expression — this pushes one value onto the stack.
        parser.parse_expression()?;

        // If the next token is ':', it's a dict literal: {k: v, ...}
        if parser.match_token(TokenKind::Colon) {
            parser.parse_expression()?;
            let mut count: usize = 1;
            while parser.match_token(TokenKind::Comma) {
                if count >= u16::MAX as usize {
                    record_error_at_current!(parser, ParseErrorKind::TooMuchItems);
                }
                parser.parse_expression()?;
                parser.consume(TokenKind::Colon, "Expect ':' after key")?;
                parser.parse_expression()?;
                count += 1;
            }
            parser.consume(TokenKind::RightBrace, "Expect '}' after dict literal.")?;
            parser.emit(Instruction::BuildDict(count));
        } else {
            // No colon → set literal: {a, b, ...}
            let mut count: usize = 1;
            while parser.match_token(TokenKind::Comma) {
                if count >= u16::MAX as usize {
                    record_error_at_current!(parser, ParseErrorKind::TooMuchItems);
                }
                parser.parse_expression()?;
                count += 1;
            }
            parser.consume(TokenKind::RightBrace, "Expect '}' after set literal.")?;
            parser.emit(Instruction::BuildSet(count));
        }
        Ok(())
    }

    fn list_literal(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        if !parser.check(TokenKind::RightBracket) {
            let mut count = 0;
            loop {
                if count >= u16::MAX as usize {
                    record_error_at_current!(parser, ParseErrorKind::TooMuchItems);
                }

                parser.parse_expression()?;
                count += 1;
                if !parser.match_token(TokenKind::Comma) {
                    break;
                }
            }
            parser.consume(TokenKind::RightBracket, "Expect ']' list literal.")?;
            parser.emit(Instruction::BuildList(count));
            Ok(())
        } else {
            parser.consume(TokenKind::RightBracket, "Expect ']' after list literal.")?;
            parser.emit(Instruction::BuildList(0));
            Ok(())
        }
    }

    fn index(parser: &mut Parser<'_>, can_assign: bool) -> ParseResult<()> {
        parser.parse_expression()?;
        parser.consume(TokenKind::RightBracket, "Expect ']' index.")?;
        if can_assign && parser.match_token(TokenKind::Equal) {
            parser.parse_expression()?;
            parser.emit(Instruction::IndexSet);
        } else {
            parser.emit(Instruction::IndexGet);
        }
        Ok(())
    }

    fn parse_argument_list(&mut self) -> ParseResult<(usize, usize, Vec<ShrString>)> {
        let mut pos_count: usize = 0;
        let mut kw_count: usize = 0;
        let mut kw_names: Vec<ShrString> = vec![];
        let mut seen_keyword = false;

        if !self.check(TokenKind::RightParen) {
            loop {
                if pos_count + kw_count >= 255 {
                    record_error_at_current!(self, ParseErrorKind::TooMuchArgument);
                }

                // Detect keyword argument: `Identifier = expr`.
                if self.check(TokenKind::Identifier)
                    && self.peek_next().kind == TokenKind::Equal
                {
                    seen_keyword = true;
                    self.advance(); // consume identifier
                    let name = ShrString::new_string(self.previous().lexeme);
                    self.advance(); // consume '='

                    // Check for duplicate keyword names.
                    if kw_names.contains(&name) {
                        record_error_at_current!(self, ParseErrorKind::DuplicateKeywordArg(name.to_string()));
                    }
                    kw_names.push(name);

                    self.parse_expression()?;
                    kw_count += 1;
                } else {
                    if seen_keyword {
                        record_error_at_current!(self, ParseErrorKind::PositionalAfterKeyword);
                    }
                    self.parse_expression()?;
                    pos_count += 1;
                }

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RightParen, "Expect ')' after arguments.")?;
        Ok((pos_count, kw_count, kw_names))
    }

    /// Parse a constant literal default value and return its handle.
    /// Supports: numbers, strings, booleans, nil, and negated numbers.
    fn parse_default_value(&mut self) -> ParseResult<ObjectHandle> {
        // Handle negative numbers: `-<number>`.
        let negate = self.match_token(TokenKind::Minus);

        // Only numbers can be negated.
        if negate && self.peek().kind != TokenKind::Number {
            return Err(error_at_current!(self, ParseErrorKind::InvalidDefaultValue));
        }

        let handle = match self.peek().kind {
            TokenKind::Number => {
                self.advance();
                let lexeme = self.previous().lexeme;
                if lexeme.contains('.') {
                    let mut value: f64 = lexeme
                        .parse()
                        .map_err(|e| error_at_previous!(self, ParseErrorKind::InvalidFloat(e)))?;
                    if negate { value = -value; }
                    self.obj_heap.alloc_float_instance(value)
                } else {
                    let mut value: i64 = lexeme
                        .parse()
                        .map_err(|e| error_at_previous!(self, ParseErrorKind::InvalidInteger(e)))?;
                    if negate { value = -value; }
                    self.obj_heap.alloc_integer_instance(value)
                }
            }
            TokenKind::String => {
                self.advance();
                let lexeme = self.previous().lexeme;
                let inner = &lexeme[1..lexeme.len() - 1];
                let unescaped = unescape_string(inner)
                    .map_err(|c| error_at_previous!(self, ParseErrorKind::InvalidEscape(c)))?;
                self.obj_heap.alloc_string_instance(unescaped.into())
            }
            TokenKind::True => {
                self.advance();
                self.obj_heap.true_instance
            }
            TokenKind::False => {
                self.advance();
                self.obj_heap.false_instance
            }
            TokenKind::Nil => {
                self.advance();
                ObjectHandle::NIL
            }
            _ => {
                return Err(error_at_current!(self, ParseErrorKind::InvalidDefaultValue));
            }
        };
        Ok(handle)
    }

    fn resolve_and_emit_variable(&mut self, name: ShrString, can_assign: bool) -> ParseResult<()> {
        match self.resolve_local_or_upvalue(&name) {
            Some(LocalAccess::Local(slot)) => {
                if can_assign && self.match_token(TokenKind::Equal) {
                    self.parse_expression()?;
                    self.emit(Instruction::SetLocal(slot));
                } else {
                    self.emit(Instruction::GetLocal(slot));
                }
            }
            Some(LocalAccess::Upvalue(slot)) => {
                if can_assign && self.match_token(TokenKind::Equal) {
                    self.parse_expression()?;
                    self.emit(Instruction::SetUpvalue(slot));
                } else {
                    self.emit(Instruction::GetUpvalue(slot));
                }
            }
            None => {
                if can_assign && self.match_token(TokenKind::Equal) {
                    // Require explicit declaration with `var` before assignment.
                    if !self.declared_globals.contains(&name) {
                        bail_error_at_previous!(
                            self,
                            ParseErrorKind::UndefinedVariable(name.to_string())
                        );
                    }
                    self.parse_expression()?;
                    self.emit(Instruction::SetGlobal(name));
                } else {
                    self.emit(Instruction::GetGlobal(name));
                }
            }
        }
        Ok(())
    }

    /// Resolve `name` first as a local in the current function, then as an
    /// upvalue captured from an enclosing function, and finally fall back to a global.
    fn resolve_local_or_upvalue(&mut self, name: &ShrString) -> Option<LocalAccess> {
        // 1. Current function's locals
        if let Some(slot) = self.resolve_local_in_current(name) {
            return Some(LocalAccess::Local(slot));
        }
        // 2. Walk up enclosing chain
        self.resolve_upvalue(name).map(LocalAccess::Upvalue)
    }

    /// Look up `name` in the *current* function's locals only.
    fn resolve_local_in_current(&mut self, name: &ShrString) -> Option<usize> {
        for (i, local) in self.cur_unit().locals.iter().enumerate().rev() {
            if *name == local.name {
                if local.depth == -1 {
                    record_error_at_previous!(
                        self,
                        ParseErrorKind::VariableRedefine(format!("Cannot read local variable '{}' in its own initializer", name.as_str(),))
                    );
                    return None;
                }
                return Some(i);
            }
        }
        None
    }

    /// Look up `name` in a specific unit's locals.
    fn resolve_local_in_unit(&self, unit_idx: usize, name: &ShrString) -> Option<usize> {
        for (i, local) in self.units[unit_idx].locals.iter().enumerate().rev() {
            if *name == local.name {
                if local.depth == -1 {
                    return None;
                }
                return Some(i);
            }
        }
        None
    }

    /// Walk the enclosing chain to resolve `name` as an upvalue.
    fn resolve_upvalue(&mut self, name: &ShrString) -> Option<usize> {
        let enclosing = self.cur_unit().enclosing;
        if enclosing == self.current_unit {
            return None; // root unit — no enclosing
        }

        // Found as a local in the *immediate* enclosing function?
        if let Some(local_slot) = self.resolve_local_in_unit(enclosing, name) {
            self.units[enclosing].locals[local_slot].is_captured = true;
            return Some(self.add_upvalue(local_slot, true));
        }

        // Recurse: search in the enclosing's enclosing.
        let saved = self.current_unit;
        self.current_unit = enclosing;
        let result = self.resolve_upvalue(name);
        self.current_unit = saved;

        if let Some(upvalue_idx) = result {
            return Some(self.add_upvalue(upvalue_idx, false));
        }

        None
    }

    /// Add an upvalue to the *current* unit and return its index.
    fn add_upvalue(&mut self, index: usize, is_local: bool) -> usize {
        let unit = self.cur_unit_mut();
        for (i, uv) in unit.upvalues.iter().enumerate() {
            if uv.index == index && uv.is_local == is_local {
                return i;
            }
        }
        let i = unit.upvalues.len();
        unit.upvalues.push(UpvalueDesc { index, is_local });
        i
    }

    // ------------------------------------------------------------------------
    //  Token helpers
    // ------------------------------------------------------------------------

    fn emit(&mut self, inst: Instruction) {
        // SAFETY: We need simultaneous `&mut Chunk` and `&mut ObjectHeap`, but
        // both live behind `&mut self` (via `self.units` and `self.obj_heap`),
        // so Rust's borrow checker can't prove they're disjoint.  We cast
        // `obj_heap` to a raw pointer to obtain the second `&mut`.
        //
        // This is sound because:
        // - `self.obj_heap` and `self.units[..].chunk` are separate allocations
        //   (different fields, no pointer aliasing).
        // - `write_instruction` only pushes new constant objects into the heap;
        //   it never reads or modifies `self.units` or any chunk.
        // - The lifetime `'a` on `Parser<'a>` guarantees the `&'a mut ObjectHeap`
        //   outlives the parser, so the raw pointer remains valid.
        let (line, column) = if self.current > 0 {
            let prev = self.previous();
            (prev.line, prev.column)
        } else {
            (1, 1) // No token consumed yet — default to line 1, column 1.
        };
        let heap = self.obj_heap as *mut ObjectHeap;
        let chunk = &mut self.units[self.current_unit].chunk;
        unsafe {
            chunk.write_instruction(inst, line, column, &mut *heap);
        }
    }

    fn peek(&self) -> &Token<'a> {
        &self.tokens[self.current]
    }

    fn peek_next(&self) -> &Token<'a> {
        if self.current + 1 < self.tokens.len() {
            &self.tokens[self.current + 1]
        } else {
            &self.tokens[self.current]
        }
    }

    fn previous(&self) -> &Token<'a> {
        &self.tokens[self.current - 1]
    }

    fn at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn advance(&mut self) {
        if !self.at_end() {
            self.current += 1;
        }
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, kind: TokenKind, msg: &'static str) -> ParseResult<()> {
        if self.check(kind) {
            self.advance();
            Ok(())
        } else {
            bail_error_at_current!(self, ParseErrorKind::ExpectedToken(msg))
        }
    }

    // ------------------------------------------------------------------------
    //  Synchronization
    // ------------------------------------------------------------------------
    fn synchronize(&mut self, error: ParseError) {
        self.errors.push(error);
        while !self.at_end() {
            if self.previous().kind == TokenKind::Semicolon {
                return;
            }

            match self.peek().kind {
                TokenKind::Class | TokenKind::Fun | TokenKind::Import | TokenKind::Var |
                TokenKind::For | TokenKind::If | TokenKind::While |
                TokenKind::Break | TokenKind::Continue |
                TokenKind::Return => {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }
}

// ========================================================================== //
//                    String unescape helper
// ========================================================================== //

/// Process escape sequences in a raw string literal (without surrounding
/// quotes).  Returns the unescaped string, or the offending character if an
/// unknown escape sequence is encountered.
fn unescape_string(raw: &str) -> Result<String, char> {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n')  => result.push('\n'),
                Some('r')  => result.push('\r'),
                Some('t')  => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('"')  => result.push('"'),
                Some('0')  => result.push('\0'),
                Some(c)    => return Err(c),
                None       => return Err('\\'), // trailing backslash at EOF
            }
        } else {
            result.push(c);
        }
    }
    Ok(result)
}
