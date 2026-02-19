//! Runtime value representation
//!
//! This module defines the [`Value`] enum, which represents all possible runtime values
//! in the C interpreter. Unlike C's raw memory model, values are tagged and type-safe.
//!
//! # Value Types
//!
//! - [`Value::Int`]: 32-bit signed integer
//! - [`Value::Char`]: 8-bit signed character
//! - [`Value::Pointer`]: 64-bit memory address
//! - [`Value::Null`]: Null pointer (address 0)
//! - [`Value::Struct`]: Struct with named fields
//! - [`Value::Array`]: Fixed-size array of values
//! - [`Value::Uninitialized`]: Marker for uninitialized memory
//!
//! # Initialization Tracking
//!
//! The `Uninitialized` variant enables detection of reads from uninitialized memory,
//! a common source of undefined behavior in C.

use rustc_hash::FxHashMap;

/// Runtime values in the interpreter
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    Int(i32),
    Char(i8),
    Pointer(Address),
    Null,
    Struct(FxHashMap<String, Value>), // Field name -> field value
    Array(Vec<Value>),
    #[default]
    Uninitialized, // Special marker for uninitialized memory
}

/// Memory address type (64-bit)
pub type Address = u64;

impl Value {
    /// Check if this value is initialized
    pub fn is_initialized(&self) -> bool {
        !matches!(self, Value::Uninitialized)
    }

    /// Get the integer value, returns None if not an Int
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Get the char value, returns None if not a Char
    pub fn as_char(&self) -> Option<i8> {
        match self {
            Value::Char(c) => Some(*c),
            _ => None,
        }
    }

    /// Get the pointer value, returns None if not a Pointer or Null
    pub fn as_pointer(&self) -> Option<Address> {
        match self {
            Value::Pointer(addr) => Some(*addr),
            Value::Null => Some(0),
            _ => None,
        }
    }

    /// Expect an integer value, returns error message if not an Int
    pub fn expect_int(&self) -> Result<i32, String> {
        self.as_int()
            .ok_or_else(|| format!("Expected Int, got {:?}", self))
    }

    /// Expect a char value, returns error message if not a Char
    pub fn expect_char(&self) -> Result<i8, String> {
        self.as_char()
            .ok_or_else(|| format!("Expected Char, got {:?}", self))
    }

    /// Expect a pointer value, returns error message if not a Pointer
    pub fn expect_pointer(&self) -> Result<Address, String> {
        self.as_pointer()
            .ok_or_else(|| format!("Expected Pointer, got {:?}", self))
    }

    /// Check if this value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Check if this value is a pointer (including null)
    pub fn is_pointer(&self) -> bool {
        matches!(self, Value::Pointer(_) | Value::Null)
    }
}
