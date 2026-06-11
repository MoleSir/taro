use super::token::{Token, TokenKind};
use crate::{format_shr, Chunk, Instruction, ObjectFunction, ObjectHandle, ObjectHeap, ShrString, UpvalueDesc, Value};

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
///
/// This is the Rust equivalent of the `rules[]` array in Crafting
/// Interpreters — every [`TokenKind`] variant gets an explicit entry.
fn get_rule(kind: TokenKind) -> ParseRule {
    match kind {
        // Single-character tokens -------------------------------------------
        TokenKind::LeftParen    => ParseRule::new(Some(Parser::grouping), Some(Parser::call), Prec::Call),
        TokenKind::RightParen   => ParseRule::NONE,
        TokenKind::LeftBrace    => ParseRule::NONE,
        TokenKind::RightBrace   => ParseRule::NONE,
        TokenKind::Comma        => ParseRule::NONE,
        TokenKind::Dot          => ParseRule::NONE,
        TokenKind::Minus        => ParseRule::new(Some(Parser::unary), Some(Parser::binary), Prec::Term),
        TokenKind::Plus         => ParseRule::new(None, Some(Parser::binary), Prec::Term),
        TokenKind::Semicolon    => ParseRule::NONE,
        TokenKind::Slash        => ParseRule::new(None, Some(Parser::binary), Prec::Factor),
        TokenKind::Star         => ParseRule::new(None, Some(Parser::binary), Prec::Factor),

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
        TokenKind::Class        => ParseRule::NONE,
        TokenKind::Else         => ParseRule::NONE,
        TokenKind::False        => ParseRule::new(Some(Parser::literal), None, Prec::None),
        TokenKind::For          => ParseRule::NONE,
        TokenKind::Fun          => ParseRule::NONE,
        TokenKind::If           => ParseRule::NONE,
        TokenKind::Nil          => ParseRule::new(Some(Parser::literal), None, Prec::None),
        TokenKind::Or           => ParseRule::new(None, Some(Parser::or), Prec::Or),
        TokenKind::Return       => ParseRule::NONE,
        TokenKind::Super        => ParseRule::NONE,
        TokenKind::This         => ParseRule::NONE,
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
    ObjectFunction,
    Script,
}

struct CompilationUnit {
    function:    ObjectHandle,
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
}

/// Result of resolving a variable name to a local slot or upvalue.
enum LocalAccess {
    Local(usize),
    Upvalue(usize),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseReason {
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

    #[error("Can't return from top-level code.")]
    ReturnInTop,
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub lexeme: String,
    pub reason: ParseReason,
}

pub type ParseResult<T> = std::result::Result<T, ParseError>;

macro_rules! error_at_current {
    ($p:ident, $reason:expr) => {{
        let token = $p.peek();
        ParseError { line: token.line, lexeme: token.lexeme.to_string(), reason: $reason }
    }};
}

macro_rules! bail_error_at_current {
    ($p:ident, $reason:expr) => {{
        Err(error_at_current!($p, $reason))?
    }};
}

macro_rules! record_error_at_current {
    ($p:ident, $reason:expr) => {{
        $p.errors.push(error_at_current!($p, $reason));
    }};
}

#[allow(unused)]
macro_rules! error_at_previous {
    ($p:ident, $reason:expr) => {{
        let token = $p.previous();
        ParseError { line: token.line, lexeme: token.lexeme.to_string(), reason: $reason }
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
    fn new(obj_heap: &mut ObjectHeap, name: impl Into<ShrString>, kind: FunctionKind, enclosing: usize) -> Self {
        Self {
            function: obj_heap.alloc_function(name.into(), 0, Chunk::new()),
            kind,
            locals: vec![Local { depth: 0, name: "".into(), is_captured: false }],
            scope_depth: 0,
            upvalues: vec![],
            enclosing,
        }
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

    fn cur_function(&mut self) -> &mut ObjectFunction {
        let handle = self.cur_unit().function;
        self.obj_heap.get_mut(handle).as_function_mut().expect("must function")
    }

    // ------------------------------------------------------------------------
    //  Public entry point
    // ------------------------------------------------------------------------

    pub(crate) fn parse(mut self) -> Result<ObjectHandle, Vec<ParseError>> {
        while !self.at_end() {
            if let Err(e) = self.declaration() {
                self.synchronize(e);
            }
        }

        if !self.errors.is_empty() {
            Err(self.errors)
        } else {
            Ok(self.end_parse().0)
        }
    }

    /// Finish the current compilation unit: emit an implicit `return nil`,
    /// pop the unit from the stack, and restore the enclosing unit.
    fn end_parse(&mut self) -> (ObjectHandle, Vec<UpvalueDesc>) {
        self.emit_return();
        let unit = self.units.pop().expect("at least the root unit");
        self.current_unit = unit.enclosing;
        (unit.function, unit.upvalues)
    }

    fn declaration(&mut self) -> ParseResult<()> {
        if self.match_token(TokenKind::Var) {
            self.var_declaration()
        } else if self.match_token(TokenKind::Fun) {
            self.fun_declaration()
        } else {
            self.statement()
        }
    }

    fn fun_declaration(&mut self) -> ParseResult<()> {
        let var_name = self.parse_variable("Expect variable name.")?;
        self.mark_initialized();
        self.function(FunctionKind::ObjectFunction)?;
        self.define_variable(var_name)?;
        Ok(())
    }

    fn var_declaration(&mut self) -> ParseResult<()> {
        let var_name = self.parse_variable("Expect variable name.")?;

        if self.match_token(TokenKind::Equal) {
            self.expression()?;
        } else {
            self.emit(Instruction::Nil);
        }

        self.consume(TokenKind::Semicolon, "Expect ';' after variable declaration.")?;
        self.define_variable(var_name)?;

        Ok(())
    }

    fn define_variable(&mut self, var_name: Option<ShrString>) -> ParseResult<()> {
        match var_name {
            Some(var_name) => {
                // global
                self.emit(Instruction::DefineGlobal(var_name));
            }
            None => {
                // local
                assert!(self.cur_unit().scope_depth > 0);
                self.mark_initialized();
            }
        }
        Ok(())
    }

    fn statement(&mut self) -> ParseResult<()> {
        if self.match_token(TokenKind::If) {
            self.if_statement()
        } else if self.match_token(TokenKind::While) {
            self.while_statement()
        } else if self.match_token(TokenKind::For) {
            self.for_statement()
        } else if self.match_token(TokenKind::Return) {
            self.return_statement()
        } else if self.match_token(TokenKind::LeftBrace) {
            self.begin_scope();
            self.block()?;
            self.end_scope();
            Ok(())
        } else {
            self.expression_statement()
        }
    }

    fn block(&mut self) -> ParseResult<()> {
        while !self.check(TokenKind::RightBrace) && !self.check(TokenKind::Eof) {
            self.declaration()?;
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

    fn return_statement(&mut self) -> ParseResult<()> {
        if self.cur_unit().kind == FunctionKind::Script {
            record_error_at_current!(self, ParseReason::ReturnInTop);
        }
        
        if self.match_token(TokenKind::Semicolon) {
            self.emit_return();
        } else {
            self.expression()?;
            self.consume(TokenKind::Semicolon, "Expect ';' after return value.")?;
            self.emit(Instruction::Return);

        }
        Ok(())
    }

    fn for_statement(&mut self) -> ParseResult<()> {
        self.begin_scope();
        
        self.consume(TokenKind::LeftParen, "Expect '(' after 'for'.")?;
        // initializer
        if self.match_token(TokenKind::Semicolon) {
            // No initializer.
        } else if self.match_token(TokenKind::Var) {
            self.var_declaration()?;
        } else {
            self.expression_statement()?;
        }

        let mut loop_start = self.cur_function().chunk.codes.len();

        // loop condition
        let mut exit_jump_opt = None;
        if !self.match_token(TokenKind::Semicolon) {
            self.expression()?;
            self.consume(TokenKind::Semicolon, "Expect ';' after loop condition.")?;
        
            // Jump out of the loop if the condition is false.
            exit_jump_opt = Some(self.emit_jump(true));
            self.emit(Instruction::Pop);
        }

        // increment clause
        if !self.match_token(TokenKind::RightParen) {
            // exit increment clause, we must jump here, each time body done
            let body_jump = self.emit_jump(false);
            let increment_start = self.cur_function().chunk.codes.len();
            self.expression()?;
            self.emit(Instruction::Pop);
            self.consume(TokenKind::RightParen, "Expect ')' after for clauses.")?;

            self.emit_loop(loop_start)?;
            loop_start = increment_start;
            self.patch_jump(body_jump)?;
        }

        // while statement
        self.statement()?;

        /*
            if while statement done, jump to loop_start, it may be 
            1. loop condition
            2. increment clause
            3. begin of while statement(inf loop)
        */
        self.emit_loop(loop_start)?;

        // exit a jump condition, fill it jump out addr
        if let Some(exit_jump) = exit_jump_opt {
            self.patch_jump(exit_jump)?;
            self.emit(Instruction::Pop);
        }

        self.end_scope();

        Ok(())
    }
    
    fn while_statement(&mut self) -> ParseResult<()> {
        let loop_start = self.cur_function().chunk.codes.len();

        self.consume(TokenKind::LeftParen, "Expect '(' after 'while'.")?;
        self.expression()?;
        self.consume(TokenKind::RightParen, "Expect ')' after condition.")?;

        let exit_jump = self.emit_jump(true);
        self.emit(Instruction::Pop);
        self.statement()?;

        self.emit_loop(loop_start)?;

        self.patch_jump(exit_jump)?;
        self.emit(Instruction::Pop);
        Ok(())
    }

    fn if_statement(&mut self) -> ParseResult<()> {
        self.consume(TokenKind::LeftParen, "Expect '(' after 'if'.")?;
        self.expression()?;
        self.consume(TokenKind::RightParen, "Expect ')' after condition.")?;

        // write jump inst, but keep jump pos as 0, save the jump pos index
        let then_jump = self.emit_jump(true);
        self.emit(Instruction::Pop);

        // consume if code
        self.statement()?;

        let else_jump = self.emit_jump(false);

        // now we can insert jump pos
        self.patch_jump(then_jump)?;
        self.emit(Instruction::Pop);

        if self.match_token(TokenKind::Else) {
            self.statement()?;
        }
        self.patch_jump(else_jump)?;

        Ok(())
    }

    fn emit_return(&mut self) {
        self.emit(Instruction::Nil);
        self.emit(Instruction::Return);
    }

    fn emit_loop(&mut self, loop_start: usize) -> ParseResult<()> {        
        let offset = self.cur_function().chunk.codes.len() - loop_start + 3;
        if offset > u16::MAX as usize {
            record_error_at_current!(self, ParseReason::TooMuchCodeToJumpOver(offset));
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
        self.cur_function().chunk.codes.len() - 2

    }

    fn patch_jump(&mut self, jump_addr: usize) -> ParseResult<()> {
        // distance of if and cur
        let offset = self.cur_function().chunk.codes.len() - jump_addr - 2;
        if offset > u16::MAX as usize {
            record_error_at_current!(self, ParseReason::TooMuchCodeToJumpOver(offset));
        }

        let bytes = (offset as u16).to_le_bytes();
        assert!(jump_addr + 1 < self.cur_function().chunk.codes.len());
        self.cur_function().chunk.codes[jump_addr] = bytes[0];
        self.cur_function().chunk.codes[jump_addr+1] = bytes[1];
        Ok(())
    }

    fn expression_statement(&mut self) -> ParseResult<()> {
        self.expression()?;
        self.consume(TokenKind::Semicolon, "expect ';' after expression.")?;
        self.emit(Instruction::Pop);
        Ok(())
    }

    pub(crate) fn expression(&mut self) -> ParseResult<()> {
        self.parse_precedence(Prec::Assignment)
    }

    fn parse_precedence(&mut self, precedence: Prec) -> ParseResult<()> {
        self.advance();
        let prefix_rule = get_rule(self.previous().kind).prefix;

        let Some(prefix_fn) = prefix_rule else {
            bail_error_at_previous!(self, ParseReason::ExpectedExpression)
        };

        let can_assign = precedence <= Prec::Assignment;
        prefix_fn(self, can_assign)?;

        loop {
            let next_rule = get_rule(self.peek().kind);
            if precedence > next_rule.precedence {
                break;
            }
            self.advance();

            if let Some(infix_fn) = next_rule.infix {
                infix_fn(self, can_assign)?;
            }
        }

        if can_assign && self.check(TokenKind::Equal) {
            bail_error_at_current!(self, ParseReason::InvalidAssignmentTarget);
        }

        Ok(())
    }

    fn parse_variable(&mut self, msg: &'static str) -> ParseResult<Option<ShrString>> {
        self.consume(TokenKind::Identifier, msg)?;
        let var_name = ShrString::new_string(self.previous().lexeme);

        if self.cur_unit().scope_depth > 0 {
            // local
            let scope_depth = self.cur_unit().scope_depth;
            for local in self.cur_unit().locals.iter().rev() {
                // Sentinel depth (-1) means the variable is still in its initializer;
                // it's still "in scope" for redefinition checking so we skip the break.
                if local.depth != -1 && local.depth < scope_depth {
                    break;
                }
                if var_name == local.name {
                    bail_error_at_previous!(self, ParseReason::VariableRedefine(var_name.to_string()));
                }
            }
            self.add_local(var_name)?;
            Ok(None)
        } else {
            // global
            Ok(Some(var_name))
        }
    }

    fn add_local(&mut self, name: ShrString) -> ParseResult<()> {
        // `-1` is the sentinel: the variable is declared but not yet
        // initialized.  `mark_initialized()` will set the real depth.
        let local = Local { name, depth: -1, is_captured: false };
        self.cur_unit_mut().locals.push(local);
        Ok(())
    }

    /// Mark the most recently declared local as ready for use.
    fn mark_initialized(&mut self) {
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
                    error_at_previous!(parser, ParseReason::InvalidFloat(e))
                )?;
            parser.emit(Instruction::Constant(Value::Float(value)));
        } else {
            let value: i64 = lexeme
                .parse()
                .map_err(|e| 
                    error_at_previous!(parser, ParseReason::InvalidInteger(e))
                )?;
            parser.emit(Instruction::Constant(Value::Integer(value)));
        }
        Ok(())
    }

    /// `string` — prefix parser for string literals.
    fn string(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        // The lexeme includes the surrounding quotes — strip them.
        let lexeme = parser.previous().lexeme;
        let inner = &lexeme[1..lexeme.len() - 1];
        parser.emit(Instruction::Constant(Value::String(format_shr!("{}", inner))));
        Ok(())
    }

    /// `literal` — prefix parser for `true`, `false`, `nil`.
    ///
    /// Dispatches on the *previous* token kind to emit the correct dedicated
    /// opcode (not a constant‑pool load).
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
        parser.expression()?;
        parser.consume(TokenKind::RightParen, "expect ')' after expression.")?;
        Ok(())
    }

    /// `unary` — prefix parser for `-` (negate) and `!` (not).
    ///
    /// Compiles the operand at [`Prec::Unary`] so that, e.g.,
    /// `-a * b` parses as `(-a) * b`.
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
    ///
    /// Looks up the operator's precedence from the rule table, compiles the
    /// right‑hand side at the next‑higher precedence, then emits the matching
    /// instruction.
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
        parser.named_variable(name, can_assign)
    }

    fn call(parser: &mut Parser<'_>, _can_assign: bool) -> ParseResult<()> {
        let arg_count = parser.argument_list()?;
        parser.emit(Instruction::Call(arg_count));
        Ok(())
    }

    fn argument_list(&mut self) -> ParseResult<usize> {
        let mut arg_count = 0;
        if !self.check(TokenKind::RightParen) {
            loop {
                self.expression()?;
                if arg_count >= 255 {
                    record_error_at_current!(self, ParseReason::TooMuchArgument);
                }
                arg_count += 1;
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RightParen,"Expect ')' after arguments.")?;
        Ok(arg_count)
    }

    fn function(&mut self, kind: FunctionKind) -> ParseResult<()> {
        let name = if kind != FunctionKind::Script {
            self.previous().lexeme.to_string()
        } else {
            String::new()
        };

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
                let function = self.cur_function();
                function.arity += 1;
                if function.arity > 255 {
                    record_error_at_current!(self, ParseReason::TooMuchParameter);
                }
                let param_name = self.parse_variable("Expect parameter name.")?;
                self.define_variable(param_name)?;

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }

        self.consume(TokenKind::RightParen, "Expect ')' after parameters.")?;
        self.consume(TokenKind::LeftBrace, "Expect '{' before function body.")?;
        self.block()?;

        // Finish the nested function and pop its unit.
        // `end_parse` restores `self.current_unit` to the enclosing.
        let (inner_function, upvalues) = self.end_parse();
        self.emit(Instruction::Closure { function: Value::Object(inner_function), upvalues });

        Ok(())
    }

    fn named_variable(&mut self, name: ShrString, can_assign: bool) -> ParseResult<()> {
        match self.resolve_local_or_upvalue(&name) {
            Some(LocalAccess::Local(slot)) => {
                if can_assign && self.match_token(TokenKind::Equal) {
                    self.expression()?;
                    self.emit(Instruction::SetLocal(slot));
                } else {
                    self.emit(Instruction::GetLocal(slot));
                }
            }
            Some(LocalAccess::Upvalue(slot)) => {
                if can_assign && self.match_token(TokenKind::Equal) {
                    self.expression()?;
                    self.emit(Instruction::SetUpvalue(slot));
                } else {
                    self.emit(Instruction::GetUpvalue(slot));
                }
            }
            None => {
                if can_assign && self.match_token(TokenKind::Equal) {
                    self.expression()?;
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
                    // variable is declared but still inside its own initializer
                    // e.g. `var a = a;` — report the error and bail.
                    record_error_at_previous!(
                        self, 
                        ParseReason::VariableRedefine(format!("Cannot read local variable '{}' in its own initializer", name.as_str(),))
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
    ///
    /// Returns the upvalue index in the *current* function on success, or
    /// `None` if the variable must be a global.
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
            // The enclosing found it as an upvalue — chain through.
            return Some(self.add_upvalue(upvalue_idx, false));
        }

        None
    }

    /// Add an upvalue to the *current* unit and return its index.
    /// Deduplicates: returns the existing index if the same upvalue is
    /// already captured.
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
        self.cur_function().chunk.write_instruction(inst);
    }

    fn peek(&self) -> &Token<'a> {
        &self.tokens[self.current]
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
            bail_error_at_current!(self, ParseReason::ExpectedToken(msg))
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
                TokenKind::Class | TokenKind::Fun | TokenKind::Var |
                TokenKind::For | TokenKind::If | TokenKind::While |
                TokenKind::Return => {
                    return;
                }
                _ => {} 
            }

            self.advance();
        }
    }
}