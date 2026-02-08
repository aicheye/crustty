//! Stack pane rendering with local variables and call frames
//!
//! This module renders the stack pane, displaying the call stack with
//! function frames and their local variables.
//!
//! # Features
//!
//! - Call stack visualization with function names and parameters
//! - Local variable display with types, values, and memory addresses
//! - Nested structure and array rendering
//! - Scroll support for large stacks
//! - Type annotations for complex data types
//!
//! # Layout
//!
//! Each stack frame is displayed hierarchically:
//! - Function name and parameter list
//! - Local variables with addresses and values
//! - Nested structures and arrays with indentation

use super::utils::{
    format_type_annotation, format_value_styled, render_array_elements, render_struct_fields,
};
use crate::memory::{stack::Stack, value::Value};
use crate::parser::ast::StructDef;
use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use std::collections::HashMap;
use std::hash::BuildHasher;

/// Scroll state for the stack pane
pub struct StackScrollState {
    pub offset: usize,
    pub prev_item_count: usize,
}

/// Data needed to render the stack pane
pub struct StackRenderData<'a, S: BuildHasher, T: BuildHasher> {
    pub stack: &'a Stack,
    pub struct_defs: &'a HashMap<String, StructDef, S>,
    pub source_code: &'a str,
    pub return_value: Option<&'a Value>,
    pub function_defs: &'a HashMap<String, crate::interpreter::engine::FunctionDef, T>,
    pub error_address: Option<u64>,
}

