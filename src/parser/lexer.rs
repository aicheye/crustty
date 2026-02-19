//! Lexer (tokenizer) for C source code
//!
//! Converts raw source text into a flat [`Token`] stream consumed by the parser.
//! `#include` and other preprocessor directives are silently skipped rather than
//! parsed, matching the interpreter's no-preprocessor policy.

use super::ast::SourceLocation;
use std::fmt;

/// All token variants produced by the lexer.
///
/// Every variant carries a [`SourceLocation`] so that parse errors can report
/// an accurate line and column without a separate token→location table.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    IntLiteral(i32, SourceLocation),
    CharLiteral(i8, SourceLocation),
    StringLiteral(String, SourceLocation),

    // Identifiers
    Ident(String, SourceLocation),

    // Keywords
    Int(SourceLocation),
    Char(SourceLocation),
    Void(SourceLocation),
    Struct(SourceLocation),
    Const(SourceLocation),
    If(SourceLocation),
    Else(SourceLocation),
    While(SourceLocation),
    Do(SourceLocation),
    For(SourceLocation),
    Switch(SourceLocation),
    Case(SourceLocation),
    Default(SourceLocation),
    Break(SourceLocation),
    Continue(SourceLocation),
    Return(SourceLocation),
    Goto(SourceLocation),
    Sizeof(SourceLocation),
    Null(SourceLocation),

    // Operators (single and multi-character)
    // Arithmetic
    Plus(SourceLocation),    // +
    Minus(SourceLocation),   // -
    Star(SourceLocation),    // *
    Slash(SourceLocation),   // /
    Percent(SourceLocation), // %

    // Comparison
    EqEq(SourceLocation),  // ==
    NotEq(SourceLocation), // !=
    Lt(SourceLocation),    // <
    Le(SourceLocation),    // <=
    Gt(SourceLocation),    // >
    Ge(SourceLocation),    // >=

    // Logical
    AndAnd(SourceLocation), // &&
    OrOr(SourceLocation),   // ||
    Bang(SourceLocation),   // !

    // Bitwise
    Amp(SourceLocation),   // &
    Pipe(SourceLocation),  // |
    Caret(SourceLocation), // ^
    Tilde(SourceLocation), // ~
    LtLt(SourceLocation),  // <<
    GtGt(SourceLocation),  // >>

    // Assignment
    Eq(SourceLocation),        // =
    PlusEq(SourceLocation),    // +=
    MinusEq(SourceLocation),   // -=
    StarEq(SourceLocation),    // *=
    SlashEq(SourceLocation),   // /=
    PercentEq(SourceLocation), // %=

    // Increment/Decrement
    PlusPlus(SourceLocation),   // ++
    MinusMinus(SourceLocation), // --

    // Member access
    Dot(SourceLocation),   // .
    Arrow(SourceLocation), // ->

    // Ternary
    Question(SourceLocation), // ?
    Colon(SourceLocation),    // :

    // Punctuation
    LParen(SourceLocation),    // (
    RParen(SourceLocation),    // )
    LBrace(SourceLocation),    // {
    RBrace(SourceLocation),    // }
    LBracket(SourceLocation),  // [
    RBracket(SourceLocation),  // ]
    Semicolon(SourceLocation), // ;
    Comma(SourceLocation),     // ,

    // End of file
    Eof(SourceLocation),
}

