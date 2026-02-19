//! Declaration parsing implementation
//!
//! This module handles parsing of top-level declarations in C programs:
//!
//! - Struct definitions: `struct Name { ... }`
//! - Function definitions: `type name(params) { ... }`
//! - Type parsing: base types, pointers, arrays
//! - Function parameters and struct fields
//!
//! # Grammar
//!
//! ```text
//! declaration ::= struct_def | function_def
//! struct_def  ::= "struct" identifier "{" field_list "}"
//! function_def ::= type identifier "(" params ")" "{" statements "}"
//! type        ::= base_type pointer* array_dims*
//! ```
//!
//! All parsing methods are implemented as `pub(crate)` methods on the [`Parser`] struct.

use crate::parser::ast::*;
use crate::parser::lexer::Token;
use crate::parser::parse::{ParseError, Parser};

impl Parser {
    /// Parse a top-level declaration (function or struct definition)
    pub(crate) fn parse_top_level_declaration(&mut self) -> Result<AstNode, ParseError> {
        // Check for struct definition vs function with struct return type
        // We need to distinguish:
        //   struct Name { ... };           <- struct definition
        //   struct Name func_name(...) ... <- function with struct return type
        if self.check(&Token::Struct(self.current_location())) {
            // Look ahead to determine which it is
            // Save position to restore if needed
            let saved_pos = self.position;
            self.advance(); // consume 'struct'

            if matches!(self.peek_token(), Token::Ident(_, _)) {
                self.advance(); // consume struct name

                // Check what follows the struct name
                if self.check(&Token::LBrace(self.current_location())) {
                    // It's a struct definition: struct Name { ... }
                    // Restore position and parse as struct def
                    self.position = saved_pos;
                    self.match_token(&Token::Struct(self.current_location())); // consume 'struct' again
                    return self.parse_struct_definition();
                } else {
                    // It's a function with struct return type: struct Name func_name(...)
                    // Restore position and parse as function
                    self.position = saved_pos;
                    return self.parse_function_definition();
                }
            }

            // Restore position if we couldn't determine
            self.position = saved_pos;
        }

        // Otherwise, parse function definition
        self.parse_function_definition()
    }

    /// Parse struct definition: struct Name { fields };
    pub(crate) fn parse_struct_definition(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        let name = self.expect_identifier()?;

        self.expect_lbrace("after struct name")?;

        let mut fields = Vec::new();
        while !self.check(&Token::RBrace(self.current_location())) {
            let field_type = self.parse_type()?;
            let field_name = self.expect_identifier()?;
            self.expect_semicolon("after struct field")?;

            fields.push(Field {
                name: field_name,
                field_type,
            });
        }

        self.expect_rbrace("after struct fields")?;
        self.expect_semicolon("after struct definition")?;

        Ok(AstNode::StructDef {
            name,
            fields,
            location: loc,
        })
    }

    /// Parse function definition: type name(params) { body }
    pub(crate) fn parse_function_definition(&mut self) -> Result<AstNode, ParseError> {
        let return_type = self.parse_type()?;
        let name = self.expect_identifier()?;
        let loc = self.previous_location();

        self.expect_lparen("after function name")?;

        let params = self.parse_parameter_list()?;

        self.expect_rparen("after parameters")?;
        self.expect_lbrace("before function body")?;

        let body = self.parse_block_statements()?;

        self.expect_rbrace("after function body")?;

        Ok(AstNode::FunctionDef {
            name,
            params,
            return_type,
            body,
            location: loc,
        })
    }

    /// Parse parameter list: (type name, type name, ...)
    pub(crate) fn parse_parameter_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();

        if self.check(&Token::RParen(self.current_location())) {
            return Ok(params);
        }

        // Special case: (void) means no parameters in C
        if self.check(&Token::Void(self.current_location())) {
            self.advance(); // consume 'void'
            return Ok(params);
        }

        loop {
            let param_type = self.parse_type()?;
            let param_name = self.expect_identifier()?;
            params.push(Param {
                name: param_name,
                param_type,
            });

            if !self.match_token(&Token::Comma(self.current_location())) {
                break;
            }
        }

        Ok(params)
    }

    /// Parse type: [const] base_type [*]* [[size]]*
    pub(crate) fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut is_const = false;
        if self.match_token(&Token::Const(self.current_location())) {
            is_const = true;
        }

        // Parse base type
        let base = if self.match_token(&Token::Int(self.current_location())) {
            BaseType::Int
        } else if self.match_token(&Token::Char(self.current_location())) {
            BaseType::Char
        } else if self.match_token(&Token::Void(self.current_location())) {
            BaseType::Void
        } else if self.match_token(&Token::Struct(self.current_location())) {
            let name = self.expect_identifier()?;
            BaseType::Struct(name)
        } else {
            return Err(ParseError {
                message: format!("Expected type, found {}", self.peek()),
                location: self.current_location(),
            });
        };

        let mut pointer_depth = 0;
        while self.match_token(&Token::Star(self.current_location())) {
            pointer_depth += 1;
        }

        let mut array_dims = Vec::new();
        while self.match_token(&Token::LBracket(self.current_location())) {
            if self.check(&Token::RBracket(self.current_location())) {
                // Unsized array []
                array_dims.push(None);
                self.advance();
            } else {
                // Sized array [N]
                let size_expr = self.parse_expression()?;
                // For now, require compile-time constant (int literal)
                if let AstNode::IntLiteral(n, _) = size_expr {
                    array_dims.push(Some(n as usize));
                } else {
                    return Err(ParseError {
                        message: "Array size must be a constant integer".to_string(),
                        location: self.current_location(),
                    });
                }
                self.expect_token(
                    &Token::RBracket(self.current_location()),
                    "Expected ']' after array size",
                )?;
            }
        }

        Ok(Type {
            base,
            is_const,
            pointer_depth,
            array_dims,
        })
    }
}
