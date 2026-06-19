
pub struct Token<'a> {
    pub kind: TokenKind,
    pub lexeme: &'a str,
    pub line: usize,
    /// 1-based column offset of the token start within its source line.
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // Single-character tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Colon,
    Star,
    Percent,

    // One or two character tokens.
    TildeSlash,
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    And,
    As,
    Break,
    Class,
    Continue,
    Else,
    Extends,
    False,
    For,
    Fun,
    If,
    In,
    Import,
    Nil,
    Or,
    Return,
    Super,
    True,
    Var,
    While,

    Error,
    Eof,
}