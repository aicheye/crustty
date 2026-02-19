//! Memory operations and struct field utilities
//!
//! This module provides memory-related operations for the interpreter:
//!
//! - Variable and pointer assignment (lvalue resolution)
//! - Heap memory serialization and deserialization
//! - Struct field offset calculation and access
//! - Array element access
//!
//! # Performance Optimizations
//!
//! - Field offset calculation is cached using `FxHashMap` to avoid repeated computation
//! - Hot-path functions (`calculate_field_offset`, `get_field_type`) use `#[inline]`
//!
//! # Memory Layout
//!
//! - Structs: Fields laid out sequentially with proper alignment
//! - Arrays: Elements stored contiguously in memory
//! - Pointers: Heap pointers start at `0x0000_1000`, stack pointers at `0x7fff_0000`

use crate::interpreter::constants::HEAP_ADDRESS_START;
use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::*;

impl Interpreter {
    /// Assign a value to an l-value (variable, array element, struct field, etc.)
    pub(crate) fn assign_to_lvalue(
        &mut self,
        lvalue: &AstNode,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        match lvalue {
            AstNode::Variable(name, _) => {
                self.assign_to_variable(name, value, location)
            }

            AstNode::MemberAccess {
                object,
                member,
                location: _,
            } => self.assign_to_member_access(object, member, value, location),

            AstNode::PointerMemberAccess {
                object,
                member,
                location: _,
            } => self.assign_to_pointer_member_access(
                object, member, value, location,
            ),

            AstNode::UnaryOp {
                op: UnOp::Deref,
                operand,
                location: _,
            } => self.assign_to_dereference(operand, value, location),

            AstNode::ArrayAccess {
                array,
                index,
                location: _,
            } => self.assign_to_array_access(array, index, value, location),

            _ => Err(RuntimeError::UnsupportedOperation {
                message: format!(
                    "Assignment to this l-value type not yet implemented: {:?}",
                    lvalue
                ),
                location,
            }),
        }
    }

    fn assign_to_variable(
        &mut self,
        name: &str,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Assign to a variable in the current frame
        let frame = self
            .stack
            .current_frame_mut()
            .ok_or(RuntimeError::NoStackFrame { location })?;

        let var = frame.get_var_mut(name).ok_or_else(|| {
            RuntimeError::UndefinedVariable {
                name: name.to_string(),
                location,
            }
        })?;

        // Check if const
        if var.is_const {
            return Err(RuntimeError::ConstModification {
                var: name.to_string(),
                location,
            });
        }

        var.value = value;
        var.init_state = crate::memory::stack::InitState::Initialized;
        Ok(())
    }

    fn assign_to_member_access(
        &mut self,
        object: &AstNode,
        member: &str,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Assign to a struct field using . operator
        // Recursive read-modify-write approach to handle nested access
        let mut struct_val = self.evaluate_expr(object)?;

        match &mut struct_val {
            Value::Struct(fields) => {
                fields.insert(member.to_string(), value);
            }
            _ => {
                return Err(RuntimeError::TypeError {
                    expected: "struct".to_string(),
                    got: format!("{:?}", struct_val),
                    location,
                });
            }
        }

        // Assign the modified struct back to the object
        self.assign_to_lvalue(object, struct_val, location)?;
        Ok(())
    }

