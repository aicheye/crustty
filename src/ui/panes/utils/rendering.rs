use super::formatting::{format_type_annotation, format_value_styled};
use super::memory::calculate_field_offsets;
use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::{BaseType, StructDef, Type};
use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::ListItem,
};
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::hash::BuildHasher;

pub(crate) struct RenderCtx<'a, S: BuildHasher> {
    pub struct_defs: &'a HashMap<String, StructDef, S>,
    pub content_width: usize,
}

/// Render array elements recursively with proper nesting and indentation
pub(crate) fn render_array_elements<'a, S: BuildHasher>(
    all_items: &mut Vec<ListItem<'a>>,
    elements: &[Value],
    array_type: &Type,
    base_address: u64,
    indent_level: usize,
    ctx: &RenderCtx<'a, S>,
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
    let elem_size = sizeof_type(&elem_type, ctx.struct_defs) as u64;

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
                let type_str =
                    format_type_annotation(&elem_type, ctx.struct_defs); // struct_defs added

                // Calculate padding for right-alignment
                // addr(11) + indent + "[idx] " + ": " = 11 + indent + index_str + 2
                let index_str = format!("[{}] ", idx);
                let left_width = 11 + indent.len() + index_str.len() + 2; // +2 for ": "
                let type_width = type_str.len();
                let padding =
                    ctx.content_width.saturating_sub(left_width + type_width);

                let mut spans = vec![
                    addr_span,
                    Span::raw(indent),
                    Span::styled(
                        index_str,
                        Style::default().fg(DEFAULT_THEME.fg),
                    ),
                    Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                ];

                if !type_str.is_empty() {
                    spans.push(Span::raw(" ".repeat(padding)));
                    spans.push(Span::styled(
                        type_str,
                        Style::default().fg(DEFAULT_THEME.type_name),
                    ));
                }

                all_items.push(ListItem::new(Line::from(spans)));

                // Recursively render nested array
                render_array_elements(
                    all_items,
                    nested_elements,
                    &elem_type,
                    elem_address,
                    indent_level + 1,
                    ctx,
                );
            }
            Value::Struct(fields) => {
                // Struct element - show header and recurse
                let type_str =
                    format_type_annotation(&elem_type, ctx.struct_defs);

                // Calculate padding for right-alignment
                let index_str = format!("[{}] ", idx);
                let left_width = 11 + indent.len() + index_str.len() + 2; // +2 for ": "
                let type_width = type_str.len();
                let padding =
                    ctx.content_width.saturating_sub(left_width + type_width);

                let mut spans = vec![
                    addr_span,
                    Span::raw(indent),
                    Span::styled(
                        index_str,
                        Style::default().fg(DEFAULT_THEME.fg),
                    ),
                    Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                ];

                if !type_str.is_empty() {
                    spans.push(Span::raw(" ".repeat(padding)));
                    spans.push(Span::styled(
                        type_str,
                        Style::default().fg(DEFAULT_THEME.type_name),
                    ));
                }

                all_items.push(ListItem::new(Line::from(spans)));

                // Recursively render struct fields
                render_struct_fields(
                    all_items,
                    fields,
                    &elem_type,
                    elem_address,
                    indent_level + 1,
                    ctx,
                );
            }
            _ => {
                // Primitive value - show on single line
                let val_spans =
                    format_value_styled(elem_value, ctx.struct_defs, 1);
                let type_str =
                    format_type_annotation(&elem_type, ctx.struct_defs);

                // Calculate padding for right-alignment
                let index_str = format!("[{}] ", idx);
                let val_width: usize =
                    val_spans.iter().map(|s| s.content.len()).sum();
                let left_width =
                    11 + indent.len() + index_str.len() + 2 + val_width;
                let type_width = type_str.len();
                let padding =
                    ctx.content_width.saturating_sub(left_width + type_width);

                let mut spans = vec![
                    addr_span,
                    Span::raw(indent),
                    Span::styled(
                        index_str,
                        Style::default().fg(DEFAULT_THEME.fg),
                    ),
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

                all_items.push(ListItem::new(Line::from(spans)));
            }
        }
    }
}

pub(crate) fn render_struct_fields<'a, S: BuildHasher>(
    all_items: &mut Vec<ListItem<'a>>,
    fields: &FxHashMap<String, Value>,
    parent_type: &Type,
    base_address: u64,
    indent_level: usize,
    ctx: &RenderCtx<'a, S>,
) {
    // Calculate field offsets to show addresses and types
    let field_info: std::collections::HashMap<String, (usize, Type)> =
        if let BaseType::Struct(struct_name) = &parent_type.base {
            if let Some(struct_def) = ctx.struct_defs.get(struct_name) {
                calculate_field_offsets(&struct_def.fields, ctx.struct_defs)
                    .into_iter()
                    .map(|(name, offset, _size, field_type)| {
                        (name, (offset, field_type))
                    })
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
                let type_str =
                    format_type_annotation(field_type, ctx.struct_defs);
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
            let padding =
                ctx.content_width.saturating_sub(left_width + type_width);

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

            all_items.push(ListItem::new(Line::from(spans)));

            // Recursively render nested struct fields
            if let Some(field_type) = field_type_opt {
                if let Some((offset, _)) = field_info.get(field_name) {
                    render_struct_fields(
                        all_items,
                        nested_fields,
                        &field_type,
                        base_address + (*offset as u64),
                        indent_level + 1,
                        ctx,
                    );
                }
            }
        } else {
            // Non-struct field - render as a single line
            let val_spans =
                format_value_styled(field_value, ctx.struct_defs, 1);

            // Calculate padding for right-alignment
            let field_str = format!(".{} ", field_name);
            let val_width: usize =
                val_spans.iter().map(|s| s.content.len()).sum();
            let left_width =
                11 + indent.len() + field_str.len() + 2 + val_width; // +2 for ": "
            let type_width = type_annotation.len();
            let padding =
                ctx.content_width.saturating_sub(left_width + type_width);

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

            all_items.push(ListItem::new(Line::from(spans)));
        }
    }
}
