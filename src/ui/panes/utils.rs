//! Shared utility functions for pane rendering
//!
//! This module provides common functionality used across multiple pane modules
//! to maintain consistency and reduce code duplication.
//!
//! # Utilities
//!
//! - **Value Formatting**: Convert `Value` to styled display strings
//! - **Type Annotations**: Format type information for display
//! - **Array Rendering**: Recursively render nested arrays with proper indentation
//! - **Struct Rendering**: Render struct fields with addresses and values
//! - **Field Calculations**: Compute struct field offsets and read typed values from memory
//!
//! # Architecture
//!
//! All functions in this module are `pub(super)`, making them accessible only
//! within the panes module for encapsulation.

use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::{BaseType, Field, StructDef, Type};
use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};
use std::collections::HashMap;
use std::hash::BuildHasher;

/// Render array elements recursively with proper nesting and indentation
pub(super) fn render_array_elements<'a, S: BuildHasher>(
    all_items: &mut Vec<ListItem<'a>>,
    elements: &[Value],
    array_type: &Type,
    base_address: u64,
    indent_level: usize,
    struct_defs: &HashMap<String, StructDef, S>,
    content_width: usize,
) {
    // Get the element type (strip one array dimension)
    let elem_type = if !array_type.array_dims.is_empty() {
        let mut elem_type = array_type.clone();
        elem_type.array_dims.remove(0);
        elem_type
    } else {
        array_type.clone()
    };

    // Calculate element size
    let elem_size = sizeof_type(&elem_type, struct_defs) as u64;

    for (idx, elem_value) in elements.iter().enumerate() {
        let elem_address = base_address + (idx as u64 * elem_size);
        let addr_span = Span::styled(
            format!("0x{:08x} ", elem_address),
            Style::default().fg(DEFAULT_THEME.comment),
        );

        let indent = "  ".repeat(indent_level);

        // Check if this element is itself a nested array or struct
        match elem_value {
            Value::Array(nested_elements) => {
                // Nested array - show header and recurse
                let type_str = format_type_annotation(&elem_type, struct_defs);

                // Calculate padding for right-alignment
                // addr(11) + indent + "[idx] " + ": " = 11 + indent + index_str + 2
                let index_str = format!("[{}] ", idx);
                let left_width = 11 + indent.len() + index_str.len() + 2; // +2 for ": "
                let type_width = type_str.len();
                let padding = content_width.saturating_sub(left_width + type_width);

                let mut spans = vec![
                    addr_span,
                    Span::raw(indent),
                    Span::styled(index_str, Style::default().fg(DEFAULT_THEME.fg)),
                    Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                ];

                if !type_str.is_empty() {
                    spans.push(Span::raw(" ".repeat(padding)));
                    spans.push(Span::styled(
                        type_str,
                        Style::default().fg(DEFAULT_THEME.type_name),
                    ));
                }

                let line = Line::from(spans);
                all_items.push(ListItem::new(line));

                // Recursively render nested array
                render_array_elements(
                    all_items,
                    nested_elements,
                    &elem_type,
                    elem_address,
                    indent_level + 1,
                    struct_defs,
                    content_width,
                );
            }
            Value::Struct(fields) => {
                // Struct element - show header and recurse
                let type_str = format_type_annotation(&elem_type, struct_defs);

                // Calculate padding for right-alignment
                let index_str = format!("[{}] ", idx);
                let left_width = 11 + indent.len() + index_str.len() + 2; // +2 for ": "
                let type_width = type_str.len();
                let padding = content_width.saturating_sub(left_width + type_width);

                let mut spans = vec![
                    addr_span,
                    Span::raw(indent),
                    Span::styled(index_str, Style::default().fg(DEFAULT_THEME.fg)),
                    Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                ];

                if !type_str.is_empty() {
                    spans.push(Span::raw(" ".repeat(padding)));
                    spans.push(Span::styled(
                        type_str,
                        Style::default().fg(DEFAULT_THEME.type_name),
                    ));
                }

                let line = Line::from(spans);
                all_items.push(ListItem::new(line));

                // Recursively render struct fields
                render_struct_fields(
                    all_items,
                    fields,
                    &elem_type,
                    elem_address,
                    indent_level + 1,
                    struct_defs,
                    content_width,
                );
            }
            _ => {
                // Primitive value - show on single line
                let val_spans = format_value_styled(elem_value, struct_defs, 1);
                let type_str = format_type_annotation(&elem_type, struct_defs);

                // Calculate padding for right-alignment
                let index_str = format!("[{}] ", idx);
                let val_width: usize = val_spans.iter().map(|s| s.content.len()).sum();
                let left_width = 11 + indent.len() + index_str.len() + 2 + val_width;
                let type_width = type_str.len();
                let padding = content_width.saturating_sub(left_width + type_width);

                let mut spans = vec![
                    addr_span,
                    Span::raw(indent),
                    Span::styled(index_str, Style::default().fg(DEFAULT_THEME.fg)),
                    Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                ];

                spans.extend(val_spans);

                if !type_str.is_empty() {
                    spans.push(Span::raw(" ".repeat(padding)));
                    spans.push(Span::styled(
                        type_str,
                        Style::default().fg(DEFAULT_THEME.type_name),
                    ));
                }

                let line = Line::from(spans);
                all_items.push(ListItem::new(line));
            }
        }
    }
}

