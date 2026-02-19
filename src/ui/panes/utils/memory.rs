use crate::memory::sizeof_type;
use crate::parser::ast::{BaseType, Field, StructDef, Type};
use std::collections::HashMap;
use std::hash::BuildHasher;

/// Calculate field offsets and types for a struct
pub(crate) fn calculate_field_offsets<S: BuildHasher>(
    fields: &[Field],
    struct_defs: &HashMap<String, StructDef, S>,
) -> Vec<(String, usize, usize, Type)> {
    let mut current_offset = 0;
    let mut result = Vec::with_capacity(fields.len());

    for field in fields {
        let size = sizeof_type(&field.field_type, struct_defs);
        result.push((
            field.name.clone(),
            current_offset,
            size,
            field.field_type.clone(),
        ));
        current_offset += size;
    }

    result
}

pub(crate) fn read_typed_value<S: BuildHasher>(
    data: &[u8],
    init_map: &[bool],
    typ: &Type,
    struct_defs: &HashMap<String, StructDef, S>,
) -> Option<String> {
    let size = sizeof_type(typ, struct_defs);
    if data.len() < size || init_map.len() < size {
        return None;
    }

    // Check if all required bytes are initialized
    let all_initialized = init_map[0..size].iter().all(|&b| b);
    if !all_initialized {
        // Check if any bytes are initialized
        let any_initialized = init_map[0..size].iter().any(|&b| b);
        if any_initialized {
            return Some("[partial]".to_string()); // Partially initialized
        } else {
            return Some("[uninit]".to_string()); // Completely uninitialized
        }
    }

    match &typ.base {
        _ if typ.pointer_depth > 0 => {
            if size >= 4 {
                // Assuming 32-bit pointers or similar size check
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&data[0..4]);
                let addr = u32::from_le_bytes(bytes);
                if addr == 0 {
                    Some("NULL".to_string())
                } else {
                    Some(format!("0x{:08x}", addr))
                }
            } else {
                None
            }
        }
        BaseType::Int => {
            if size >= 4 {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&data[0..4]);
                let value = i32::from_le_bytes(bytes);
                Some(format!("{}", value))
            } else {
                None
            }
        }
        BaseType::Char => {
            if size == 1 {
                let byte = data[0];
                if byte.is_ascii_graphic() || byte == b' ' {
                    Some(format!("'{}'", byte as char))
                } else {
                    Some(format!("'\\x{:02x}'", byte))
                }
            } else {
                // Array of chars - try as string
                let mut chars = String::new();
                for &byte in &data[0..size] {
                    if byte == 0 {
                        break;
                    }
                    if byte.is_ascii_graphic() || byte == b' ' {
                        chars.push(byte as char);
                    } else {
                        // Not a string if contains non-printable
                        return None;
                    }
                }
                if !chars.is_empty() {
                    Some(format!("\"{}\"", chars))
                } else {
                    None
                }
            }
        }
        BaseType::Struct(name) => Some(format!("struct {}", name)),
        BaseType::Void => None,
    }
}
