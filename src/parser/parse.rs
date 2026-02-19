//! Main parser coordinator
//!
//! This module provides the [`Parser`] struct and core parsing infrastructure,
//! including error types, helper methods, and the main parse entry point.
//!
//! # Parser Architecture
//!
//! The Parser uses a recursive descent approach with the following organization:
//! - This module: Parser struct, helper methods, and coordination
//! - `declarations`: Parsing struct and function declarations
//! - `statements`: Parsing statements (if, while, for, etc.)
//! - `expressions`: Parsing expressions with precedence climbing
//!
//! # Implementation
//!
//! Parser methods are split across multiple files using `impl Parser` blocks,
//! allowing each module to extend the Parser with related functionality while
//! maintaining access to the shared parser state.

use crate::parser::ast::*;
use crate::parser::lexer::{LexError, Lexer, Token};
use std::fmt;

/// Parser error type
#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub location: SourceLocation,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Parse error at line {}, column {}: {}",
            self.location.line, self.location.column, self.message
        )
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(err: LexError) -> Self {
        ParseError {
            message: err.message,
            location: err.location,
        }
    }
}

/// Recursive descent parser for C subset
pub struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) position: usize,
}

impl Parser {
    pub fn new(source: &str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        Ok(Self {
            tokens,
            position: 0,
        })
    }

    /// Parse the entire program (top-level declarations)
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut program = Program::new();

        while !self.is_at_end() {
            // Parse top-level declaration (function or struct)
            let decl = self.parse_top_level_declaration()?;
            program.nodes.push(decl);
        }

        Ok(program)
    }

    // ===== Helper methods =====

    pub(crate) fn is_type_keyword(&self) -> bool {
        matches!(
            self.peek_token(),
            Token::Int(_)
                | Token::Char(_)
                | Token::Void(_)
                | Token::Struct(_)
                | Token::Const(_)
        )
    }

    pub(crate) fn match_token(&mut self, token: &Token) -> bool {
        if std::mem::discriminant(&self.peek_token())
            == std::mem::discriminant(token)
        {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn check(&self, token: &Token) -> bool {
        std::mem::discriminant(&self.peek_token())
            == std::mem::discriminant(token)
    }

    pub(crate) fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.position += 1;
        }
        self.previous()
    }

    pub(crate) fn is_at_end(&self) -> bool {
        matches!(self.peek_token(), Token::Eof(_))
    }

    pub(crate) fn peek(&self) -> &Token {
        &self.tokens[self.position]
    }

    pub(crate) fn peek_token(&self) -> Token {
        self.tokens[self.position].clone()
    }

    pub(crate) fn peek_ahead(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.position + n)
    }

    pub(crate) fn previous(&self) -> &Token {
        &self.tokens[self.position - 1]
    }

    pub(crate) fn previous_location(&self) -> SourceLocation {
        self.previous().location()
    }

    pub(crate) fn current_location(&self) -> SourceLocation {
        self.peek().location()
    }

    pub(crate) fn expect_token(
        &mut self,
        token: &Token,
        message: &str,
    ) -> Result<(), ParseError> {
        if self.check(token) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError {
                message: format!("{}, found {}", message, self.peek()),
                location: self.current_location(),
            })
        }
    }

    pub(crate) fn expect_lparen(
        &mut self,
        ctx: &str,
    ) -> Result<(), ParseError> {
        self.expect_token(
            &Token::LParen(self.current_location()),
            &format!("Expected '(' {ctx}"),
        )
    }

    pub(crate) fn expect_rparen(
        &mut self,
        ctx: &str,
    ) -> Result<(), ParseError> {
        self.expect_token(
            &Token::RParen(self.current_location()),
            &format!("Expected ')' {ctx}"),
        )
    }

    pub(crate) fn expect_lbrace(
        &mut self,
        ctx: &str,
    ) -> Result<(), ParseError> {
        self.expect_token(
            &Token::LBrace(self.current_location()),
            &format!("Expected '{{' {ctx}"),
        )
    }

    pub(crate) fn expect_rbrace(
        &mut self,
        ctx: &str,
    ) -> Result<(), ParseError> {
        self.expect_token(
            &Token::RBrace(self.current_location()),
            &format!("Expected '}}' {ctx}"),
        )
    }

    pub(crate) fn expect_semicolon(
        &mut self,
        ctx: &str,
    ) -> Result<(), ParseError> {
        self.expect_token(
            &Token::Semicolon(self.current_location()),
            &format!("Expected ';' {ctx}"),
        )
    }

    pub(crate) fn expect_identifier(&mut self) -> Result<String, ParseError> {
        if let Token::Ident(name, _) = self.peek_token() {
            self.advance();
            Ok(name)
        } else {
            Err(ParseError {
                message: format!("Expected identifier, found {}", self.peek()),
                location: self.current_location(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_function() {
        let source = "int main() { return 0; }";
        let mut parser = Parser::new(source).unwrap();
        let program = parser.parse_program().unwrap();

        assert_eq!(program.nodes.len(), 1);
        match &program.nodes[0] {
            AstNode::FunctionDef {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                assert_eq!(name, "main");
                assert_eq!(params.len(), 0);
                assert_eq!(return_type.base, BaseType::Int);
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected function definition"),
        }
    }

    #[test]
    fn test_parse_expression() {
        let source = "int main() { int x = 1 + 2 * 3; }";
        let mut parser = Parser::new(source).unwrap();
        let program = parser.parse_program().unwrap();

        // Just check it parses without error
        assert_eq!(program.nodes.len(), 1);
    }

    #[test]
    fn test_parse_if_statement() {
        let source = "int main() { if (x > 0) return 1; else return 0; }";
        let mut parser = Parser::new(source).unwrap();
        let program = parser.parse_program().unwrap();

        assert_eq!(program.nodes.len(), 1);
    }

    #[test]
    fn test_parse_struct() {
        let source = "struct Point { int x; int y; };";
        let mut parser = Parser::new(source).unwrap();
        let program = parser.parse_program().unwrap();

        assert_eq!(program.nodes.len(), 1);
        match &program.nodes[0] {
            AstNode::StructDef { name, fields, .. } => {
                assert_eq!(name, "Point");
                assert_eq!(fields.len(), 2);
            }
            _ => panic!("Expected struct definition"),
        }
    }
}
