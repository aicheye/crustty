//! Built-in function implementations
//!
//! This module provides the implementation of C built-in functions that are
//! directly handled by the interpreter rather than being defined in user code.
//!
//! # Supported Built-ins
//!
//! - `printf(format, ...)`: Formatted output to terminal
//! - `malloc(size)`: Dynamic memory allocation on the heap
//! - `free(ptr)`: Free dynamically allocated memory
//!
//! # Implementation Notes
//!
//! - `printf` supports format specifiers: `%d`, `%u`, `%x`, `%c`, `%s`, `%%`
//! - `malloc` returns heap pointers starting at `0x0000_1000`
//! - `free` marks memory as deallocated but doesn't zero it (matches C behavior)
//! - All built-ins are implemented as methods on the [`Interpreter`] struct

use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::value::Value;
use crate::parser::ast::{AstNode, SourceLocation, UnOp};

fn expect_int_arg(
    args: &[Value],
    arg_index: usize,
    specifier: char,
    location: SourceLocation,
) -> Result<i32, RuntimeError> {
    let val = args
        .get(arg_index)
        .ok_or_else(|| RuntimeError::InvalidPrintfFormat {
            message: "Not enough arguments for format string".to_string(),
            location,
        })?;
    match val {
        Value::Int(n) => Ok(*n),
        Value::Char(c) => Ok(*c as i32),
        _ => Err(RuntimeError::InvalidPrintfFormat {
            message: format!("%{specifier} expects int, got {val:?}"),
            location,
        }),
    }
}

impl Interpreter {
    pub(crate) fn builtin_printf(
        &mut self,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if args.is_empty() {
            return Err(RuntimeError::InvalidPrintfFormat {
                message: "printf requires at least one argument".to_string(),
                location,
            });
        }

        let format_str = match &args[0] {
            AstNode::StringLiteral(s, _) => s.clone(),
            _ => {
                return Err(RuntimeError::InvalidPrintfFormat {
                    message: "printf format must be a string literal".to_string(),
                    location,
                });
            }
        };

        let mut arg_values = Vec::new();
        for arg in &args[1..] {
            arg_values.push(self.evaluate_expr(arg)?);
        }

        let output = self.format_printf(&format_str, &arg_values, location)?;
        self.terminal.print(output, self.current_location);

        Ok(Value::Int(0))
    }

    fn format_printf(
        &self,
        format: &str,
        args: &[Value],
        location: SourceLocation,
    ) -> Result<String, RuntimeError> {
        let mut output = String::new();
        let mut chars = format.chars().peekable();
        let mut arg_index = 0;

        while let Some(ch) = chars.next() {
            if ch == '%' {
                if let Some(&next_ch) = chars.peek() {
                    chars.next();

                    match next_ch {
                        '%' => output.push('%'),
                        'd' => {
                            let n = expect_int_arg(args, arg_index, 'd', location)?;
                            output.push_str(&n.to_string());
                            arg_index += 1;
                        }
                        'u' => {
                            let n = expect_int_arg(args, arg_index, 'u', location)?;
                            output.push_str(&(n as u32).to_string());
                            arg_index += 1;
                        }
                        'x' => {
                            let n = expect_int_arg(args, arg_index, 'x', location)?;
                            output.push_str(&format!("{:x}", n as u32));
                            arg_index += 1;
                        }
                        'c' => {
                            let n = expect_int_arg(args, arg_index, 'c', location)?;
                            output.push((n as u8) as char);
                            arg_index += 1;
                        }
                        's' => {
                            if arg_index >= args.len() {
                                return Err(RuntimeError::InvalidPrintfFormat {
                                    message: "Not enough arguments for format string".to_string(),
                                    location,
                                });
                            }
                            match &args[arg_index] {
                                Value::Pointer(addr) => {
                                    let string = self.read_string_from_heap(*addr, location)?;
                                    output.push_str(&string);
                                }
                                _ => {
                                    return Err(RuntimeError::InvalidPrintfFormat {
                                        message: format!(
                                            "%s expects pointer, got {:?}",
                                            args[arg_index]
                                        ),
                                        location,
                                    });
                                }
                            }
                            arg_index += 1;
                        }
                        'n' => {
                            return Err(RuntimeError::UnsupportedOperation {
                                message: "%n format specifier not yet implemented".to_string(),
                                location,
                            });
                        }
                        _ => {
                            return Err(RuntimeError::InvalidPrintfFormat {
                                message: format!("Unsupported format specifier: %{}", next_ch),
                                location,
                            });
                        }
                    }
                } else {
                    output.push('%');
                }
            } else if ch == '\\' {
                if let Some(&next_ch) = chars.peek() {
                    chars.next();
                    match next_ch {
                        'n' => output.push('\n'),
                        't' => output.push('\t'),
                        'r' => output.push('\r'),
                        '\\' => output.push('\\'),
                        '"' => output.push('"'),
                        _ => {
                            output.push('\\');
                            output.push(next_ch);
                        }
                    }
                } else {
                    output.push('\\');
                }
            } else {
                output.push(ch);
            }
        }

        Ok(output)
    }