impl Token {
    /// Returns the source location where this token appears.
    pub fn location(&self) -> SourceLocation {
        match self {
            Token::IntLiteral(_, loc)
            | Token::CharLiteral(_, loc)
            | Token::StringLiteral(_, loc)
            | Token::Ident(_, loc)
            | Token::Int(loc)
            | Token::Char(loc)
            | Token::Void(loc)
            | Token::Struct(loc)
            | Token::Const(loc)
            | Token::If(loc)
            | Token::Else(loc)
            | Token::While(loc)
            | Token::Do(loc)
            | Token::For(loc)
            | Token::Switch(loc)
            | Token::Case(loc)
            | Token::Default(loc)
            | Token::Break(loc)
            | Token::Continue(loc)
            | Token::Return(loc)
            | Token::Goto(loc)
            | Token::Sizeof(loc)
            | Token::Null(loc)
            | Token::Plus(loc)
            | Token::Minus(loc)
            | Token::Star(loc)
            | Token::Slash(loc)
            | Token::Percent(loc)
            | Token::EqEq(loc)
            | Token::NotEq(loc)
            | Token::Lt(loc)
            | Token::Le(loc)
            | Token::Gt(loc)
            | Token::Ge(loc)
            | Token::AndAnd(loc)
            | Token::OrOr(loc)
            | Token::Bang(loc)
            | Token::Amp(loc)
            | Token::Pipe(loc)
            | Token::Caret(loc)
            | Token::Tilde(loc)
            | Token::LtLt(loc)
            | Token::GtGt(loc)
            | Token::Eq(loc)
            | Token::PlusEq(loc)
            | Token::MinusEq(loc)
            | Token::StarEq(loc)
            | Token::SlashEq(loc)
            | Token::PercentEq(loc)
            | Token::PlusPlus(loc)
            | Token::MinusMinus(loc)
            | Token::Dot(loc)
            | Token::Arrow(loc)
            | Token::Question(loc)
            | Token::Colon(loc)
            | Token::LParen(loc)
            | Token::RParen(loc)
            | Token::LBrace(loc)
            | Token::RBrace(loc)
            | Token::LBracket(loc)
            | Token::RBracket(loc)
            | Token::Semicolon(loc)
            | Token::Comma(loc)
            | Token::Eof(loc) => *loc,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::IntLiteral(n, _) => write!(f, "int literal {}", n),
            Token::CharLiteral(c, _) => {
                let byte = *c as u8;
                if byte.is_ascii_graphic() || byte == b' ' {
                    write!(f, "char literal '{}'", byte as char)
                } else {
                    write!(f, "char literal '\\x{:02x}'", byte)
                }
            }
            Token::StringLiteral(s, _) => write!(f, "string literal \"{}\"", s),
            Token::Ident(s, _) => write!(f, "identifier '{}'", s),
            Token::Int(_) => write!(f, "'int'"),
            Token::Char(_) => write!(f, "'char'"),
            Token::Void(_) => write!(f, "'void'"),
            Token::Struct(_) => write!(f, "'struct'"),
            Token::Const(_) => write!(f, "'const'"),
            Token::If(_) => write!(f, "'if'"),
            Token::Else(_) => write!(f, "'else'"),
            Token::While(_) => write!(f, "'while'"),
            Token::Do(_) => write!(f, "'do'"),
            Token::For(_) => write!(f, "'for'"),
            Token::Switch(_) => write!(f, "'switch'"),
            Token::Case(_) => write!(f, "'case'"),
            Token::Default(_) => write!(f, "'default'"),
            Token::Break(_) => write!(f, "'break'"),
            Token::Continue(_) => write!(f, "'continue'"),
            Token::Return(_) => write!(f, "'return'"),
            Token::Goto(_) => write!(f, "'goto'"),
            Token::Sizeof(_) => write!(f, "'sizeof'"),
            Token::Null(_) => write!(f, "'NULL'"),
            Token::Plus(_) => write!(f, "'+'"),
            Token::Minus(_) => write!(f, "'-'"),
            Token::Star(_) => write!(f, "'*'"),
            Token::Slash(_) => write!(f, "'/'"),
            Token::Percent(_) => write!(f, "'%'"),
            Token::EqEq(_) => write!(f, "'=='"),
            Token::NotEq(_) => write!(f, "'!='"),
            Token::Lt(_) => write!(f, "'<'"),
            Token::Le(_) => write!(f, "'<='"),
            Token::Gt(_) => write!(f, "'>'"),
            Token::Ge(_) => write!(f, "'>='"),
            Token::AndAnd(_) => write!(f, "'&&'"),
            Token::OrOr(_) => write!(f, "'||'"),
            Token::Bang(_) => write!(f, "'!'"),
            Token::Amp(_) => write!(f, "'&'"),
            Token::Pipe(_) => write!(f, "'|'"),
            Token::Caret(_) => write!(f, "'^'"),
            Token::Tilde(_) => write!(f, "'~'"),
            Token::LtLt(_) => write!(f, "'<<'"),
            Token::GtGt(_) => write!(f, "'>>'"),
            Token::Eq(_) => write!(f, "'='"),
            Token::PlusEq(_) => write!(f, "'+='"),
            Token::MinusEq(_) => write!(f, "'-='"),
            Token::StarEq(_) => write!(f, "'*='"),
            Token::SlashEq(_) => write!(f, "'/='"),
            Token::PercentEq(_) => write!(f, "'%='"),
            Token::PlusPlus(_) => write!(f, "'++'"),
            Token::MinusMinus(_) => write!(f, "'--'"),
            Token::Dot(_) => write!(f, "'.'"),
            Token::Arrow(_) => write!(f, "'->'"),
            Token::Question(_) => write!(f, "'?'"),
            Token::Colon(_) => write!(f, "':'"),
            Token::LParen(_) => write!(f, "'('"),
            Token::RParen(_) => write!(f, "')'"),
            Token::LBrace(_) => write!(f, "'{{'"),
            Token::RBrace(_) => write!(f, "'}}'"),
            Token::LBracket(_) => write!(f, "'['"),
            Token::RBracket(_) => write!(f, "']'"),
            Token::Semicolon(_) => write!(f, "';'"),
            Token::Comma(_) => write!(f, "','"),
            Token::Eof(_) => write!(f, "end of file"),
        }
    }
}

