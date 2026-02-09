//! Statement parsing implementation
//!
//! This module handles parsing of all C statement types:
//!
//! - Variable declarations: `int x = 42;`
//! - Control flow: `if`, `while`, `for`, `do-while`, `switch`
//! - Jump statements: `return`, `break`, `continue`
//! - Compound statements: `{ ... }`
//! - Expression statements: function calls, assignments
//!
//! # Grammar
//!
//! ```text
//! statement ::= var_decl | if_stmt | while_stmt | for_stmt
//!             | do_while_stmt | switch_stmt | return_stmt
//!             | break_stmt | continue_stmt | block | expr_stmt
//! ```
//!
//! All parsing methods are implemented as `pub(crate)` methods on the [`Parser`] struct.

use crate::parser::ast::*;
use crate::parser::lexer::Token;
use crate::parser::parse::{ParseError, Parser};

impl Parser {
    /// Parse block statements (inside braces, excluding the braces themselves)
    pub(crate) fn parse_block_statements(&mut self) -> Result<Vec<AstNode>, ParseError> {
        let mut statements = Vec::new();

        while !self.check(&Token::RBrace(self.current_location())) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }

        Ok(statements)
    }

    /// Parse a statement
    pub(crate) fn parse_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.current_location();

        // Check for keywords first
        if self.match_token(&Token::Return(loc)) {
            return self.parse_return_statement();
        }

        if self.match_token(&Token::If(loc)) {
            return self.parse_if_statement();
        }

        if self.match_token(&Token::While(loc)) {
            return self.parse_while_statement();
        }

        if self.match_token(&Token::Do(loc)) {
            return self.parse_do_while_statement();
        }

        if self.match_token(&Token::For(loc)) {
            return self.parse_for_statement();
        }

        if self.match_token(&Token::Switch(loc)) {
            return self.parse_switch_statement();
        }

        if self.match_token(&Token::Break(loc)) {
            self.expect_token(
                &Token::Semicolon(self.current_location()),
                "Expected ';' after 'break'",
            )?;
            return Ok(AstNode::Break { location: loc });
        }

        if self.match_token(&Token::Continue(loc)) {
            self.expect_token(
                &Token::Semicolon(self.current_location()),
                "Expected ';' after 'continue'",
            )?;
            return Ok(AstNode::Continue { location: loc });
        }

        if self.match_token(&Token::Goto(loc)) {
            let label = self.expect_identifier()?;
            self.expect_token(
                &Token::Semicolon(self.current_location()),
                "Expected ';' after 'goto'",
            )?;
            return Ok(AstNode::Goto {
                label,
                location: loc,
            });
        }

        if self.match_token(&Token::LBrace(loc)) {
            let statements = self.parse_block_statements()?;
            self.expect_token(
                &Token::RBrace(self.current_location()),
                "Expected '}' after block",
            )?;
            return Ok(AstNode::Block {
                statements,
                location: loc,
            });
        }

        // Check for label: identifier followed by colon
        if let Token::Ident(_, _) = self.peek_token() {
            if self
                .peek_ahead(1)
                .map(|t| matches!(t, Token::Colon(_)))
                .unwrap_or(false)
            {
                let name = self.expect_identifier()?;
                self.expect_token(
                    &Token::Colon(self.current_location()),
                    "Expected ':' after label",
                )?;
                return Ok(AstNode::Label {
                    name,
                    location: loc,
                });
            }
        }

        // Check for variable declaration (type followed by identifier)
        if self.is_type_keyword() {
            return self.parse_variable_declaration();
        }

        // Otherwise, it's an expression statement
        let expr = self.parse_expression()?;
        self.expect_token(
            &Token::Semicolon(self.current_location()),
            "Expected ';' after expression",
        )?;
        Ok(AstNode::ExpressionStatement {
            expr: Box::new(expr),
            location: loc,
        })
    }

    /// Parse return statement
    fn parse_return_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        let expr = if self.check(&Token::Semicolon(self.current_location())) {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };

        self.expect_token(
            &Token::Semicolon(self.current_location()),
            "Expected ';' after return",
        )?;

        Ok(AstNode::Return {
            expr,
            location: loc,
        })
    }

    /// Parse if statement
    fn parse_if_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        self.expect_token(
            &Token::LParen(self.current_location()),
            "Expected '(' after 'if'",
        )?;
        let condition = Box::new(self.parse_expression()?);
        self.expect_token(
            &Token::RParen(self.current_location()),
            "Expected ')' after if condition",
        )?;

        let then_branch = self.parse_statement_or_block()?;

        let else_branch = if self.match_token(&Token::Else(self.current_location())) {
            Some(self.parse_statement_or_block()?)
        } else {
            None
        };

        Ok(AstNode::If {
            condition,
            then_branch,
            else_branch,
            location: loc,
        })
    }

    /// Parse while statement
    fn parse_while_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        self.expect_token(
            &Token::LParen(self.current_location()),
            "Expected '(' after 'while'",
        )?;
        let condition = Box::new(self.parse_expression()?);
        self.expect_token(
            &Token::RParen(self.current_location()),
            "Expected ')' after while condition",
        )?;

        let body = self.parse_statement_or_block()?;

        Ok(AstNode::While {
            condition,
            body,
            location: loc,
        })
    }

    /// Parse do-while statement
    fn parse_do_while_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        let body = self.parse_statement_or_block()?;

        self.expect_token(
            &Token::While(self.current_location()),
            "Expected 'while' after do body",
        )?;
        self.expect_token(
            &Token::LParen(self.current_location()),
            "Expected '(' after 'while'",
        )?;
        let condition = Box::new(self.parse_expression()?);
        self.expect_token(
            &Token::RParen(self.current_location()),
            "Expected ')' after do-while condition",
        )?;
        self.expect_token(
            &Token::Semicolon(self.current_location()),
            "Expected ';' after do-while",
        )?;

        Ok(AstNode::DoWhile {
            body,
            condition,
            location: loc,
        })
    }

    /// Parse for statement
    fn parse_for_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        self.expect_token(
            &Token::LParen(self.current_location()),
            "Expected '(' after 'for'",
        )?;

        // Init (optional)
        let init = if self.check(&Token::Semicolon(self.current_location())) {
            self.advance();
            None
        } else if self.is_type_keyword() {
            // Variable declaration
            let decl = self.parse_variable_declaration()?;
            // Declaration includes semicolon, so don't expect another
            Some(Box::new(decl))
        } else {
            // Expression
            let expr = self.parse_expression()?;
            self.expect_token(
                &Token::Semicolon(self.current_location()),
                "Expected ';' after for init",
            )?;
            Some(Box::new(expr))
        };

        // Condition (optional)
        let condition = if self.check(&Token::Semicolon(self.current_location())) {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };
        self.expect_token(
            &Token::Semicolon(self.current_location()),
            "Expected ';' after for condition",
        )?;

        // Increment (optional)
        let increment = if self.check(&Token::RParen(self.current_location())) {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };

        self.expect_token(
            &Token::RParen(self.current_location()),
            "Expected ')' after for clauses",
        )?;

        let body = self.parse_statement_or_block()?;

        Ok(AstNode::For {
            init,
            condition,
            increment,
            body,
            location: loc,
        })
    }

    /// Parse switch statement
    fn parse_switch_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        self.expect_token(
            &Token::LParen(self.current_location()),
            "Expected '(' after 'switch'",
        )?;
        let expr = Box::new(self.parse_expression()?);
        self.expect_token(
            &Token::RParen(self.current_location()),
            "Expected ')' after switch expression",
        )?;
        self.expect_token(
            &Token::LBrace(self.current_location()),
            "Expected '{' before switch body",
        )?;

        let mut cases = Vec::new();

        while !self.check(&Token::RBrace(self.current_location())) && !self.is_at_end() {
            if self.match_token(&Token::Case(self.current_location())) {
                let case_loc = self.previous_location(); // Capture case keyword location
                let value = self.parse_expression()?;
                self.expect_token(
                    &Token::Colon(self.current_location()),
                    "Expected ':' after case value",
                )?;

                let mut statements = Vec::new();
                while !self.check(&Token::Case(self.current_location()))
                    && !self.check(&Token::Default(self.current_location()))
                    && !self.check(&Token::RBrace(self.current_location()))
                    && !self.is_at_end()
                {
                    statements.push(self.parse_statement()?);
                }

                cases.push(CaseNode::Case {
                    value: Box::new(value),
                    statements,
                    location: case_loc,
                });
            } else if self.match_token(&Token::Default(self.current_location())) {
                let default_loc = self.previous_location(); // Capture default keyword location
                self.expect_token(
                    &Token::Colon(self.current_location()),
                    "Expected ':' after 'default'",
                )?;

                let mut statements = Vec::new();
                while !self.check(&Token::Case(self.current_location()))
                    && !self.check(&Token::Default(self.current_location()))
                    && !self.check(&Token::RBrace(self.current_location()))
                    && !self.is_at_end()
                {
                    statements.push(self.parse_statement()?);
                }

                cases.push(CaseNode::Default {
                    statements,
                    location: default_loc,
                });
            } else {
                return Err(ParseError {
                    message: "Expected 'case' or 'default' in switch body".to_string(),
                    location: self.current_location(),
                });
            }
        }

        self.expect_token(
            &Token::RBrace(self.current_location()),
            "Expected '}' after switch body",
        )?;

        Ok(AstNode::Switch {
            expr,
            cases,
            location: loc,
        })
    }

    /// Parse variable declaration: type name[[size]]* [= init];
    /// Supports C-style array declarations: int arr[5];
    pub(crate) fn parse_variable_declaration(&mut self) -> Result<AstNode, ParseError> {
        let mut var_type = self.parse_type()?;
        let name = self.expect_identifier()?;
        let loc = self.previous_location();

        // Check for C-style array dimensions after the variable name: int arr[5];
        while self.match_token(&Token::LBracket(self.current_location())) {
            if self.check(&Token::RBracket(self.current_location())) {
                // Unsized array []
                var_type.array_dims.push(None);
                self.advance();
            } else {
                // Sized array [N]
                let size_expr = self.parse_expression()?;
                // For now, require compile-time constant (int literal)
                if let AstNode::IntLiteral(n, _) = size_expr {
                    var_type.array_dims.push(Some(n as usize));
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

        let init = if self.match_token(&Token::Eq(self.current_location())) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        self.expect_token(
            &Token::Semicolon(self.current_location()),
            "Expected ';' after variable declaration",
        )?;

        Ok(AstNode::VarDecl {
            name,
            var_type,
            init,
            location: loc,
        })
    }

    /// Parse statement or block (for if/while/for bodies)
    pub(crate) fn parse_statement_or_block(&mut self) -> Result<Vec<AstNode>, ParseError> {
        if self.match_token(&Token::LBrace(self.current_location())) {
            let statements = self.parse_block_statements()?;
            self.expect_token(
                &Token::RBrace(self.current_location()),
                "Expected '}' after block",
            )?;
            Ok(statements)
        } else {
            // Single statement
            Ok(vec![self.parse_statement()?])
        }
    }
}
