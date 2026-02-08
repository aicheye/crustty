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

use crate::interpreter::constants::HEAP_ADDRESS_START;
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
                    self.heap.write_byte(addr + i as u64, byte).map_err(|e| {
                        RuntimeError::InvalidMemoryOperation {
                            message: e,
                            location: *loc,
                        }
                    })?;
                }
                self.heap
                    .write_byte(addr + bytes.len() as u64, 0)
                    .map_err(|e| RuntimeError::InvalidMemoryOperation {
                        message: e,
                        location: *loc,
                    })?;

                Ok(Value::Pointer(addr))
            }

            AstNode::Null { .. } => Ok(Value::Null),

            AstNode::Variable(name, loc) => {
                let frame = self
                    .stack
                    .current_frame()
                    .ok_or(RuntimeError::NoStackFrame { location: *loc })?;

                let var = frame
                    .get_var(name)
                    .ok_or_else(|| RuntimeError::UndefinedVariable {
                        name: name.clone(),
                        location: *loc,
                    })?;

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
            } => {
                let obj_val = self.evaluate_expr(object)?;

                match obj_val {
                    Value::Struct(fields) => {
                        // Extract struct name from the object expression type
                        let obj_type = self.infer_expr_type(object)?;
                        let struct_name = match &obj_type.base {
                            BaseType::Struct(name) => name.clone(),
                            _ => "unknown".to_string(),
                        };

                        fields.get(member).cloned().ok_or_else(|| {
                            RuntimeError::MissingStructField {
                                struct_name,
                                field_name: member.clone(),
                                location: *location,
                            }
                        })
                    }
                    _ => Err(RuntimeError::TypeError {
                        expected: "struct".to_string(),
                        got: format!("{:?}", obj_val),
                        location: *location,
                    }),
                }
            }

            AstNode::PointerMemberAccess {
                object,
                member,
                location,
            } => {
                let ptr_val = self.evaluate_expr(object)?;

                match ptr_val {
                    Value::Pointer(addr) => {
                        if addr < HEAP_ADDRESS_START {
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::InvalidPointer {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    address: Some(addr),
                                    location: *location,
                                })?
                                .clone();

                            let frame = self.stack.frames().get(frame_depth).ok_or({
                                RuntimeError::InvalidFrameDepth {
                                    location: *location,
                                }
                            })?;

                            let var = frame.get_var(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location: *location,
                                }
                            })?;

                            match &var.value {
                                Value::Struct(fields) => {
                                    let obj_type = &var.var_type;
                                    let struct_name = match &obj_type.base {
                                        BaseType::Struct(name) => name.clone(),
                                        _ => "unknown".to_string(),
                                    };

                                    fields.get(member).cloned().ok_or_else(|| {
                                        RuntimeError::MissingStructField {
                                            struct_name,
                                            field_name: member.clone(),
                                            location: *location,
                                        }
                                    })
                                }
                                _ => Err(RuntimeError::TypeError {
                                    expected: "struct".to_string(),
                                    got: format!("{:?}", var.value),
                                    location: *location,
                                }),
                            }
                        } else {
                            let pointee_type = self.pointer_types.get(&addr)
                                .ok_or_else(|| RuntimeError::InvalidPointer {
                                    message: format!("Unknown type for pointer 0x{:x}. Did you cast the result of malloc?", addr),
                                    address: Some(addr),
                                    location: *location,
                                })?;

                            let struct_name = match &pointee_type.base {
                                BaseType::Struct(name) => name.clone(),
                                _ => {
                                    return Err(RuntimeError::TypeError {
                                        expected: "struct pointer".to_string(),
                                        got: format!("{:?}", pointee_type),
                                        location: *location,
                                    });
                                }
                            };

                            let offset =
                                self.calculate_field_offset(&struct_name, member, *location)?;
                            let field_type =
                                self.get_field_type(&struct_name, member, *location)?;

                            self.deserialize_value_from_heap(
                                &field_type,
                                addr + offset as u64,
                                *location,
                            )
                        }
                    }
                    Value::Null => Err(RuntimeError::NullDereference {
                        location: *location,
                    }),
                    _ => Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", ptr_val),
                        location: *location,
                    }),
                }
            }

            AstNode::ArrayAccess {
                array,
                index,
                location,
            } => {
                let arr_val = self.evaluate_expr(array)?;
                let idx_val = self.evaluate_expr(index)?;

                let idx = match idx_val {
                    Value::Int(i) => i,
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "int".to_string(),
                            got: format!("{:?}", idx_val),
                            location: *location,
                        });
                    }
                };

                match arr_val {
                    Value::Array(elements) => {
                        if idx < 0 || idx as usize >= elements.len() {
                            return Err(RuntimeError::BufferOverrun {
                                index: idx as usize,
                                size: elements.len(),
                                location: *location,
                            });
                        }
                        Ok(elements[idx as usize].clone())
                    }
                    Value::Pointer(addr) => {
                        if addr == 0 {
                            return Err(RuntimeError::NullDereference {
                                location: *location,
                            });
                        }

                        if addr < HEAP_ADDRESS_START {
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::InvalidPointer {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    address: Some(addr),
                                    location: *location,
                                })?
                                .clone();

                            let frame = self.stack.frames().get(frame_depth).ok_or({
                                RuntimeError::InvalidFrameDepth {
                                    location: *location,
                                }
                            })?;

                            let var = frame.get_var(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location: *location,
                                }
                            })?;

                            match &var.value {
                                Value::Array(elements) => {
                                    if idx < 0 || idx as usize >= elements.len() {
                                        return Err(RuntimeError::BufferOverrun {
                                            index: idx as usize,
                                            size: elements.len(),
                                            location: *location,
                                        });
                                    }
                                    Ok(elements[idx as usize].clone())
                                }
                                _ => {
                                    if idx == 0 {
                                        Ok(var.value.clone())
                                    } else {
                                        Err(RuntimeError::InvalidPointer {
                                            message: format!(
                                                "Pointer to non-array stack variable, index {} out of bounds",
                                                idx
                                            ),
                                            address: Some(addr),
                                            location: *location,
                                        })
                                    }
                                }
                            }
                        } else if let Some(elem_type) = self.pointer_types.get(&addr).cloned() {
                            let elem_size = sizeof_type(&elem_type, &self.struct_defs);

                            let offset = (idx as i64) * (elem_size as i64);
                            let target_addr = if offset >= 0 {
                                addr + (offset as u64)
                            } else {
                                addr - ((-offset) as u64)
                            };

                            self.deserialize_value_from_heap(&elem_type, target_addr, *location)
                        } else {
                            Err(RuntimeError::InvalidPointer {
                                message: format!(
                                    "Unknown pointer type for indexing at 0x{:x}",
                                    addr
                                ),
                                address: Some(addr),
                                location: *location,
                            })
                        }
                    }
                    _ => Err(RuntimeError::TypeError {
                        expected: "array or pointer".to_string(),
                        got: format!("{:?}", arr_val),
                        location: *location,
                    }),
                }
            }

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

    pub(crate) fn evaluate_binary_op(
        &mut self,
        op: &BinOp,
        left: &AstNode,
        right: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        use BinOp::*;

        match op {
            AddAssign | SubAssign | MulAssign | DivAssign | ModAssign => {
                let right_val = self.evaluate_expr(right)?;
                let left_val = self.evaluate_expr(left)?;

                let result = match op {
                    AddAssign => self.checked_add_values(&left_val, &right_val, location)?,
                    SubAssign => self.checked_sub_values(&left_val, &right_val, location)?,
                    MulAssign => self.checked_mul_values(&left_val, &right_val, location)?,
                    DivAssign => self.checked_div_values(&left_val, &right_val, location)?,
                    ModAssign => self.checked_mod_values(&left_val, &right_val, location)?,
                    _ => unreachable!(),
                };

                self.assign_to_lvalue(left, result.clone(), location)?;
                Ok(result)
            }

            _ => {
                let left_val = self.evaluate_expr(left)?;
                let right_val = self.evaluate_expr(right)?;

                match op {
                    Add => self.checked_add_values(&left_val, &right_val, location),
                    Sub => self.checked_sub_values(&left_val, &right_val, location),
                    Mul => self.checked_mul_values(&left_val, &right_val, location),
                    Div => self.checked_div_values(&left_val, &right_val, location),
                    Mod => self.checked_mod_values(&left_val, &right_val, location),

                    Eq => self.compare_values(&left_val, &right_val, |a, b| a == b, location),
                    Ne => self.compare_values(&left_val, &right_val, |a, b| a != b, location),
                    Lt => self.compare_values(&left_val, &right_val, |a, b| a < b, location),
                    Le => self.compare_values(&left_val, &right_val, |a, b| a <= b, location),
                    Gt => self.compare_values(&left_val, &right_val, |a, b| a > b, location),
                    Ge => self.compare_values(&left_val, &right_val, |a, b| a >= b, location),

                    And => {
                        let left_bool = Self::value_to_bool(&left_val, location)?;
                        if !left_bool {
                            Ok(Value::Int(0))
                        } else {
                            let right_bool = Self::value_to_bool(&right_val, location)?;
                            Ok(Value::Int(if right_bool { 1 } else { 0 }))
                        }
                    }
                    Or => {
                        let left_bool = Self::value_to_bool(&left_val, location)?;
                        if left_bool {
                            Ok(Value::Int(1))
                        } else {
                            let right_bool = Self::value_to_bool(&right_val, location)?;
                            Ok(Value::Int(if right_bool { 1 } else { 0 }))
                        }
                    }

                    BitAnd | BitOr | BitXor | BitShl | BitShr => {
                        self.bitwise_op(&left_val, &right_val, op, location)
                    }

                    _ => unreachable!("Compound assignment should be handled above"),
                }
            }
        }
    }

    pub(crate) fn evaluate_unary_op(
        &mut self,
        op: &UnOp,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        use UnOp::*;

        match op {
            Neg => {
                let val = self.evaluate_expr(operand)?;
                match val {
                    Value::Int(n) => n
                        .checked_neg()
                        .ok_or(RuntimeError::IntegerOverflow {
                            operation: format!("-{}", n),
                            location,
                        })
                        .map(Value::Int),
                    _ => Err(RuntimeError::TypeError {
                        expected: "int".to_string(),
                        got: format!("{:?}", val),
                        location,
                    }),
                }
            }

            Not => {
                let val = self.evaluate_expr(operand)?;
                let b = Self::value_to_bool(&val, location)?;
                Ok(Value::Int(if b { 0 } else { 1 }))
            }

            BitNot => {
                let val = self.evaluate_expr(operand)?;
                match val {
                    Value::Int(n) => Ok(Value::Int(!n)),
                    _ => Err(RuntimeError::TypeError {
                        expected: "int".to_string(),
                        got: format!("{:?}", val),
                        location,
                    }),
                }
            }

            PreInc | PreDec | PostInc | PostDec => {
                let current_val = self.evaluate_expr(operand)?;
                let one = Value::Int(1);

                let new_val = match op {
                    PreInc | PostInc => self.checked_add_values(&current_val, &one, location)?,
                    PreDec | PostDec => self.checked_sub_values(&current_val, &one, location)?,
                    _ => unreachable!(),
                };

                self.assign_to_lvalue(operand, new_val.clone(), location)?;

                match op {
                    PreInc | PreDec => Ok(new_val),
                    PostInc | PostDec => Ok(current_val),
                    _ => unreachable!(),
                }
            }

            Deref => {
                let val = self.evaluate_expr(operand)?;
                match val {
                    Value::Pointer(addr) => {
                        if addr < HEAP_ADDRESS_START {
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::InvalidPointer {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    address: Some(addr),
                                    location,
                                })?
                                .clone();

                            let frame = self
                                .stack
                                .frames()
                                .get(frame_depth)
                                .ok_or(RuntimeError::InvalidFrameDepth { location })?;

                            let var = frame.get_var(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location,
                                }
                            })?;

                            Ok(var.value.clone())
                        } else {
                            let pointee_type = self.pointer_types.get(&addr).cloned();

                            if let Some(ptr_type) = pointee_type {
                                self.deserialize_value_from_heap(&ptr_type, addr, location)
                            } else {
                                let bytes = self.heap.read_bytes_at(addr, 4).map_err(|e| {
                                    RuntimeError::InvalidMemoryOperation {
                                        message: format!(
                                            "Failed to read from address 0x{:x}: {}",
                                            addr, e
                                        ),
                                        location,
                                    }
                                })?;

                                if bytes.len() == 4 {
                                    let int_val = i32::from_le_bytes([
                                        bytes[0], bytes[1], bytes[2], bytes[3],
                                    ]);
                                    Ok(Value::Int(int_val))
                                } else {
                                    Err(RuntimeError::InvalidPointer {
                                        message: format!(
                                            "Cannot dereference pointer at 0x{:x}: type unknown. Did you cast the result of malloc?",
                                            addr
                                        ),
                                        address: Some(addr),
                                        location,
                                    })
                                }
                            }
                        }
                    }
                    Value::Null => Err(RuntimeError::NullDereference { location }),
                    _ => Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", val),
                        location,
                    }),
                }
            }

            AddrOf => match operand {
                AstNode::Variable(name, _) => {
                    let frame = self
                        .stack
                        .current_frame()
                        .ok_or(RuntimeError::NoStackFrame { location })?;

                    let var =
                        frame
                            .get_var(name)
                            .ok_or_else(|| RuntimeError::UndefinedVariable {
                                name: name.clone(),
                                location,
                            })?;

                    Ok(Value::Pointer(var.address))
                }
                _ => Err(RuntimeError::UnsupportedOperation {
                    message: "Address-of operator only supports variables currently".to_string(),
                    location,
                }),
            },
        }
    }

    #[inline]
    pub(crate) fn checked_add_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_add(*b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} + {}", a, b),
                    location,
                })
                .map(Value::Int),
            (Value::Pointer(addr), Value::Int(offset)) => {
                Ok(Value::Pointer((*addr as i64 + *offset as i64) as u64))
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int or pointer".to_string(),
                got: format!("{:?} + {:?}", left, right),
                location,
            }),
        }
    }

    #[inline]
    pub(crate) fn checked_sub_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_sub(*b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} - {}", a, b),
                    location,
                })
                .map(Value::Int),
            (Value::Pointer(addr), Value::Int(offset)) => {
                Ok(Value::Pointer((*addr as i64 - *offset as i64) as u64))
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int or pointer".to_string(),
                got: format!("{:?} - {:?}", left, right),
                location,
            }),
        }
    }

    #[inline]
    pub(crate) fn checked_mul_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_mul(*b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} * {}", a, b),
                    location,
                })
                .map(Value::Int),
            _ => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: format!("{:?} * {:?}", left, right),
                location,
            }),
        }
    }

    #[inline]
    pub(crate) fn checked_div_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(RuntimeError::DivisionError {
                        operation: "Division by zero".to_string(),
                        location,
                    });
                }
                a.checked_div(*b)
                    .ok_or(RuntimeError::IntegerOverflow {
                        operation: format!("{} / {}", a, b),
                        location,
                    })
                    .map(Value::Int)
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: format!("{:?} / {:?}", left, right),
                location,
            }),
        }
    }

    #[inline]
    pub(crate) fn checked_mod_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(RuntimeError::DivisionError {
                        operation: "Modulo by zero".to_string(),
                        location,
                    });
                }
                a.checked_rem(*b)
                    .ok_or(RuntimeError::IntegerOverflow {
                        operation: format!("{} % {}", a, b),
                        location,
                    })
                    .map(Value::Int)
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: format!("{:?} % {:?}", left, right),
                location,
            }),
        }
    }

    #[inline]
    pub(crate) fn compare_values<F>(
        &self,
        left: &Value,
        right: &Value,
        cmp: F,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError>
    where
        F: Fn(i32, i32) -> bool,
    {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(if cmp(*a, *b) { 1 } else { 0 })),
            (Value::Char(a), Value::Char(b)) => {
                Ok(Value::Int(if cmp(*a as i32, *b as i32) { 1 } else { 0 }))
            }
            (Value::Pointer(a), Value::Pointer(b)) => {
                Ok(Value::Int(if cmp(*a as i32, *b as i32) { 1 } else { 0 }))
            }
            (Value::Pointer(a), Value::Null) | (Value::Null, Value::Pointer(a)) => {
                Ok(Value::Int(if cmp(*a as i32, 0) { 1 } else { 0 }))
            }
            (Value::Null, Value::Null) => Ok(Value::Int(if cmp(0, 0) { 1 } else { 0 })),
            _ => Err(RuntimeError::TypeError {
                expected: "comparable types".to_string(),
                got: format!("{:?} vs {:?}", left, right),
                location,
            }),
        }
    }

    #[inline]
    pub(crate) fn bitwise_op(
        &self,
        left: &Value,
        right: &Value,
        op: &BinOp,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                let result = match op {
                    BinOp::BitAnd => a & b,
                    BinOp::BitOr => a | b,
                    BinOp::BitXor => a ^ b,
                    BinOp::BitShl => a << b,
                    BinOp::BitShr => a >> b,
                    _ => unreachable!(),
                };
                Ok(Value::Int(result))
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: format!("{:?} op {:?}", left, right),
                location,
            }),
        }
    }
}