/// Lexer error type
#[derive(Debug)]
pub struct LexError {
    pub message: String,
    pub location: SourceLocation,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Lexer error at line {}, column {}: {}",
            self.location.line, self.location.column, self.message
        )
    }
}

impl std::error::Error for LexError {}

/// Lexer for C source code
pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    /// Create a new lexer for the given source string.
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments()?;

            if self.is_at_end() {
                tokens.push(Token::Eof(self.current_location()));
                break;
            }

            // Skip #include directives (per DISCOVERY.md)
            if self.peek() == Some('#') {
                self.skip_preprocessor_directive()?;
                continue;
            }

            tokens.push(self.next_token()?);
        }

        Ok(tokens)
    }

    /// Get next token
    fn next_token(&mut self) -> Result<Token, LexError> {
        let loc = self.current_location();
        let ch = self.advance().ok_or_else(|| LexError {
            message: "Unexpected end of file".to_string(),
            location: loc,
        })?;

        match ch {
            // String literals
            '"' => self.string_literal(),

            // Character literals
            '\'' => self.char_literal(),

            // Numeric literals
            '0'..='9' => self.number_literal(ch),

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => self.identifier_or_keyword(ch),

            // Operators and punctuation
            '+' => {
                if self.peek() == Some('+') {
                    self.advance();
                    Ok(Token::PlusPlus(loc))
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::PlusEq(loc))
                } else {
                    Ok(Token::Plus(loc))
                }
            }
            '-' => {
                if self.peek() == Some('-') {
                    self.advance();
                    Ok(Token::MinusMinus(loc))
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::MinusEq(loc))
                } else if self.peek() == Some('>') {
                    self.advance();
                    Ok(Token::Arrow(loc))
                } else {
                    Ok(Token::Minus(loc))
                }
            }
            '*' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::StarEq(loc))
                } else {
                    Ok(Token::Star(loc))
                }
            }
            '/' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::SlashEq(loc))
                } else {
                    Ok(Token::Slash(loc))
                }
            }
            '%' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::PercentEq(loc))
                } else {
                    Ok(Token::Percent(loc))
                }
            }
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::EqEq(loc))
                } else {
                    Ok(Token::Eq(loc))
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::NotEq(loc))
                } else {
                    Ok(Token::Bang(loc))
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::Le(loc))
                } else if self.peek() == Some('<') {
                    self.advance();
                    Ok(Token::LtLt(loc))
                } else {
                    Ok(Token::Lt(loc))
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::Ge(loc))
                } else if self.peek() == Some('>') {
                    self.advance();
                    Ok(Token::GtGt(loc))
                } else {
                    Ok(Token::Gt(loc))
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    Ok(Token::AndAnd(loc))
                } else {
                    Ok(Token::Amp(loc))
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    Ok(Token::OrOr(loc))
                } else {
                    Ok(Token::Pipe(loc))
                }
            }
            '^' => Ok(Token::Caret(loc)),
            '~' => Ok(Token::Tilde(loc)),
            '.' => Ok(Token::Dot(loc)),
            '?' => Ok(Token::Question(loc)),
            ':' => Ok(Token::Colon(loc)),
            '(' => Ok(Token::LParen(loc)),
            ')' => Ok(Token::RParen(loc)),
            '{' => Ok(Token::LBrace(loc)),
            '}' => Ok(Token::RBrace(loc)),
            '[' => Ok(Token::LBracket(loc)),
            ']' => Ok(Token::RBracket(loc)),
            ';' => Ok(Token::Semicolon(loc)),
            ',' => Ok(Token::Comma(loc)),

            _ => Err(LexError {
                message: format!("Unexpected character: '{}'", ch),
                location: loc,
            }),
        }
    }

    /// Parse string literal
    fn string_literal(&mut self) -> Result<Token, LexError> {
        let loc = SourceLocation::new(self.line, self.column - 1);
        let mut string = String::new();

        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance(); // consume closing quote
                return Ok(Token::StringLiteral(string, loc));
            }

            if ch == '\\' {
                self.advance();
                let escaped = self.advance().ok_or_else(|| LexError {
                    message: "Unexpected end of file in string literal"
                        .to_string(),
                    location: self.current_location(),
                })?;

                let unescaped = match escaped {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '"' => '"',
                    '0' => '\0',
                    _ => {
                        return Err(LexError {
                            message: format!(
                                "Unknown escape sequence: \\{}",
                                escaped
                            ),
                            location: self.current_location(),
                        });
                    }
                };
                string.push(unescaped);
            } else {
                string.push(ch);
                self.advance();
            }
        }

        Err(LexError {
            message: "Unterminated string literal".to_string(),
            location: loc,
        })
    }

    /// Parse character literal
    fn char_literal(&mut self) -> Result<Token, LexError> {
        let loc = SourceLocation::new(self.line, self.column - 1);

        let ch = self.advance().ok_or_else(|| LexError {
            message: "Unexpected end of file in character literal".to_string(),
            location: self.current_location(),
        })?;

        let value = if ch == '\\' {
            // Handle escape sequences
            let escaped = self.advance().ok_or_else(|| LexError {
                message: "Unexpected end of file in character literal"
                    .to_string(),
                location: self.current_location(),
            })?;

            match escaped {
                'n' => '\n' as i8,
                't' => '\t' as i8,
                'r' => '\r' as i8,
                '\\' => '\\' as i8,
                '\'' => '\'' as i8,
                '0' => 0,
                'x' => {
                    // Hex escape: \xHH
                    let hex1 = self.advance().ok_or_else(|| LexError {
                        message: "Incomplete hex escape sequence".to_string(),
                        location: self.current_location(),
                    })?;
                    let hex2 = self.advance().ok_or_else(|| LexError {
                        message: "Incomplete hex escape sequence".to_string(),
                        location: self.current_location(),
                    })?;

                    let hex_str = format!("{}{}", hex1, hex2);
                    u8::from_str_radix(&hex_str, 16).map(|v| v as i8).map_err(
                        |_| LexError {
                            message: format!(
                                "Invalid hex escape sequence: \\x{}",
                                hex_str
                            ),
                            location: self.current_location(),
                        },
                    )?
                }
                _ => {
                    return Err(LexError {
                        message: format!(
                            "Unknown escape sequence: \\{}",
                            escaped
                        ),
                        location: self.current_location(),
                    });
                }
            }
        } else {
            ch as i8
        };

        // Expect closing quote
        if self.advance() != Some('\'') {
            return Err(LexError {
                message: "Expected closing quote in character literal"
                    .to_string(),
                location: self.current_location(),
            });
        }

        Ok(Token::CharLiteral(value, loc))
    }

    /// Parse numeric literal (integers only)
    fn number_literal(&mut self, first_digit: char) -> Result<Token, LexError> {
        let loc = SourceLocation::new(self.line, self.column - 1);
        let mut num_str = String::new();
        num_str.push(first_digit);

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                num_str.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let value = num_str.parse::<i32>().map_err(|_| LexError {
            message: format!("Invalid integer literal: {}", num_str),
            location: loc,
        })?;

        Ok(Token::IntLiteral(value, loc))
    }

    /// Parse identifier or keyword
    fn identifier_or_keyword(
        &mut self,
        first_char: char,
    ) -> Result<Token, LexError> {
        let loc = SourceLocation::new(self.line, self.column - 1);
        let mut ident = String::new();
        ident.push(first_char);

        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check if it's a keyword
        let token = match ident.as_str() {
            "int" => Token::Int(loc),
            "char" => Token::Char(loc),
            "void" => Token::Void(loc),
            "struct" => Token::Struct(loc),
            "const" => Token::Const(loc),
            "if" => Token::If(loc),
            "else" => Token::Else(loc),
            "while" => Token::While(loc),
            "do" => Token::Do(loc),
            "for" => Token::For(loc),
            "switch" => Token::Switch(loc),
            "case" => Token::Case(loc),
            "default" => Token::Default(loc),
            "break" => Token::Break(loc),
            "continue" => Token::Continue(loc),
            "return" => Token::Return(loc),
            "goto" => Token::Goto(loc),
            "sizeof" => Token::Sizeof(loc),
            "NULL" => Token::Null(loc),
            _ => Token::Ident(ident, loc),
        };

        Ok(token)
    }

    /// Skip whitespace and comments
    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\r') | Some('\n') => {
                    self.advance();
                }
                Some('/') => {
                    if self.peek_ahead(1) == Some('/') {
                        // Single-line comment
                        self.skip_line_comment();
                    } else if self.peek_ahead(1) == Some('*') {
                        // Multi-line comment
                        self.skip_block_comment()?;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Skip single-line comment (// ...)
    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.peek() {
            self.advance();
            if ch == '\n' {
                break;
            }
        }
    }

    /// Skip multi-line comment (/* ... */)
    fn skip_block_comment(&mut self) -> Result<(), LexError> {
        let start_loc = self.current_location();
        self.advance(); // skip '/'
        self.advance(); // skip '*'

        while !self.is_at_end() {
            if self.peek() == Some('*') && self.peek_ahead(1) == Some('/') {
                self.advance(); // skip '*'
                self.advance(); // skip '/'
                return Ok(());
            }
            self.advance();
        }

        Err(LexError {
            message: "Unterminated block comment".to_string(),
            location: start_loc,
        })
    }

    /// Skip preprocessor directive (#include, etc.)
    fn skip_preprocessor_directive(&mut self) -> Result<(), LexError> {
        while let Some(ch) = self.peek() {
            self.advance();
            if ch == '\n' {
                break;
            }
        }
        Ok(())
    }

    /// Peek at current character without consuming
    fn peek(&self) -> Option<char> {
        if self.position < self.input.len() {
            Some(self.input[self.position])
        } else {
            None
        }
    }

    /// Peek ahead n characters
    fn peek_ahead(&self, n: usize) -> Option<char> {
        let pos = self.position + n;
        if pos < self.input.len() {
            Some(self.input[pos])
        } else {
            None
        }
    }

    /// Advance to next character
    fn advance(&mut self) -> Option<char> {
        if self.position >= self.input.len() {
            return None;
        }

        let ch = self.input[self.position];
        self.position += 1;

        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        Some(ch)
    }

    /// Check if at end of input
    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }

    /// Get current source location
    fn current_location(&self) -> SourceLocation {
        SourceLocation::new(self.line, self.column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokens() {
        let mut lexer = Lexer::new("int main() { return 0; }");
        let tokens = lexer.tokenize().unwrap();

        assert!(matches!(tokens[0], Token::Int(_)));
        assert!(matches!(tokens[1], Token::Ident(ref s, _) if s == "main"));
        assert!(matches!(tokens[2], Token::LParen(_)));
        assert!(matches!(tokens[3], Token::RParen(_)));
        assert!(matches!(tokens[4], Token::LBrace(_)));
        assert!(matches!(tokens[5], Token::Return(_)));
        assert!(matches!(tokens[6], Token::IntLiteral(0, _)));
        assert!(matches!(tokens[7], Token::Semicolon(_)));
        assert!(matches!(tokens[8], Token::RBrace(_)));
        assert!(matches!(tokens[9], Token::Eof(_)));
    }

    #[test]
    fn test_operators() {
        let mut lexer = Lexer::new("++ -- += -= == != && ||");
        let tokens = lexer.tokenize().unwrap();

        assert!(matches!(tokens[0], Token::PlusPlus(_)));
        assert!(matches!(tokens[1], Token::MinusMinus(_)));
        assert!(matches!(tokens[2], Token::PlusEq(_)));
        assert!(matches!(tokens[3], Token::MinusEq(_)));
        assert!(matches!(tokens[4], Token::EqEq(_)));
        assert!(matches!(tokens[5], Token::NotEq(_)));
        assert!(matches!(tokens[6], Token::AndAnd(_)));
        assert!(matches!(tokens[7], Token::OrOr(_)));
    }

    #[test]
    fn test_comments() {
        let mut lexer =
            Lexer::new("int x; // comment\nint y; /* block\ncomment */ int z;");
        let tokens = lexer.tokenize().unwrap();

        // Should skip comments
        assert!(matches!(tokens[0], Token::Int(_)));
        assert!(matches!(tokens[1], Token::Ident(ref s, _) if s == "x"));
        assert!(matches!(tokens[2], Token::Semicolon(_)));
        assert!(matches!(tokens[3], Token::Int(_)));
        assert!(matches!(tokens[4], Token::Ident(ref s, _) if s == "y"));
        assert!(matches!(tokens[5], Token::Semicolon(_)));
        assert!(matches!(tokens[6], Token::Int(_)));
        assert!(matches!(tokens[7], Token::Ident(ref s, _) if s == "z"));
    }

    #[test]
    fn test_string_literal() {
        let mut lexer = Lexer::new(r#""hello\nworld""#);
        let tokens = lexer.tokenize().unwrap();

        match &tokens[0] {
            Token::StringLiteral(s, _) => {
                assert_eq!(s, "hello\nworld");
            }
            _ => panic!("Expected string literal"),
        }
    }

    #[test]
    fn test_preprocessor_skip() {
        let mut lexer = Lexer::new("#include <stdio.h>\nint x;");
        let tokens = lexer.tokenize().unwrap();

        // Should skip #include line
        assert!(matches!(tokens[0], Token::Int(_)));
        assert!(matches!(tokens[1], Token::Ident(ref s, _) if s == "x"));
    }
}
