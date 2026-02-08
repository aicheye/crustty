//! Main TUI application state and logic

use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout},
};
use std::io;
use std::time::{Duration, Instant};

/// Which pane is currently focused
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPane {
    Source,
    Stack,
    Heap,
    Terminal,
}

impl FocusedPane {
    /// Move focus to the next pane (clockwise: source -> terminal -> stack -> heap)
    pub fn next(self) -> Self {
        match self {
            FocusedPane::Source => FocusedPane::Terminal,
            FocusedPane::Terminal => FocusedPane::Stack,
            FocusedPane::Stack => FocusedPane::Heap,
            FocusedPane::Heap => FocusedPane::Source,
        }
    }

    /// Move focus to the previous pane (counter-clockwise)
    pub fn prev(self) -> Self {
        match self {
            FocusedPane::Source => FocusedPane::Heap,
            FocusedPane::Terminal => FocusedPane::Source,
            FocusedPane::Stack => FocusedPane::Terminal,
            FocusedPane::Heap => FocusedPane::Stack,
        }
    }
}

/// The main application state
pub struct App {
    /// The interpreter instance
    pub interpreter: Interpreter,

    /// The source code being executed
    pub source_code: String,

    /// Currently focused pane
    pub focused_pane: FocusedPane,

    /// Per-pane scroll offsets
    pub source_scroll: usize,
    pub stack_scroll: usize,
    pub heap_scroll: usize,
    pub terminal_scroll: usize,

    /// Target visual row for the current line (None = not initialized yet)
    /// This keeps the highlighted line at a fixed position when stepping
    pub target_line_row: Option<usize>,

    /// Previous item count for stack pane (for smart auto-scroll)
    pub prev_stack_items: usize,

    /// Previous item count for heap pane (for smart auto-scroll)
    pub prev_heap_items: usize,

    /// Whether the app should quit
    pub should_quit: bool,

    /// Status message to display
    pub status_message: String,

    /// Whether auto-play mode is active
    pub is_playing: bool,

    /// Last time a step was taken in play mode
    pub last_play_time: Instant,

    /// Last time space was pressed (for debouncing)
    pub last_space_press: Instant,
}

impl App {
    /// Create a new app with the given interpreter and source code
    pub fn new(interpreter: Interpreter, source_code: String) -> Self {
        App {
            interpreter,
            source_code,
            focused_pane: FocusedPane::Source,
            source_scroll: 0,
            stack_scroll: 0,
            heap_scroll: 0,
            terminal_scroll: 0,
            target_line_row: None, // Will be set to center on first render
            prev_stack_items: 0,
            prev_heap_items: 0,
            should_quit: false,
            status_message: String::from("Ready!"),
            is_playing: false,
            last_play_time: Instant::now(),
            last_space_press: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or(Instant::now()),
        }
    }

    /// Run the TUI application
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if self.should_quit {
                break;
            }

            // Handle auto-play mode
            if self.is_playing {
                if self.last_play_time.elapsed() >= Duration::from_secs(1) {
                    // Try to step forward
                    if self.interpreter.step_forward().is_ok() {
                        self.status_message = "Playing...".to_string();
                        self.terminal_scroll = usize::MAX;
                    } else {
                        // No more steps available
                        self.is_playing = false;
                        self.status_message = "Playback complete".to_string();
                    }
                    self.last_play_time = Instant::now();
                }
            }

