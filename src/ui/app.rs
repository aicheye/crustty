//! Main TUI application state and logic

use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::parser::ast::SourceLocation;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
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

/// Represents an error that occurred during parsing or execution
#[derive(Debug, Clone)]
pub enum ErrorState {
    /// Parsing error that occurred before execution
    ParseError {
        message: String,
        location: SourceLocation,
    },
    /// Runtime error that occurred during execution
    RuntimeError(RuntimeError),
}

impl ErrorState {
    /// Get the line number where the error occurred
    pub fn line(&self) -> usize {
        match self {
            ErrorState::ParseError { location, .. } => location.line,
            ErrorState::RuntimeError(err) => err.location().map(|loc| loc.line).unwrap_or(0),
        }
    }

    /// Get the memory address if applicable (for memory-related errors)
    pub fn memory_address(&self) -> Option<u64> {
        match self {
            ErrorState::RuntimeError(RuntimeError::UninitializedRead { address, .. }) => *address,
            ErrorState::RuntimeError(RuntimeError::UseAfterFree { address, .. })
            | ErrorState::RuntimeError(RuntimeError::DoubleFree { address, .. })
            | ErrorState::RuntimeError(RuntimeError::InvalidFree { address, .. }) => Some(*address),
            _ => None,
        }
    }

