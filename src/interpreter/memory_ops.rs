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
use std::collections::HashMap;

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
                // Assign to a variable in the current frame
                let frame = self
                    .stack
                    .current_frame_mut()
                    .ok_or(RuntimeError::NoStackFrame { location })?;

                let var =
                    frame
                        .get_var_mut(name)
                        .ok_or_else(|| RuntimeError::UndefinedVariable {
                            name: name.clone(),
                            location,
                        })?;

                // Check if const
                if var.is_const {
                    return Err(RuntimeError::ConstModification {
                        var: name.clone(),
                        location,
                    });
                }

                var.value = value;
                var.init_state = crate::memory::stack::InitState::Initialized;
                Ok(())
            }

            AstNode::MemberAccess {
                object,
                member,
                location: _,
            } => {
                // Assign to a struct field using . operator
                // Recursive read-modify-write approach to handle nested access
                let mut struct_val = self.evaluate_expr(object)?;

                match &mut struct_val {
                    Value::Struct(fields) => {
                        fields.insert(member.clone(), value);
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

            AstNode::PointerMemberAccess {
                object,
                member,
                location: _,
            } => {
                // Assign to a struct field using -> operator
                let ptr_val = self.evaluate_expr(object)?;

                match ptr_val {
                    Value::Pointer(addr) => {
                        if addr < HEAP_ADDRESS_START {
                            // Stack address
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::InvalidPointer {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    address: Some(addr),
                                    location,
                                })?
                                .clone();

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
                                    fields.insert(member.clone(), value);
                                    Ok(())
                                }
                                _ => Err(RuntimeError::TypeError {
                                    expected: "struct".to_string(),
                                    got: format!("{:?}", var.value),
                                    location,
                                }),
                            }
                        } else {
                            // Heap address - write struct field to heap
                            // Look up the pointer type
                            let pointee_type = self.pointer_types.get(&addr)
                                .ok_or_else(|| RuntimeError::InvalidPointer {
                                    message: format!("Unknown type for pointer 0x{:x}. Did you cast the result of malloc?", addr),
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
                    }
                    Value::Null => Err(RuntimeError::NullDereference { location }),
                    _ => Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", ptr_val),
                        location,
                    }),
                }
            }

            AstNode::UnaryOp {
                op: UnOp::Deref,
                operand,
                location: _,
            } => {
                // Dereference assignment: *ptr = value
                let ptr_val = self.evaluate_expr(operand)?;

                match ptr_val {
                    Value::Pointer(addr) => {
                        // Check if this is a stack address
                        if addr < HEAP_ADDRESS_START {
                            // Stack address - look up in map
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::InvalidPointer {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    address: Some(addr),
                                    location,
                                })?
                                .clone();

                            // Get mutable access to the frame
                            let frames_len = self.stack.frames().len();
                            if frame_depth >= frames_len {
                                return Err(RuntimeError::InvalidFrameDepth { location });
                            }

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

                            // Update the variable's value
                            var.value = value;
                            var.init_state = crate::memory::stack::InitState::Initialized;
                            Ok(())
                        } else {
                            // Heap address
                            // Determine the type of value we're writing
                            // For now, handle basic types (int, char, pointer)
                            match &value {
                                Value::Int(n) => {
                                    let bytes = n.to_le_bytes();
                                    for (i, &byte) in bytes.iter().enumerate() {
                                        self.heap.write_byte(addr + i as u64, byte).map_err(
                                            |e| Self::map_heap_error(e, location),
                                        )?;
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
                                        self.heap.write_byte(addr + i as u64, byte).map_err(
                                            |e| Self::map_heap_error(e, location),
                                        )?;
                                    }
                                    Ok(())
                                }
                                Value::Null => {
                                    let bytes = 0u64.to_le_bytes();
                                    for (i, &byte) in bytes.iter().enumerate() {
                                        self.heap.write_byte(addr + i as u64, byte).map_err(
                                            |e| Self::map_heap_error(e, location),
                                        )?;
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

            AstNode::ArrayAccess {
                array,
                index,
                location: _,
            } => {
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
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::InvalidPointer {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    address: Some(addr),
                                    location,
                                })?
                                .clone();

                            let frames_len = self.stack.frames().len();
                            if frame_depth >= frames_len {
                                return Err(RuntimeError::InvalidFrameDepth { location });
                            }

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

                            // Handle array indexing for stack arrays
                            match &mut var.value {
                                Value::Array(elements) => {
                                    if idx < 0 || idx as usize >= elements.len() {
                                        return Err(RuntimeError::BufferOverrun {
                                            index: idx as usize,
                                            size: elements.len(),
                                            location,
                                        });
                                    }
                                    elements[idx as usize] = value;
                                    // Mark as initialized if this was the variable's first write
                                    var.init_state = crate::memory::stack::InitState::Initialized;
                                }
                                _ => {
                                    // Not an array - only allow index 0
                                    if idx == 0 {
                                        var.value = value;
                                        var.init_state =
                                            crate::memory::stack::InitState::Initialized;
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
                            if let Some(elem_type) = self.pointer_types.get(&addr).cloned() {
                                let elem_size = sizeof_type(&elem_type, &self.struct_defs);
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

            _ => Err(RuntimeError::UnsupportedOperation {
                message: format!(
                    "Assignment to this l-value type not yet implemented: {:?}",
                    lvalue
                ),
                location,
            }),
        }
    }

    /// Calculate the byte offset of a field within a struct
    /// Uses sequential packing (no padding/alignment)
    #[inline]
    pub(crate) fn calculate_field_offset(
        &mut self,
        struct_name: &str,
        field_name: &str,
        location: SourceLocation,
    ) -> Result<usize, RuntimeError> {
        // Check cache first
        let cache_key = (struct_name.to_string(), field_name.to_string());
        if let Some((offset, _)) = self.field_info_cache.get(&cache_key) {
            return Ok(*offset);
        }

        let struct_def =
            self.struct_defs
                .get(struct_name)
                .ok_or_else(|| RuntimeError::StructNotDefined {
                    name: struct_name.to_string(),
                    location,
                })?;

        let mut offset = 0;
        for field in &struct_def.fields {
            if field.name == field_name {
                // Cache the result before returning
                let field_type = field.field_type.clone();
                self.field_info_cache
                    .insert(cache_key, (offset, field_type));
                return Ok(offset);
            }
            offset += sizeof_type(&field.field_type, &self.struct_defs);
        }

        Err(RuntimeError::MissingStructField {
            struct_name: struct_name.to_string(),
            field_name: field_name.to_string(),
            location,
        })
    }

    /// Get the type of a specific field within a struct
    #[inline]
    pub(crate) fn get_field_type(
        &mut self,
        struct_name: &str,
        field_name: &str,
        location: SourceLocation,
    ) -> Result<Type, RuntimeError> {
        // Check cache first
        let cache_key = (struct_name.to_string(), field_name.to_string());
        if let Some((_, field_type)) = self.field_info_cache.get(&cache_key) {
            return Ok(field_type.clone());
        }

        let struct_def =
            self.struct_defs
                .get(struct_name)
                .ok_or_else(|| RuntimeError::StructNotDefined {
                    name: struct_name.to_string(),
                    location,
                })?;

        // Calculate offset and cache both offset and type
        let mut offset = 0;
        for field in &struct_def.fields {
            if field.name == field_name {
                let field_type = field.field_type.clone();
                self.field_info_cache
                    .insert(cache_key, (offset, field_type.clone()));
                return Ok(field_type);
            }
            offset += sizeof_type(&field.field_type, &self.struct_defs);
        }

        Err(RuntimeError::MissingStructField {
            struct_name: struct_name.to_string(),
            field_name: field_name.to_string(),
            location,
        })
    }

    /// Serialize a value to heap bytes (sequential packing, no padding)
    pub(crate) fn serialize_value_to_heap(
        &mut self,
        value: &Value,
        value_type: &Type,
        base_addr: u64,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        match value {
            Value::Int(n) => {
                // Write 4 bytes (little-endian)
                let bytes = n.to_le_bytes();
                for (i, byte) in bytes.iter().enumerate() {
                    self.heap
                        .write_byte(base_addr + i as u64, *byte)
                        .map_err(|e| Self::map_heap_error(e, location))?;
                }
                Ok(())
            }
            Value::Char(c) => {
                // Write 1 byte (c is already i8)
                self.heap
                    .write_byte(base_addr, *c as u8)
                    .map_err(|e| Self::map_heap_error(e, location))?;
                Ok(())
            }
            Value::Uninitialized => {
                // Don't write anything for uninitialized values
                // The heap already marks bytes as uninitialized by default
                Ok(())
            }
            Value::Pointer(addr) => {
                // Write 8 bytes (little-endian)
                let bytes = addr.to_le_bytes();
                for (i, byte) in bytes.iter().enumerate() {
                    self.heap
                        .write_byte(base_addr + i as u64, *byte)
                        .map_err(|e| Self::map_heap_error(e, location))?;
                }
                Ok(())
            }
            Value::Null => {
                // Write 8 bytes of zeros
                for i in 0..8 {
                    self.heap
                        .write_byte(base_addr + i, 0)
                        .map_err(|e| Self::map_heap_error(e, location))?;
                }
                Ok(())
            }
            Value::Struct(fields) => {
                // Get struct name from type
                let struct_name = match &value_type.base {
                    BaseType::Struct(name) => name,
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "struct type".to_string(),
                            got: format!("{:?}", value_type.base),
                            location,
                        });
                    }
                };

                // Get struct definition
                let struct_def = self
                    .struct_defs
                    .get(struct_name)
                    .ok_or_else(|| RuntimeError::StructNotDefined {
                        name: struct_name.to_string(),
                        location,
                    })?
                    .clone(); // Clone to avoid borrow checker issues

                // Write each field sequentially
                let mut offset = 0;
                for field in &struct_def.fields {
                    if let Some(field_value) = fields.get(&field.name) {
                        self.serialize_value_to_heap(
                            field_value,
                            &field.field_type,
                            base_addr + offset as u64,
                            location,
                        )?;
                    }
                    offset += sizeof_type(&field.field_type, &self.struct_defs);
                }
                Ok(())
            }
            Value::Array(elements) => {
                // Get element type
                let elem_type = match &value_type.base {
                    BaseType::Int => Type {
                        base: BaseType::Int,
                        pointer_depth: 0,
                        is_const: false,
                        array_dims: Vec::new(),
                    },
                    BaseType::Char => Type {
                        base: BaseType::Char,
                        pointer_depth: 0,
                        is_const: false,
                        array_dims: Vec::new(),
                    },
                    BaseType::Struct(name) => Type {
                        base: BaseType::Struct(name.clone()),
                        pointer_depth: 0,
                        is_const: false,
                        array_dims: Vec::new(),
                    },
                    _ => {
                        return Err(RuntimeError::UnsupportedOperation {
                            message: format!(
                                "Unsupported array element type: {:?}",
                                value_type.base
                            ),
                            location,
                        });
                    }
                };

                let elem_size = sizeof_type(&elem_type, &self.struct_defs);
                for (i, elem) in elements.iter().enumerate() {
                    self.serialize_value_to_heap(
                        elem,
                        &elem_type,
                        base_addr + (i * elem_size) as u64,
                        location,
                    )?;
                }
                Ok(())
            }
        }
    }

    /// Deserialize a value from heap bytes
    pub(crate) fn deserialize_value_from_heap(
        &self,
        value_type: &Type,
        base_addr: u64,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match &value_type.base {
            BaseType::Int if value_type.pointer_depth == 0 => {
                // Read 4 bytes (little-endian)
                let mut bytes = [0u8; 4];
                for (i, byte) in bytes.iter_mut().enumerate() {
                    *byte = self
                        .heap
                        .read_byte(base_addr + i as u64)
                        .map_err(|e| Self::map_heap_error(e, location))?;
                }
                Ok(Value::Int(i32::from_le_bytes(bytes)))
            }
            BaseType::Char if value_type.pointer_depth == 0 => {
                // Read 1 byte
                let byte = self
                    .heap
                    .read_byte(base_addr)
                    .map_err(|e| Self::map_heap_error(e, location))?;
                Ok(Value::Char(byte as i8))
            }
            _ if value_type.pointer_depth > 0 => {
                // Read 8 bytes (pointer)
                let mut bytes = [0u8; 8];
                for (i, byte) in bytes.iter_mut().enumerate() {
                    *byte = self
                        .heap
                        .read_byte(base_addr + i as u64)
                        .map_err(|e| Self::map_heap_error(e, location))?;
                }
                let addr = u64::from_le_bytes(bytes);
                if addr == 0 {
                    Ok(Value::Null)
                } else {
                    Ok(Value::Pointer(addr))
                }
            }
            BaseType::Struct(struct_name) if value_type.pointer_depth == 0 => {
                // Read struct fields
                let struct_def = self
                    .struct_defs
                    .get(struct_name)
                    .ok_or_else(|| RuntimeError::StructNotDefined {
                        name: struct_name.to_string(),
                        location,
                    })?
                    .clone(); // Clone to avoid borrow checker issues

                let mut fields = HashMap::new();
                let mut offset = 0;
                for field in &struct_def.fields {
                    let field_value = self.deserialize_value_from_heap(
                        &field.field_type,
                        base_addr + offset as u64,
                        location,
                    )?;
                    fields.insert(field.name.clone(), field_value);
                    offset += sizeof_type(&field.field_type, &self.struct_defs);
                }
                Ok(Value::Struct(fields))
            }
            _ => Err(RuntimeError::UnsupportedOperation {
                message: format!(
                    "Deserialization not yet implemented for type: {:?}",
                    value_type
                ),
                location,
            }),
        }
    }
}