/// Recursively render struct fields with proper nesting and indentation
pub(super) fn render_struct_fields<'a, S: BuildHasher>(
    all_items: &mut Vec<ListItem<'a>>,
    fields: &HashMap<String, Value>,
    parent_type: &Type,
    base_address: u64,
    indent_level: usize,
    struct_defs: &HashMap<String, StructDef, S>,
    content_width: usize,
) {
    // Calculate field offsets to show addresses and types
    let field_info: std::collections::HashMap<String, (usize, Type)> =
        if let crate::parser::ast::BaseType::Struct(struct_name) = &parent_type.base {
            if let Some(struct_def) = struct_defs.get(struct_name) {
                calculate_field_offsets(&struct_def.fields, struct_defs)
                    .into_iter()
                    .map(|(name, offset, _size, field_type)| (name, (offset, field_type)))
                    .collect()
            } else {
                std::collections::HashMap::new()
            }
        } else {
            std::collections::HashMap::new()
        };

    // Sort fields alphabetically for consistent display
    let mut sorted_fields: Vec<_> = fields.iter().collect();
    sorted_fields.sort_by_key(|(k, _)| *k);

    for (field_name, field_value) in sorted_fields {
        // Calculate field address and get type if we have field information
        let (field_addr_span, type_annotation, field_type_opt) =
            if let Some((offset, field_type)) = field_info.get(field_name) {
                let addr_span = Span::styled(
                    format!("0x{:08x} ", base_address + (*offset as u64)),
                    Style::default().fg(DEFAULT_THEME.comment),
                );
                let type_str = format_type_annotation(field_type, struct_defs);
                (addr_span, type_str, Some(field_type.clone()))
            } else {
                (Span::raw("              "), String::new(), None)
            };

        let indent = "  ".repeat(indent_level);

        // Check if this field is itself a struct
        if let Value::Struct(nested_fields) = field_value {
            // Struct field - render header and recurse
            // Calculate padding for right-alignment
            let field_str = format!(".{} ", field_name);
            let left_width = 11 + indent.len() + field_str.len() + 2; // +2 for ": "
            let type_width = type_annotation.len();
            let padding = content_width.saturating_sub(left_width + type_width);

            let mut spans = vec![
                field_addr_span,
                Span::raw(indent),
                Span::styled(field_str, Style::default().fg(DEFAULT_THEME.fg)),
                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
            ];

            if !type_annotation.is_empty() {
                spans.push(Span::raw(" ".repeat(padding)));
                spans.push(Span::styled(
                    type_annotation,
                    Style::default().fg(DEFAULT_THEME.type_name),
                ));
            }

            let line = Line::from(spans);
            all_items.push(ListItem::new(line));

            // Recursively render nested struct fields
            if let Some(field_type) = field_type_opt {
                if let Some((offset, _)) = field_info.get(field_name) {
                    render_struct_fields(
                        all_items,
                        nested_fields,
                        &field_type,
                        base_address + (*offset as u64),
                        indent_level + 1,
                        struct_defs,
                        content_width,
                    );
                }
            }
        } else {
            // Non-struct field - render as a single line
            let val_spans = format_value_styled(field_value, struct_defs, 1);

            // Calculate padding for right-alignment
            let field_str = format!(".{} ", field_name);
            let val_width: usize = val_spans.iter().map(|s| s.content.len()).sum();
            let left_width = 11 + indent.len() + field_str.len() + 2 + val_width; // +2 for ": "
            let type_width = type_annotation.len();
            let padding = content_width.saturating_sub(left_width + type_width);

            let mut spans = vec![
                field_addr_span,
                Span::raw(indent),
                Span::styled(field_str, Style::default().fg(DEFAULT_THEME.fg)),
                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
            ];

            spans.extend(val_spans);

            if !type_annotation.is_empty() {
                spans.push(Span::raw(" ".repeat(padding)));
                spans.push(Span::styled(
                    type_annotation,
                    Style::default().fg(DEFAULT_THEME.type_name),
                ));
            }

            let line = Line::from(spans);
            all_items.push(ListItem::new(line));
        }
    }
}

