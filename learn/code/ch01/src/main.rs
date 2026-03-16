// Chapter 1: Hello Rust CLI — a working REPL with clap + rustyline

use clap::Parser;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::Write;

/// Kodai — a CLI coding agent built in Rust
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Enable verbose/debug output
    #[arg(short, long)]
    verbose: bool,

    /// Model to use for LLM calls
    #[arg(short, long, default_value = "claude-sonnet")]
    model: String,

    /// Optional initial prompt (skip interactive mode)
    prompt: Option<String>,
}

/// The three possible outcomes of processing a command.
enum CommandResult {
    Continue(String), // Print the string and continue the loop
    Silent,           // Continue without printing anything
    Exit,             // Break out of the loop
}

fn print_banner() {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!("  Kodai v{version}");
    println!("  Your CLI coding agent");
    println!();
    println!("  Type /help for available commands");
    println!("  Type /quit or press Ctrl+D to exit");
    println!();
}

fn handle_command(input: &str, verbose: bool) -> CommandResult {
    // Check for built-in commands (start with /)
    if let Some(cmd) = input.strip_prefix('/') {
        return handle_builtin(cmd, verbose);
    }

    // Everything else is a user message (future: send to LLM)
    handle_user_message(input, verbose)
}

fn handle_builtin(cmd: &str, verbose: bool) -> CommandResult {
    if verbose {
        println!("[DEBUG] Built-in command: /{cmd}");
    }

    match cmd {
        "help" | "h" => {
            let help_text = "\
Available commands:
  /help, /h       Show this help message
  /quit, /q       Exit the agent
  /clear          Clear the terminal screen
  /verbose        Show verbose mode status

Or just type a message to chat with the agent.";
            CommandResult::Continue(help_text.to_string())
        }
        "quit" | "q" => CommandResult::Exit,
        "clear" => {
            print!("\x1B[2J\x1B[1;1H");
            std::io::stdout().flush().ok();
            CommandResult::Silent
        }
        "verbose" => {
            let status = if verbose { "ON" } else { "OFF" };
            CommandResult::Continue(format!("Verbose mode is {status}"))
        }
        unknown => CommandResult::Continue(format!(
            "Unknown command: /{unknown}. Type /help for available commands."
        )),
    }
}

fn handle_user_message(message: &str, verbose: bool) -> CommandResult {
    if verbose {
        println!("[DEBUG] Processing message ({} chars): {message}", message.len());
    }

    // For now, echo the message back. In Chapter 2, this sends to the LLM.
    let response = format!("You said: {message}\n\n(LLM integration coming in Chapter 2!)");
    CommandResult::Continue(response)
}

fn history_path() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|home| std::path::PathBuf::from(home).join(".kodai_history"))
}

fn main() {
    let cli = Cli::parse();

    // Handle single-shot mode: run one prompt and exit
    if let Some(prompt) = &cli.prompt {
        if cli.verbose {
            println!("[DEBUG] Single-shot mode, model: {}", cli.model);
        }
        match handle_command(prompt, cli.verbose) {
            CommandResult::Continue(response) => println!("{response}"),
            CommandResult::Silent => {}
            CommandResult::Exit => {}
        }
        return;
    }

    // Interactive mode: start the REPL
    print_banner();

    let mut editor = DefaultEditor::new().expect("Failed to initialize line editor");

    // Load history from previous sessions
    let hist = history_path();
    if let Some(ref path) = hist {
        let _ = editor.load_history(path);
    }

    if cli.verbose {
        println!("[DEBUG] Model: {}", cli.model);
        println!("[DEBUG] Verbose: true");
        println!();
    }

    loop {
        match editor.readline("kodai> ") {
            Ok(line) => {
                let trimmed = line.trim();

                // Skip empty lines
                if trimmed.is_empty() {
                    continue;
                }

                // Add non-empty input to history
                let _ = editor.add_history_entry(trimmed);

                // Process the input
                match handle_command(trimmed, cli.verbose) {
                    CommandResult::Continue(response) => {
                        println!("{response}");
                        println!();
                    }
                    CommandResult::Silent => {}
                    CommandResult::Exit => {
                        println!("Goodbye!");
                        break;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C: don't exit, just inform the user
                println!("(Use /quit or Ctrl+D to exit)");
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D: exit gracefully
                println!("\nGoodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error reading input: {err}");
                break;
            }
        }
    }

    // Save history on exit
    if let Some(ref path) = hist {
        let _ = editor.save_history(path);
    }
}
