//! Status bar rendering with keybindings and state indicators

use crate::ui::theme::DEFAULT_THEME;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Render the status bar at the bottom.
///
/// `total_steps` is `None` when execution is paused at a scanf (total unknown).
#[allow(clippy::too_many_arguments)]
pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    current_step: usize,
    total_steps: Option<usize>,
    error_state: Option<&crate::ui::app::ErrorState>,
    is_playing: bool,
    is_scanf_input: bool,
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
    let step_text = if error_state.is_some() {
        format!(" Step {}/? ", current_step + 1)
    } else if let Some(total) = total_steps {
        format!(" Step {}/{} ", current_step + 1, total)
    } else {
        // paused at scanf — total is unknown
        format!(" Step {}/? ", current_step + 1)
    };

    let left_spans = vec![
        Span::styled(
            step_text,
            Style::default()
                .bg(if error_state.is_some() {
                    DEFAULT_THEME.error
                } else if is_scanf_input {
                    DEFAULT_THEME.secondary
                } else {
                    DEFAULT_THEME.primary
                })
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
                .fg(if error_state.is_some() {
                    DEFAULT_THEME.error
                } else {
                    DEFAULT_THEME.fg
                }),
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
        Span::styled(" s ", key_style),
        Span::styled(" step over ", desc_style),
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
    let is_at_end = total_steps.is_some_and(|total| current_step + 1 >= total);

    if is_scanf_input {
        right_spans.push(Span::styled("│", sep_style));
        right_spans.push(Span::styled(
            " ⌨ INPUT ",
            Style::default()
                .bg(DEFAULT_THEME.secondary)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    } else if is_playing {
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
