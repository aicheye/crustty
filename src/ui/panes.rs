//! Rendering logic for each TUI pane

use crate::memory::{heap::BlockState, heap::Heap, sizeof_type, stack::Stack, value::Value};
use crate::parser::ast::{BaseType, Field, StructDef, Type};
use crate::snapshot::MockTerminal;
use crate::ui::theme::DEFAULT_THEME;

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
};
use std::collections::HashMap;

/// Simple syntax highlighting for C-like C code
fn highlight_source_code(line: &str) -> Line<'_> {
    let mut spans = Vec::new();
    let mut current_word = String::new();

    // Simple tokenizer
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Handle comments
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            if !current_word.is_empty() {
                spans.push(Span::raw(current_word.clone()));
                current_word.clear();
            }
            spans.push(Span::styled(
                line[i..].to_string(),
                Style::default().fg(DEFAULT_THEME.comment),
            ));
            break;
        }

        // Handle strings
        if c == '"' {
            if !current_word.is_empty() {
                spans.push(Span::raw(current_word.clone()));
                current_word.clear();
            }
            let mut end = i + 1;
            while end < chars.len() && chars[end] != '"' {
                if chars[end] == '\\' {
                    end += 2;
                } else {
                    end += 1;
                }
            }
            if end < chars.len() {
                end += 1;
            }
            spans.push(Span::styled(
                line[i..end].to_string(),
                Style::default().fg(DEFAULT_THEME.string), // Strings
            ));
            i = end;
            continue;
        }

        // Handle non-alphanumeric (delimiters)
        if !c.is_alphanumeric() && c != '_' {
            if !current_word.is_empty() {
                let is_func = c == '(';
                let style = get_keyword_style(&current_word, is_func);
                spans.push(Span::styled(current_word.clone(), style));
                current_word.clear();
            }

            // Color some operators/delimiters
            let style = match c {
                '{' | '}' | '(' | ')' | '[' | ']' => Style::default().fg(DEFAULT_THEME.primary), // Brackets
                ';' => Style::default().fg(DEFAULT_THEME.fg), // Semicolons
                ',' => Style::default().fg(DEFAULT_THEME.fg),
                '.' => Style::default().fg(DEFAULT_THEME.fg),
                '+' | '-' | '*' | '/' | '=' | '&' | '|' | '!' | '<' | '>' => {
                    Style::default().fg(DEFAULT_THEME.fg)
                } // Operators
                _ => Style::default(),
            };

            spans.push(Span::styled(c.to_string(), style));
            i += 1;
            continue;
        }

        current_word.push(c);
        i += 1;
    }

    if !current_word.is_empty() {
        let style = get_keyword_style(&current_word, false);
        spans.push(Span::styled(current_word, style));
    }

    Line::from(spans)
}

fn get_keyword_style(word: &str, is_function: bool) -> Style {
    match word {
        "int" | "char" | "void" | "bool" | "float" | "double" | "long" | "short" | "unsigned"
        | "signed" => {
            Style::default().fg(DEFAULT_THEME.type_name) // Types
        }
        "struct" | "return" | "if" | "else" | "while" | "for" | "do" | "switch" | "case"
        | "default" | "break" | "continue" | "goto" | "sizeof" => {
            Style::default()
                .fg(DEFAULT_THEME.keyword)
                .add_modifier(Modifier::BOLD) // Keywords
        }
        "NULL" => Style::default().fg(DEFAULT_THEME.number), // Constants
        _ => {
            if is_function {
                Style::default().fg(DEFAULT_THEME.function)
            } else {
                Style::default().fg(DEFAULT_THEME.fg) // Variables/Identifiers
            }
        }
    }
}

