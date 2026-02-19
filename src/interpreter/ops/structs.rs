use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::sizeof_type;
use crate::parser::ast::{SourceLocation, Type};

impl Interpreter {
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
            self.struct_defs.get(struct_name).ok_or_else(|| {
                RuntimeError::StructNotDefined {
                    name: struct_name.to_string(),
                    location,
                }
            })?;

        let mut offset = 0;
        for field in &struct_def.fields {
            if field.name == field_name {
                // Cache the result before returning
                let field_type = field.field_type.clone();
                let cache_key =
                    (struct_name.to_string(), field_name.to_string());
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

        // If not in cache, calculate offset (which populates cache)
        self.calculate_field_offset(struct_name, field_name, location)?;

        // Now it should be in cache
        if let Some((_, field_type)) = self.field_info_cache.get(&cache_key) {
            return Ok(field_type.clone());
        }

        // Should be unreachable if calculate_field_offset succeeded
        Err(RuntimeError::MissingStructField {
            struct_name: struct_name.to_string(),
            field_name: field_name.to_string(),
            location,
        })
    }
}