/// Format a value with styled spans
pub(super) fn format_value_styled<S: BuildHasher>(
    value: &Value,
    struct_defs: &HashMap<String, StructDef, S>,
    indent: usize,
) -> Vec<Span<'static>> {
    // Helper to format recursively as string for complex types if needed, or simple spans
    match value {
        Value::Int(n) => vec![Span::styled(
            format!("{}", n),
            Style::default().fg(DEFAULT_THEME.number),
        )],
        Value::Char(c) => {
            let byte = *c as u8;
            if byte.is_ascii_graphic() || byte == b' ' {
                vec![Span::styled(
                    format!("'{}'", byte as char),
                    Style::default().fg(DEFAULT_THEME.string),
                )]
            } else {
                vec![Span::styled(
                    format!("'\\x{:02x}'", byte),
                    Style::default().fg(DEFAULT_THEME.string),
                )]
            }
        }
        Value::Pointer(addr) => {
            if *addr == 0 {
                vec![Span::styled(
                    "NULL",
                    Style::default().fg(DEFAULT_THEME.number),
                )]
            } else {
                vec![Span::styled(
                    format!("0x{:08x}", addr),
                    Style::default().fg(DEFAULT_THEME.secondary),
                )]
            }
        }
        Value::Null => vec![Span::styled(
            "NULL",
            Style::default().fg(DEFAULT_THEME.error),
        )],
        Value::Struct(_) => {
            // Should be handled by caller for multi-line, or fallback here
            vec![Span::styled(
                "{...}",
                Style::default().fg(DEFAULT_THEME.comment),
            )]
        }
        Value::Array(_elements) => {
            if indent > 2 {
                return vec![Span::styled(
                    "[...]",
                    Style::default().fg(DEFAULT_THEME.comment),
                )];
            }
            // For arrays, get the string representation and highlight it
            let s = format_value_string(value, struct_defs, indent);
            highlight_value_string(&s)
        }
        Value::Uninitialized => vec![Span::styled(
            "[uninit]",
            Style::default()
                .fg(DEFAULT_THEME.error)
                .add_modifier(Modifier::DIM),
        )],
    }
}

