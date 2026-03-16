---
title: Building a Simple REPL
description: Combine input reading, command parsing, and output formatting into a working Read-Eval-Print Loop — the skeleton of your coding agent.
---

# Building a Simple REPL

> **What you'll learn:**
> - How to structure a REPL loop that reads input, dispatches commands, and prints formatted results
> - How to implement built-in commands like `/help`, `/quit`, and `/clear` inside the loop
> - How to handle multi-line input and graceful shutdown so the REPL feels polished and reliable

This is where everything comes together. You have learned variables, functions, modules, error handling, CLI parsing, and user input. Now you combine them into a working REPL (Read-Eval-Print Loop) — the interactive shell that becomes the foundation of your coding agent. By the end of this subchapter, you have a program you can compile, run, and actually use.

## What Is a REPL?

A REPL is a loop with four steps:

```
Read  → get input from the user
Eval  → process the input (parse commands, execute logic)
Print → display the result
Loop  → go back to Read
```

Python's interactive interpreter (`python3` with no arguments) is a REPL. Node's `node` command is a REPL. Your coding agent will be a REPL that eventually sends user messages to an LLM and displays the responses.

::: wild In the Wild
Every production coding agent has a REPL at its core. Claude Code presents a `>` prompt and reads user messages in a loop. OpenCode uses a terminal UI built on Bubble Tea, but underneath it is the same pattern: read input, send to the LLM, display the response, repeat. The REPL you build here is the first version of that loop.
:::

## The Complete REPL

Here is the full implementation. We will walk through each piece after you see the whole thing:

Create or update `src/main.rs`:

```rust
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
    match cmd {
        "help" | "h" => {
            let help_text = r#"Available commands:
  /help, /h       Show this help message
  /quit, /q       Exit the agent
  /clear          Clear the terminal screen
  /model          Show the current model
  /verbose        Toggle verbose mode

Or just type a message to chat with the agent."#;
            CommandResult::Continue(help_text.to_string())
        }
        "quit" | "q" => CommandResult::Exit,
        "clear" => {
            print!("\x1B[2J\x1B[1;1H");
            std::io::stdout().flush().ok();
            CommandResult::Silent
        }
        "model" => {
            CommandResult::Continue(String::from("Model display requires passing model to handler — coming in Chapter 2!"))
        }
        "verbose" => {
            if verbose {
                CommandResult::Continue(String::from("Verbose mode is ON"))
            } else {
                CommandResult::Continue(String::from("Verbose mode is OFF"))
            }
        }
        unknown => {
            CommandResult::Continue(format!("Unknown command: /{unknown}. Type /help for available commands."))
        }
    }
}

fn handle_user_message(message: &str, verbose: bool) -> CommandResult {
    if verbose {
        println!("[DEBUG] Processing message: {message}");
    }

    // For now, echo the message back. In Chapter 2, this sends to the LLM.
    let response = format!("You said: {message}\n\n(LLM integration coming in Chapter 2!)");
    CommandResult::Continue(response)
}

enum CommandResult {
    Continue(String),  // Print the string and continue the loop
    Silent,            // Continue without printing anything
    Exit,              // Break out of the loop
}

fn main() {
    let cli = Cli::parse();

    // Handle single-shot mode: run one prompt and exit
    if let Some(prompt) = &cli.prompt {
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

    // Load history
    let history_path = home_dir_history();
    if let Some(ref path) = history_path {
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

                // Add to history
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

    // Save history
    if let Some(ref path) = history_path {
        let _ = editor.save_history(path);
    }
}

fn home_dir_history() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|home| std::path::PathBuf::from(home).join(".kodai_history"))
}
```

