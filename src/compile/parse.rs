use super::token::{Token, TokenKind};
use crate::{format_shr, Chunk, Instruction, ObjectFunction, ObjectHandle, ObjectHeap, ShrString, Value};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionKind {
    ObjectFunction,
    Script,
}

struct ParseState {
    function:    ObjectHandle,
    kind:        FunctionKind,
    locals:      Vec<Local>,
    scope_depth: isize,
}

pub struct Parser<'a> {
    obj_heap:    &'a mut ObjectHeap,
    tokens:      Vec<Token<'a>>,
    current:     usize,
    errors:      Vec<ParseError>,
    state:       ParseState,
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

macro_rules! report_error_at_current {
    ($p:ident, $reason:expr) => {{
        let token = $p.peek();
        ParseError { line: token.line, lexeme: token.lexeme.to_string(), reason: $reason }
    }};
}

macro_rules! report_error_at_previous {
    ($p:ident, $reason:expr) => {{
        let token = $p.previous();
        ParseError { line: token.line, lexeme: token.lexeme.to_string(), reason: $reason }
    }};
}

impl ParseState {
    fn new(obj_heap: &mut ObjectHeap, name: impl Into<ShrString>, kind: FunctionKind) -> Self {
        Self {
            function: obj_heap.alloc_function(name.into(),  0, Chunk::new()),
            kind, 
            locals: vec![Local { depth: 0, name: "".into() }],
            scope_depth: 0,
        }
    } 
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>, obj_heap: &'a mut ObjectHeap) -> Self {
        let state = ParseState::new(obj_heap, "", FunctionKind::Script);
        Self {
            obj_heap,
            tokens,
            current: 0,
            errors: vec![],
            state,
        }
    }

    // ------------------------------------------------------------------------
    //  Public entry point
    // ------------------------------------------------------------------------

    pub(crate) fn parse(mut self) -> Result<ObjectHandle, Vec<ParseError>> {
        while !self.at_end() {
            if let Err(e) = self.declaration() {
                self.errors.push(e);
                self.synchronize();
            }
        }
        
        if !self.errors.is_empty() {
            Err(self.errors)
        } else {
            Ok(self.end_parse())
        }
    }

    fn end_parse(&mut self) -> ObjectHandle {
        self.emit_return();
        self.state.function
    }

    fn cur_function(&mut self) -> &mut ObjectFunction {
        self.obj_heap.get_mut(self.state.function).as_function_mut().expect("must funtion")
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
                assert!(self.state.scope_depth > 0);
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
        self.state.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.state.scope_depth -= 1;
        while self.state.locals.len() > 0 && self.state.locals.last().unwrap().depth > self.state.scope_depth {
            self.emit(Instruction::Pop);
            self.state.locals.pop();
        }
    }

    fn return_statement(&mut self) -> ParseResult<()> {
        if self.state.kind == FunctionKind::Script {
            Err(report_error_at_current!(self, ParseReason::ReturnInTop))?;
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
            Err(report_error_at_current!(self, ParseReason::TooMuchCodeToJumpOver(offset)))?;
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
            Err(report_error_at_current!(self, ParseReason::TooMuchCodeToJumpOver(offset)))?;
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
            return Err(report_error_at_previous!(self, ParseReason::ExpectedExpression));
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
            return Err(report_error_at_current!(self, ParseReason::InvalidAssignmentTarget));
        }

        Ok(())
    }

    fn parse_variable(&mut self, msg: &'static str) -> ParseResult<Option<ShrString>> {
        self.consume(TokenKind::Identifier, msg)?;
        let var_name = ShrString::new_string(self.previous().lexeme);
        
        if self.state.scope_depth > 0 {
            // local
            for local in self.state.locals.iter().rev() {
                // Sentinel depth (-1) means the variable is still in its initializer;
                // it's still "in scope" for redefinition checking so we skip the break.
                if local.depth != -1 && local.depth < self.state.scope_depth {
                    break;
                }
                if var_name == local.name {
                    return Err(report_error_at_current!(self, ParseReason::VariableRedefine(var_name.to_string())));
                }
            }
            self.add_local(var_name)?;
            Ok(None)
        } else {
            // global
            Ok(Some(var_name))
        }
        
        // self.declare_variable()?;
        // if self.scope_depth > 0 {
        //     // MARK
        //     return Ok("".into());
        // }
        
        // let name = ShrString::new_string(self.previous().lexeme);
        // Ok(name)
    }

    // fn declare_variable(&mut self) -> ParseResult<()> {
    //     if self.scope_depth == 0 {
    //         return Ok(());
    //     }
        
    //     let name = ShrString::new_string(self.previous().lexeme);

    //     for local in self.locals.iter().rev() {
    //         // Sentinel depth (-1) means the variable is still in its initializer;
    //         // it's still "in scope" for redefinition checking so we skip the break.
    //         if local.depth != -1 && local.depth < self.scope_depth {
    //             break;
    //         }
    //         if name == local.name {
    //             return Err(report_error_at_current!(self, ParseReason::VariableRedefine(name.to_string())));
    //         }
    //     }
        
    //     self.add_local(name)?;
    //     Ok(())
    // }

    fn add_local(&mut self, name: ShrString) -> ParseResult<()> {
        // `-1` is the sentinel: the variable is declared but not yet
        // initialized.  `mark_initialized()` will set the real depth.
        let local = Local { name, depth: -1 };
        self.state.locals.push(local);
        Ok(())
    }

    /// Mark the most recently declared local as ready for use.
    fn mark_initialized(&mut self) {
        if self.state.scope_depth == 0 {
            return;
        }
        if let Some(last) = self.state.locals.last_mut() {
            last.depth = self.state.scope_depth;
        }
    }

    // static uint8_t identifierConstant(Token* name) {
    //     return makeConstant(OBJ_VAL(copyString(name->start,
    //                                            name->length)));
    //   }

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
                    report_error_at_current!(parser, ParseReason::InvalidFloat(e))
                )?;
            parser.emit(Instruction::Constant(Value::Float(value)));
        } else {
            let value: i64 = lexeme
                .parse()
                .map_err(|e| 
                    report_error_at_current!(parser, ParseReason::InvalidInteger(e))
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
                    Err(report_error_at_current!(self, ParseReason::TooMuchArgument))?;
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
        let mut state = if kind != FunctionKind::Script {
            let name = self.previous().lexeme;
            ParseState::new(&mut self.obj_heap, name.to_string(), kind)
        } else {
            ParseState::new(&mut self.obj_heap, "", kind)
        };

        std::mem::swap(&mut self.state, &mut state);
        
        self.begin_scope();
        
        self.consume(TokenKind::LeftParen, "Expect '(' after function name.")?;
        if !self.check(TokenKind::RightParen) {
            loop {
                let function = self.cur_function();
                function.arity += 1;
                if function.arity > 255 {
                    Err(report_error_at_current!(self, ParseReason::TooMuchParameter))?;
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

        let function = self.end_parse();
        std::mem::swap(&mut self.state, &mut state);

        self.emit(Instruction::Constant(Value::Object(function)));

        Ok(())
    }

    fn named_variable(&mut self, name: ShrString, can_assign: bool) -> ParseResult<()> {
        match self.resolve_local(name.clone()) {
            Some(slot) => {
                if can_assign && self.match_token(TokenKind::Equal) {
                    self.expression()?;
                    self.emit(Instruction::SetLocal(slot));
                } else {
                    self.emit(Instruction::GetLocal(slot));
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

    /// Look up `name` in the current scope.  Returns the stack-slot index
    /// (position in the locals vector) if found and already initialized.
    fn resolve_local(&mut self, name: ShrString) -> Option<usize> {
        for (i, local) in self.state.locals.iter().enumerate().rev() {
            if name == local.name {
                if local.depth == -1 {
                    // variable is declared but still inside its own initializer
                    // e.g. `var a = a;` — report the error and bail.
                    self.errors.push(ParseError {
                        line: 0, // we don't have the token here; approximate
                        lexeme: name.to_string(),
                        reason: ParseReason::VariableRedefine(format!(
                            "Cannot read local variable '{}' in its own initializer",
                            name.as_str(),
                        )),
                    });
                    return None;
                }
                return Some(i as usize);
            }
        }
        None
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
            Err(report_error_at_current!(self, ParseReason::ExpectedToken(msg)))
        }
    }

    // ------------------------------------------------------------------------
    //  Synchronization
    // ------------------------------------------------------------------------
    fn synchronize(&mut self) {
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