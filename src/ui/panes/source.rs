//! Source code pane rendering with syntax highlighting
//!
//! This module renders the source code pane, which displays the C program
//! being executed with basic syntax highlighting and execution indicators.
//!
//! # Features
//!
//! - Syntax highlighting for C keywords, types, strings, numbers, and comments
//! - Current line highlighting with arrow indicator
//! - Scroll state management for navigating large files
//! - Line numbering
//!
//! # Rendering
//!
//! The pane uses a simple character-by-character tokenizer to apply syntax
//! highlighting styles without requiring a full lexer.

use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

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

/// Scroll state for the source pane
pub struct SourceScrollState {
    pub offset: usize,
    pub target_line_row: Option<usize>,
}

/// Render the source code pane
#[allow(clippy::too_many_arguments)]
pub fn render_source_pane(
    frame: &mut Frame,
    area: Rect,
    source_code: &str,
    current_line: usize,
    is_error: bool,
    is_scanf: bool,
    is_focused: bool,
    scroll_state: &mut SourceScrollState,
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
    if scroll_state.target_line_row.is_none() {
        scroll_state.target_line_row = Some(visible_height / 2);
    }

    // Get the target row, clamping to stay within visible area
    let target_row = scroll_state
        .target_line_row
        .unwrap()
        .min(visible_height.saturating_sub(1));
    scroll_state.target_line_row = Some(target_row);

    // Calculate scroll offset to keep current line at target visual row
    if current_line > 0 && current_line <= total_lines {
        let target_line_idx = current_line.saturating_sub(1); // Convert to 0-based
        scroll_state.offset = target_line_idx.saturating_sub(target_row);

        // Clamp scroll offset to valid range
        if total_lines > visible_height {
            let max_scroll = total_lines - visible_height;
            scroll_state.offset = scroll_state.offset.min(max_scroll);
        } else {
            scroll_state.offset = 0;
        }
    }

    let visible_lines: Vec<Line> = lines
        .iter()
        .enumerate()
        .skip(scroll_state.offset)
        .take(visible_height)
        .map(|(idx, line)| {
            let line_num = idx + 1;
            let is_current = line_num == current_line;
            let line_num_str = format!("{:4} ", line_num);

            // Base style for the line
            let (num_style, content_base_style) = if is_error {
                // ERROR LINE: Red background with bold line number
                (
                    Style::default()
                        .fg(DEFAULT_THEME.error)
                        .add_modifier(Modifier::BOLD),
                    Style::default()
                        .bg(DEFAULT_THEME.error)
                        .fg(ratatui::style::Color::White) // White text on red for visibility
                        .add_modifier(Modifier::BOLD),
                )
            } else if is_scanf {
                // SCANF LINE: Secondary background with bold line number
                (
                    Style::default()
                        .fg(DEFAULT_THEME.secondary)
                        .add_modifier(Modifier::BOLD),
                    Style::default()
                        .bg(DEFAULT_THEME.secondary)
                        .fg(ratatui::style::Color::Black) // Black text on orange for visibility
                        .add_modifier(Modifier::BOLD),
                )
            } else if is_current {
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

            // Apply background style
            if is_error {
                // For error lines, override all styling with error style
                for span in &mut content_line.spans {
                    span.style = content_base_style;
                }
            } else if is_current {
                // For current line, just apply background
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