/// Render the stack pane
pub fn render_stack_pane<S: BuildHasher, T: BuildHasher>(
    frame: &mut Frame,
    area: Rect,
    data: StackRenderData<S, T>,
    is_focused: bool,
    scroll_state: &mut StackScrollState,
) {
    let border_style = if is_focused {
        Style::default()
            .fg(DEFAULT_THEME.border_focused)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DEFAULT_THEME.border_normal)
    };

    let block = Block::default()
        .title(" Call Stack ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let frames = data.stack.frames();
    let mut all_items = Vec::new();

    // Calculate available width for text wrapping (account for borders)
    let content_width = area.width.saturating_sub(2) as usize; // borders only

    if frames.is_empty() {
        all_items.push(ListItem::new("(empty)").style(Style::default().fg(DEFAULT_THEME.comment)));
    } else {
        for (depth, stack_frame) in frames.iter().enumerate() {
            // Create emphasized frame header with box-drawing characters
            let frame_header = Line::from(vec![
                Span::styled("▸ ", Style::default().fg(DEFAULT_THEME.secondary)),
                Span::styled(
                    format!("Frame {} ", depth),
                    Style::default().fg(DEFAULT_THEME.comment),
                ),
                Span::styled("│ ", Style::default().fg(DEFAULT_THEME.comment)),
                Span::styled(
                    format!("{}()", stack_frame.function_name),
                    Style::default()
                        .fg(DEFAULT_THEME.function)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);

            all_items.push(ListItem::new(frame_header));

            // Show the complete call chain from main to this frame
            // Collect all call sites from frame 1 up to current frame
            for caller_depth in 1..=depth {
                if let Some(loc) = &frames[caller_depth].return_location {
                    // Get the line content from source code
                    let line_content = data
                        .source_code
                        .lines()
                        .nth(loc.line.saturating_sub(1))
                        .unwrap_or("???");
                    let trimmed = line_content.trim();

                    // Get caller function name and frame number
                    let caller_name = &frames[caller_depth - 1].function_name;
                    let frame_num = caller_depth - 1;

                    // Format: ↪ [N] function() → call_site
                    let caller_info = format!("  ↪ [{}] {}() → ", frame_num, caller_name);

                    // Wrap the call site text if it's too long
                    if caller_info.len() + trimmed.len() <= content_width {
                        // Fits on one line
                        let location_line = Line::from(vec![
                            Span::styled("  ↪ ", Style::default().fg(DEFAULT_THEME.comment)),
                            Span::styled(
                                format!("[{}] ", frame_num),
                                Style::default().fg(DEFAULT_THEME.comment),
                            ),
                            Span::styled(
                                format!("{}()", caller_name),
                                Style::default().fg(DEFAULT_THEME.muted_function),
                            ),
                            Span::styled(" → ", Style::default().fg(DEFAULT_THEME.comment)),
                            Span::styled(
                                trimmed.to_string(),
                                Style::default().fg(DEFAULT_THEME.comment),
                            ),
                        ]);
                        all_items.push(ListItem::new(location_line));
                    } else {
                        // Need to wrap - first line has the caller info
                        let call_site_indent = 4; // Indent for continuation lines
                        let first_line_chars = content_width.saturating_sub(caller_info.len());
                        let (first_part, rest) = split_at_char_boundary(trimmed, first_line_chars);

                        let first_line = Line::from(vec![
                            Span::styled("  ↪ ", Style::default().fg(DEFAULT_THEME.comment)),
                            Span::styled(
                                format!("[{}] ", frame_num),
                                Style::default().fg(DEFAULT_THEME.comment),
                            ),
                            Span::styled(
                                format!("{}()", caller_name),
                                Style::default().fg(DEFAULT_THEME.muted_function),
                            ),
                            Span::styled(" → ", Style::default().fg(DEFAULT_THEME.comment)),
                            Span::styled(
                                first_part.to_string(),
                                Style::default().fg(DEFAULT_THEME.comment),
                            ),
                        ]);
                        all_items.push(ListItem::new(first_line));

                        // Wrap remaining text
                        let mut remaining = rest;
                        while !remaining.is_empty() {
                            let wrap_width = content_width.saturating_sub(call_site_indent);
                            let (part, next_rest) = split_at_char_boundary(remaining, wrap_width);
                            let continuation = Line::from(vec![
                                Span::raw("    "), // Indent continuation lines
                                Span::styled(
                                    part.to_string(),
                                    Style::default().fg(DEFAULT_THEME.comment),
                                ),
                            ]);
                            all_items.push(ListItem::new(continuation));
                            remaining = next_rest;
                        }
                    }
                }
            }

            // Display return value if present (only for top frame - the currently executing function)
            if depth == frames.len() - 1 {
                if let Some(ret_val) = data.return_value {
                    let val_spans = format_value_styled(ret_val, data.struct_defs, 0);

                    // Get return type from function definition
                    let return_type_str = data
                        .function_defs
                        .get(&stack_frame.function_name)
                        .map(|func_def| {
                            format_type_annotation(&func_def.return_type, data.struct_defs)
                        })
                        .unwrap_or_else(|| "?".to_string());

                    // Calculate widths for alignment
                    let val_width: usize = val_spans.iter().map(|s| s.content.len()).sum();
                    // "     ↖ " (7) + "return " (7) + ": " (2) = 16
                    let left_width = 16 + val_width;
                    let right_width = return_type_str.len();
                    let padding = content_width.saturating_sub(left_width + right_width);

                    let mut spans = vec![
                        Span::styled(
                            "     ↖ ",
                            Style::default()
                                .fg(DEFAULT_THEME.return_value)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "return ",
                            Style::default()
                                .fg(DEFAULT_THEME.return_value)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(": ", Style::default().fg(DEFAULT_THEME.return_value)),
                    ];

                    spans.extend(val_spans.into_iter().map(|span| {
                        Span::styled(
                            span.content.to_string(),
                            span.style.fg(DEFAULT_THEME.return_value),
                        )
                    }));

                    spans.push(Span::raw(" ".repeat(padding)));

                    spans.push(Span::styled(
                        return_type_str,
                        Style::default().fg(DEFAULT_THEME.type_name),
                    ));

                    let line = Line::from(spans);
                    all_items.push(ListItem::new(line));
                }
            }

            // Local variables
            // Iterate in declaration order (insertion_order)
            for var_name in &stack_frame.insertion_order {
                if let Some(local_var) = stack_frame.locals.get(var_name) {
                    let init_state = match &local_var.init_state {
                        crate::memory::stack::InitState::Initialized => None,
                        crate::memory::stack::InitState::Uninitialized => Some(" [uninit]"),
                        crate::memory::stack::InitState::PartiallyInitialized(_) => {
                            Some(" [partial]")
                        }
                    };

                    // Format the address
                    let addr_style = if Some(local_var.address) == data.error_address {
                        Style::default()
                            .fg(DEFAULT_THEME.error)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(DEFAULT_THEME.comment)
                    };

                    let addr_span =
                        Span::styled(format!("0x{:08x} ", local_var.address), addr_style);

                    // Show structs with fields on separate lines
                    match &local_var.value {
                        Value::Array(elements) => {
                            // Treat arrays similarly to structs - show each index with address
                            let init_span = if let Some(s) = init_state {
                                Span::styled(s, Style::default().fg(DEFAULT_THEME.error))
                            } else {
                                Span::raw("")
                            };

                            // Get the array type name
                            let type_str =
                                format_type_annotation(&local_var.var_type, data.struct_defs);

                            // Align type to right
                            let type_width = type_str.len();
                            let init_len = if let Some(s) = init_state { s.len() } else { 0 };
                            // addr(11) + " " + name + " " + ": " + init = 15 + name + init
                            let left_width = 15 + var_name.len() + init_len;
                            let padding = content_width.saturating_sub(left_width + type_width);

                            let header = Line::from(vec![
                                addr_span,
                                Span::styled(
                                    format!(" {} ", var_name),
                                    Style::default().fg(DEFAULT_THEME.fg),
                                ),
                                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                                init_span,
                                Span::raw(" ".repeat(padding)),
                                Span::styled(
                                    type_str,
                                    Style::default().fg(DEFAULT_THEME.type_name),
                                ),
                            ]);

                            all_items.push(ListItem::new(header));

                            // Render array elements with addresses
                            render_array_elements(
                                &mut all_items,
                                elements,
                                &local_var.var_type,
                                local_var.address,
                                1, // indent level
                                data.struct_defs,
                                content_width,
                            );
                        }
                        Value::Struct(fields) => {
                            let init_span = if let Some(s) = init_state {
                                Span::styled(s, Style::default().fg(DEFAULT_THEME.error))
                            } else {
                                Span::raw("")
                            };

                            // Get the struct type name
                            let type_str =
                                format_type_annotation(&local_var.var_type, data.struct_defs);

                            // Align type to right
                            let type_width = type_str.len();
                            let init_len = if let Some(s) = init_state { s.len() } else { 0 };
                            // addr(11) + " " + name + " " + ": " + init = 15 + name + init
                            let left_width = 15 + var_name.len() + init_len;
                            let padding = content_width.saturating_sub(left_width + type_width);

                            let header = Line::from(vec![
                                addr_span,
                                Span::styled(
                                    format!(" {} ", var_name),
                                    Style::default().fg(DEFAULT_THEME.fg),
                                ),
                                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                                init_span,
                                Span::raw(" ".repeat(padding)),
                                Span::styled(
                                    type_str,
                                    Style::default().fg(DEFAULT_THEME.type_name),
                                ),
                            ]);

                            all_items.push(ListItem::new(header));

                            // Render struct fields recursively
                            render_struct_fields(
                                &mut all_items,
                                fields,
                                &local_var.var_type,
                                local_var.address,
                                1, // indent level
                                data.struct_defs,
                                content_width,
                            );
                        }
                        _ => {
                            let val_spans =
                                format_value_styled(&local_var.value, data.struct_defs, 0);

                            // Only add init_span if the value isn't already displaying its uninitialized state
                            let init_span = if matches!(local_var.value, Value::Uninitialized) {
                                // Value::Uninitialized already displays [uninit], don't duplicate
                                Span::raw("")
                            } else if let Some(s) = init_state {
                                Span::styled(s, Style::default().fg(DEFAULT_THEME.error))
                            } else {
                                Span::raw("")
                            };

                            // Add type annotation for non-struct variables
                            let type_str =
                                format_type_annotation(&local_var.var_type, data.struct_defs);
                            let type_width = if type_str.is_empty() {
                                0
                            } else {
                                type_str.len()
                            };

                            // Width calculation for alignment
                            let val_width: usize = val_spans.iter().map(|s| s.content.len()).sum();
                            let init_content: &str =
                                if matches!(local_var.value, Value::Uninitialized) {
                                    ""
                                } else {
                                    init_state.unwrap_or_default()
                                };

                            // addr(11) + name + " " + ": " + val + init = 14 + name + val + init
                            let left_width = 14 + var_name.len() + val_width + init_content.len();
                            let padding = content_width.saturating_sub(left_width + type_width);

                            let mut spans = vec![
                                addr_span,
                                Span::styled(
                                    format!("{} ", var_name),
                                    Style::default().fg(DEFAULT_THEME.fg),
                                ),
                                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                            ];

                            spans.extend(val_spans);
                            spans.push(init_span);

                            // Add type annotation aligned to right
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

            // Add spacing between frames
            if depth < frames.len() - 1 {
                let separator =
                    Line::from(Span::styled("", Style::default().fg(DEFAULT_THEME.comment)));
                all_items.push(ListItem::new(separator));
            }
        }
    }

    // Calculate visible range for scrolling
    let total_items = all_items.len();
    let visible_height = area.height.saturating_sub(2).max(1) as usize; // Account for borders, min 1

    // Smart auto-scroll: scroll to bottom only when content grows
    if total_items > scroll_state.prev_item_count {
        // Content grew (new frame/variable added), auto-scroll to bottom
        if total_items > visible_height {
            scroll_state.offset = total_items - visible_height;
        } else {
            scroll_state.offset = 0;
        }
    } else {
        // Content same or shrank, respect user's scroll position (just clamp)
        if total_items > visible_height {
            let max_scroll = total_items - visible_height;
            scroll_state.offset = scroll_state.offset.min(max_scroll);
        } else {
            scroll_state.offset = 0;
        }
    }

    // Update previous item count for next render
    scroll_state.prev_item_count = total_items;

    // Take only visible items
    let visible_items: Vec<ListItem> = all_items
        .into_iter()
        .skip(scroll_state.offset)
        .take(visible_height)
        .collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

/// Split a string at a character boundary, ensuring we don't cut in the middle of a char
fn split_at_char_boundary(s: &str, max_chars: usize) -> (&str, &str) {
    if s.len() <= max_chars {
        return (s, "");
    }

    // Find the last valid char boundary at or before max_chars
    let mut end = max_chars.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    if end == 0 {
        // Edge case: first char is wider than max_chars, just take it
        let first_char_end = s.char_indices().nth(1).map(|(i, _)| i).unwrap_or(s.len());
        (&s[..first_char_end], &s[first_char_end..])
    } else {
        (&s[..end], &s[end..])
    }
}
