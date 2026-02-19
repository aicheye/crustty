use crate::interpreter::constants::HEAP_ADDRESS_START;
use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::{BinOp, SourceLocation};

impl Interpreter {
    /// Helper to coerce numeric types (Char, Int) to i32
    #[inline]
    pub(crate) fn coerce_to_int(&self, value: &Value) -> Option<i32> {
        match value {
            Value::Int(n) => Some(*n),
            // Explicit cast to i32 handles sign extension for i8 (Char)
            Value::Char(c) => Some(*c as i32),
            _ => None,
        }
    }

    /// Helper to get the size of the type pointed to by a pointer
    pub(crate) fn get_pointer_scale(
        &self,
        addr: u64,
        location: SourceLocation,
    ) -> Result<u64, RuntimeError> {
        if addr < HEAP_ADDRESS_START {
            let (_, frame_depth, var_name) = self.resolve_stack_pointer(addr, location)?;
            // Logic to get variable without borrowing issues (resolve returns clones/indices)
            let frames = self.stack.frames();
            let frame = frames
                .get(frame_depth)
                .ok_or(RuntimeError::InvalidFrameDepth { location })?;
            let var = frame
                .get_var(&var_name)
                .ok_or(RuntimeError::UndefinedVariable {
                    name: var_name,
                    location,
                })?;

            if !var.var_type.array_dims.is_empty() {
                let elem_type = var.var_type.element_type();
                Ok(sizeof_type(&elem_type, &self.struct_defs) as u64)
            } else {
                Ok(sizeof_type(&var.var_type, &self.struct_defs) as u64)
            }
        } else {
            let pointee = self
                .pointer_types
                .get(&addr)
                .ok_or(RuntimeError::InvalidPointer {
                    message: format!("Unknown type for pointer 0x{:x}", addr),
                    address: Some(addr),
                    location,
                })?;
            Ok(sizeof_type(pointee, &self.struct_defs) as u64)
        }
    }

    #[inline]
    pub(crate) fn checked_add_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        // 1. Numeric addition (Int/Char + Int/Char)
        if let (Some(a), Some(b)) = (self.coerce_to_int(left), self.coerce_to_int(right)) {
            return a
                .checked_add(b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} + {}", a, b),
                    location,
                })
                .map(Value::Int);
        }

        // 2. Pointer arithmetic
        match (left, right) {
            (Value::Pointer(addr), right_val) | (right_val, Value::Pointer(addr)) => {
                if let Some(offset) = self.coerce_to_int(right_val) {
                    let scale = self.get_pointer_scale(*addr, location)?;
                    let scaled_offset = offset as i64 * scale as i64;
                    Ok(Value::Pointer((*addr as i64 + scaled_offset) as u64))
                } else {
                    Err(RuntimeError::TypeError {
                        expected: "int or pointer".to_string(),
                        got: format!("{:?} + {:?}", left, right),
                        location,
                    })
                }
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
        // 1. Numeric subtraction
        if let (Some(a), Some(b)) = (self.coerce_to_int(left), self.coerce_to_int(right)) {
            return a
                .checked_sub(b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} - {}", a, b),
                    location,
                })
                .map(Value::Int);
        }

        match (left, right) {
            (Value::Pointer(addr), right_val) => {
                if let Some(offset) = self.coerce_to_int(right_val) {
                    let scale = self.get_pointer_scale(*addr, location)?;
                    let scaled_offset = offset as i64 * scale as i64;
                    Ok(Value::Pointer((*addr as i64 - scaled_offset) as u64))
                } else if let Value::Pointer(addr2) = right_val {
                    let scale = self.get_pointer_scale(*addr, location)?;
                    let diff_bytes = (*addr as i64) - (*addr2 as i64);
                    let diff_elems = if scale > 0 {
                        diff_bytes / scale as i64
                    } else {
                        0
                    };
                    Ok(Value::Int(diff_elems as i32))
                } else {
                    Err(RuntimeError::TypeError {
                        expected: "int or pointer".to_string(),
                        got: format!("{:?} - {:?}", left, right),
                        location,
                    })
                }
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
        if let (Some(a), Some(b)) = (self.coerce_to_int(left), self.coerce_to_int(right)) {
            return a
                .checked_mul(b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} * {}", a, b),
                    location,
                })
                .map(Value::Int);
        }

        Err(RuntimeError::TypeError {
            expected: "int".to_string(),
            got: format!("{:?} * {:?}", left, right),
            location,
        })
    }

    #[inline]
    pub(crate) fn checked_div_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if let (Some(a), Some(b)) = (self.coerce_to_int(left), self.coerce_to_int(right)) {
            if b == 0 {
                return Err(RuntimeError::DivisionError {
                    operation: "Division by zero".to_string(),
                    location,
                });
            }
            return a
                .checked_div(b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} / {}", a, b),
                    location,
                })
                .map(Value::Int);
        }

        Err(RuntimeError::TypeError {
            expected: "int".to_string(),
            got: format!("{:?} / {:?}", left, right),
            location,
        })
    }

    #[inline]
    pub(crate) fn checked_mod_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if let (Some(a), Some(b)) = (self.coerce_to_int(left), self.coerce_to_int(right)) {
            if b == 0 {
                return Err(RuntimeError::DivisionError {
                    operation: "Modulo by zero".to_string(),
                    location,
                });
            }
            return a
                .checked_rem(b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} % {}", a, b),
                    location,
                })
                .map(Value::Int);
        }

        Err(RuntimeError::TypeError {
            expected: "int".to_string(),
            got: format!("{:?} % {:?}", left, right),
            location,
        })
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
        F: Fn(i64, i64) -> bool,
    {
        if let (Some(a), Some(b)) = (self.coerce_to_int(left), self.coerce_to_int(right)) {
            return Ok(Value::Int(if cmp(a as i64, b as i64) { 1 } else { 0 }));
        }

        match (left, right) {
            (Value::Pointer(a), Value::Pointer(b)) => {
                Ok(Value::Int(if cmp(*a as i64, *b as i64) { 1 } else { 0 }))
            }
            (Value::Pointer(a), Value::Null) | (Value::Null, Value::Pointer(a)) => {
                Ok(Value::Int(if cmp(*a as i64, 0) { 1 } else { 0 }))
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
        if let (Some(a), Some(b)) = (self.coerce_to_int(left), self.coerce_to_int(right)) {
            let result = match op {
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                BinOp::BitXor => a ^ b,
                BinOp::BitShl => a << b,
                BinOp::BitShr => a >> b,
                _ => unreachable!(),
            };
            return Ok(Value::Int(result));
        }

        Err(RuntimeError::TypeError {
            expected: "int".to_string(),
            got: format!("{:?} op {:?}", left, right),
            location,
        })
    }

    pub(crate) fn evaluate_binary_op(
        &mut self,
        op: &BinOp,
        left: &crate::parser::ast::AstNode,
        right: &crate::parser::ast::AstNode,
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

                    And | Or => unreachable!(
                        "Logical AND/OR must be handled in evaluate_expr for short-circuiting"
                    ),

                    BitAnd | BitOr | BitXor | BitShl | BitShr => {
                        self.bitwise_op(&left_val, &right_val, op, location)
                    }

                    _ => unreachable!("Compound assignment should be handled above"),
                }
            }
        }
    }
}
