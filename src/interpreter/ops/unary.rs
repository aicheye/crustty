//! Unary operator evaluation

use crate::interpreter::constants::HEAP_ADDRESS_START;
use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::*;

impl Interpreter {
    pub(crate) fn evaluate_unary_op(
        &mut self,
        op: &UnOp,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        use UnOp::*;

        match op {
            Neg => self.evaluate_neg_op(operand, location),
            Not => self.evaluate_not_op(operand, location),
            BitNot => self.evaluate_bitnot_op(operand, location),
            PreInc | PreDec | PostInc | PostDec => {
                self.evaluate_inc_dec_op(op, operand, location)
            }
            Deref => self.evaluate_deref_op(operand, location),
            AddrOf => self.evaluate_addr_of_op(operand, location),
        }
    }

    fn evaluate_neg_op(
        &mut self,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
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

    fn evaluate_not_op(
        &mut self,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        let val = self.evaluate_expr(operand)?;
        let b = Self::value_to_bool(&val, location)?;
        Ok(Value::Int(if b { 0 } else { 1 }))
    }

    fn evaluate_bitnot_op(
        &mut self,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
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

    fn evaluate_inc_dec_op(
        &mut self,
        op: &UnOp,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        use UnOp::*;
        let current_val = self.evaluate_expr(operand)?;
        let one = Value::Int(1);

        let new_val = match op {
            PreInc | PostInc => {
                self.checked_add_values(&current_val, &one, location)?
            }
            PreDec | PostDec => {
                self.checked_sub_values(&current_val, &one, location)?
            }
            _ => unreachable!(),
        };

        self.assign_to_lvalue(operand, new_val.clone(), location)?;

        match op {
            PreInc | PreDec => Ok(new_val),
            PostInc | PostDec => Ok(current_val),
            _ => unreachable!(),
        }
    }

    fn evaluate_deref_op(
        &mut self,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        let val = self.evaluate_expr(operand)?;
        match val {
            Value::Pointer(addr) => {
                if addr < HEAP_ADDRESS_START {
                    self.dereference_stack_pointer(addr, location)
                } else {
                    self.dereference_heap_pointer(addr, location)
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

    fn dereference_stack_pointer(
        &mut self,
        addr: u64,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        let (base_addr, frame_depth, var_name) =
            self.resolve_stack_pointer(addr, location)?;

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

        match &var.value {
            Value::Array(elements) => {
                let offset = addr - base_addr;
                let elem_type = var.var_type.element_type();
                let elem_size =
                    sizeof_type(&elem_type, &self.struct_defs) as u64;
                let idx = if elem_size > 0 { offset / elem_size } else { 0 };

                if idx as usize >= elements.len() {
                    return Err(RuntimeError::BufferOverrun {
                        index: idx as usize,
                        size: elements.len(),
                        location,
                    });
                }
                Ok(elements[idx as usize].clone())
            }
            _ => Ok(var.value.clone()),
        }
    }

    fn dereference_heap_pointer(
        &mut self,
        addr: u64,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        let pointee_type = self.pointer_types.get(&addr).cloned();

        if let Some(ptr_type) = pointee_type {
            self.deserialize_value_from_heap(&ptr_type, addr, location)
        } else {
            let bytes = self
                .heap
                .read_bytes_at(addr, 4)
                .map_err(|e| Self::map_heap_error(e, location))?;

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

    fn evaluate_addr_of_op(
        &mut self,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match operand {
            AstNode::Variable(name, _) => {
                let var = self.get_current_frame_var(name, location)?;

                Ok(Value::Pointer(var.address))
            }
            _ => Err(RuntimeError::UnsupportedOperation {
                message:
                    "Address-of operator only supports variables currently"
                        .to_string(),
                location,
            }),
        }
    }
}
