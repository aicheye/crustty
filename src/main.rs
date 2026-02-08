// CRusTTY: Time-Travel C Interpreter with Memory Visualization

mod interpreter;
mod memory;
mod parser;
mod snapshot;
mod ui;

use std::fs;
use std::io;
use std::path::Path;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use interpreter::engine::Interpreter;
use parser::parser::Parser;
use ui::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        let program_name = args.get(0).map(|s| s.as_str()).unwrap_or("crustty");
        eprintln!("Error: No input file provided");
        eprintln!();
        eprintln!("Usage: {} <file.c>", program_name);
        eprintln!();
        eprintln!("Examples:");
        eprintln!(
            "  {} examples/default.c      # Run the comprehensive example",
            program_name
        );
        eprintln!(
            "  {} myprogram.c             # Run your own C program",
            program_name
        );
        eprintln!();
        eprintln!("Try the default example to see all supported features:");
        eprintln!("  {} examples/default.c", program_name);
        std::process::exit(1);
    }

    let test_file = &args[1];

    if !Path::new(test_file).exists() {
        eprintln!("Error: File '{}' not found", test_file);
        eprintln!(
            "Usage: {} [file.c]",
            args.get(0).map(|s| s.as_str()).unwrap_or("crustty")
        );
        std::process::exit(1);
    }

    // Read source code
    let source = fs::read_to_string(test_file)?;

    // Parse the source code
    eprintln!("Parsing {}...", test_file);
    let mut parser = match Parser::new(&source) {
        Ok(parser) => parser,
        Err(e) => {
            eprintln!("Parser error: {}", e);
            std::process::exit(1);
        }
    };

    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(e) => {
            eprintln!("Parser error: {}", e);
            std::process::exit(1);
        }
    };

    eprintln!(
        "Parsed successfully. Found {} top-level declarations.",
        program.nodes.len()
    );

    // Create interpreter with snapshot memory limit (1 GB)
    let snapshot_limit = 1024 * 1024 * 1024;
    let mut interpreter = Interpreter::new(program, snapshot_limit);

    // Run execution to build history
    eprintln!("Executing program...");
    match interpreter.run() {
        Ok(()) => {
            eprintln!("Execution completed successfully.");
            eprintln!("Total snapshots: {}", interpreter.total_snapshots());
        }
        Err(e) => {
            eprintln!("Runtime error: {:?}", e);
            eprintln!("Entering TUI with partial execution history...");
        }
    }

    // Rewind to the beginning for TUI
    if let Err(e) = interpreter.rewind_to_start() {
        eprintln!("Warning: Failed to rewind to start: {:?}", e);
    }

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create and run app
    let mut app = App::new(interpreter, source);
    let res = app.run(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}