/// Legacy format value for string contexts (e.g. array recursion)
fn format_value_string<S: BuildHasher>(
    value: &Value,
    _struct_defs: &HashMap<String, StructDef, S>,
    indent: usize,
) -> String {
    match value {
        Value::Int(n) => format!("{}", n),
        Value::Char(c) => {
            let byte = *c as u8;
            if byte.is_ascii_graphic() || byte == b' ' {
                format!("'{}'", byte as char)
            } else {
                format!("'\\x{:02x}'", byte)
            }
        }
        Value::Pointer(addr) => {
            if *addr == 0 {
                "NULL".to_string()
            } else {
                format!("0x{:08x}", addr)
            }
        }
        Value::Null => "NULL".to_string(),
        Value::Struct(fields) => {
            if indent > 2 {
                return "{...}".to_string();
            }
            let mut s = String::from("{ ");
            let mut first = true;
            for (name, val) in fields {
                if !first {
                    s.push_str(", ");
                }
                first = false;
                s.push_str(&format!(
                    "{}: {}",
                    name,
                    format_value_string(val, _struct_defs, indent + 1)
                ));
            }
            s.push_str(" }");
            s
        }
        Value::Array(elements) => {
            if indent > 2 {
                return "[...]".to_string();
            }
            let mut s = String::from("[");
            for (i, elem) in elements.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                if i >= 5 {
                    s.push_str("...");
                    break;
                }
                s.push_str(&format_value_string(elem, _struct_defs, indent + 1));
            }
            s.push(']');
            s
        }
        Value::Uninitialized => "[uninit]".to_string(),
    }
}

/// Format a type annotation for display
pub(super) fn format_type_annotation<S: BuildHasher>(
    typ: &Type,
    _struct_defs: &HashMap<String, StructDef, S>,
) -> String {
    let mut result = String::new();

    // Base type
    match &typ.base {
        BaseType::Int => result.push_str("int"),
        BaseType::Char => result.push_str("char"),
        BaseType::Void => result.push_str("void"),
        BaseType::Struct(name) => {
            result.push_str(&format!("struct {}", name));
        }
    }

    // Pointers
    for _ in 0..typ.pointer_depth {
        result.push('*');
    }

    // Arrays
    for dim in &typ.array_dims {
        if let Some(size) = dim {
            result.push_str(&format!("[{}]", size));
        } else {
            result.push_str("[]");
        }
    }

    result
}

/// Highlight a value string (simple lexer)
fn highlight_value_string(s: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_token = String::new();

    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Handle delimiters
        if matches!(c, '[' | ']' | '{' | '}' | ',' | ':' | ' ') {
            if !current_token.is_empty() {
                // Flush token
                spans.push(style_token(&current_token));
                current_token.clear();
            }
            spans.push(Span::styled(
                c.to_string(),
                Style::default().fg(DEFAULT_THEME.fg),
            ));
            i += 1;
            continue;
        }

        current_token.push(c);
        i += 1;
    }

    if !current_token.is_empty() {
        spans.push(style_token(&current_token));
    }

    spans
}

fn style_token(token: &str) -> Span<'static> {
    if token == "NULL" {
        Span::styled(token.to_string(), Style::default().fg(DEFAULT_THEME.number))
    } else if token.starts_with('\'') || token.starts_with('"') {
        Span::styled(token.to_string(), Style::default().fg(DEFAULT_THEME.string))
    } else if token.starts_with("0x") {
        Span::styled(
            token.to_string(),
            Style::default().fg(DEFAULT_THEME.secondary),
        )
    } else if token == "{" || token == "}" {
        // Fallback for struct braces if they got stuck in token
        Span::styled(token.to_string(), Style::default().fg(DEFAULT_THEME.fg))
    } else if token.chars().all(|c| c.is_ascii_digit() || c == '-') {
        Span::styled(token.to_string(), Style::default().fg(DEFAULT_THEME.number))
    } else {
        // Default (field names etc)
        Span::styled(token.to_string(), Style::default().fg(DEFAULT_THEME.fg))
    }
}

/// Calculate field offsets in a struct
pub(super) fn calculate_field_offsets<S: BuildHasher>(
    fields: &[Field],
    struct_defs: &HashMap<String, StructDef, S>,
) -> Vec<(String, usize, usize, Type)> {
    let mut result = Vec::new();
    let mut offset = 0;

    for field in fields {
        let size = sizeof_type(&field.field_type, struct_defs);
        result.push((field.name.clone(), offset, size, field.field_type.clone()));
        offset += size;
    }

    result
}

/// Read a typed value from a byte slice
pub(super) fn read_typed_value<S: BuildHasher>(
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
                let bytes = [data[0], data[1], data[2], data[3]];
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
                let bytes = [data[0], data[1], data[2], data[3]];
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