/// Render the source code pane
pub fn render_source_pane(
    frame: &mut Frame,
    area: Rect,
    source_code: &str,
    current_line: usize,
    is_focused: bool,
    scroll_offset: &mut usize,
    target_line_row: &mut Option<usize>,
) {
    let border_style = if is_focused {
        Style::default()
            .fg(DEFAULT_THEME.border_focused)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DEFAULT_THEME.border_normal)
    };

    let block = Block::default()
        .title(" Source Code ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let lines: Vec<&str> = source_code.lines().collect();
    let total_lines = lines.len();

    // Calculate visible range
    let visible_height = area.height.saturating_sub(2).max(1) as usize; // Account for borders (2), min 1

    // Initialize target_line_row to center if not set
    if target_line_row.is_none() {
        *target_line_row = Some(visible_height / 2);
    }

    // Get the target row, clamping to stay within visible area
    let target_row = target_line_row
        .unwrap()
        .min(visible_height.saturating_sub(1));
    *target_line_row = Some(target_row);

    // Calculate scroll offset to keep current line at target visual row
    if current_line > 0 && current_line <= total_lines {
        let target_line_idx = current_line.saturating_sub(1); // Convert to 0-based
        *scroll_offset = target_line_idx.saturating_sub(target_row);

        // Clamp scroll offset to valid range
        if total_lines > visible_height {
            let max_scroll = total_lines - visible_height;
            *scroll_offset = (*scroll_offset).min(max_scroll);
        } else {
            *scroll_offset = 0;
        }
    }

    let visible_lines: Vec<Line> = lines
        .iter()
        .enumerate()
        .skip(*scroll_offset)
        .take(visible_height)
        .map(|(idx, line)| {
            let line_num = idx + 1;
            let is_current = line_num == current_line;

            let line_num_str = format!("{:4} ", line_num);

            // Base style for the line
            let (num_style, content_base_style) = if is_current {
                (
                    Style::default()
                        .fg(DEFAULT_THEME.secondary)
                        .add_modifier(Modifier::BOLD),
                    Style::default().bg(DEFAULT_THEME.current_line_bg),
                )
            } else {
                (
                    Style::default().fg(DEFAULT_THEME.comment), // Line numbers
                    Style::default(),
                )
            };

            // Highlight syntax
            let mut content_line = highlight_source_code(line);

            // Apply background if current line
            if is_current {
                for span in &mut content_line.spans {
                    span.style = span.style.patch(content_base_style);
                }
            }

            let mut final_spans = vec![Span::styled(line_num_str, num_style)];
            final_spans.extend(content_line.spans);

            Line::from(final_spans)
        })
        .collect();

    let paragraph = Paragraph::new(visible_lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Recursively render struct fields with proper nesting and indentation
fn render_struct_fields<'a>(
    all_items: &mut Vec<ListItem<'a>>,
    fields: &HashMap<String, Value>,
    parent_type: &Type,
    base_address: u64,
    indent_level: usize,
    struct_defs: &HashMap<String, StructDef>,
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
            let mut spans = vec![
                field_addr_span,
                Span::raw(indent),
                Span::styled(
                    format!(".{} ", field_name),
                    Style::default().fg(DEFAULT_THEME.fg),
                ),
                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
            ];

            if !type_annotation.is_empty() {
                spans.push(Span::styled("| ", Style::default().fg(DEFAULT_THEME.fg)));
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
                    );
                }
            }
        } else {
            // Non-struct field - render as a single line
            let val_spans = format_value_styled(field_value, struct_defs, 1);

            let mut spans = vec![
                field_addr_span,
                Span::raw(indent),
                Span::styled(
                    format!(".{} ", field_name),
                    Style::default().fg(DEFAULT_THEME.fg),
                ),
                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
            ];

            spans.extend(val_spans);

            if !type_annotation.is_empty() {
                spans.push(Span::styled(" | ", Style::default().fg(DEFAULT_THEME.fg)));
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

/// Render the stack pane
pub fn render_stack_pane(
    frame: &mut Frame,
    area: Rect,
    stack: &Stack,
    struct_defs: &HashMap<String, StructDef>,
    source_code: &str,
    is_focused: bool,
    scroll_offset: &mut usize,
    prev_item_count: &mut usize,
    return_value: Option<&Value>,
    function_defs: &HashMap<String, crate::interpreter::engine::FunctionDef>,
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

    let frames = stack.frames();
    let mut all_items = Vec::new();

    // Calculate available width for text wrapping (account for borders and indent)
    let content_width = area.width.saturating_sub(2) as usize; // borders
    let call_site_indent = 4; // "  ↪ " prefix length for first line, "    " for continuation
    let call_site_width = content_width.saturating_sub(call_site_indent);

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
                    let line_content = source_code
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
                        let first_line_chars =
                            call_site_width.saturating_sub(caller_info.len() - call_site_indent);
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
                            let (part, next_rest) =
                                split_at_char_boundary(remaining, call_site_width);
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
                if let Some(ret_val) = return_value {
                    let val_spans = format_value_styled(ret_val, struct_defs, 0);

                    // Get return type from function definition
                    let return_type_str = function_defs
                        .get(&stack_frame.function_name)
                        .map(|func_def| format_type_annotation(&func_def.return_type, struct_defs))
                        .unwrap_or_else(|| "?".to_string());

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

                    spans.push(Span::styled(
                        " | ",
                        Style::default().fg(DEFAULT_THEME.return_value),
                    ));

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
                    let addr_span = Span::styled(
                        format!("0x{:08x} ", local_var.address),
                        Style::default().fg(DEFAULT_THEME.comment),
                    );

                    // Show structs with fields on separate lines
                    match &local_var.value {
                        Value::Struct(fields) => {
                            let init_span = if let Some(s) = init_state {
                                Span::styled(s, Style::default().fg(DEFAULT_THEME.error))
                            } else {
                                Span::raw("")
                            };

                            // Get the struct type name
                            let type_str = format_type_annotation(&local_var.var_type, struct_defs);

                            let header = Line::from(vec![
                                addr_span,
                                Span::styled(
                                    format!("{} ", var_name),
                                    Style::default().fg(DEFAULT_THEME.fg),
                                ),
                                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                                Span::styled("| ", Style::default().fg(DEFAULT_THEME.fg)),
                                Span::styled(
                                    type_str,
                                    Style::default().fg(DEFAULT_THEME.type_name),
                                ),
                                init_span,
                            ]);

                            all_items.push(ListItem::new(header));

                            // Render struct fields recursively
                            render_struct_fields(
                                &mut all_items,
                                fields,
                                &local_var.var_type,
                                local_var.address,
                                1, // indent level
                                struct_defs,
                            );
                        }
                        _ => {
                            let val_spans = format_value_styled(&local_var.value, struct_defs, 0);

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
                            let type_str = format_type_annotation(&local_var.var_type, struct_defs);

                            let mut spans = vec![
                                addr_span,
                                Span::styled(
                                    format!("{} ", var_name),
                                    Style::default().fg(DEFAULT_THEME.fg),
                                ),
                                Span::styled(": ", Style::default().fg(DEFAULT_THEME.fg)),
                            ];

                            spans.extend(val_spans);

                            // Add type annotation with different color after value
                            if !type_str.is_empty() {
                                spans.push(Span::styled(
                                    " | ",
                                    Style::default().fg(DEFAULT_THEME.fg),
                                ));
                                spans.push(Span::styled(
                                    type_str,
                                    Style::default().fg(DEFAULT_THEME.type_name),
                                ));
                            }

                            spans.push(init_span);

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
    if total_items > *prev_item_count {
        // Content grew (new frame/variable added), auto-scroll to bottom
        if total_items > visible_height {
            *scroll_offset = total_items - visible_height;
        } else {
            *scroll_offset = 0;
        }
    } else {
        // Content same or shrank, respect user's scroll position (just clamp)
        if total_items > visible_height {
            let max_scroll = total_items - visible_height;
            *scroll_offset = (*scroll_offset).min(max_scroll);
        } else {
            *scroll_offset = 0;
        }
    }

    // Update previous item count for next render
    *prev_item_count = total_items;

    // Take only visible items
    let visible_items: Vec<ListItem> = all_items
        .into_iter()
        .skip(*scroll_offset)
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

/// Render the heap pane
pub fn render_heap_pane(
    frame: &mut Frame,
    area: Rect,
    heap: &Heap,
    pointer_types: &std::collections::HashMap<u64, Type>,
    struct_defs: &std::collections::HashMap<String, StructDef>,
    is_focused: bool,
    scroll_offset: &mut usize,
    prev_item_count: &mut usize,
) {
    let border_style = if is_focused {
        Style::default()
            .fg(DEFAULT_THEME.border_focused)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DEFAULT_THEME.border_normal)
    };

    let block = Block::default()
        .title(" Heap Memory ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let allocations = heap.allocations();
    let mut all_items = Vec::new();

    if allocations.is_empty() {
        all_items.push(
            ListItem::new("(no allocations)").style(Style::default().fg(DEFAULT_THEME.comment)),
        );
    } else {
        // Filter out tombstones (freed blocks)
        let mut sorted_allocs: Vec<_> = allocations
            .iter()
            .filter(|(_, block)| block.state == BlockState::Allocated)
            .collect();
        sorted_allocs.sort_by_key(|(addr, _)| *addr);

        let alloc_count = sorted_allocs.len();

        if alloc_count == 0 {
            all_items.push(
                ListItem::new("(no active allocations)")
                    .style(Style::default().fg(DEFAULT_THEME.comment)),
            );
        }

        for (i, (addr, block)) in sorted_allocs.into_iter().enumerate() {
            let style = match block.state {
                BlockState::Allocated => Style::default().fg(DEFAULT_THEME.success),
                BlockState::Tombstone => Style::default().fg(DEFAULT_THEME.error),
            };

            // Build header with type annotation if available
            let type_str = if let Some(typ) = pointer_types.get(addr) {
                format_type_annotation(typ, struct_defs)
            } else {
                String::new()
            };

            let header = if type_str.is_empty() {
                Line::from(vec![
                    Span::styled(
                        format!("0x{:08x}", addr),
                        Style::default().fg(DEFAULT_THEME.comment),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        format!("{} bytes", block.size),
                        Style::default().fg(DEFAULT_THEME.primary),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("0x{:08x}", addr),
                        Style::default().fg(DEFAULT_THEME.comment),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        format!("{} bytes", block.size),
                        Style::default().fg(DEFAULT_THEME.primary),
                    ),
                    Span::raw(" | "),
                    Span::styled(type_str, Style::default().fg(DEFAULT_THEME.type_name)),
                ])
            };
            all_items.push(ListItem::new(header));

            // Show hex dump of allocated data with dynamic wrapping
            if block.state == BlockState::Allocated {
                let typ_opt = pointer_types.get(addr);

                // Check if this is a struct type
                let is_struct = typ_opt.map_or(false, |t| matches!(t.base, BaseType::Struct(_)));

                if is_struct {
                    // Show struct with field annotations
                    if let Some(Type {
                        base: BaseType::Struct(struct_name),
                        ..
                    }) = typ_opt
                    {
                        if let Some(struct_def) = struct_defs.get(struct_name) {
                            let field_info =
                                calculate_field_offsets(&struct_def.fields, struct_defs);
                            let max_field_len = field_info
                                .iter()
                                .map(|(n, _, _, _)| n.len())
                                .max()
                                .unwrap_or(0);

                            // Calculate formatting constants
                            // Find the largest field size to determine hex alignment
                            let max_field_size = field_info
                                .iter()
                                .map(|(_, _, size, _)| *size)
                                .max()
                                .unwrap_or(4);
                            let target_hex_width = max_field_size * 3; // Each byte is "XX "

                            for (field_name, offset, size, field_type) in field_info {
                                // Show hex dump for this field
                                let field_end = (offset + size).min(block.size);

                                // Build hex part
                                let mut hex_part = format!("  {:04x}: ", offset);
                                for i in offset..field_end {
                                    if block.init_map[i] {
                                        hex_part.push_str(&format!("{:02x} ", block.data[i]));
                                    } else {
                                        hex_part.push_str("?? ");
                                    }
                                }

                                // Pad hex part for alignment
                                // hex_part starts with "  addr: " (8 chars), so target length is 8 + target_hex_width
                                let target_len = 8 + target_hex_width;
                                if hex_part.len() < target_len {
                                    let padding = " ".repeat(target_len - hex_part.len());
                                    hex_part.push_str(&padding);
                                }

                                // Prepare annotation parts
                                let value_str_opt = read_typed_value(
                                    &block.data[offset..],
                                    &block.init_map[offset..],
                                    &field_type,
                                    struct_defs,
                                );
                                let mut annotation_spans = vec![
                                    Span::styled("=> ", Style::default().fg(DEFAULT_THEME.comment)),
                                    Span::styled(".", Style::default().fg(DEFAULT_THEME.fg)),
                                    Span::styled(
                                        format!("{:<width$} : ", field_name, width = max_field_len),
                                        Style::default().fg(DEFAULT_THEME.fg),
                                    ),
                                ];

                                let mut annotation_len = 3 + 1 + max_field_len + 3; // "=> " + "." + padded_field_name + " : "

                                if let Some(ref val) = value_str_opt {
                                    if val == "NULL" {
                                        annotation_spans.push(Span::styled(
                                            "NULL",
                                            Style::default().fg(DEFAULT_THEME.error),
                                        ));
                                        annotation_len += 4;
                                    } else if val.starts_with("[") {
                                        // Uninitialized or partially initialized value
                                        annotation_spans.push(Span::styled(
                                            val.clone(),
                                            Style::default().fg(DEFAULT_THEME.error),
                                        ));
                                        annotation_len += val.len();
                                    } else {
                                        annotation_spans.push(Span::styled(
                                            val.clone(),
                                            Style::default().fg(DEFAULT_THEME.secondary),
                                        ));
                                        annotation_len += val.len();
                                    }
                                } else {
                                    annotation_spans.push(Span::styled(
                                        "?",
                                        Style::default().fg(DEFAULT_THEME.comment),
                                    ));
                                    annotation_len += 1;
                                }

                                // Check available width to decide layout
                                let max_width = area.width.saturating_sub(4) as usize; // Borders/padding
                                let hex_len = hex_part.len();
                                let indent_len = 2; // "  " spacing

                                if hex_len + indent_len + annotation_len <= max_width {
                                    // Single line layout
                                    let mut line_spans = vec![
                                        Span::styled(
                                            hex_part,
                                            Style::default().fg(DEFAULT_THEME.comment),
                                        ),
                                        Span::raw("  "),
                                    ];
                                    line_spans.extend(annotation_spans);
                                    all_items.push(ListItem::new(Line::from(line_spans)));
                                } else {
                                    // Multi-line layout
                                    all_items.push(
                                        ListItem::new(hex_part)
                                            .style(Style::default().fg(DEFAULT_THEME.comment)),
                                    );

                                    let mut next_line_spans = vec![Span::raw("          ")]; // Indent
                                    next_line_spans.extend(annotation_spans);

                                    all_items.push(ListItem::new(Line::from(next_line_spans)));
                                }
                            }
                        }
                    }
                } else {
                    // Regular display for non-struct types (primitives, arrays, pointers)
                    // For arrays, show each element with its value
                    if let Some(typ) = typ_opt {
                        let elem_size = sizeof_type(typ, struct_defs);
                        if elem_size > 0 && block.size >= elem_size {
                            // Display as array of elements
                            let num_elements = block.size / elem_size;
                            let max_elem_size = elem_size;
                            let target_hex_width = max_elem_size * 3; // Each byte is "XX "

                            for elem_idx in 0..num_elements {
                                let offset = elem_idx * elem_size;
                                let elem_end = (offset + elem_size).min(block.size);

                                // Build hex part for this element
                                let mut hex_part = format!("  {:04x}: ", offset);
                                for i in offset..elem_end {
                                    if block.init_map[i] {
                                        hex_part.push_str(&format!("{:02x} ", block.data[i]));
                                    } else {
                                        hex_part.push_str("?? ");
                                    }
                                }

                                // Pad hex part for alignment
                                let target_len = 8 + target_hex_width;
                                if hex_part.len() < target_len {
                                    let padding = " ".repeat(target_len - hex_part.len());
                                    hex_part.push_str(&padding);
                                }

                                // Get value interpretation for this element
                                let mut line_spans = vec![Span::styled(
                                    hex_part,
                                    Style::default().fg(DEFAULT_THEME.comment),
                                )];

                                if let Some(value_str) = read_typed_value(
                                    &block.data[offset..],
                                    &block.init_map[offset..],
                                    typ,
                                    struct_defs,
                                ) {
                                    line_spans.push(Span::styled(
                                        "  => ",
                                        Style::default().fg(DEFAULT_THEME.comment),
                                    ));
                                    if value_str == "NULL" {
                                        line_spans.push(Span::styled(
                                            value_str,
                                            Style::default().fg(DEFAULT_THEME.error),
                                        ));
                                    } else if value_str.starts_with("[") {
                                        // Uninitialized or partially initialized value
                                        line_spans.push(Span::styled(
                                            value_str,
                                            Style::default().fg(DEFAULT_THEME.error),
                                        ));
                                    } else {
                                        line_spans.push(Span::styled(
                                            value_str,
                                            Style::default().fg(DEFAULT_THEME.secondary),
                                        ));
                                    }
                                }

                                all_items.push(ListItem::new(Line::from(line_spans)));
                            }

                            // If there are remaining bytes that don't fit in an element, show them
                            let remaining_offset = num_elements * elem_size;
                            if remaining_offset < block.size {
                                let mut hex_part = format!("  {:04x}: ", remaining_offset);
                                for i in remaining_offset..block.size {
                                    if block.init_map[i] {
                                        hex_part.push_str(&format!("{:02x} ", block.data[i]));
                                    } else {
                                        hex_part.push_str("?? ");
                                    }
                                }
                                all_items.push(
                                    ListItem::new(hex_part)
                                        .style(Style::default().fg(DEFAULT_THEME.comment)),
                                );
                            }
                        } else {
                            // Fallback: element size is 0 or invalid, show raw hex dump
                            let available_width = area.width.saturating_sub(40) as usize;
                            let max_bytes_per_line = (available_width / 3).max(1);
                            let bytes_per_line = 16.min(max_bytes_per_line);

                            for line_start in (0..block.size).step_by(bytes_per_line) {
                                let line_end = (line_start + bytes_per_line).min(block.size);
                                let mut hex_part = format!("  {:04x}: ", line_start);
                                for i in line_start..line_end {
                                    if block.init_map[i] {
                                        hex_part.push_str(&format!("{:02x} ", block.data[i]));
                                    } else {
                                        hex_part.push_str("?? ");
                                    }
                                }
                                all_items.push(
                                    ListItem::new(hex_part)
                                        .style(Style::default().fg(DEFAULT_THEME.comment)),
                                );
                            }
                        }
                    } else {
                        // No type info, show raw hex dump
                        let available_width = area.width.saturating_sub(40) as usize;
                        let max_bytes_per_line = (available_width / 3).max(1);
                        let bytes_per_line = 16.min(max_bytes_per_line);

                        for line_start in (0..block.size).step_by(bytes_per_line) {
                            let line_end = (line_start + bytes_per_line).min(block.size);
                            let mut hex_part = format!("  {:04x}: ", line_start);
                            for i in line_start..line_end {
                                if block.init_map[i] {
                                    hex_part.push_str(&format!("{:02x} ", block.data[i]));
                                } else {
                                    hex_part.push_str("?? ");
                                }
                            }
                            all_items.push(
                                ListItem::new(hex_part)
                                    .style(Style::default().fg(DEFAULT_THEME.comment)),
                            );
                        }
                    }
                }
            }

            if i < alloc_count - 1 {
                let separator =
                    Line::from(Span::styled("", Style::default().fg(DEFAULT_THEME.comment)));
                all_items.push(ListItem::new(separator));
            }
        }
    }

    // Calculate visible range for scrolling
    let total_items = all_items.len();
    let visible_height = area.height.saturating_sub(2).max(1) as usize; // Account for borders, min 1

    // Smart auto-scroll: scroll to bottom only when content grows (new allocation)
    if total_items > *prev_item_count {
        // Content grew (new allocation), auto-scroll to bottom
        if total_items > visible_height {
            *scroll_offset = total_items - visible_height;
        } else {
            *scroll_offset = 0;
        }
    } else {
        // Content same or shrank, respect user's scroll position (just clamp)
        if total_items > visible_height {
            let max_scroll = total_items - visible_height;
            *scroll_offset = (*scroll_offset).min(max_scroll);
        } else {
            *scroll_offset = 0;
        }
    }

    // Update previous item count for next render
    *prev_item_count = total_items;

    // Take only visible items
    let visible_items: Vec<ListItem> = all_items
        .into_iter()
        .skip(*scroll_offset)
        .take(visible_height)
        .collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

/// Render the terminal output pane
pub fn render_terminal_pane(
    frame: &mut Frame,
    area: Rect,
    terminal: &MockTerminal,
    is_focused: bool,
    scroll_offset: &mut usize,
) {
    let border_style = if is_focused {
        Style::default()
            .fg(DEFAULT_THEME.border_focused)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DEFAULT_THEME.border_normal)
    };

    let block = Block::default()
        .title(" Terminal Output ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let lines = terminal.get_output();

    if lines.is_empty() {
        let paragraph = Paragraph::new("(no output)")
            .block(block)
            .style(Style::default().fg(DEFAULT_THEME.comment));
        frame.render_widget(paragraph, area);
    } else {
        let block = block.padding(Padding::new(1, 0, 0, 0));
        // Build all items
        let all_items: Vec<ListItem> = lines
            .iter()
            .map(|line| ListItem::new(line.as_str()).style(Style::default().fg(DEFAULT_THEME.fg))) // Content with padding via Block
            .collect();

        // Calculate visible range for scrolling
        let total_items = all_items.len();
        let visible_height = area.height.saturating_sub(2).max(1) as usize; // Account for borders, min 1

        // Clamp scroll offset only if content exceeds visible area
        if total_items > visible_height {
            let max_scroll = total_items - visible_height;
            *scroll_offset = (*scroll_offset).min(max_scroll);
        } else {
            *scroll_offset = 0;
        }

        // Take only visible items
        let visible_items: Vec<ListItem> = all_items
            .into_iter()
            .skip(*scroll_offset)
            .take(visible_height)
            .collect();

        let list = List::new(visible_items).block(block); // Block style (border/title) is separate from content style
        frame.render_widget(list, area);
    }
}

/// Render the status bar at the bottom
pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    current_step: usize,
    total_steps: usize,
    is_playing: bool,
) {
    // Split status bar into left and right
    let layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage(50),
            ratatui::layout::Constraint::Percentage(50),
        ])
        .split(area);

    // Left side: Step info and status
    let left_spans = vec![
        Span::styled(
            format!(" Step {}/{} ", current_step + 1, total_steps),
            Style::default()
                .bg(DEFAULT_THEME.primary)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " | ",
            Style::default()
                .bg(DEFAULT_THEME.current_line_bg)
                .fg(DEFAULT_THEME.comment),
        ),
        Span::styled(
            format!(" {} ", message),
            Style::default()
                .bg(DEFAULT_THEME.current_line_bg)
                .fg(DEFAULT_THEME.fg),
        ),
    ];

    let left_paragraph = Paragraph::new(Line::from(left_spans))
        .style(Style::default().bg(DEFAULT_THEME.current_line_bg))
        .alignment(Alignment::Left);

    frame.render_widget(left_paragraph, layout[0]);

    // Right side: Keybinds with visual grouping
    let key_style = Style::default().bg(DEFAULT_THEME.comment).fg(Color::Black);
    let desc_style = Style::default()
        .bg(DEFAULT_THEME.current_line_bg)
        .fg(DEFAULT_THEME.fg);
    let sep_style = Style::default()
        .bg(DEFAULT_THEME.current_line_bg)
        .fg(DEFAULT_THEME.comment);

    let mut right_spans = vec![
        Span::styled(" ←/→ ", key_style),
        Span::styled(" step ", desc_style),
        Span::styled("│", sep_style),
        Span::styled(" ", desc_style),
        Span::styled(" 1-9 ", key_style),
        Span::styled(" jump ", desc_style),
        Span::styled("│", sep_style),
        Span::styled(" ", desc_style),
        Span::styled(" ⎵ ", key_style),
        Span::styled(" play ", desc_style),
        Span::styled("│", sep_style),
        Span::styled(" ", desc_style),
        Span::styled(" ↵ / ⌫ ", key_style),
        Span::styled(" end/start ", desc_style),
        Span::styled("│", sep_style),
        Span::styled(" ", desc_style),
        Span::styled("q", key_style),
        Span::styled(" quit ", desc_style),
    ];

    // Show status indicators based on position and state
    let is_at_start = current_step == 0;
    let is_at_end = current_step + 1 >= total_steps;

    if is_playing {
        right_spans.push(Span::styled("│", sep_style));
        right_spans.push(Span::styled(
            " ▶ PLAYING ",
            Style::default()
                .bg(DEFAULT_THEME.secondary)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    } else if is_at_end {
        right_spans.push(Span::styled("│", sep_style));
        right_spans.push(Span::styled(
            " END ",
            Style::default()
                .bg(DEFAULT_THEME.error)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    } else if is_at_start {
        right_spans.push(Span::styled("│", sep_style));
        right_spans.push(Span::styled(
            " START ",
            Style::default()
                .bg(DEFAULT_THEME.success)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let right_paragraph = Paragraph::new(Line::from(right_spans))
        .style(Style::default().bg(DEFAULT_THEME.current_line_bg))
        .alignment(Alignment::Right);

    frame.render_widget(right_paragraph, layout[1]);
}

/// Format a value for display with styling
/// Format a value with syntax highlighting
fn format_value_styled(
    value: &Value,
    struct_defs: &HashMap<String, StructDef>,
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
fn format_value_string(
    value: &Value,
    struct_defs: &HashMap<String, StructDef>,
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
                    format_value_string(val, struct_defs, indent + 1)
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
                s.push_str(&format_value_string(elem, struct_defs, indent + 1));
            }
            s.push(']');
            s
        }
        Value::Uninitialized => "[uninit]".to_string(),
    }
}

/// Format a type annotation for display in the heap
fn format_type_annotation(typ: &Type, _struct_defs: &HashMap<String, StructDef>) -> String {
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

/// Interpret heap data based on type information
/// Returns None to indicate struct fields will be shown separately
fn interpret_heap_data(
    block: &crate::memory::heap::HeapBlock,
    typ: &Type,
    _struct_defs: &HashMap<String, StructDef>,
) -> Option<String> {
    // Only interpret if data is fully initialized
    if !block.init_map.iter().all(|&b| b) {
        return None;
    }

    // Handle pointers first
    if typ.pointer_depth > 0 {
        if block.size >= 4 {
            let bytes = [block.data[0], block.data[1], block.data[2], block.data[3]];
            let addr = u32::from_le_bytes(bytes);
            if addr == 0 {
                return Some("NULL".to_string());
            } else {
                return Some(format!("0x{:08x}", addr));
            }
        }
        return None;
    }

    // Handle basic types (structs are handled separately in hex dump)
    match &typ.base {
        BaseType::Int => {
            // Read as i32 (4 bytes, little-endian)
            if block.size >= 4 {
                let bytes = [block.data[0], block.data[1], block.data[2], block.data[3]];
                let value = i32::from_le_bytes(bytes);
                Some(format!("{}", value))
            } else {
                None
            }
        }
        BaseType::Char => {
            // Show as character or array of characters
            if block.size == 1 {
                let byte = block.data[0];
                if byte.is_ascii_graphic() || byte == b' ' {
                    Some(format!("'{}'", byte as char))
                } else {
                    Some(format!("'\\x{:02x}'", byte))
                }
            } else {
                // Try to interpret as string
                let mut chars = String::new();
                let mut all_printable = true;
                for &byte in &block.data {
                    if byte == 0 {
                        break;
                    }
                    if byte.is_ascii_graphic() || byte == b' ' {
                        chars.push(byte as char);
                    } else {
                        all_printable = false;
                        break;
                    }
                }
                if all_printable && !chars.is_empty() {
                    Some(format!("\"{}\"", chars))
                } else {
                    None
                }
            }
        }
        BaseType::Struct(_) => {
            // Structs are handled separately with field annotations
            None
        }
        BaseType::Void => None,
    }
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
fn calculate_field_offsets(
    fields: &[Field],
    struct_defs: &HashMap<String, StructDef>,
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
fn read_typed_value(
    data: &[u8],
    init_map: &[bool],
    typ: &Type,
    struct_defs: &HashMap<String, StructDef>,
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