    fn assign_to_pointer_member_access(
        &mut self,
        object: &AstNode,
        member: &str,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Assign to a struct field using -> operator
        let ptr_val = self.evaluate_expr(object)?;

        match ptr_val {
            Value::Pointer(addr) => {
                if addr < HEAP_ADDRESS_START {
                    self.assign_to_stack_pointer_member(
                        addr, member, value, location,
                    )
                } else {
                    self.assign_to_heap_pointer_member(
                        addr, member, value, location,
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

    fn assign_to_stack_pointer_member(
        &mut self,
        addr: u64,
        member: &str,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Stack address
        let (base_addr, frame_depth, var_name) =
            self.resolve_stack_pointer(addr, location)?;

        // Get mutable access to the frame
        // This is tricky with Rust's borrow checker
        // We need to use an index-based approach
        let frames_len = self.stack.frames().len();
        if frame_depth >= frames_len {
            return Err(RuntimeError::InvalidFrameDepth { location });
        }

        // Get mutable reference to the specific frame
        let frame = self
            .stack
            .frame_mut(frame_depth)
            .ok_or(RuntimeError::InvalidFrameDepth { location })?;

        let var = frame.get_var_mut(&var_name).ok_or_else(|| {
            RuntimeError::UndefinedVariable {
                name: var_name.clone(),
                location,
            }
        })?;

        match &mut var.value {
            Value::Struct(fields) => {
                fields.insert(member.to_string(), value);
                Ok(())
            }
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

                match &mut elements[idx as usize] {
                    Value::Struct(fields) => {
                        fields.insert(member.to_string(), value);
                        Ok(())
                    }
                    _ => Err(RuntimeError::TypeError {
                        expected: "struct element".to_string(),
                        got: format!("{:?}", elements[idx as usize]),
                        location,
                    }),
                }
            }
            _ => Err(RuntimeError::TypeError {
                expected: "struct".to_string(),
                got: format!("{:?}", var.value),
                location,
            }),
        }
    }

    fn assign_to_heap_pointer_member(
        &mut self,
        addr: u64,
        member: &str,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Heap address - write struct field to heap
        // Look up the pointer type
        let pointee_type = self
            .pointer_types
            .get(&addr)
            .ok_or_else(|| RuntimeError::InvalidPointer {
                message: format!(
                    "Unknown type for pointer 0x{:x}. Did you cast the result of malloc?",
                    addr
                ),
                address: Some(addr),
                location,
            })?
            .clone(); // Clone to avoid borrow checker issues

        // Ensure it's a struct type
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

        // Calculate field offset
        let offset =
            self.calculate_field_offset(&struct_name, member, location)?;

        // Get field type
        let field_type = self.get_field_type(&struct_name, member, location)?;

        // Serialize field value to heap
        self.serialize_value_to_heap(
            &value,
            &field_type,
            addr + offset as u64,
            location,
        )?;
        Ok(())
    }

    fn assign_to_dereference(
        &mut self,
        operand: &AstNode,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Dereference assignment: *ptr = value
        let ptr_val = self.evaluate_expr(operand)?;

        match ptr_val {
            Value::Pointer(addr) => {
                // Check if this is a stack address
                if addr < HEAP_ADDRESS_START {
                    // Stack address - look up in map
                    let (base_addr, frame_depth, var_name) =
                        self.resolve_stack_pointer(addr, location)?;

                    // Get mutable access to the frame
                    let frames_len = self.stack.frames().len();
                    if frame_depth >= frames_len {
                        return Err(RuntimeError::InvalidFrameDepth {
                            location,
                        });
                    }

                    let frame = self
                        .stack
                        .frame_mut(frame_depth)
                        .ok_or(RuntimeError::InvalidFrameDepth { location })?;

                    let var =
                        frame.get_var_mut(&var_name).ok_or_else(|| {
                            RuntimeError::UndefinedVariable {
                                name: var_name.clone(),
                                location,
                            }
                        })?;

                    // Update the variable's value handling array indexing
                    match &mut var.value {
                        Value::Array(elements) => {
                            let offset = addr - base_addr;
                            let elem_type = var.var_type.element_type();
                            let elem_size =
                                sizeof_type(&elem_type, &self.struct_defs)
                                    as u64;
                            let idx = if elem_size > 0 {
                                offset / elem_size
                            } else {
                                0
                            };

                            if idx as usize >= elements.len() {
                                return Err(RuntimeError::BufferOverrun {
                                    index: idx as usize,
                                    size: elements.len(),
                                    location,
                                });
                            }
                            elements[idx as usize] = value;
                            var.init_state =
                                crate::memory::stack::InitState::Initialized;
                        }
                        _ => {
                            var.value = value;
                            var.init_state =
                                crate::memory::stack::InitState::Initialized;
                        }
                    }
                    Ok(())
                } else {
                    // Heap address
                    // Determine the type of value we're writing
                    // For now, handle basic types (int, char, pointer)
                    match &value {
                        Value::Int(n) => {
                            let bytes = n.to_le_bytes();
                            for (i, &byte) in bytes.iter().enumerate() {
                                self.heap
                                    .write_byte(addr + i as u64, byte)
                                    .map_err(|e| Self::map_heap_error(e, location))?;
                            }
                            Ok(())
                        }
                        Value::Char(c) => {
                            self.heap
                                .write_byte(addr, *c as u8)
                                .map_err(|e| Self::map_heap_error(e, location))?;
                            Ok(())
                        }
                        Value::Pointer(ptr_addr) => {
                            let bytes = ptr_addr.to_le_bytes();
                            for (i, &byte) in bytes.iter().enumerate() {
                                self.heap
                                    .write_byte(addr + i as u64, byte)
                                    .map_err(|e| Self::map_heap_error(e, location))?;
                            }
                            Ok(())
                        }
                        Value::Null => {
                            let bytes = 0u64.to_le_bytes();
                            for (i, &byte) in bytes.iter().enumerate() {
                                self.heap
                                    .write_byte(addr + i as u64, byte)
                                    .map_err(|e| Self::map_heap_error(e, location))?;
                            }
                            Ok(())
                        }
                        _ => Err(RuntimeError::UnsupportedOperation {
                            message: format!(
                                "Cannot assign value of type {:?} through pointer dereference",
                                value
                            ),
                            location,
                        }),
                    }
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

    fn assign_to_array_access(
        &mut self,
        array: &AstNode,
        index: &AstNode,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Assign to an array element: arr[idx] = value
        // Recursive read-modify-write approach
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

        let mut array_val = self.evaluate_expr(array)?;

        // Track whether we need to write back the modified value
        // (true for Value::Array read-modify-write, false for Value::Pointer in-place modification)
        let needs_writeback = matches!(array_val, Value::Array(_));

        match &mut array_val {
            Value::Array(elements) => {
                if idx < 0 || idx as usize >= elements.len() {
                    return Err(RuntimeError::BufferOverrun {
                        index: idx as usize,
                        size: elements.len(),
                        location,
                    });
                }
                elements[idx as usize] = value;
            }
            Value::Pointer(addr) => {
                let addr = *addr;
                if addr == 0 {
                    return Err(RuntimeError::NullDereference { location });
                }

                if addr < HEAP_ADDRESS_START {
                    // Stack pointer dereference assignment (for decayed arrays)
                    let (base_addr, frame_depth, var_name) =
                        self.resolve_stack_pointer(addr, location)?;

                    let frames_len = self.stack.frames().len();
                    if frame_depth >= frames_len {
                        return Err(RuntimeError::InvalidFrameDepth {
                            location,
                        });
                    }

                    let frame = self
                        .stack
                        .frame_mut(frame_depth)
                        .ok_or(RuntimeError::InvalidFrameDepth { location })?;

                    let var =
                        frame.get_var_mut(&var_name).ok_or_else(|| {
                            RuntimeError::UndefinedVariable {
                                name: var_name.clone(),
                                location,
                            }
                        })?;

                    // Handle array indexing for stack arrays
                    match &mut var.value {
                        Value::Array(elements) => {
                            let offset = addr - base_addr;
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
                            elements[final_idx as usize] = value;
                            // Mark as initialized if this was the variable's first write
                            // Note: granular array tracking is pending in Todo 4
                            var.init_state =
                                crate::memory::stack::InitState::Initialized;
                        }
                        _ => {
                            // Not an array - only allow index 0
                            if idx == 0 {
                                var.value = value;
                                var.init_state = crate::memory::stack::InitState::Initialized;
                            } else {
                                return Err(RuntimeError::InvalidPointer {
                                    message: format!(
                                        "Pointer to non-array stack variable, index {} out of bounds",
                                        idx
                                    ),
                                    address: Some(addr),
                                    location,
                                });
                            }
                        }
                    }
                } else {
                    // Heap pointer assignment
                    if let Some(elem_type) =
                        self.pointer_types.get(&addr).cloned()
                    {
                        let elem_size =
                            sizeof_type(&elem_type, &self.struct_defs);
                        let offset = (idx as i64) * (elem_size as i64);
                        let target_addr = if offset >= 0 {
                            addr + (offset as u64)
                        } else {
                            addr - ((-offset) as u64)
                        };

                        self.serialize_value_to_heap(
                            &value,
                            &elem_type,
                            target_addr,
                            location,
                        )?;
                    } else {
                        return Err(RuntimeError::InvalidPointer {
                            message: format!(
                                "Unknown pointer type for indexing at 0x{:x}",
                                addr
                            ),
                            address: Some(addr),
                            location,
                        });
                    }
                }
            }
            _ => {
                return Err(RuntimeError::TypeError {
                    expected: "array or pointer".to_string(),
                    got: format!("{:?}", array_val),
                    location,
                });
            }
        }

        // Assign the modified array back to the object only if we did read-modify-write
        // For pointers to stack/heap arrays, we modify in place, so no writeback needed
        if needs_writeback {
            self.assign_to_lvalue(array, array_val, location)?;
        }
        Ok(())
    }
}
