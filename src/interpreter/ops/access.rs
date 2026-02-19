use crate::interpreter::constants::HEAP_ADDRESS_START;
use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::{AstNode, BaseType, SourceLocation};

impl Interpreter {
    pub(crate) fn evaluate_member_access(
        &mut self,
        object: &AstNode,
        member: &str,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
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
                        field_name: member.to_string(),
                        location,
                    }
                })
            }
            _ => Err(RuntimeError::TypeError {
                expected: "struct".to_string(),
                got: format!("{:?}", obj_val),
                location,
            }),
        }
    }

    pub(crate) fn evaluate_pointer_member_access(
        &mut self,
        object: &AstNode,
        member: &str,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        let ptr_val = self.evaluate_expr(object)?;

        match ptr_val {
            Value::Pointer(addr) => {
                if addr < HEAP_ADDRESS_START {
                    let (_base_addr, frame_depth, var_name) =
                        self.resolve_stack_pointer(addr, location)?;

                    let frame =
                        self.stack.frames().get(frame_depth).ok_or(
                            RuntimeError::InvalidFrameDepth { location },
                        )?;

                    let var = frame.get_var(&var_name).ok_or_else(|| {
                        RuntimeError::UndefinedVariable {
                            name: var_name.clone(),
                            location,
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
                                    field_name: member.to_string(),
                                    location,
                                }
                            })
                        }
                        _ => Err(RuntimeError::TypeError {
                            expected: "struct".to_string(),
                            got: format!("{:?}", var.value),
                            location,
                        }),
                    }
                } else {
                    let pointee_type = self.pointer_types.get(&addr)
                        .ok_or_else(|| RuntimeError::InvalidPointer {
                            message: format!("Unknown type for pointer 0x{:x}. Did you cast the result of malloc?", addr),
                            address: Some(addr),
                            location,
                        })?;

                    let struct_name = match &pointee_type.base {
                        BaseType::Struct(name) => name.clone(),
                        _ => {
                            return Err(RuntimeError::TypeError {
                                expected: "struct pointer".to_string(),
                                got: format!("{:?}", pointee_type),
                                location,
                            });
                        }
                    };

                    let offset = self.calculate_field_offset(
                        &struct_name,
                        member,
                        location,
                    )?;
                    let field_type =
                        self.get_field_type(&struct_name, member, location)?;

                    self.deserialize_value_from_heap(
                        &field_type,
                        addr + offset as u64,
                        location,
                    )
                }
            }
            Value::Null => Err(RuntimeError::NullDereference { location }),
            _ => Err(RuntimeError::TypeError {
                expected: "pointer".to_string(),
                got: format!("{:?}", ptr_val),
                location,
            }),
        }
    }

    pub(crate) fn evaluate_array_access(
        &mut self,
        array: &AstNode,
        index: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        let arr_val = self.evaluate_expr(array)?;
        let idx_val = self.evaluate_expr(index)?;

        let idx = match idx_val {
            Value::Int(i) => i,
            _ => {
                return Err(RuntimeError::TypeError {
                    expected: "int".to_string(),
                    got: format!("{:?}", idx_val),
                    location,
                });
            }
        };

        match arr_val {
            Value::Array(elements) => {
                if idx < 0 || idx as usize >= elements.len() {
                    return Err(RuntimeError::BufferOverrun {
                        index: idx as usize,
                        size: elements.len(),
                        location,
                    });
                }
                Ok(elements[idx as usize].clone())
            }
            Value::Pointer(addr) => {
                if addr == 0 {
                    return Err(RuntimeError::NullDereference { location });
                }

                if addr < HEAP_ADDRESS_START {
                    let (base_addr, frame_depth, var_name) =
                        self.resolve_stack_pointer(addr, location)?;

                    let frame =
                        self.stack.frames().get(frame_depth).ok_or(
                            RuntimeError::InvalidFrameDepth { location },
                        )?;

                    let var = frame.get_var(&var_name).ok_or_else(|| {
                        RuntimeError::UndefinedVariable {
                            name: var_name.clone(),
                            location,
                        }
                    })?;

                    match &var.value {
                        Value::Array(elements) => {
                            // Calculate offset due to pointer arithmetic
                            let offset = addr - base_addr;

                            // Calculate element size to determine start index
                            let elem_type = var.var_type.element_type();
                            let elem_size =
                                sizeof_type(&elem_type, &self.struct_defs)
                                    as u64;
                            let start_index = if elem_size > 0 {
                                offset / elem_size
                            } else {
                                0
                            };

                            let final_idx = (start_index as i64) + (idx as i64);

                            if final_idx < 0
                                || final_idx as usize >= elements.len()
                            {
                                return Err(RuntimeError::BufferOverrun {
                                    index: final_idx as usize,
                                    size: elements.len(),
                                    location,
                                });
                            }
                            Ok(elements[final_idx as usize].clone())
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
                                    location,
                                })
                            }
                        }
                    }
                } else if let Some(elem_type) =
                    self.pointer_types.get(&addr).cloned()
                {
                    let elem_size = sizeof_type(&elem_type, &self.struct_defs);

                    let offset = (idx as i64) * (elem_size as i64);
                    let target_addr = if offset >= 0 {
                        addr + (offset as u64)
                    } else {
                        addr - ((-offset) as u64)
                    };

                    self.deserialize_value_from_heap(
                        &elem_type,
                        target_addr,
                        location,
                    )
                } else {
                    Err(RuntimeError::InvalidPointer {
                        message: format!(
                            "Unknown pointer type for indexing at 0x{:x}",
                            addr
                        ),
                        address: Some(addr),
                        location,
                    })
                }
            }
            _ => Err(RuntimeError::TypeError {
                expected: "array or pointer".to_string(),
                got: format!("{:?}", arr_val),
                location,
            }),
        }
    }
}