    pub(crate) fn read_string_from_heap(
        &self,
        addr: u64,
        location: SourceLocation,
    ) -> Result<String, RuntimeError> {
        let mut bytes = Vec::new();
        let mut current_addr = addr;

        loop {
            let byte = self
                .heap
                .read_byte(current_addr)
                .map_err(|e| Self::map_heap_error(e, location))?;

            if byte == 0 {
                break;
            }

            bytes.push(byte);
            current_addr += 1;

            if bytes.len() > 10000 {
                return Err(RuntimeError::InvalidString {
                    message: "String too long or missing null terminator".to_string(),
                    location,
                });
            }
        }

        String::from_utf8(bytes).map_err(|_| RuntimeError::InvalidString {
            message: "Invalid UTF-8 in string".to_string(),
            location,
        })
    }

    pub(crate) fn builtin_scanf(
        &mut self,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if args.is_empty() {
            return Err(RuntimeError::ArgumentCountMismatch {
                function: "scanf".to_string(),
                expected: 1,
                got: 0,
                location,
            });
        }

        let format_str = match &args[0] {
            AstNode::StringLiteral(s, _) => s.clone(),
            _ => {
                return Err(RuntimeError::InvalidPrintfFormat {
                    message: "scanf format must be a string literal".to_string(),
                    location,
                });
            }
        };

        let matched = self.parse_scanf_input(&format_str, &args[1..], location)?;
        Ok(Value::Int(matched as i32))
    }

    /// Parse a scanf format string, consuming tokens from the shared stdin queue and writing
    /// converted values to the pointer arguments. Returns `ScanfNeedsInput` if the token
    /// queue runs dry before all specifiers are satisfied. Echoes consumed tokens to the
    /// terminal (one echo per scanf call).
    fn parse_scanf_input(
        &mut self,
        format: &str,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<usize, RuntimeError> {
        let initial_index = self.stdin_token_index;
        let mut arg_idx = 0;
        let mut matched = 0;
        let mut chars = format.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch != '%' {
                continue;
            }
            let spec = match chars.next() {
                Some(s) => s,
                None => break,
            };

            if arg_idx >= args.len() {
                break;
            }

            // Signal a pause if the token queue is exhausted
            if self.stdin_token_index >= self.stdin_tokens.len() {
                return Err(RuntimeError::ScanfNeedsInput { location });
            }

            let token = self.stdin_tokens[self.stdin_token_index].clone();
            self.stdin_token_index += 1;

            match spec {
                'd' | 'i' => {
                    if let Ok(n) = token.parse::<i64>() {
                        let val = Value::Int(n as i32);
                        self.write_scanf_value(val, &args[arg_idx], location)?;
                        matched += 1;
                    }
                    arg_idx += 1;
                }
                'u' => {
                    if let Ok(n) = token.parse::<u64>() {
                        let val = Value::Int(n as i32);
                        self.write_scanf_value(val, &args[arg_idx], location)?;
                        matched += 1;
                    }
                    arg_idx += 1;
                }
                'x' | 'X' => {
                    let stripped = token
                        .strip_prefix("0x")
                        .or_else(|| token.strip_prefix("0X"))
                        .unwrap_or(token.as_str());
                    if let Ok(n) = u32::from_str_radix(stripped, 16) {
                        let val = Value::Int(n as i32);
                        self.write_scanf_value(val, &args[arg_idx], location)?;
                        matched += 1;
                    }
                    arg_idx += 1;
                }
                'c' => {
                    if let Some(c) = token.chars().next() {
                        let val = Value::Char(c as i8);
                        self.write_scanf_value(val, &args[arg_idx], location)?;
                        matched += 1;
                    }
                    arg_idx += 1;
                }
                's' => {
                    self.write_scanf_string(&token, &args[arg_idx], location)?;
                    matched += 1;
                    arg_idx += 1;
                }
                _ => {
                    // Unknown specifier — skip the arg
                    arg_idx += 1;
                }
            }
        }

        // Echo all tokens consumed by this scanf call to the terminal
        let echo = self.stdin_tokens[initial_index..self.stdin_token_index].join(" ");
        if !echo.is_empty() {
            self.terminal.print_input(format!("{}\n", echo), location);
        }

        Ok(matched)
    }

