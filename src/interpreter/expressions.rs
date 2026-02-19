//! Expression evaluation implementation
//!
//! This module handles evaluation of all C expression types, including:
//!
//! - Literals (integers, characters, strings)
//! - Variables and identifiers
//! - Binary operators (arithmetic, comparison, logical, bitwise)
//! - Unary operators (negation, not, address-of, dereference, pre/post increment/decrement)
//! - Array subscripting and struct member access
//! - Function calls (including built-ins)
//! - Type casts and sizeof operator
//!
//! # Performance Optimizations
//!
//! Hot-path functions are marked with `#[inline]` for better performance:
//! - Arithmetic operations with overflow checking
//! - Value comparison
//! - Bitwise operations
//!
//! # Safety
//!
//! All arithmetic operations use checked math to detect overflows and emit
//! appropriate runtime errors rather than panicking or producing undefined behavior.

use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::*;

impl Interpreter {
    /// Evaluate an expression and return its value
    pub(crate) fn evaluate_expr(&mut self, expr: &AstNode) -> Result<Value, RuntimeError> {
        let location = Self::get_location(expr).unwrap_or(self.current_location);

        match expr {
            AstNode::IntLiteral(n, _) => Ok(Value::Int(*n)),

            AstNode::CharLiteral(c, _) => Ok(Value::Char(*c)),

            AstNode::StringLiteral(s, loc) => {
                let bytes = s.as_bytes();
                let addr =
                    self.heap
                        .allocate(bytes.len() + 1)
                        .map_err(|_| RuntimeError::OutOfMemory {
                            requested: bytes.len() + 1,
                            limit: self.heap.max_size(),
                        })?;

                for (i, &byte) in bytes.iter().enumerate() {
                    self.heap
                        .write_byte(addr + i as u64, byte)
                        .map_err(|e| Self::map_heap_error(e, *loc))?;
                }
                self.heap
                    .write_byte(addr + bytes.len() as u64, 0)
                    .map_err(|e| Self::map_heap_error(e, *loc))?;

                Ok(Value::Pointer(addr))
            }

            AstNode::Null { .. } => Ok(Value::Null),

            AstNode::Variable(name, loc) => {
                let var = self.get_current_frame_var(name, *loc)?;

                if !var.var_type.array_dims.is_empty() {
                    Ok(Value::Pointer(var.address))
                } else {
                    if !var.init_state.is_initialized() {
                        return Err(RuntimeError::UninitializedRead {
                            var: name.clone(),
                            address: Some(var.address),
                            location: *loc,
                        });
                    }
                    Ok(var.value.clone())
                }
            }

            AstNode::BinaryOp {
                op: BinOp::And,
                left,
                right,
                location,
            } => {
                let left_val = self.evaluate_expr(left)?;
                if !Self::value_to_bool(&left_val, *location)? {
                    Ok(Value::Int(0))
                } else {
                    let right_val = self.evaluate_expr(right)?;
                    let right_bool = Self::value_to_bool(&right_val, *location)?;
                    Ok(Value::Int(if right_bool { 1 } else { 0 }))
                }
            }

            AstNode::BinaryOp {
                op: BinOp::Or,
                left,
                right,
                location,
            } => {
                let left_val = self.evaluate_expr(left)?;
                if Self::value_to_bool(&left_val, *location)? {
                    Ok(Value::Int(1))
                } else {
                    let right_val = self.evaluate_expr(right)?;
                    let right_bool = Self::value_to_bool(&right_val, *location)?;
                    Ok(Value::Int(if right_bool { 1 } else { 0 }))
                }
            }

            AstNode::BinaryOp {
                op,
                left,
                right,
                location,
            } => self.evaluate_binary_op(op, left, right, *location),

            AstNode::UnaryOp {
                op,
                operand,
                location,
            } => self.evaluate_unary_op(op, operand, *location),

            AstNode::TernaryOp {
                condition,
                true_expr,
                false_expr,
                location,
            } => {
                let cond_val = self.evaluate_expr(condition)?;
                let cond_int = Self::value_to_bool(&cond_val, *location)?;

                if cond_int {
                    self.evaluate_expr(true_expr)
                } else {
                    self.evaluate_expr(false_expr)
                }
            }

            AstNode::FunctionCall {
                name,
                args,
                location,
            } => self.execute_function_call(name, args, *location),

            AstNode::Cast {
                target_type,
                expr,
                location: _,
            } => {
                let val = self.evaluate_expr(expr)?;

                if let Value::Pointer(addr) = val {
                    if target_type.pointer_depth > 0 {
                        let mut pointee_type = target_type.clone();
                        pointee_type.pointer_depth -= 1;
                        self.pointer_types.insert(addr, pointee_type);
                    }
                }

                Ok(val)
            }

            AstNode::MemberAccess {
                object,
                member,
                location,
            } => self.evaluate_member_access(object, member, *location),

            AstNode::PointerMemberAccess {
                object,
                member,
                location,
            } => self.evaluate_pointer_member_access(object, member, *location),

            AstNode::ArrayAccess {
                array,
                index,
                location,
            } => self.evaluate_array_access(array, index, *location),

            AstNode::Assignment { lhs, rhs, location } => {
                let value = self.evaluate_expr(rhs)?;
                self.assign_to_lvalue(lhs, value.clone(), *location)?;
                Ok(value)
            }

            AstNode::SizeofType {
                target_type,
                location: _,
            } => {
                let size = sizeof_type(target_type, &self.struct_defs);
                Ok(Value::Int(size as i32))
            }

            AstNode::SizeofExpr { expr, location: _ } => {
                let expr_type = self.infer_expr_type(expr)?;
                let size = sizeof_type(&expr_type, &self.struct_defs);
                Ok(Value::Int(size as i32))
            }

            AstNode::CompoundAssignment {
                lhs,
                op,
                rhs,
                location,
            } => {
                self.execute_compound_assignment(lhs, op, rhs, *location)?;
                self.evaluate_expr(lhs)
            }

            _ => Err(RuntimeError::UnsupportedOperation {
                message: format!("Cannot evaluate expression: {:?}", expr),
                location,
            }),
        }
    }
}
