//! Memory model for the C interpreter
//!
//! This module provides the core memory abstractions:
//! - [`value`]: Runtime value representation (Int, Char, Pointer, Struct, Array)
//! - [`stack`]: Call stack with frames and local variables
//! - [`heap`]: Heap allocation with malloc/free and tombstone tracking
//!
//! # Type Sizes
//!
//! Unlike real C, this interpreter uses fixed, platform-independent sizes:
//! - `int`: 4 bytes
//! - `char`: 1 byte
//! - `pointer`: 8 bytes (regardless of pointee type)
//! - `struct`: sum of field sizes (no padding or alignment)
//!
//! # Pointer Arithmetic
//!
//! Pointer arithmetic is scaled by pointee size:
//! ```text
//! ptr + n  →  ptr + (n * sizeof(*ptr))
//! ```
//!
//! Helper functions [`pointer_add`], [`pointer_sub`], and [`pointer_diff`] handle
//! this scaling automatically.

pub mod heap;
pub mod stack;
pub mod value;

use crate::parser::ast::{BaseType, StructDef, Type};
use std::collections::HashMap;
use std::hash::BuildHasher;
use value::Address;

/// Calculate the size of a type in bytes
pub fn sizeof_type<S: BuildHasher>(
    t: &Type,
    struct_defs: &HashMap<String, StructDef, S>,
) -> usize {
    // If it's a pointer, size is always 8 bytes
    if t.pointer_depth > 0 {
        return 8;
    }

    // Calculate base type size
    let base_size = match &t.base {
        BaseType::Int => 4,
        BaseType::Char => 1,
        BaseType::Void => 0, // sizeof(void) is technically undefined, but we use 0
        BaseType::Struct(name) => {
            let def = struct_defs
                .get(name)
                .unwrap_or_else(|| panic!("Unknown struct: {}", name));
            // Sum of all field sizes (no padding)
            def.fields
                .iter()
                .map(|f| sizeof_type(&f.field_type, struct_defs))
                .sum()
        }
    };

    // For arrays, multiply by dimensions
    if t.array_dims.is_empty() {
        base_size
    } else {
        t.array_dims.iter().fold(base_size, |size, dim| {
            size * dim.expect("Array size must be known for sizeof")
        })
    }
}

/// Perform pointer arithmetic: addr + offset (scaled by pointee size)
pub fn pointer_add<S: BuildHasher>(
    addr: Address,
    offset: i32,
    pointee_type: &Type,
    struct_defs: &HashMap<String, StructDef, S>,
) -> Address {
    let pointee_size = sizeof_type(pointee_type, struct_defs);
    let byte_offset = offset as i64 * pointee_size as i64;
    (addr as i64 + byte_offset) as Address
}

/// Perform pointer subtraction: addr - offset (scaled by pointee size)
pub fn pointer_sub<S: BuildHasher>(
    addr: Address,
    offset: i32,
    pointee_type: &Type,
    struct_defs: &HashMap<String, StructDef, S>,
) -> Address {
    pointer_add(addr, -offset, pointee_type, struct_defs)
}

/// Calculate the difference between two pointers (in elements, not bytes)
pub fn pointer_diff<S: BuildHasher>(
    addr1: Address,
    addr2: Address,
    pointee_type: &Type,
    struct_defs: &HashMap<String, StructDef, S>,
) -> i32 {
    let pointee_size = sizeof_type(pointee_type, struct_defs);
    ((addr1 as i64 - addr2 as i64) / pointee_size as i64) as i32
}
