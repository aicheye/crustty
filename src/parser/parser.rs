use crate::parser::ast::*;
use crate::parser::lexer::{Lexer, Token, LexError};
use std::fmt;

/// Parser error type
#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub location: SourceLocation,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parse error at line {}, column {}: {}",
               self.location.line, self.location.column, self.message)
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
    tokens: Vec<Token>,
    position: usize,
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

    /// Parse a top-level declaration (function or struct definition)
    fn parse_top_level_declaration(&mut self) -> Result<AstNode, ParseError> {
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
    fn parse_struct_definition(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        let name = self.expect_identifier()?;

        self.expect_token(&Token::LBrace(self.current_location()), "Expected '{' after struct name")?;

        let mut fields = Vec::new();
        while !self.check(&Token::RBrace(self.current_location())) {
            let field_type = self.parse_type()?;
            let field_name = self.expect_identifier()?;
            self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after struct field")?;

            fields.push(Field {
                name: field_name,
                field_type,
            });
        }

        self.expect_token(&Token::RBrace(self.current_location()), "Expected '}' after struct fields")?;
        self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after struct definition")?;

        Ok(AstNode::StructDef {
            name,
            fields,
            location: loc,
        })
    }

    /// Parse function definition: type name(params) { body }
    fn parse_function_definition(&mut self) -> Result<AstNode, ParseError> {
        let return_type = self.parse_type()?;
        let name = self.expect_identifier()?;
        let loc = self.previous_location();

        self.expect_token(&Token::LParen(self.current_location()), "Expected '(' after function name")?;

        let params = self.parse_parameter_list()?;

        self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after parameters")?;
        self.expect_token(&Token::LBrace(self.current_location()), "Expected '{' before function body")?;

        let body = self.parse_block_statements()?;

        self.expect_token(&Token::RBrace(self.current_location()), "Expected '}' after function body")?;

        Ok(AstNode::FunctionDef {
            name,
            params,
            return_type,
            body,
            location: loc,
        })
    }

    /// Parse parameter list: (type name, type name, ...)
    fn parse_parameter_list(&mut self) -> Result<Vec<Param>, ParseError> {
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
    fn parse_type(&mut self) -> Result<Type, ParseError> {
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
                self.expect_token(&Token::RBracket(self.current_location()), "Expected ']' after array size")?;
            }
        }

        Ok(Type {
            base,
            is_const,
            pointer_depth,
            array_dims,
        })
    }

    /// Parse block statements (inside braces, excluding the braces themselves)
    fn parse_block_statements(&mut self) -> Result<Vec<AstNode>, ParseError> {
        let mut statements = Vec::new();

        while !self.check(&Token::RBrace(self.current_location())) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }

        Ok(statements)
    }

    /// Parse a statement
    fn parse_statement(&mut self) -> Result<AstNode, ParseError> {
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
            self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after 'break'")?;
            return Ok(AstNode::Break { location: loc });
        }

        if self.match_token(&Token::Continue(loc)) {
            self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after 'continue'")?;
            return Ok(AstNode::Continue { location: loc });
        }

        if self.match_token(&Token::Goto(loc)) {
            let label = self.expect_identifier()?;
            self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after 'goto'")?;
            return Ok(AstNode::Goto { label, location: loc });
        }

        // Check for label: identifier followed by colon
        if let Token::Ident(_, _) = self.peek_token() {
            if self.peek_ahead(1).map(|t| matches!(t, Token::Colon(_))).unwrap_or(false) {
                let name = self.expect_identifier()?;
                self.expect_token(&Token::Colon(self.current_location()), "Expected ':' after label")?;
                return Ok(AstNode::Label { name, location: loc });
            }
        }

        // Check for variable declaration (type followed by identifier)
        if self.is_type_keyword() {
            return self.parse_variable_declaration();
        }

        // Otherwise, it's an expression statement
        let expr = self.parse_expression()?;
        self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after expression")?;
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

        self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after return")?;

        Ok(AstNode::Return { expr, location: loc })
    }

    /// Parse if statement
    fn parse_if_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        self.expect_token(&Token::LParen(self.current_location()), "Expected '(' after 'if'")?;
        let condition = Box::new(self.parse_expression()?);
        self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after if condition")?;

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

        self.expect_token(&Token::LParen(self.current_location()), "Expected '(' after 'while'")?;
        let condition = Box::new(self.parse_expression()?);
        self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after while condition")?;

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

        self.expect_token(&Token::While(self.current_location()), "Expected 'while' after do body")?;
        self.expect_token(&Token::LParen(self.current_location()), "Expected '(' after 'while'")?;
        let condition = Box::new(self.parse_expression()?);
        self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after do-while condition")?;
        self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after do-while")?;

        Ok(AstNode::DoWhile {
            body,
            condition,
            location: loc,
        })
    }

    /// Parse for statement
    fn parse_for_statement(&mut self) -> Result<AstNode, ParseError> {
        let loc = self.previous_location();

        self.expect_token(&Token::LParen(self.current_location()), "Expected '(' after 'for'")?;

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
            self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after for init")?;
            Some(Box::new(expr))
        };

        // Condition (optional)
        let condition = if self.check(&Token::Semicolon(self.current_location())) {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };
        self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after for condition")?;

        // Increment (optional)
        let increment = if self.check(&Token::RParen(self.current_location())) {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };

        self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after for clauses")?;

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

        self.expect_token(&Token::LParen(self.current_location()), "Expected '(' after 'switch'")?;
        let expr = Box::new(self.parse_expression()?);
        self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after switch expression")?;
        self.expect_token(&Token::LBrace(self.current_location()), "Expected '{' before switch body")?;

        let mut cases = Vec::new();

        while !self.check(&Token::RBrace(self.current_location())) && !self.is_at_end() {
            if self.match_token(&Token::Case(self.current_location())) {
                let case_loc = self.previous_location(); // Capture case keyword location
                let value = self.parse_expression()?;
                self.expect_token(&Token::Colon(self.current_location()), "Expected ':' after case value")?;

                let mut statements = Vec::new();
                while !self.check(&Token::Case(self.current_location()))
                    && !self.check(&Token::Default(self.current_location()))
                    && !self.check(&Token::RBrace(self.current_location()))
                    && !self.is_at_end() {
                    statements.push(self.parse_statement()?);
                }

                cases.push(CaseNode::Case {
                    value: Box::new(value),
                    statements,
                    location: case_loc,
                });
            } else if self.match_token(&Token::Default(self.current_location())) {
                let default_loc = self.previous_location(); // Capture default keyword location
                self.expect_token(&Token::Colon(self.current_location()), "Expected ':' after 'default'")?;

                let mut statements = Vec::new();
                while !self.check(&Token::Case(self.current_location()))
                    && !self.check(&Token::Default(self.current_location()))
                    && !self.check(&Token::RBrace(self.current_location()))
                    && !self.is_at_end() {
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

        self.expect_token(&Token::RBrace(self.current_location()), "Expected '}' after switch body")?;

        Ok(AstNode::Switch {
            expr,
            cases,
            location: loc,
        })
    }

    /// Parse variable declaration: type name[[size]]* [= init];
    /// Supports C-style array declarations: int arr[5];
    fn parse_variable_declaration(&mut self) -> Result<AstNode, ParseError> {
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
                self.expect_token(&Token::RBracket(self.current_location()), "Expected ']' after array size")?;
            }
        }

        let init = if self.match_token(&Token::Eq(self.current_location())) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        self.expect_token(&Token::Semicolon(self.current_location()), "Expected ';' after variable declaration")?;

        Ok(AstNode::VarDecl {
            name,
            var_type,
            init,
            location: loc,
        })
    }

    /// Parse statement or block (for if/while/for bodies)
    fn parse_statement_or_block(&mut self) -> Result<Vec<AstNode>, ParseError> {
        if self.match_token(&Token::LBrace(self.current_location())) {
            let statements = self.parse_block_statements()?;
            self.expect_token(&Token::RBrace(self.current_location()), "Expected '}' after block")?;
            Ok(statements)
        } else {
            // Single statement
            Ok(vec![self.parse_statement()?])
        }
    }

    /// Parse expression (top-level entry point)
    fn parse_expression(&mut self) -> Result<AstNode, ParseError> {
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
            self.expect_token(&Token::Colon(self.current_location()), "Expected ':' in ternary expression")?;
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
                self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after cast type")?;
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
            self.expect_token(&Token::LParen(self.current_location()), "Expected '(' after 'sizeof'")?;

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
            self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after sizeof expression")?;

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
                self.expect_token(&Token::RBracket(self.current_location()), "Expected ']' after array index")?;
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
                self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after function arguments")?;

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
            self.expect_token(&Token::RParen(self.current_location()), "Expected ')' after expression")?;
            return Ok(expr);
        }

        Err(ParseError {
            message: format!("Unexpected token: {}", self.peek()),
            location: loc,
        })
    }

    // ===== Helper methods =====

    fn is_type_keyword(&self) -> bool {
        matches!(
            self.peek_token(),
            Token::Int(_) | Token::Char(_) | Token::Void(_) | Token::Struct(_) | Token::Const(_)
        )
    }

    fn match_token(&mut self, token: &Token) -> bool {
        if std::mem::discriminant(&self.peek_token()) == std::mem::discriminant(token) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, token: &Token) -> bool {
        std::mem::discriminant(&self.peek_token()) == std::mem::discriminant(token)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.position += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_token(), Token::Eof(_))
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn peek_token(&self) -> Token {
        self.tokens[self.position].clone()
    }

    fn peek_ahead(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.position + n)
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.position - 1]
    }

    fn previous_location(&self) -> SourceLocation {
        self.previous().location()
    }

    fn current_location(&self) -> SourceLocation {
        self.peek().location()
    }

    fn expect_token(&mut self, token: &Token, message: &str) -> Result<(), ParseError> {
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

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
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
            AstNode::FunctionDef { name, params, return_type, body, .. } => {
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
