//! Type inference and type compatibility
//!
//! This module provides type inference for expressions, which is necessary for:
//!
//! - `sizeof(expr)`: Computing the size of an expression's result type
//! - Type checking: Ensuring operations are performed on compatible types
//! - Pointer arithmetic: Computing correct offsets based on pointed-to type
//!
//! # Type Inference Rules
//!
//! - Literals have their natural type (int for numbers, char for characters)
//! - Variables are looked up in the current stack frame
//! - Binary operations follow C's type promotion rules
//! - Pointer dereference yields the pointed-to type
//! - Struct member access yields the field's type

use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::value::Value;
use crate::parser::ast::*;

impl Interpreter {
    /// Infer the type of an expression
    /// This is needed for sizeof(expr) to work properly
    pub(crate) fn infer_expr_type(&mut self, expr: &AstNode) -> Result<Type, RuntimeError> {
        match expr {
            AstNode::IntLiteral(_, _) => Ok(Type::new(BaseType::Int)),

            AstNode::CharLiteral(_, _) => Ok(Type::new(BaseType::Char)),

            AstNode::StringLiteral(_, _) => {
                // String literals have type char*
                Ok(Type::new(BaseType::Char).with_pointer())
            }

            AstNode::Null { .. } => {
                // NULL has type void*
                Ok(Type::new(BaseType::Void).with_pointer())
            }

            AstNode::Variable(name, location) => {
                // Look up variable type in current frame
                let var = self.get_current_frame_var(name, *location)?;

                Ok(var.var_type.clone())
            }

            AstNode::BinaryOp {
                op,
                left,
                right,
                location: _,
            } => {
                // Most binary ops return int, but pointer arithmetic returns pointer
                match op {
                    BinOp::Add | BinOp::Sub => {
                        // Check if either operand is a pointer
                        let left_type = self.infer_expr_type(left)?;
                        let right_type = self.infer_expr_type(right)?;

                        if left_type.pointer_depth > 0 {
                            Ok(left_type)
                        } else if right_type.pointer_depth > 0 {
                            Ok(right_type)
                        } else {
                            Ok(Type::new(BaseType::Int))
                        }
                    }
                    _ => Ok(Type::new(BaseType::Int)),
                }
            }

            AstNode::UnaryOp {
                op,
                operand,
                location,
            } => {
                match op {
                    UnOp::Deref => {
                        // *ptr: if operand is T*, result is T
                        let operand_type = self.infer_expr_type(operand)?;
                        if operand_type.pointer_depth == 0 {
                            return Err(RuntimeError::TypeError {
                                expected: "pointer".to_string(),
                                got: format!("{:?}", operand_type),
                                location: *location,
                            });
                        }
                        let mut result_type = operand_type;
                        result_type.pointer_depth -= 1;
                        Ok(result_type)
                    }
                    UnOp::AddrOf => {
                        // &var: if operand is T, result is T*
                        let operand_type = self.infer_expr_type(operand)?;
                        let mut result_type = operand_type;
                        result_type.pointer_depth += 1;
                        Ok(result_type)
                    }
                    UnOp::Neg | UnOp::BitNot => Ok(Type::new(BaseType::Int)),
                    UnOp::Not => Ok(Type::new(BaseType::Int)), // logical not returns int (0 or 1)
                    UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec => {
                        // ++/-- returns the type of the operand
                        self.infer_expr_type(operand)
                    }
                }
            }

            AstNode::TernaryOp { true_expr, .. } => {
                // Ternary operator returns the type of the true branch (simplified)
                // In real C, it's more complex with implicit conversions
                self.infer_expr_type(true_expr)
            }

            AstNode::FunctionCall { name, location, .. } => {
                // Look up function return type
                let func_def = self.function_defs.get(name).ok_or_else(|| {
                    RuntimeError::UndefinedFunction {
                        name: name.clone(),
                        location: *location,
                    }
                })?;
                Ok(func_def.return_type.clone())
            }

            AstNode::ArrayAccess {
                array, location, ..
            } => {
                // arr[i]: if arr is T[], result is T
                let array_type = self.infer_expr_type(array)?;

                if !array_type.array_dims.is_empty() {
                    // Array type - remove one dimension
                    let mut result_type = array_type;
                    result_type.array_dims.remove(0);
                    Ok(result_type)
                } else if array_type.pointer_depth > 0 {
                    // Pointer type - dereference
                    let mut result_type = array_type;
                    result_type.pointer_depth -= 1;
                    Ok(result_type)
                } else {
                    Err(RuntimeError::TypeError {
                        expected: "array or pointer".to_string(),
                        got: format!("{:?}", array_type),
                        location: *location,
                    })
                }
            }

            AstNode::MemberAccess {
                object,
                member,
                location,
            } => {
                // obj.field: get the field type from the struct definition
                let object_type = self.infer_expr_type(object)?;

                let struct_name = match &object_type.base {
                    BaseType::Struct(name) => name,
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "struct".to_string(),
                            got: format!("{:?}", object_type),
                            location: *location,
                        });
                    }
                };

                let field_type = self.get_field_type(struct_name, member, *location)?;
                Ok(field_type)
            }

            AstNode::PointerMemberAccess {
                object,
                member,
                location,
            } => {
                // ptr->field: dereference pointer then get field type
                let pointer_type = self.infer_expr_type(object)?;

                if pointer_type.pointer_depth == 0 {
                    return Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", pointer_type),
                        location: *location,
                    });
                }

                let struct_name = match &pointer_type.base {
                    BaseType::Struct(name) => name,
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "struct pointer".to_string(),
                            got: format!("{:?}", pointer_type),
                            location: *location,
                        });
                    }
                };

                let field_type = self.get_field_type(struct_name, member, *location)?;
                Ok(field_type)
            }

            AstNode::Cast { target_type, .. } => {
                // Cast returns the target type
                Ok(target_type.clone())
            }

            AstNode::SizeofType { .. } | AstNode::SizeofExpr { .. } => {
                // sizeof returns int
                Ok(Type::new(BaseType::Int))
            }

            _ => Err(RuntimeError::UnsupportedOperation {
                message: format!("Cannot infer type of expression: {:?}", expr),
                location: SourceLocation::new(1, 1),
            }),
        }
    }

    #[inline]
    pub(crate) fn coerce_value_to_type(
        &self,
        value: Value,
        target_type: &Type,
        _location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if target_type.pointer_depth > 0 || !target_type.array_dims.is_empty() {
            return Ok(value);
        }

        match (&target_type.base, &value) {
            (BaseType::Char, Value::Int(n)) => Ok(Value::Char((*n & 0xFF) as i8)),
            (BaseType::Char, Value::Char(_)) => Ok(value),
            (BaseType::Int, Value::Char(c)) => Ok(Value::Int(*c as i32)),
            (BaseType::Int, Value::Int(_)) => Ok(value),
            _ => Ok(value),
        }
    }

    /// Convert a value to a boolean (for conditionals)
    #[inline]
    pub(crate) fn value_to_bool(
        val: &Value,
        location: SourceLocation,
    ) -> Result<bool, RuntimeError> {
        match val {
            Value::Int(n) => Ok(*n != 0),
            Value::Char(c) => Ok(*c != 0),
            Value::Pointer(_) => Ok(true),
            Value::Null => Ok(false),
            _ => Err(RuntimeError::TypeError {
                expected: "int or pointer".to_string(),
                got: format!("{:?}", val),
                location,
            }),
        }
    }
}