    /// Get a human-readable error message
    pub fn message(&self) -> String {
        match self {
            ErrorState::ParseError { message, .. } => format!("Parse Error: {}", message),
            ErrorState::RuntimeError(err) => format!("Runtime Error: {}", err),
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

    /// Source pane scroll state
    pub source_scroll: super::panes::SourceScrollState,
    /// Stack pane scroll state
    pub stack_scroll: super::panes::StackScrollState,
    /// Heap pane scroll state
    pub heap_scroll: super::panes::HeapScrollState,
    /// Terminal scroll offset
    pub terminal_scroll: usize,

    /// Whether the app should quit
    pub should_quit: bool,

    /// Status message to display
    pub status_message: String,

    /// Error state if an error occurred
    pub error_state: Option<ErrorState>,

    /// Whether auto-play mode is active
    pub is_playing: bool,

    /// Last time a step was taken in play mode
    pub last_play_time: Instant,

    /// Last time space was pressed (for debouncing)
    pub last_space_press: Instant,

    /// Typed text buffered while in scanf input mode
    pub scanf_input_buffer: String,
}

impl App {
    /// Create a new app with the given interpreter and source code
    pub fn new(interpreter: Interpreter, source_code: String) -> Self {
        App {
            interpreter,
            source_code,
            focused_pane: FocusedPane::Source,
            source_scroll: super::panes::SourceScrollState {
                offset: 0,
                target_line_row: None,
            },
            stack_scroll: super::panes::StackScrollState {
                offset: 0,
                prev_item_count: 0,
            },
            heap_scroll: super::panes::HeapScrollState {
                offset: 0,
                prev_item_count: 0,
            },
            terminal_scroll: 0,
            should_quit: false,
            status_message: String::from("Ready!"),
            error_state: None,
            is_playing: false,
            last_play_time: Instant::now(),
            last_space_press: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or(Instant::now()),
            scanf_input_buffer: String::new(),
        }
    }

    /// Create a new app with an error state (for parse or runtime errors)
    pub fn new_with_error(
        interpreter: Interpreter,
        source_code: String,
        error: ErrorState,
    ) -> Self {
        let mut app = Self::new(interpreter, source_code);
        app.status_message = error.message();
        app.error_state = Some(error);
        app
    }

    /// Returns true when the TUI should present a scanf input prompt.
    ///
    /// This is the case only when we are at the very last available snapshot AND
    /// the interpreter is paused waiting for user input.
    fn is_in_scanf_input_mode(&self) -> bool {
        let at_last = self.interpreter.history_position() + 1 >= self.interpreter.total_snapshots();
        at_last && self.interpreter.is_paused_at_scanf()
    }

    /// Check if we just landed on the scanf snapshot.
    fn check_and_activate_scanf_mode(&mut self) {
        if self.is_in_scanf_input_mode() {
            self.is_playing = false;
            if self.status_message != "Waiting for scanf input…" {
                self.status_message = "Waiting for scanf input…".to_string();
            }
        }
    }

    /// The total number of snapshots, or `None` when execution is paused at scanf.
    fn total_steps_display(&self) -> Option<usize> {
        if self.interpreter.is_paused_at_scanf() {
            None
        } else {
            Some(self.interpreter.total_snapshots())
        }
    }

    /// Run the TUI application
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        // Activate scanf mode immediately if we start paused at one
        self.check_and_activate_scanf_mode();

        loop {
            terminal.draw(|f| self.render(f))?;

            if self.should_quit {
                break;
            }

            // Handle auto-play mode
            if self.is_playing {
                // Stop playing if we hit an error or need scanf input
                if self.error_state.is_some() || self.is_in_scanf_input_mode() {
                    self.is_playing = false;
                    if self.is_in_scanf_input_mode() {
                        self.check_and_activate_scanf_mode();
                    } else if let Some(error) = &self.error_state {
                        self.status_message = format!("Stopped: {}", error.message());
                    }
                } else if self.last_play_time.elapsed() >= Duration::from_secs(1) {
                    match self.interpreter.step_forward() {
                        Ok(()) => {
                            self.status_message = "Playing...".to_string();
                            self.terminal_scroll = usize::MAX;
                            self.check_and_activate_scanf_mode();
                        }
                        Err(e) => {
                            if let RuntimeError::HistoryOperationFailed { message, .. } = &e {
                                if message == "Reached end of execution" {
                                    self.is_playing = false;
                                    self.status_message = message.clone();
                                    self.last_play_time = Instant::now();
                                    return Ok(());
                                }
                            }
                            self.error_state = Some(ErrorState::RuntimeError(e.clone()));
                            self.is_playing = false;
                            self.status_message = format!("Error: {}", e);
                        }
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

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(size);

        let pane_area = main_chunks[0];
        let status_area = main_chunks[1];

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(pane_area);

        let left_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(columns[0]);

        let right_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(columns[1]);

        let scanf_mode = self.is_in_scanf_input_mode();

        super::panes::render_source_pane(
            frame,
            left_rows[0],
            &self.source_code,
            self.interpreter.current_location().line,
            self.error_state.as_ref().is_some(),
            self.is_in_scanf_input_mode(),
            self.focused_pane == FocusedPane::Source,
            &mut self.source_scroll,
        );

        super::panes::render_terminal_pane(
            frame,
            left_rows[1],
            self.interpreter.terminal(),
            self.focused_pane == FocusedPane::Terminal,
            &mut self.terminal_scroll,
            scanf_mode,
            &self.scanf_input_buffer,
        );

        super::panes::render_stack_pane(
            frame,
            right_rows[0],
            super::panes::StackRenderData {
                stack: self.interpreter.stack(),
                struct_defs: self.interpreter.struct_defs(),
                source_code: &self.source_code,
                return_value: self.interpreter.return_value(),
                function_defs: self.interpreter.function_defs(),
                error_address: self.error_state.as_ref().and_then(|e| e.memory_address()),
            },
            self.focused_pane == FocusedPane::Stack,
            &mut self.stack_scroll,
        );

        super::panes::render_heap_pane(
            frame,
            right_rows[1],
            super::panes::HeapRenderData {
                heap: self.interpreter.heap(),
                pointer_types: self.interpreter.pointer_types(),
                struct_defs: self.interpreter.struct_defs(),
                error_address: self.error_state.as_ref().and_then(|e| e.memory_address()),
            },
            self.focused_pane == FocusedPane::Heap,
            &mut self.heap_scroll,
        );

        super::panes::render_status_bar(
            frame,
            status_area,
            &self.status_message,
            self.interpreter.history_position(),
            self.total_steps_display(),
            self.error_state.as_ref(),
            self.is_playing,
            scanf_mode,
        );
    }

    /// Handle keyboard events
    fn handle_key_event(&mut self, key: KeyEvent) {
        // ── scanf input mode ──────────────────────────────────────────────────
        if self.is_in_scanf_input_mode() {
            match key.code {
                KeyCode::Enter => {
                    let input = std::mem::take(&mut self.scanf_input_buffer);
                    match self.interpreter.provide_scanf_input(input) {
                        Ok(()) => {
                            self.terminal_scroll = usize::MAX;
                            // If another scanf is immediately reached, stay in input mode
                            self.check_and_activate_scanf_mode();
                            if !self.is_in_scanf_input_mode() {
                                self.status_message = "Input accepted".to_string();
                            }
                        }
                        Err(e) => {
                            self.error_state = Some(ErrorState::RuntimeError(e.clone()));
                            self.status_message = format!("Error: {}", e);
                        }
                    }
                    return;
                }
                KeyCode::Backspace => {
                    self.scanf_input_buffer.pop();
                    return;
                }
                // Allow Esc and Left to cancel input and step back
                KeyCode::Esc | KeyCode::Left => {
                    self.is_playing = false;
                    self.step_backward();
                    return;
                }
                // Printable character goes into the input buffer
                KeyCode::Char(c) => {
                    self.scanf_input_buffer.push(c);
                    return;
                }
                // Ignore all other keys in input mode
                _ => return,
            }
        }

        // ── normal mode ───────────────────────────────────────────────────────
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            // Number keys step forward N times directly
            KeyCode::Char(c @ '1'..='9') => {
                if let Some(error) = &self.error_state {
                    self.status_message = error.message();
                    return;
                }

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
                self.check_and_activate_scanf_mode();
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.is_playing = false;
                self.step_over();
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.is_playing = false;
                self.step_back_over();
            }
            KeyCode::Tab => {
                self.focused_pane = self.focused_pane.next();
            }
            KeyCode::BackTab => {
                self.focused_pane = self.focused_pane.prev();
            }
            KeyCode::Left => {
                self.is_playing = false;
                self.step_backward();
            }
            KeyCode::Right => {
                self.is_playing = false;
                self.step_forward();
            }
            KeyCode::Up => match self.focused_pane {
                FocusedPane::Source => {
                    if let Some(row) = self.source_scroll.target_line_row {
                        self.source_scroll.target_line_row = Some(row.saturating_add(1));
                    }
                }
                FocusedPane::Stack => {
                    if self.stack_scroll.offset > 0 {
                        self.stack_scroll.offset = self.stack_scroll.offset.saturating_sub(1);
                    }
                }
                FocusedPane::Heap => {
                    if self.heap_scroll.offset > 0 {
                        self.heap_scroll.offset = self.heap_scroll.offset.saturating_sub(1);
                    }
                }
                FocusedPane::Terminal => {
                    if self.terminal_scroll > 0 {
                        self.terminal_scroll = self.terminal_scroll.saturating_sub(1);
                    }
                }
            },
            KeyCode::Down => match self.focused_pane {
                FocusedPane::Source => {
                    if let Some(row) = self.source_scroll.target_line_row {
                        self.source_scroll.target_line_row = Some(row.saturating_sub(1));
                    }
                }
                FocusedPane::Stack => {
                    self.stack_scroll.offset = self.stack_scroll.offset.saturating_add(1);
                }
                FocusedPane::Heap => {
                    self.heap_scroll.offset = self.heap_scroll.offset.saturating_add(1);
                }
                FocusedPane::Terminal => {
                    self.terminal_scroll = self.terminal_scroll.saturating_add(1);
                }
            },
            KeyCode::Char(' ') => {
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
                if let Some(error) = &self.error_state {
                    self.status_message = error.message();
                    return;
                }

                self.is_playing = false;
                let total = self.interpreter.total_snapshots();
                if total > 0 {
                    while self.interpreter.history_position() + 1 < total {
                        if self.interpreter.step_forward().is_err() {
                            break;
                        }
                    }
                }
                self.status_message = "Jumped to end".to_string();
                self.terminal_scroll = usize::MAX;
                self.check_and_activate_scanf_mode();
            }
            KeyCode::Backspace => {
                if let Some(error) = &self.error_state {
                    self.status_message = error.message();
                    return;
                }

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
        if let Some(error) = &self.error_state {
            self.status_message = error.message();
            return;
        }

        match self.interpreter.step_forward() {
            Ok(()) => {
                self.status_message = "Stepped forward".to_string();
                self.terminal_scroll = usize::MAX;
                self.check_and_activate_scanf_mode();
            }
            Err(e) => {
                if let RuntimeError::HistoryOperationFailed { message, .. } = &e {
                    if message == "Reached end of execution" {
                        self.status_message = message.clone();
                        self.check_and_activate_scanf_mode();
                        return;
                    }
                }
                self.error_state = Some(ErrorState::RuntimeError(e.clone()));
                self.status_message = format!("Error: {}", e);
            }
        }
    }

    /// Step backward in execution
    fn step_backward(&mut self) {
        if let Some(error) = &self.error_state {
            self.status_message = error.message();
            return;
        }

        match self.interpreter.step_backward() {
            Ok(()) => {
                self.status_message = "Stepped backward".to_string();
                self.terminal_scroll = usize::MAX;
            }
            Err(e) => {
                self.status_message = e.to_string();
            }
        }
    }

    /// Step over: skip entire loops and function calls
    fn step_over(&mut self) {
        if let Some(error) = &self.error_state {
            self.status_message = error.message();
            return;
        }

        match self.interpreter.step_over() {
            Ok(()) => {
                self.status_message = "Stepped over".to_string();
                self.terminal_scroll = usize::MAX;
                self.check_and_activate_scanf_mode();
            }
            Err(e) => {
                if let RuntimeError::HistoryOperationFailed { message, .. } = &e {
                    if message == "Reached end of execution" {
                        self.status_message = message.clone();
                        self.check_and_activate_scanf_mode();
                        return;
                    }
                }
                self.error_state = Some(ErrorState::RuntimeError(e.clone()));
                self.status_message = format!("Error: {}", e);
            }
        }
    }

    /// Step back over: inverse of step over
    fn step_back_over(&mut self) {
        if let Some(error) = &self.error_state {
            self.status_message = error.message();
            return;
        }

        match self.interpreter.step_back_over() {
            Ok(()) => {
                self.status_message = "Stepped back over".to_string();
                self.terminal_scroll = usize::MAX;
            }
            Err(e) => {
                if let RuntimeError::HistoryOperationFailed { message, .. } = &e {
                    if message == "Reached start of execution"
                        || message == "Already at the beginning of execution"
                    {
                        self.status_message = message.clone();
                        return;
                    }
                }
                self.error_state = Some(ErrorState::RuntimeError(e.clone()));
                self.status_message = format!("Error: {}", e);
            }
        }
    }
}
