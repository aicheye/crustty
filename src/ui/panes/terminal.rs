//! Terminal output pane rendering

use crate::snapshot::MockTerminal;
use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};

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