            // Use poll with timeout to allow auto-play to work
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key_event(key);
                    }
                }
            }
        }

        Ok(())
    }

    /// Render the UI
    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Create layout: 4 panes in 2 columns, plus status bar at bottom
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(size);

        let pane_area = main_chunks[0];
        let status_area = main_chunks[1];

        // Split into 2 columns
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(pane_area);

        // Left column: Source (top) | Terminal (bottom)
        let left_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(columns[0]);

        // Right column: Stack (top) | Heap (bottom)
        let right_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(columns[1]);

        // Render each pane
        super::panes::render_source_pane(
            frame,
            left_rows[0],
            &self.source_code,
            self.interpreter.current_location().line,
            self.focused_pane == FocusedPane::Source,
            &mut self.source_scroll,
            &mut self.target_line_row,
        );

        super::panes::render_terminal_pane(
            frame,
            left_rows[1],
            self.interpreter.terminal(),
            self.focused_pane == FocusedPane::Terminal,
            &mut self.terminal_scroll,
        );

        super::panes::render_stack_pane(
            frame,
            right_rows[0],
            self.interpreter.stack(),
            self.interpreter.struct_defs(),
            &self.source_code,
            self.focused_pane == FocusedPane::Stack,
            &mut self.stack_scroll,
            &mut self.prev_stack_items,
            self.interpreter.return_value(),
            self.interpreter.function_defs(),
        );

        super::panes::render_heap_pane(
            frame,
            right_rows[1],
            self.interpreter.heap(),
            self.interpreter.pointer_types(),
            self.interpreter.struct_defs(),
            self.focused_pane == FocusedPane::Heap,
            &mut self.heap_scroll,
            &mut self.prev_heap_items,
        );

        // Render status bar
        super::panes::render_status_bar(
            frame,
            status_area,
            &self.status_message,
            self.interpreter.history_position(),
            self.interpreter.total_snapshots(),
            self.is_playing,
        );
    }

    /// Handle keyboard events
    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            // Number keys step forward N times directly
            KeyCode::Char(c @ '1'..='9') => {
                self.is_playing = false;
                let n = c.to_digit(10).unwrap() as usize;
                let mut stepped = 0;
                for _ in 0..n {
                    if self.interpreter.step_forward().is_ok() {
                        stepped += 1;
                    } else {
                        break;
                    }
                }
                self.status_message = format!("Stepped forward {} step(s)", stepped);
                self.terminal_scroll = usize::MAX;
            }
            KeyCode::Tab => {
                self.focused_pane = self.focused_pane.next();
            }
            KeyCode::Left => {
                self.is_playing = false;
                self.step_backward();
            }
            KeyCode::Right => {
                self.is_playing = false;
                self.step_forward();
            }
            KeyCode::Up => {
                match self.focused_pane {
                    FocusedPane::Source => {
                        // Scrolling up makes the current line move down visually
                        if let Some(row) = self.target_line_row {
                            self.target_line_row = Some(row.saturating_add(1));
                        }
                    }
                    FocusedPane::Stack => {
                        if self.stack_scroll > 0 {
                            self.stack_scroll = self.stack_scroll.saturating_sub(1);
                        }
                    }
                    FocusedPane::Heap => {
                        if self.heap_scroll > 0 {
                            self.heap_scroll = self.heap_scroll.saturating_sub(1);
                        }
                    }
                    FocusedPane::Terminal => {
                        if self.terminal_scroll > 0 {
                            self.terminal_scroll = self.terminal_scroll.saturating_sub(1);
                        }
                    }
                }
            }
            KeyCode::Down => {
                match self.focused_pane {
                    FocusedPane::Source => {
                        // Scrolling down makes the current line move up visually
                        if let Some(row) = self.target_line_row {
                            self.target_line_row = Some(row.saturating_sub(1));
                        }
                    }
                    FocusedPane::Stack => {
                        self.stack_scroll = self.stack_scroll.saturating_add(1);
                    }
                    FocusedPane::Heap => {
                        self.heap_scroll = self.heap_scroll.saturating_add(1);
                    }
                    FocusedPane::Terminal => {
                        self.terminal_scroll = self.terminal_scroll.saturating_add(1);
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Toggle auto-play mode (with 200ms debounce to prevent key repeat spam)
                if self.last_space_press.elapsed() >= Duration::from_millis(200) {
                    self.last_space_press = Instant::now();
                    self.is_playing = !self.is_playing;
                    if self.is_playing {
                        self.last_play_time = Instant::now()
                            .checked_sub(Duration::from_secs(1))
                            .unwrap_or(Instant::now());
                        self.status_message = "Playing...".to_string();
                    } else {
                        self.status_message = "Paused".to_string();
                    }
                }
            }
            KeyCode::Enter => {
                // Jump to end of execution
                self.is_playing = false;
                let total = self.interpreter.total_snapshots();
                if total > 0 {
                    // Step forward to the last snapshot
                    while self.interpreter.history_position() + 1 < total {
                        if let Err(_) = self.interpreter.step_forward() {
                            break;
                        }
                    }
                }
                self.status_message = "Jumped to end".to_string();
                self.terminal_scroll = usize::MAX;
            }
            KeyCode::Backspace => {
                // Jump to start of execution
                self.is_playing = false;
                let _ = self.interpreter.rewind_to_start();
                self.status_message = "Jumped to start".to_string();
                self.terminal_scroll = usize::MAX;
            }
            _ => {}
        }
    }

    /// Step forward in execution
    fn step_forward(&mut self) {
        match self.interpreter.step_forward() {
            Ok(()) => {
                self.status_message = "Stepped forward".to_string();
                // Auto-scroll terminal to bottom
                self.terminal_scroll = usize::MAX;
            }
            Err(RuntimeError::Generic { message, .. }) => {
                self.status_message = format!("Cannot step forward: {}", message);
            }
            Err(e) => {
                self.status_message = format!("Error: {:?}", e);
            }
        }
    }

    /// Step backward in execution
    fn step_backward(&mut self) {
        match self.interpreter.step_backward() {
            Ok(()) => {
                self.status_message = "Stepped backward".to_string();
                // Auto-scroll terminal to bottom
                self.terminal_scroll = usize::MAX;
            }
            Err(RuntimeError::Generic { message, .. }) => {
                self.status_message = format!("Cannot step backward: {}", message);
            }
            Err(e) => {
                self.status_message = format!("Error: {:?}", e);
            }
        }
    }
}
