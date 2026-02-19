use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::{BaseType, SourceLocation, Type};
use rustc_hash::FxHashMap;

impl Interpreter {
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

                let mut fields = FxHashMap::default();
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
