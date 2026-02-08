//! Heap pane rendering with memory allocations and hex dumps
//!
//! This module renders the heap pane, showing dynamically allocated memory
//! blocks with their contents, types, and allocation status.
//!
//! # Features
//!
//! - Memory block visualization with addresses and sizes
//! - Allocation status indicators (allocated, freed, never allocated)
//! - Typed value rendering for allocated blocks
//! - Hex dump view for raw memory inspection
//! - Scroll support for large heaps
//!
//! # Display Modes
//!
//! - **Typed View**: Shows values interpreted according to their declared type
//! - **Hex Dump**: Shows raw byte values for low-level inspection
//! - **Struct Layout**: Visualizes struct field layout with offsets

use super::utils::{calculate_field_offsets, format_type_annotation, read_typed_value};
use crate::memory::{heap::BlockState, heap::Heap, sizeof_type};
use crate::parser::ast::{BaseType, StructDef, Type};
use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use std::hash::BuildHasher;

/// Scroll state for the heap pane
pub struct HeapScrollState {
    pub offset: usize,
    pub prev_item_count: usize,
}

/// Data needed to render the heap pane
pub struct HeapRenderData<'a, S: BuildHasher, T: BuildHasher> {
    pub heap: &'a Heap,
    pub pointer_types: &'a std::collections::HashMap<u64, Type, S>,
    pub struct_defs: &'a std::collections::HashMap<String, StructDef, T>,
    pub error_address: Option<u64>,
}

/// Render the heap pane
pub fn render_heap_pane<S: BuildHasher, T: BuildHasher>(
    frame: &mut Frame,
    area: Rect,
    data: HeapRenderData<S, T>,
    is_focused: bool,
    scroll_state: &mut HeapScrollState,
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

    let content_width = area.width.saturating_sub(2) as usize; // borders

    let allocations = data.heap.allocations();
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
            let _style = match block.state {
                BlockState::Allocated => Style::default().fg(DEFAULT_THEME.success),
                BlockState::Tombstone => Style::default().fg(DEFAULT_THEME.error),
            };

            // Build header with type annotation if available
            let type_str = if let Some(typ) = data.pointer_types.get(addr) {
                format_type_annotation(typ, data.struct_defs)
            } else {
                String::new()
            };

            // Check if this address matches the error address
            let addr_style = if Some(*addr) == data.error_address {
                Style::default()
                    .fg(DEFAULT_THEME.error)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(DEFAULT_THEME.comment)
            };

            let header = if type_str.is_empty() {
                Line::from(vec![
                    Span::styled(format!("0x{:08x}", addr), addr_style),
                    Span::raw(" | "),
                    Span::styled(
                        format!("{} bytes", block.size),
                        Style::default().fg(DEFAULT_THEME.primary),
                    ),
                ])
            } else {
                // Calculate padding for right alignment
                // Left part: "0xADDR | SIZE bytes"
                // 10 chars for addr, 3 for " | ", len of size + " bytes"
                let left_len = 10 + 3 + format!("{} bytes", block.size).len();
                let right_len = type_str.len();
                let padding = content_width.saturating_sub(left_len + right_len);

                Line::from(vec![
                    Span::styled(format!("0x{:08x}", addr), addr_style),
                    Span::raw(" | "),
                    Span::styled(
                        format!("{} bytes", block.size),
                        Style::default().fg(DEFAULT_THEME.primary),
                    ),
                    Span::raw(" ".repeat(padding)),
                    Span::styled(type_str, Style::default().fg(DEFAULT_THEME.type_name)),
                ])
            };
            all_items.push(ListItem::new(header));

            // Show hex dump of allocated data with dynamic wrapping
            if block.state == BlockState::Allocated {
                let typ_opt = data.pointer_types.get(addr);

                // Check if this is a struct type
                let is_struct = typ_opt.is_some_and(|t| matches!(t.base, BaseType::Struct(_)));

                if is_struct {
                    // Show struct with field annotations
                    if let Some(Type {
                        base: BaseType::Struct(struct_name),
                        ..
                    }) = typ_opt
                    {
                        if let Some(struct_def) = data.struct_defs.get(struct_name) {
                            let field_info =
                                calculate_field_offsets(&struct_def.fields, data.struct_defs);
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

                                // Build hex part with full address
                                let full_addr = *addr + offset as u64;
                                let mut hex_part = format!("  0x{:08x}: ", full_addr);
                                for i in offset..field_end {
                                    if block.init_map[i] {
                                        hex_part.push_str(&format!("{:02x} ", block.data[i]));
                                    } else {
                                        hex_part.push_str("?? ");
                                    }
                                }

                                // Pad hex part for alignment
                                // hex_part starts with "  0xADDR: " (14 chars), so target length is 14 + target_hex_width
                                let target_len = 14 + target_hex_width;
                                if hex_part.len() < target_len {
                                    let padding = " ".repeat(target_len - hex_part.len());
                                    hex_part.push_str(&padding);
                                }

                                // Prepare annotation parts
                                let value_str_opt = read_typed_value(
                                    &block.data[offset..],
                                    &block.init_map[offset..],
                                    &field_type,
                                    data.struct_defs,
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
                        let elem_size = sizeof_type(typ, data.struct_defs);
                        if elem_size > 0 && block.size >= elem_size {
                            // Display as array of elements
                            let num_elements = block.size / elem_size;
                            let max_elem_size = elem_size;
                            let target_hex_width = max_elem_size * 3; // Each byte is "XX "

                            for elem_idx in 0..num_elements {
                                let offset = elem_idx * elem_size;
                                let elem_end = (offset + elem_size).min(block.size);

                                // Build hex part for this element with full address
                                let full_addr = *addr + offset as u64;
                                let mut hex_part = format!("  0x{:08x}: ", full_addr);
                                for i in offset..elem_end {
                                    if block.init_map[i] {
                                        hex_part.push_str(&format!("{:02x} ", block.data[i]));
                                    } else {
                                        hex_part.push_str("?? ");
                                    }
                                }

                                // Pad hex part for alignment
                                let target_len = 14 + target_hex_width;
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
                                    data.struct_defs,
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
                                let full_addr = *addr + remaining_offset as u64;
                                let mut hex_part = format!("  0x{:08x}: ", full_addr);
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
                                let full_addr = *addr + line_start as u64;
                                let mut hex_part = format!("  0x{:08x}: ", full_addr);
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
                            let full_addr = *addr + line_start as u64;
                            let mut hex_part = format!("  0x{:08x}: ", full_addr);
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
    if total_items > scroll_state.prev_item_count {
        // Content grew (new allocation), auto-scroll to bottom
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
