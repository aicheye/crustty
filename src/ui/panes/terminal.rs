//! Terminal output pane rendering

use crate::snapshot::{MockTerminal, TerminalLineKind};
use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};

/// Render the terminal output pane.
///
/// When `is_scanf_input` is true, an input prompt line is shown at the bottom and
/// one row of the content area is reserved for it.
pub fn render_terminal_pane(
    frame: &mut Frame,
    area: Rect,
    terminal: &MockTerminal,
    is_focused: bool,
    scroll_offset: &mut usize,
    is_scanf_input: bool,
    input_buffer: &str,
) {
    let border_style = if is_focused {
        Style::default()
            .fg(DEFAULT_THEME.border_focused)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DEFAULT_THEME.border_normal)
    };

    let title = if is_scanf_input {
        " Terminal — waiting for input "
    } else {
        " Terminal "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let lines = terminal.get_output();

    // Always reserve 1 row at the bottom for the stdin input bar
    let inner_height = area.height.saturating_sub(2) as usize;
    let content_height = inner_height.saturating_sub(1).max(1);

    let block = block.padding(Padding::new(1, 1, 0, 0));

    // Pre-compute inner area before block is consumed by the List widget
    let inner = block.inner(area);

    // Build list items; show a placeholder when there is no output yet
    let all_items: Vec<ListItem> = if lines.is_empty() {
        vec![ListItem::new("(no output)").style(Style::default().fg(DEFAULT_THEME.comment))]
    } else {
        lines
            .iter()
            .map(|(text, kind)| {
                let style = match kind {
                    TerminalLineKind::Output => Style::default().fg(DEFAULT_THEME.fg),
                    TerminalLineKind::Input => Style::default()
                        .fg(DEFAULT_THEME.secondary)
                        .add_modifier(Modifier::ITALIC),
                };
                ListItem::new(text.as_str()).style(style)
            })
            .collect()
    };

    let total_items = all_items.len();

    // Clamp scroll
    if total_items > content_height {
        let max_scroll = total_items - content_height;
        *scroll_offset = (*scroll_offset).min(max_scroll);
    } else {
        *scroll_offset = 0;
    }

    let visible_items: Vec<ListItem> = all_items
        .into_iter()
        .skip(*scroll_offset)
        .take(content_height)
        .collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);

    // Always render the stdin input bar at the very bottom of the inner area.
    // Active (waiting for input): bright accent colour + blinking cursor.
    // Inactive: dimmed, no cursor — shows the pre-fill buffer if any.
    if inner.height > 0 {
        let prompt_y = inner.y + inner.height - 1;
        let prompt_area = Rect {
            x: inner.x,
            y: prompt_y,
            width: inner.width,
            height: 1,
        };

        let prompt_line = if is_scanf_input {
            let cursor = if (frame.count() / 8).is_multiple_of(2) {
                "█"
            } else {
                " "
            };
            Line::from(vec![
                Span::styled(
                    "> ",
                    Style::default()
                        .fg(DEFAULT_THEME.secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    input_buffer,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(cursor, Style::default().fg(DEFAULT_THEME.secondary)),
            ])
        } else {
            Line::from(vec![
                Span::styled("> ", Style::default().fg(DEFAULT_THEME.comment)),
                Span::styled(input_buffer, Style::default().fg(DEFAULT_THEME.comment)),
            ])
        };

        let bg = if is_scanf_input {
            DEFAULT_THEME.current_line_bg
        } else {
            Color::Reset
        };
        let prompt_para = Paragraph::new(prompt_line).style(Style::default().bg(bg));
        frame.render_widget(prompt_para, prompt_area);
    }
}
