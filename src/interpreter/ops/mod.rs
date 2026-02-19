//! Operator implementations for the interpreter.
//!
//! Each submodule adds methods to [`crate::interpreter::engine::Interpreter`] via
//! `impl Interpreter` blocks, grouped by the class of operation:
//!
//! | Submodule    | Responsibility |
//! |-------------|----------------|
//! | [`binary`]  | Arithmetic, comparison, bitwise, and compound-assignment operators |
//! | [`unary`]   | Unary negation, logical NOT, bitwise NOT, pre/post increment/decrement |
//! | [`assign`]  | Assignment to arbitrary lvalues (variables, pointer dereferences, array indices, struct fields) |
//! | [`access`]  | Reading lvalues and resolving memory addresses |
//! | [`structs`] | Struct initialization and field-offset helpers |
//!
//! All methods are `pub(crate)` — they are implementation details of the interpreter
//! and are not part of the public library API.

pub mod access;
pub mod assign;
pub mod binary;
pub mod structs;
pub mod unary;
