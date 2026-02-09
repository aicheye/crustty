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
use crate::parser::ast::{AstNode, SourceLocation};

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
                            if arg_index >= args.len() {
                                return Err(RuntimeError::InvalidPrintfFormat {
                                    message: "Not enough arguments for format string".to_string(),
                                    location,
                                });
                            }
                            match &args[arg_index] {
                                Value::Int(n) => output.push_str(&n.to_string()),
                                _ => {
                                    return Err(RuntimeError::InvalidPrintfFormat {
                                        message: format!(
                                            "%d expects int, got {:?}",
                                            args[arg_index]
                                        ),
                                        location,
                                    });
                                }
                            }
                            arg_index += 1;
                        }
                        'u' => {
                            if arg_index >= args.len() {
                                return Err(RuntimeError::InvalidPrintfFormat {
                                    message: "Not enough arguments for format string".to_string(),
                                    location,
                                });
                            }
                            match &args[arg_index] {
                                Value::Int(n) => output.push_str(&(*n as u32).to_string()),
                                _ => {
                                    return Err(RuntimeError::InvalidPrintfFormat {
                                        message: format!(
                                            "%u expects int, got {:?}",
                                            args[arg_index]
                                        ),
                                        location,
                                    });
                                }
                            }
                            arg_index += 1;
                        }
                        'x' => {
                            if arg_index >= args.len() {
                                return Err(RuntimeError::InvalidPrintfFormat {
                                    message: "Not enough arguments for format string".to_string(),
                                    location,
                                });
                            }
                            match &args[arg_index] {
                                Value::Int(n) => output.push_str(&format!("{:x}", *n as u32)),
                                _ => {
                                    return Err(RuntimeError::InvalidPrintfFormat {
                                        message: format!(
                                            "%x expects int, got {:?}",
                                            args[arg_index]
                                        ),
                                        location,
                                    });
                                }
                            }
                            arg_index += 1;
                        }
                        'c' => {
                            if arg_index >= args.len() {
                                return Err(RuntimeError::InvalidPrintfFormat {
                                    message: "Not enough arguments for format string".to_string(),
                                    location,
                                });
                            }
                            match &args[arg_index] {
                                Value::Char(c) => output.push(*c as u8 as char),
                                Value::Int(n) => output.push((*n as u8) as char),
                                _ => {
                                    return Err(RuntimeError::InvalidPrintfFormat {
                                        message: format!(
                                            "%c expects char or int, got {:?}",
                                            args[arg_index]
                                        ),
                                        location,
                                    });
                                }
                            }
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