    /// Write a single scalar value to the lvalue pointed to by a scanf argument.
    /// The argument is expected to be an address-of expression (e.g. `&x`), so we
    /// synthesise a dereference lvalue `*(arg)` and delegate to `assign_to_lvalue`.
    fn write_scanf_value(
        &mut self,
        value: Value,
        arg: &AstNode,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        let deref_lvalue = AstNode::UnaryOp {
            op: UnOp::Deref,
            operand: Box::new(arg.clone()),
            location,
        };
        self.assign_to_lvalue(&deref_lvalue, value, location)
    }

    /// Write a null-terminated string to the buffer pointed to by a scanf `%s` argument.
    /// Works with both stack char arrays (array decay to pointer) and heap char pointers.
    fn write_scanf_string(
        &mut self,
        s: &str,
        arg: &AstNode,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Write each character then the null terminator via arr[i] = c
        for (i, c) in s.chars().enumerate() {
            let index_node = AstNode::IntLiteral(i as i32, location);
            let lvalue = AstNode::ArrayAccess {
                array: Box::new(arg.clone()),
                index: Box::new(index_node),
                location,
            };
            self.assign_to_lvalue(&lvalue, Value::Char(c as i8), location)?;
        }
        // Null terminator
        let null_index = AstNode::IntLiteral(s.len() as i32, location);
        let null_lvalue = AstNode::ArrayAccess {
            array: Box::new(arg.clone()),
            index: Box::new(null_index),
            location,
        };
        self.assign_to_lvalue(&null_lvalue, Value::Char(0), location)
    }

    pub(crate) fn builtin_malloc(
        &mut self,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if args.len() != 1 {
            return Err(RuntimeError::ArgumentCountMismatch {
                function: "malloc".to_string(),
                expected: 1,
                got: args.len(),
                location,
            });
        }

        let size_val = self.evaluate_expr(&args[0])?;
        let size = match size_val {
            Value::Int(n) if n > 0 => n as usize,
            Value::Int(n) => {
                return Err(RuntimeError::InvalidMallocSize { size: n, location });
            }
            _ => {
                return Err(RuntimeError::TypeError {
                    expected: "int".to_string(),
                    got: format!("{:?}", size_val),
                    location,
                });
            }
        };

        let addr = self
            .heap
            .allocate(size)
            .map_err(|_| RuntimeError::OutOfMemory {
                requested: size,
                limit: self.heap.max_size(),
            })?;

        Ok(Value::Pointer(addr))
    }

    pub(crate) fn builtin_free(
        &mut self,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if args.len() != 1 {
            return Err(RuntimeError::ArgumentCountMismatch {
                function: "free".to_string(),
                expected: 1,
                got: args.len(),
                location,
            });
        }

        let ptr_val = self.evaluate_expr(&args[0])?;
        let addr = match ptr_val {
            Value::Pointer(a) => a,
            Value::Null => {
                return Ok(Value::Int(0));
            }
            _ => {
                return Err(RuntimeError::TypeError {
                    expected: "pointer".to_string(),
                    got: format!("{:?}", ptr_val),
                    location,
                });
            }
        };

        self.heap.free(addr).map_err(|e| {
            if e.contains("Double free") {
                RuntimeError::DoubleFree {
                    address: addr,
                    location,
                }
            } else {
                RuntimeError::InvalidFree {
                    address: addr,
                    location,
                }
            }
        })?;

        Ok(Value::Int(0))
    }
}
