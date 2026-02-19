use crate::memory::value::Value;
use crate::parser::ast::{StructDef, Type};
use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};
use std::collections::HashMap;
use std::hash::BuildHasher;

/// Format a value with styled spans
pub(crate) fn format_value_styled<S: BuildHasher>(
    value: &Value,
    struct_defs: &HashMap<String, StructDef, S>,
    indent: usize,
) -> Vec<Span<'static>> {
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
        Value::Struct(_) => {
            vec![Span::styled(
                "{ struct }",
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
            let s = format_value_string(value, struct_defs, indent);
            highlight_value_string(&s)
        }
        Value::Uninitialized => vec![Span::styled(
            "[uninit]",
            Style::default()
                .fg(DEFAULT_THEME.error)
                .add_modifier(Modifier::DIM),
        )],
        // Handle explicit Null case if Value enum has it (it seemed to in read_file output)
        // Wait, check Value enum. read_file output had Value::Null around line 348.
        // It might be redundant with Pointer(0) but if it exists...
        // Let's check src/memory/value.rs
        _ => vec![Span::styled(
            "Unknown",
            Style::default().fg(DEFAULT_THEME.error),
        )],
    }
}

pub(crate) fn format_type_annotation<S: BuildHasher>(
    type_: &Type,
    _struct_defs: &HashMap<String, StructDef, S>,
) -> String {
    let mut s = String::new();

    match &type_.base {
        crate::parser::ast::BaseType::Int => s.push_str("int"),
        crate::parser::ast::BaseType::Char => s.push_str("char"),
        crate::parser::ast::BaseType::Void => s.push_str("void"),
        crate::parser::ast::BaseType::Struct(name) => {
            s.push_str("struct ");
            s.push_str(name);
        }
    }

    if type_.is_const {
        s.insert_str(0, "const ");
    }

    for _ in 0..type_.pointer_depth {
        s.push('*');
    }

    for dim in &type_.array_dims {
        if let Some(size) = dim {
            s.push_str(&format!("[{}]", size));
        } else {
            s.push_str("[]");
        }
    }

    s
}

fn format_value_string<S: BuildHasher>(
    value: &Value,
    _struct_defs: &HashMap<String, StructDef, S>,
    _indent: usize,
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
        Value::Struct(_) => "{ struct }".to_string(),
        Value::Array(elements) => {
            let mut s = String::from("[");
            for (i, val) in elements.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                if i >= 3 {
                    s.push_str("...");
                    break;
                }
                s.push_str(&format_value_string(val, _struct_defs, _indent + 1));
            }
            s.push(']');
            s
        }
        Value::Uninitialized => "[uninit]".to_string(),
        _ => "Unknown".to_string(),
    }
}

fn highlight_value_string(s: &str) -> Vec<Span<'static>> {
    vec![Span::raw(s.to_string())]
}