Make sure your `Cargo.toml` dependencies include:

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
rustyline = "15"
```

Build and run:

```bash
cargo run
```

## Walking Through the Design

### The CommandResult Enum

```rust
enum CommandResult {
    Continue(String),
    Silent,
    Exit,
}
```

This custom enum captures the three possible outcomes of processing a command:

- **`Continue(String)`** — print the response and keep looping
- **`Silent`** — keep looping without printing (used by `/clear`)
- **`Exit`** — break out of the loop

This is a common Rust pattern: use an enum to make all possible states explicit. The compiler ensures you handle every variant in the `match` expression.

::: python Coming from Python
In Python, you might return a string for normal responses, `None` for silent commands, and raise `SystemExit` or return a sentinel value for exit. Rust's enum approach is cleaner — all three outcomes are part of the same type, and the compiler verifies you handle all of them. No sentinel values, no exceptions, no ambiguity.
:::

### Separating Concerns

The REPL is structured into distinct functions:

- **`main()`** — handles CLI parsing, the REPL loop, and history management
- **`handle_command()`** — routes input to the appropriate handler
- **`handle_builtin()`** — processes `/`-prefixed commands
- **`handle_user_message()`** — processes user messages (future: sends to LLM)
- **`print_banner()`** — displays the welcome message

Each function does one thing. When you add LLM integration in Chapter 2, you only modify `handle_user_message()` — the rest of the REPL stays unchanged.

### Single-Shot Mode

The CLI supports an optional positional argument:

```bash
# Interactive mode (no argument)
cargo run

# Single-shot mode (process one prompt and exit)
cargo run -- "What does this code do?"
```

This is useful for scripting — you can pipe output from the agent into other tools without entering interactive mode.

### History Management

Command history is saved to `~/.kodai_history`. When you restart the agent, your previous commands are still available via the up-arrow key. The `home_dir_history()` function constructs the path using the `HOME` environment variable, returning `None` if it is not set.

### Graceful Shutdown

The REPL handles three exit paths:

1. **`/quit` command** — the user types it explicitly
2. **Ctrl+D (EOF)** — rustyline returns `ReadlineError::Eof`
3. **Ctrl+C** — does *not* exit; instead prints a hint. This prevents accidental exits in the middle of a session.

## Testing the REPL

Run the agent and try these interactions:

```
$ cargo run

  Kodai v0.1.0
  Your CLI coding agent

  Type /help for available commands
  Type /quit or press Ctrl+D to exit

kodai> /help
Available commands:
  /help, /h       Show this help message
  /quit, /q       Exit the agent
  /clear          Clear the terminal screen
  /model          Show the current model
  /verbose        Toggle verbose mode

Or just type a message to chat with the agent.

kodai> Hello, agent!
You said: Hello, agent!

(LLM integration coming in Chapter 2!)

kodai> /unknown
Unknown command: /unknown. Type /help for available commands.

kodai> /quit
Goodbye!
```

Try pressing up-arrow to recall previous commands. Try Ctrl+C (should not exit) and Ctrl+D (should exit with "Goodbye!").

## What Comes Next

This REPL is the skeleton you build on for the rest of the book:

- **Chapter 2** replaces the echo response with a real LLM API call
- **Chapter 3** adds the agentic loop that calls tools and feeds results back to the LLM
- **Chapter 6** adds shell command execution as a tool
- **Chapter 7** adds file reading and writing tools
- **Chapter 8** adds a rich terminal UI

Every one of those features plugs into this REPL structure. The `handle_user_message()` function is the seam where LLM integration happens. The `handle_builtin()` function is where you add new slash commands. The architecture is deliberately simple now so it is easy to extend later.

## Key Takeaways

- A REPL is a loop: Read input, Evaluate/process it, Print the result, Loop back. It is the foundation of every interactive coding agent.
- Use a custom enum (`CommandResult`) to represent all possible outcomes of command processing. The compiler ensures you handle every case.
- Separate concerns: the loop handles I/O, dedicated functions handle command logic. This makes the code easy to extend and test.
- Support both interactive mode (REPL) and single-shot mode (process one prompt and exit) to make your agent scriptable.
- Handle Ctrl+C and Ctrl+D gracefully — users expect Ctrl+C to cancel the current input, not crash the program.
