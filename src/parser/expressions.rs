//! Expression parsing implementation
//!
//! This module handles parsing of C expressions using precedence climbing
//! for binary operators and recursive descent for other expression forms.
//!
//! # Supported Expressions
//!
//! - Literals: integers, characters, strings
//! - Identifiers and variables
//! - Binary operators: arithmetic, comparison, logical, bitwise
//! - Unary operators: `-`, `!`, `&`, `*`, `++`, `--`
//! - Postfix: `[]`, `.`, `->`, `()`, `++`, `--`
//! - Ternary: `? :`
//! - Type casts: `(type)expr`
//! - `sizeof` operator
//!
//! # Precedence
//!
//! Binary operators follow C precedence rules using a precedence climbing
//! algorithm for efficient and correct parsing.
//!
//! All parsing methods are implemented as `pub(crate)` methods on the [`Parser`] struct.

use crate::parser::ast::*;
use crate::parser::lexer::Token;
use crate::parser::parse::{ParseError, Parser};

impl Parser {
    /// Parse expression (top-level entry point)
    pub(crate) fn parse_expression(&mut self) -> Result<AstNode, ParseError> {
        self.parse_assignment()
    }

    /// Parse assignment or ternary (right-associative)
    fn parse_assignment(&mut self) -> Result<AstNode, ParseError> {
        let expr = self.parse_ternary()?;

        // Check for assignment operators
        let loc = self.current_location();
        if self.match_token(&Token::Eq(loc)) {
            let rhs = Box::new(self.parse_assignment()?);
            return Ok(AstNode::Assignment {
                lhs: Box::new(expr),
                rhs,
                location: loc,
            });
        }

        // Compound assignments
        let compound_op = if self.match_token(&Token::PlusEq(loc)) {
            Some(BinOp::AddAssign)
        } else if self.match_token(&Token::MinusEq(loc)) {
            Some(BinOp::SubAssign)
        } else if self.match_token(&Token::StarEq(loc)) {
            Some(BinOp::MulAssign)
        } else if self.match_token(&Token::SlashEq(loc)) {
            Some(BinOp::DivAssign)
        } else if self.match_token(&Token::PercentEq(loc)) {
            Some(BinOp::ModAssign)
        } else {
            None
        };

        if let Some(op) = compound_op {
            let rhs = Box::new(self.parse_assignment()?);
            return Ok(AstNode::CompoundAssignment {
                lhs: Box::new(expr),
                op,
                rhs,
                location: loc,
            });
        }

        Ok(expr)
    }

    /// Parse ternary: condition ? true_expr : false_expr
    fn parse_ternary(&mut self) -> Result<AstNode, ParseError> {
        let expr = self.parse_logical_or()?;

        if self.match_token(&Token::Question(self.current_location())) {
            let loc = self.previous_location();
            let true_expr = Box::new(self.parse_expression()?);
            self.expect_token(
                &Token::Colon(self.current_location()),
                "Expected ':' in ternary expression",
            )?;
            let false_expr = Box::new(self.parse_ternary()?);

            return Ok(AstNode::TernaryOp {
                condition: Box::new(expr),
                true_expr,
                false_expr,
                location: loc,
            });
        }

        Ok(expr)
    }

