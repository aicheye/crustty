// CRusTTY: Time-Travel C Interpreter with Memory Visualization

use crustty::interpreter;
use crustty::parser;
use crustty::ui;

use std::fs;
use std::io;
use std::path::Path;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use interpreter::engine::Interpreter;
use parser::ast::Program;
use parser::parse::Parser;
use ui::app::ErrorState;
use ui::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();

    // Get the first argument
    let arg = if args.len() < 2 {
        // If no arguments, check if we should run an example or show help
        let program_name = args.first().map(|s| s.as_str()).unwrap_or("crustty");
        eprintln!("Error: No input file provided");
        eprintln!();
        eprintln!("Usage: {} <file.c> | <example>", program_name);
        eprintln!();
        eprintln!("Examples:");
        eprintln!(
            "  {} default                 # Run the comprehensive example",
            program_name
        );
        eprintln!(
            "  {} myprogram.c             # Run your own C program",
            program_name
        );
        eprintln!();
        std::process::exit(1);
    } else {
        &args[1]
    };

    // Determine source code and filename for display
    let (source, filename) = match arg.as_str() {
        "default" => (
            include_str!("../examples/default.c").to_string(),
            "examples/default.c",
        ),
        _ => {
            let path = Path::new(arg);
            if !path.exists() {
                eprintln!("Error: File '{}' not found", arg);
                std::process::exit(1);
            }
            (fs::read_to_string(path)?, arg.as_str())
        }
    };

    // Parse the source code
    eprintln!("Parsing {}...", filename);
    let (program, parse_error) = match Parser::new(&source) {
        Ok(mut parser) => match parser.parse_program() {
            Ok(prog) => {
                eprintln!(
                    "Parsed successfully. Found {} top-level declarations.",
                    prog.nodes.len()
                );
                (prog, None)
            }
            Err(e) => {
                eprintln!("Parser error: {}", e);
                eprintln!("Entering TUI to show error...");
                // Create empty program and store error
                let error = ErrorState::ParseError {
                    message: e.message.clone(),
                    location: e.location,
                };
                (Program { nodes: Vec::new() }, Some(error))
            }
        },
        Err(e) => {
            eprintln!("Parser initialization error: {}", e);
            eprintln!("Entering TUI to show error...");
            // Create empty program and store error
            let error = ErrorState::ParseError {
                message: e.message.clone(),
                location: e.location,
            };
            (Program { nodes: Vec::new() }, Some(error))
        }
    };

    // Create interpreter with snapshot memory limit (1 GB)
    let snapshot_limit = 1024 * 1024 * 1024;
    let mut interpreter = Interpreter::new(program, snapshot_limit);

    // Run execution to build history
    // Note: We intentionally don't pass runtime errors to the App initially.
    // The error will be shown when the user steps forward to the line where it occurred.
    if parse_error.is_none() {
        eprintln!("Executing program...");
        match interpreter.run() {
            Ok(()) => {
                eprintln!("Execution completed successfully.");
                eprintln!("Total snapshots: {}", interpreter.total_snapshots());
            }
            Err(e) => {
                eprintln!("Runtime error: {:?}", e);
                eprintln!("Error will be shown when stepping to the error line in TUI...");
            }
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
    // Only show parse errors initially; runtime errors will appear when stepping to them
    let mut app = if let Some(error) = parse_error {
        App::new_with_error(interpreter, source, error)
    } else {
        App::new(interpreter, source)
    };
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
