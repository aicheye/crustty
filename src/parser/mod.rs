//! C source code parser
//!
//! This module transforms C source text into an Abstract Syntax Tree (AST):
//! - [`lexer`]: Tokenization (source text → tokens)
//! - [`parser`]: Parsing (tokens → AST)
//! - [`ast`]: AST node definitions
//!
//! # Supported C Subset
//!
//! The parser supports a pedagogical subset of C:
//! - Types: `int`, `char`, `void`, structs, pointers, arrays
//! - Statements: declarations, assignments, control flow (`if`, `while`, `for`, `switch`)
//! - Expressions: arithmetic, logical, bitwise, ternary, function calls
//! - No preprocessor (except `#include` directives are skipped)
//! - No typedefs, unions, enums, or function pointers
//!
//! # Parser Implementation
//!
//! Hand-written recursive descent parser with precedence climbing for binary operators.
//! No external parser generator dependencies.

pub mod ast;
pub mod lexer;
pub mod parser;