    /// Parse logical OR (||)
    fn parse_logical_or(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_logical_and()?;

        while self.match_token(&Token::OrOr(self.current_location())) {
            let loc = self.previous_location();
            let right = Box::new(self.parse_logical_and()?);
            left = AstNode::BinaryOp {
                op: BinOp::Or,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse logical AND (&&)
    fn parse_logical_and(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_bitwise_or()?;

        while self.match_token(&Token::AndAnd(self.current_location())) {
            let loc = self.previous_location();
            let right = Box::new(self.parse_bitwise_or()?);
            left = AstNode::BinaryOp {
                op: BinOp::And,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse bitwise OR (|)
    fn parse_bitwise_or(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_bitwise_xor()?;

        while self.match_token(&Token::Pipe(self.current_location())) {
            let loc = self.previous_location();
            let right = Box::new(self.parse_bitwise_xor()?);
            left = AstNode::BinaryOp {
                op: BinOp::BitOr,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse bitwise XOR (^)
    fn parse_bitwise_xor(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_bitwise_and()?;

        while self.match_token(&Token::Caret(self.current_location())) {
            let loc = self.previous_location();
            let right = Box::new(self.parse_bitwise_and()?);
            left = AstNode::BinaryOp {
                op: BinOp::BitXor,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse bitwise AND (&)
    fn parse_bitwise_and(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_equality()?;

        while self.match_token(&Token::Amp(self.current_location())) {
            let loc = self.previous_location();
            let right = Box::new(self.parse_equality()?);
            left = AstNode::BinaryOp {
                op: BinOp::BitAnd,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse equality (== !=)
    fn parse_equality(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_relational()?;

        loop {
            let loc = self.current_location();
            let op = if self.match_token(&Token::EqEq(loc)) {
                BinOp::Eq
            } else if self.match_token(&Token::NotEq(loc)) {
                BinOp::Ne
            } else {
                break;
            };

            let right = Box::new(self.parse_relational()?);
            left = AstNode::BinaryOp {
                op,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse relational (< <= > >=)
    fn parse_relational(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_shift()?;

        loop {
            let loc = self.current_location();
            let op = if self.match_token(&Token::Lt(loc)) {
                BinOp::Lt
            } else if self.match_token(&Token::Le(loc)) {
                BinOp::Le
            } else if self.match_token(&Token::Gt(loc)) {
                BinOp::Gt
            } else if self.match_token(&Token::Ge(loc)) {
                BinOp::Ge
            } else {
                break;
            };

            let right = Box::new(self.parse_shift()?);
            left = AstNode::BinaryOp {
                op,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse bitwise shift (<< >>)
    fn parse_shift(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_additive()?;

        loop {
            let loc = self.current_location();
            let op = if self.match_token(&Token::LtLt(loc)) {
                BinOp::BitShl
            } else if self.match_token(&Token::GtGt(loc)) {
                BinOp::BitShr
            } else {
                break;
            };

            let right = Box::new(self.parse_additive()?);
            left = AstNode::BinaryOp {
                op,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse additive (+ -)
    fn parse_additive(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let loc = self.current_location();
            let op = if self.match_token(&Token::Plus(loc)) {
                BinOp::Add
            } else if self.match_token(&Token::Minus(loc)) {
                BinOp::Sub
            } else {
                break;
            };

            let right = Box::new(self.parse_multiplicative()?);
            left = AstNode::BinaryOp {
                op,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse multiplicative (* / %)
    fn parse_multiplicative(&mut self) -> Result<AstNode, ParseError> {
        let mut left = self.parse_cast()?;

        loop {
            let loc = self.current_location();
            let op = if self.match_token(&Token::Star(loc)) {
                BinOp::Mul
            } else if self.match_token(&Token::Slash(loc)) {
                BinOp::Div
            } else if self.match_token(&Token::Percent(loc)) {
                BinOp::Mod
            } else {
                break;
            };

            let right = Box::new(self.parse_cast()?);
            left = AstNode::BinaryOp {
                op,
                left: Box::new(left),
                right,
                location: loc,
            };
        }

        Ok(left)
    }

    /// Parse cast: (Type*)expr
    fn parse_cast(&mut self) -> Result<AstNode, ParseError> {
        // Check for cast: ( followed by type keyword
        if self.check(&Token::LParen(self.current_location())) {
            let saved_pos = self.position;

            // Try parsing as cast
            if self.try_parse_cast().is_ok() {
                // Restore and actually parse it
                self.position = saved_pos;
                self.advance(); // consume '('
                let target_type = self.parse_type()?;
                self.expect_token(
                    &Token::RParen(self.current_location()),
                    "Expected ')' after cast type",
                )?;
                let loc = self.previous_location();
                let expr = Box::new(self.parse_cast()?);

                return Ok(AstNode::Cast {
                    target_type,
                    expr,
                    location: loc,
                });
            } else {
                self.position = saved_pos;
            }
        }

        self.parse_unary()
    }

    /// Try to parse cast (used for lookahead)
    fn try_parse_cast(&mut self) -> Result<(), ParseError> {
        if !self.match_token(&Token::LParen(self.current_location())) {
            return Err(ParseError {
                message: "Not a cast".to_string(),
                location: self.current_location(),
            });
        }

        self.parse_type()?;

        if !self.match_token(&Token::RParen(self.current_location())) {
            return Err(ParseError {
                message: "Not a cast".to_string(),
                location: self.current_location(),
            });
        }

        Ok(())
    }

    /// Parse unary (! ~ - + & * ++ -- sizeof)
    fn parse_unary(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.current_location();

        // Prefix operators
        if self.match_token(&Token::Bang(loc)) {
            let operand = Box::new(self.parse_unary()?);
            return Ok(AstNode::UnaryOp {
                op: UnOp::Not,
                operand,
                location: loc,
            });
        }

        if self.match_token(&Token::Tilde(loc)) {
            let operand = Box::new(self.parse_unary()?);
            return Ok(AstNode::UnaryOp {
                op: UnOp::BitNot,
                operand,
                location: loc,
            });
        }

        if self.match_token(&Token::Minus(loc)) {
            let operand = Box::new(self.parse_unary()?);
            return Ok(AstNode::UnaryOp {
                op: UnOp::Neg,
                operand,
                location: loc,
            });
        }

        if self.match_token(&Token::Plus(loc)) {
            // Unary plus: just return the operand
            return self.parse_unary();
        }

        if self.match_token(&Token::Amp(loc)) {
            let operand = Box::new(self.parse_unary()?);
            return Ok(AstNode::UnaryOp {
                op: UnOp::AddrOf,
                operand,
                location: loc,
            });
        }

        if self.match_token(&Token::Star(loc)) {
            let operand = Box::new(self.parse_unary()?);
            return Ok(AstNode::UnaryOp {
                op: UnOp::Deref,
                operand,
                location: loc,
            });
        }

        if self.match_token(&Token::PlusPlus(loc)) {
            let operand = Box::new(self.parse_unary()?);
            return Ok(AstNode::UnaryOp {
                op: UnOp::PreInc,
                operand,
                location: loc,
            });
        }

        if self.match_token(&Token::MinusMinus(loc)) {
            let operand = Box::new(self.parse_unary()?);
            return Ok(AstNode::UnaryOp {
                op: UnOp::PreDec,
                operand,
                location: loc,
            });
        }

        if self.match_token(&Token::Sizeof(loc)) {
            self.expect_token(
                &Token::LParen(self.current_location()),
                "Expected '(' after 'sizeof'",
            )?;

            // Try to parse as type
            let saved_pos = self.position;
            if self.is_type_keyword() {
                let target_type = self.parse_type()?;
                if self.match_token(&Token::RParen(self.current_location())) {
                    return Ok(AstNode::SizeofType {
                        target_type,
                        location: loc,
                    });
                }
            }

            // Otherwise, parse as expression
            self.position = saved_pos;
            let expr = Box::new(self.parse_expression()?);
            self.expect_token(
                &Token::RParen(self.current_location()),
                "Expected ')' after sizeof expression",
            )?;

            return Ok(AstNode::SizeofExpr {
                expr,
                location: loc,
            });
        }

        self.parse_postfix()
    }

    /// Parse postfix (++ -- [] . -> ())
    fn parse_postfix(&mut self) -> Result<AstNode, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            let loc = self.current_location();

            if self.match_token(&Token::PlusPlus(loc)) {
                expr = AstNode::UnaryOp {
                    op: UnOp::PostInc,
                    operand: Box::new(expr),
                    location: loc,
                };
            } else if self.match_token(&Token::MinusMinus(loc)) {
                expr = AstNode::UnaryOp {
                    op: UnOp::PostDec,
                    operand: Box::new(expr),
                    location: loc,
                };
            } else if self.match_token(&Token::LBracket(loc)) {
                let index = Box::new(self.parse_expression()?);
                self.expect_token(
                    &Token::RBracket(self.current_location()),
                    "Expected ']' after array index",
                )?;
                expr = AstNode::ArrayAccess {
                    array: Box::new(expr),
                    index,
                    location: loc,
                };
            } else if self.match_token(&Token::Dot(loc)) {
                let member = self.expect_identifier()?;
                expr = AstNode::MemberAccess {
                    object: Box::new(expr),
                    member,
                    location: loc,
                };
            } else if self.match_token(&Token::Arrow(loc)) {
                let member = self.expect_identifier()?;
                expr = AstNode::PointerMemberAccess {
                    object: Box::new(expr),
                    member,
                    location: loc,
                };
            } else if self.match_token(&Token::LParen(loc)) {
                // Function call
                let args = self.parse_argument_list()?;
                self.expect_token(
                    &Token::RParen(self.current_location()),
                    "Expected ')' after function arguments",
                )?;

                // Extract function name from expr
                let name = if let AstNode::Variable(n, _) = expr {
                    n
                } else {
                    return Err(ParseError {
                        message: "Function call must be on identifier".to_string(),
                        location: loc,
                    });
                };

                expr = AstNode::FunctionCall {
                    name,
                    args,
                    location: loc,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parse argument list: (expr, expr, ...)
    fn parse_argument_list(&mut self) -> Result<Vec<AstNode>, ParseError> {
        let mut args = Vec::new();

        if self.check(&Token::RParen(self.current_location())) {
            return Ok(args);
        }

        loop {
            args.push(self.parse_expression()?);

            if !self.match_token(&Token::Comma(self.current_location())) {
                break;
            }
        }

        Ok(args)
    }

    /// Parse primary (literals, variables, parenthesized expressions)
    fn parse_primary(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.current_location();

        // Integer literal
        if let Token::IntLiteral(n, loc) = self.peek_token() {
            self.advance();
            return Ok(AstNode::IntLiteral(n, loc));
        }

        // Character literal
        if let Token::CharLiteral(c, loc) = self.peek_token() {
            self.advance();
            return Ok(AstNode::CharLiteral(c, loc));
        }

        // String literal
        if let Token::StringLiteral(s, loc) = self.peek_token() {
            self.advance();
            return Ok(AstNode::StringLiteral(s, loc));
        }

        // NULL
        if self.match_token(&Token::Null(loc)) {
            return Ok(AstNode::Null { location: loc });
        }

        // Identifier
        if let Token::Ident(name, loc) = self.peek_token() {
            self.advance();
            return Ok(AstNode::Variable(name, loc));
        }

        // Parenthesized expression
        if self.match_token(&Token::LParen(loc)) {
            let expr = self.parse_expression()?;
            self.expect_token(
                &Token::RParen(self.current_location()),
                "Expected ')' after expression",
            )?;
            return Ok(expr);
        }

        Err(ParseError {
            message: format!("Unexpected token: {}", self.peek()),
            location: loc,
        })
    }
}
