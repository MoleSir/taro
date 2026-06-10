use super::token::{Token, TokenKind};

pub struct Scanner<'a> {
    source: &'a str,
    start: usize,
    current: usize,
    line: usize,
}

#[thiserrorctx::context_error]
pub enum ScanError {
    #[error("unexpect end of source")]
    UnexpectEnd,

    #[error("unterminated string")]
    UnterminatedString,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn scan_tokens(&mut self) -> ScanResult<Vec<Token<'a>>> {
        let mut tokens = Vec::new();

        loop {
            let token = self.scan_token()?;
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof {
                break;
            }
        }

        Ok(tokens)
    }

    fn scan_token(&mut self) -> ScanResult<Token<'a>> {
        self.skip_whitespace()?;
        self.start = self.current;

        if self.at_end() {
            return Ok(self.make_token(TokenKind::Eof));
        }

        let c = self.advance()?;

        if c.is_ascii_digit() {
            return self.number();
        }

        if is_alpha(c) {
            return self.identifier();
        }

        match c {
            '(' => Ok(self.make_token(TokenKind::LeftParen)),
            ')' => Ok(self.make_token(TokenKind::RightParen)),
            '{' => Ok(self.make_token(TokenKind::LeftBrace)),
            '}' => Ok(self.make_token(TokenKind::RightBrace)),
            ',' => Ok(self.make_token(TokenKind::Comma)),
            '.' => Ok(self.make_token(TokenKind::Dot)),
            '-' => Ok(self.make_token(TokenKind::Minus)),
            '+' => Ok(self.make_token(TokenKind::Plus)),
            ';' => Ok(self.make_token(TokenKind::Semicolon)),
            '/' => Ok(self.make_token(TokenKind::Slash)),
            '*' => Ok(self.make_token(TokenKind::Star)),

            '!' => {
                let kind = if self.match_then_advance('=')? {
                    TokenKind::BangEqual
                } else {
                    TokenKind::Bang
                };
                Ok(self.make_token(kind))
            }
            '=' => {
                let kind = if self.match_then_advance('=')? {
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                };
                Ok(self.make_token(kind))
            }
            '<' => {
                let kind = if self.match_then_advance('=')? {
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                };
                Ok(self.make_token(kind))
            }
            '>' => {
                let kind = if self.match_then_advance('=')? {
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                };
                Ok(self.make_token(kind))
            }

            '"' => self.string(),

            _ => Ok(self.error_token("Unexpected character.")),
        }
    }

    fn number(&mut self) -> ScanResult<Token<'a>> {
        while !self.at_end() {
            let c = self.peek()?;
            if !c.is_ascii_digit() {
                break;
            }
            self.advance()?;
        }

        // Look for a fractional part.
        if !self.at_end() {
            let c = self.peek()?;
            if c == '.' {
                // peek_next() fails when there is no character after '.',
                // which means this dot is not part of the number.
                if let Ok(next) = self.peek_next() {
                    if next.is_ascii_digit() {
                        // Consume the '.'
                        self.advance()?;
                        while !self.at_end() {
                            let c = self.peek()?;
                            if !c.is_ascii_digit() {
                                break;
                            }
                            self.advance()?;
                        }
                    }
                }
            }
        }

        Ok(self.make_token(TokenKind::Number))
    }

    fn string(&mut self) -> ScanResult<Token<'a>> {
        loop {
            if self.at_end() {
                return Err(ScanError::UnterminatedString);
            }
            let c = self.advance()?;
            if c == '"' {
                break;
            }
            if c == '\n' {
                self.line += 1;
            }
        }
        Ok(self.make_token(TokenKind::String))
    }

    fn identifier(&mut self) -> ScanResult<Token<'a>> {
        while !self.at_end() {
            let c = self.peek()?;
            if !is_alpha(c) && !c.is_ascii_digit() {
                break;
            }
            self.advance()?;
        }

        let kind = self.identifier_type();
        Ok(self.make_token(kind))
    }

    fn identifier_type(&self) -> TokenKind {
        // We can only look at the first character because `make_token`
        // returns the lexeme slice, but we can't call `make_token` yet
        // since that would consume the lexeme. Instead we match on the
        // actual lexeme text.
        let lexeme = &self.source[self.start..self.current];
        match lexeme {
            "and" => TokenKind::And,
            "class" => TokenKind::Class,
            "else" => TokenKind::Else,
            "false" => TokenKind::False,
            "for" => TokenKind::For,
            "fun" => TokenKind::Fun,
            "if" => TokenKind::If,
            "nil" => TokenKind::Nil,
            "or" => TokenKind::Or,
            "print" => TokenKind::Print,
            "return" => TokenKind::Return,
            "super" => TokenKind::Super,
            "this" => TokenKind::This,
            "true" => TokenKind::True,
            "var" => TokenKind::Var,
            "while" => TokenKind::While,
            _ => TokenKind::Identifier,
        }
    }

    fn at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advance(&mut self) -> ScanResult<char> {
        let rest = &self.source[self.current..];
        let ch = rest
            .chars()
            .next()
            .ok_or(ScanError::UnexpectEnd)?;
        self.current += ch.len_utf8();
        Ok(ch)
    }

    fn peek(&self) -> ScanResult<char> {
        self.source[self.current..]
            .chars()
            .next()
            .ok_or(ScanError::UnexpectEnd)
    }

    fn match_then_advance(&mut self, expected: char) -> ScanResult<bool> {
        if self.at_end() {
            return Ok(false);
        }
        let nxt = self.source[self.current..]
            .chars()
            .next()
            .ok_or(ScanError::UnexpectEnd)?;

        if nxt != expected {
            Ok(false)
        } else {
            self.current += nxt.len_utf8();
            Ok(true)
        }
    }

    fn peek_next(&self) -> ScanResult<char> {
        let mut chars = self.source[self.current..].chars();
        chars.next().ok_or(ScanError::UnexpectEnd)?;
        chars.next().ok_or(ScanError::UnexpectEnd)
    }

    fn make_token(&self, kind: TokenKind) -> Token<'a> {
        Token {
            kind,
            lexeme: &self.source[self.start..self.current],
            line: self.line,
        }
    }

    fn error_token(&self, msg: &'static str) -> Token<'static> {
        Token {
            kind: TokenKind::Error,
            lexeme: msg,
            line: self.line,
        }
    }

    fn skip_whitespace(&mut self) -> ScanResult<()> {
        loop {
            if self.at_end() {
                return Ok(());
            }
            let c = self.peek()?;
            match c {
                ' ' | '\r' | '\t' => {
                    self.advance()?;
                }
                '\n' => {
                    self.line += 1;
                    self.advance()?;
                }
                '/' => {
                    // Check for comment.  peek_next() fails when there is no
                    // character after '/', which means it's just a slash token.
                    if let Ok(next) = self.peek_next() {
                        if next == '/' {
                            // Line comment — consume until end of line.
                            while !self.at_end() {
                                let c = self.peek()?;
                                if c == '\n' {
                                    break;
                                }
                                self.advance()?;
                            }
                        } else {
                            // Not a comment, just a slash — stop skipping.
                            return Ok(());
                        }
                    } else {
                        // Only one char left ('/'), not a comment.
                        return Ok(());
                    }
                }
                _ => return Ok(()),
            }
        }
    }
}

fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

// ========================================================================== //
//                    Tests
// ========================================================================== //

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: scan source and return just the token kinds (plus lexeme for
    /// literals/identifiers where it matters).
    fn scan_kinds(source: &str) -> Vec<TokenKind> {
        let mut scanner = Scanner::new(source);
        scanner
            .scan_tokens()
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    /// Helper: scan source and return the full tokens.
    fn scan_tokens(source: &str) -> Vec<Token> {
        let mut scanner = Scanner::new(source);
        scanner.scan_tokens().unwrap()
    }

    // ------------------------------------------------------------------------
    //  Single-character tokens
    // ------------------------------------------------------------------------

    #[test]
    fn test_single_char_tokens() {
        let source = "(){} ,.-+;/*";
        let kinds = scan_kinds(source);
        assert_eq!(
            kinds,
            vec![
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::Comma,
                TokenKind::Dot,
                TokenKind::Minus,
                TokenKind::Plus,
                TokenKind::Semicolon,
                TokenKind::Slash,
                TokenKind::Star,
                TokenKind::Eof,
            ]
        );
    }

    // ------------------------------------------------------------------------
    //  One-or-two character tokens
    // ------------------------------------------------------------------------

    #[test]
    fn test_bang_and_bang_equal() {
        let kinds = scan_kinds("! !=");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Bang,
                TokenKind::BangEqual,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_equal_and_equal_equal() {
        let kinds = scan_kinds("= ==");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Equal,
                TokenKind::EqualEqual,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_less_and_less_equal() {
        let kinds = scan_kinds("< <=");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Less,
                TokenKind::LessEqual,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_greater_and_greater_equal() {
        let kinds = scan_kinds("> >=");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Greater,
                TokenKind::GreaterEqual,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_two_char_tokens_no_space() {
        let kinds = scan_kinds("!= == <= >=");
        assert_eq!(
            kinds,
            vec![
                TokenKind::BangEqual,
                TokenKind::EqualEqual,
                TokenKind::LessEqual,
                TokenKind::GreaterEqual,
                TokenKind::Eof,
            ]
        );
    }

    // ------------------------------------------------------------------------
    //  Numbers
    // ------------------------------------------------------------------------

    #[test]
    fn test_integer() {
        let tokens = scan_tokens("42");
        assert_eq!(tokens[0].kind, TokenKind::Number);
        assert_eq!(tokens[0].lexeme, "42");
    }

    #[test]
    fn test_decimal() {
        let tokens = scan_tokens("3.14");
        assert_eq!(tokens[0].kind, TokenKind::Number);
        assert_eq!(tokens[0].lexeme, "3.14");
    }

    #[test]
    fn test_decimal_starting_with_dot_is_not_number() {
        // `.` alone is Dot, then the digits form a Number.
        let kinds = scan_kinds(".5");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Dot,
                TokenKind::Number,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_trailing_dot_is_separate() {
        // "12." should tokenize as Number("12"), Dot(".")
        let tokens = scan_tokens("12.");
        assert_eq!(tokens[0].kind, TokenKind::Number);
        assert_eq!(tokens[0].lexeme, "12");
        assert_eq!(tokens[1].kind, TokenKind::Dot);
    }

    #[test]
    fn test_number_followed_by_identifier() {
        // No space between number and identifier: the identifier path is not
        // entered because the first char is a digit.
        let kinds = scan_kinds("42foo");
        assert_eq!(kinds[0], TokenKind::Number);
        assert_eq!(kinds[1], TokenKind::Identifier);
        // lexeme of the identifier should be "foo"
        let tokens = scan_tokens("42foo");
        assert_eq!(tokens[1].lexeme, "foo");
    }

    // ------------------------------------------------------------------------
    //  Strings
    // ------------------------------------------------------------------------

    #[test]
    fn test_simple_string() {
        let tokens = scan_tokens("\"hello\"");
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(tokens[0].lexeme, "\"hello\"");
    }

    #[test]
    fn test_empty_string() {
        let tokens = scan_tokens("\"\"");
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(tokens[0].lexeme, "\"\"");
    }

    #[test]
    fn test_string_with_newline_increments_line() {
        let source = "\"a\nb\"";
        let mut scanner = Scanner::new(source);
        let tokens = scanner.scan_tokens().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::String);
        // The scanner's line counter should have advanced to 2 because of the
        // embedded newline.
        assert_eq!(scanner.line, 2);
    }

    #[test]
    fn test_unterminated_string_is_error() {
        let result = Scanner::new("\"no close").scan_tokens();
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------------
    //  Identifiers and keywords
    // ------------------------------------------------------------------------

    #[test]
    fn test_simple_identifier() {
        let tokens = scan_tokens("foo");
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[0].lexeme, "foo");
    }

    #[test]
    fn test_identifier_with_underscore() {
        let tokens = scan_tokens("my_var");
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[0].lexeme, "my_var");
    }

    #[test]
    fn test_identifier_with_digits() {
        let tokens = scan_tokens("x42");
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[0].lexeme, "x42");
    }

    #[test]
    fn test_identifier_starting_with_underscore() {
        let tokens = scan_tokens("_hidden");
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[0].lexeme, "_hidden");
    }

    #[test]
    fn test_all_keywords() {
        let keywords = [
            ("and", TokenKind::And),
            ("class", TokenKind::Class),
            ("else", TokenKind::Else),
            ("false", TokenKind::False),
            ("for", TokenKind::For),
            ("fun", TokenKind::Fun),
            ("if", TokenKind::If),
            ("nil", TokenKind::Nil),
            ("or", TokenKind::Or),
            ("print", TokenKind::Print),
            ("return", TokenKind::Return),
            ("super", TokenKind::Super),
            ("this", TokenKind::This),
            ("true", TokenKind::True),
            ("var", TokenKind::Var),
            ("while", TokenKind::While),
        ];

        for (source, expected_kind) in keywords {
            let tokens = scan_tokens(source);
            assert_eq!(
                tokens[0].kind, expected_kind,
                "expected keyword '{source}' to map to {expected_kind:?}",
            );
            assert_eq!(tokens[0].lexeme, source);
        }
    }

    #[test]
    fn test_keyword_prefix_is_identifier() {
        // "whilefoo" is an identifier, not the keyword "while".
        let tokens = scan_tokens("whilefoo");
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[0].lexeme, "whilefoo");
    }

    // ------------------------------------------------------------------------
    //  Comments and whitespace
    // ------------------------------------------------------------------------

    #[test]
    fn test_line_comment_ignored() {
        let kinds = scan_kinds("// this is a comment\n+");
        // Only the `+` token should appear (plus Eof).
        assert_eq!(kinds, vec![TokenKind::Plus, TokenKind::Eof]);
    }

    #[test]
    fn test_slash_is_not_comment() {
        let kinds = scan_kinds("/");
        assert_eq!(kinds, vec![TokenKind::Slash, TokenKind::Eof]);
    }

    #[test]
    fn test_slash_slash_at_eof() {
        // "//" at EOF with no newline — comment runs to end, then Eof.
        let kinds = scan_kinds("//");
        assert_eq!(kinds, vec![TokenKind::Eof]);
    }

    #[test]
    fn test_line_counting() {
        let mut scanner = Scanner::new("line1\nline2\nline3");
        let tokens = scanner.scan_tokens().unwrap();
        // All three identifiers plus Eof.
        assert_eq!(tokens.len(), 4);
        // The last token (Eof) should be on line 3.
        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[1].line, 2);
        assert_eq!(tokens[2].line, 3);
        assert_eq!(tokens[3].line, 3); // Eof stays on the last line
    }

    #[test]
    fn test_whitespace_skipped() {
        let kinds = scan_kinds("  \t  \r  +");
        assert_eq!(kinds, vec![TokenKind::Plus, TokenKind::Eof]);
    }

    // ------------------------------------------------------------------------
    //  Edge cases
    // ------------------------------------------------------------------------

    #[test]
    fn test_empty_source_gives_only_eof() {
        let kinds = scan_kinds("");
        assert_eq!(kinds, vec![TokenKind::Eof]);
    }

    #[test]
    fn test_multiple_tokens_no_spaces() {
        let kinds = scan_kinds("(){}");
        assert_eq!(
            kinds,
            vec![
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_assignment_expression() {
        let kinds = scan_kinds("var x = 3.14;");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Var,
                TokenKind::Identifier,
                TokenKind::Equal,
                TokenKind::Number,
                TokenKind::Semicolon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_lexeme_slices_are_correct() {
        let tokens = scan_tokens("var x = 42");
        assert_eq!(tokens[0].lexeme, "var");
        assert_eq!(tokens[1].lexeme, "x");
        assert_eq!(tokens[2].lexeme, "=");
        assert_eq!(tokens[3].lexeme, "42");
    }
}
