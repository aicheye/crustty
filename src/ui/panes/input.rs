//! Input pane rendering for displaying original stdin input

use crate::snapshot::{TerminalLine, TerminalLineKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Scroll state for the input pane
pub struct InputScrollState {
    pub offset: usize,
}

/// Data needed to render the input pane
pub struct InputRenderData<'a> {
    pub all_input_lines: &'a [TerminalLine],
    /// How many input lines have occurred up to the current snapshot position.
    /// Lines beyond this index are shown dimmed/greyed out.
    pub active_count: usize,
    pub is_focused: bool,
    pub source_code: &'a str,
    pub scroll_state: &'a mut InputScrollState,
}

/// Render the input pane showing the original stdin input
pub fn render_input_pane(frame: &mut Frame, area: Rect, data: InputRenderData) {
    use super::source::highlight_source_code;
    use crate::ui::theme::DEFAULT_THEME;

    let border_style = if data.is_focused {
        Style::default()
            .fg(DEFAULT_THEME.border_focused)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DEFAULT_THEME.border_normal)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Stored Input ")
        .border_style(border_style);

    // Use all stored input lines (persisted across snapshots)
    let input_lines: Vec<(usize, String)> = data
        .all_input_lines
        .iter()
        .filter(|l| l.kind == TerminalLineKind::Input)
        .map(|l| (l.location.line, l.text.clone()))
        .collect();

    let total_inputs = input_lines.len();
    let mut paragraph_lines = Vec::new();
    if input_lines.is_empty() {
        let style = Style::default().fg(DEFAULT_THEME.comment);
        paragraph_lines
            .push(Line::from(vec![Span::styled("(no input)", style)]));
    } else {
        let arrow = "\u{2190}"; // Unicode left arrow
        let max_lineno = input_lines.iter().map(|(n, _)| *n).max().unwrap_or(1);
        let lineno_width = max_lineno.to_string().len().max(2);
        let max_input_width = input_lines
            .iter()
            .map(|(_, input)| input.len())
            .max()
            .unwrap_or(1);

        let active_count = data.active_count;

        for (idx, (line_num, input)) in input_lines.iter().enumerate() {
            let is_active = idx < active_count;
            let is_latest_active =
                idx + 1 == active_count && active_count == total_inputs;

            // Fully dimmed style for future (not-yet-reached) inputs
            let dim_style = Style::default()
                .fg(DEFAULT_THEME.comment)
                .add_modifier(Modifier::DIM);

            let mut spans = Vec::new();

            // Left padding
            spans.push(Span::raw(" "));

            // Line number, right-aligned, grey
            let lineno = format!("{:>width$}", line_num, width = lineno_width);
            spans.push(Span::styled(
                lineno,
                if is_active {
                    Style::default().fg(DEFAULT_THEME.comment)
                } else {
                    dim_style
                },
            ));
            spans.push(Span::styled(
                " : ",
                Style::default().fg(DEFAULT_THEME.comment),
            ));

            // Source code — syntax highlighted if active, dimmed if future
            let src_line = data
                .source_code
                .lines()
                .nth(line_num.saturating_sub(1))
                .unwrap_or("");
            if is_active {
                spans.extend(highlight_source_code(src_line.trim()).spans);
            } else {
                spans.push(Span::styled(src_line.trim(), dim_style));
            }

            // Arrow and input, right-aligned
            let input_display =
                format!("{:>width$}", input, width = max_input_width);
            let input_style = if !is_active {
                dim_style.add_modifier(Modifier::ITALIC)
            } else if is_latest_active {
                Style::default()
                    .fg(DEFAULT_THEME.secondary)
                    .add_modifier(Modifier::ITALIC | Modifier::BOLD)
            } else {
                Style::default()
                    .fg(DEFAULT_THEME.secondary)
                    .add_modifier(Modifier::ITALIC | Modifier::DIM)
            };

            // Pad to align arrow and input to the right
            let pad = area.width.saturating_sub(
                (1 + lineno_width
                    + 3
                    + src_line.trim().len()
                    + 3
                    + max_input_width
                    + 2) as u16,
            );
            if pad > 0 {
                spans.push(Span::raw(" ".repeat(pad as usize)));
            }
            spans.push(Span::styled(
                arrow,
                if is_active {
                    Style::default().fg(DEFAULT_THEME.comment)
                } else {
                    dim_style
                },
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(input_display, input_style));
            // Right padding
            spans.push(Span::raw(" "));
            paragraph_lines.push(Line::from(spans));
        }
    }

    // Ensure the last active input line is visible, then clamp scroll offset
    let content_height = area.height.saturating_sub(2) as usize; // borders
    let total_lines = paragraph_lines.len();
    if total_lines > content_height {
        // Auto-scroll so the last active input is on screen
        if data.active_count > 0 {
            let last_active_idx = data.active_count - 1;
            // If the last active line is below the visible window, scroll down
            if last_active_idx >= data.scroll_state.offset + content_height {
                data.scroll_state.offset = last_active_idx - content_height + 1;
            }
            // If the last active line is above the visible window, scroll up
            if last_active_idx < data.scroll_state.offset {
                data.scroll_state.offset = last_active_idx;
            }
        }
        let max_scroll = total_lines - content_height;
        data.scroll_state.offset = data.scroll_state.offset.min(max_scroll);
    } else {
        data.scroll_state.offset = 0;
    }

    let paragraph = Paragraph::new(paragraph_lines)
        .block(block)
        .scroll((data.scroll_state.offset as u16, 0));
    frame.render_widget(paragraph, area);
}
