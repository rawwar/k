---
title: Reading User Input
description: Read lines from stdin interactively, handle edge cases like EOF and empty input, and add line-editing with rustyline.
---

# Reading User Input

> **What you'll learn:**
> - How to read lines from stdin using `std::io::BufRead` and handle the `Result` it returns
> - How to detect EOF, empty lines, and special control sequences in interactive input
> - How to add line-editing and history support with the `rustyline` crate

A coding agent spends most of its time waiting for you to type something. Reading user input sounds simple, but doing it well involves handling edge cases you might not expect: What happens when the user presses Ctrl+D (EOF)? What about empty lines? What about input with leading or trailing whitespace? This subchapter covers all of that, starting with Rust's standard library and then upgrading to a line-editing library.

## Reading a Single Line with stdin

The simplest way to read user input in Rust:

```rust
use std::io;

fn main() {
    println!("What is your name?");

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");

    let name = input.trim();
    println!("Hello, {name}!");
}
```

Let's break this down:

- **`let mut input = String::new()`** — create an empty, mutable `String` to hold the input.
- **`io::stdin().read_line(&mut input)`** — read one line from stdin into the string. The `&mut` passes a mutable reference — `read_line` appends to the string rather than replacing it.
- **`.expect("Failed to read line")`** — `read_line` returns `Result<usize, io::Error>`. The `usize` is the number of bytes read. We use `.expect()` to crash with a message if reading fails.
- **`input.trim()`** — remove the trailing newline (`\n`) that `read_line` includes.

::: python Coming from Python
In Python, `input("What is your name? ")` does everything: prints the prompt, reads a line, strips the newline, and returns the result. In Rust, you handle each step explicitly: print the prompt with `print!()`, read with `read_line()`, and strip with `.trim()`. More verbose, but you have full control over error handling and buffer management.
:::

## Reading in a Loop

A coding agent reads input repeatedly. Here is a basic input loop:

```rust
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        print!("> ");
        io::stdout().flush().expect("Failed to flush stdout");

        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .expect("Failed to read line");

        // EOF: user pressed Ctrl+D (Unix) or Ctrl+Z (Windows)
        if bytes_read == 0 {
            println!("\nGoodbye!");
            break;
        }

        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        println!("You said: {trimmed}");
    }
}
```

Several important details here:

### Flushing stdout

`print!()` (without the `ln`) does not automatically flush the output buffer. If you do not call `io::stdout().flush()`, the prompt might not appear before `read_line` blocks waiting for input. This is a common gotcha that does not exist in Python because `input()` handles flushing automatically.

### Detecting EOF

When `read_line` returns `Ok(0)`, it means the input stream has ended. On a terminal, this happens when the user presses Ctrl+D (Unix/macOS) or Ctrl+Z then Enter (Windows). Your program should exit gracefully when this happens.

### Locking stdin

`stdin.lock()` locks stdin for the duration of the loop, which is more efficient than locking and unlocking on every `read_line` call. For an interactive REPL that reads thousands of lines, this matters.

## Processing Commands

Let's add basic command processing to prepare for the REPL:

```rust
use std::io::{self, BufRead, Write};

fn process_input(input: &str) -> Option<String> {
    match input {
        "/help" => Some(String::from(
            "Commands:\n  /help    Show this message\n  /quit    Exit the agent\n  /clear   Clear the screen"
        )),
        "/quit" => None,
        "/clear" => {
            // ANSI escape code to clear the terminal
            print!("\x1B[2J\x1B[1;1H");
            Some(String::from("Screen cleared."))
        }
        other => Some(format!("Echo: {other}")),
    }
}

fn main() {
    println!("Kodai Agent (type /help for commands)");

    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        print!("kodai> ");
        io::stdout().flush().expect("Failed to flush stdout");

        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .expect("Failed to read line");

        if bytes_read == 0 {
            println!("\nGoodbye!");
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match process_input(trimmed) {
            Some(response) => println!("{response}"),
            None => {
                println!("Goodbye!");
                break;
            }
        }
    }
}
```

The `process_input` function returns `Option<String>` — `Some(response)` for normal commands and `None` to signal that the user wants to quit. This pattern keeps the input-processing logic separate from the loop logic.

## Upgrading to rustyline

The standard library input loop works, but it lacks features you expect from a modern CLI:

- No arrow-key navigation within a line
- No command history (up/down arrows)
- No tab completion
- No Ctrl+A/Ctrl+E to jump to start/end of line

The `rustyline` crate provides all of these. Add it:

```bash
cargo add rustyline
```

Now rewrite the input loop:

```rust
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

fn process_input(input: &str) -> Option<String> {
    match input {
        "/help" => Some(String::from(
            "Commands:\n  /help    Show this message\n  /quit    Exit the agent\n  /clear   Clear the screen"
        )),
        "/quit" => None,
        "/clear" => {
            print!("\x1B[2J\x1B[1;1H");
            Some(String::from("Screen cleared."))
        }
        other => Some(format!("Echo: {other}")),
    }
}

fn main() {
    println!("Kodai Agent (type /help for commands)");

    let mut editor = DefaultEditor::new().expect("Failed to create editor");

    loop {
        match editor.readline("kodai> ") {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // Add to history so up-arrow recalls previous commands
                let _ = editor.add_history_entry(trimmed);

                match process_input(trimmed) {
                    Some(response) => println!("{response}"),
                    None => {
                        println!("Goodbye!");
                        break;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C: print a message but don't exit
                println!("Use /quit or Ctrl+D to exit");
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D: exit gracefully
                println!("\nGoodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {err}");
                break;
            }
        }
    }
}
```

This gives you a dramatically better experience:

- **Arrow keys** — navigate within the current line
- **Up/Down arrows** — cycle through command history
- **Ctrl+C** — does not crash the program; you handle it gracefully
- **Ctrl+D** — signals EOF, triggering a clean exit
- **Home/End** — jump to the beginning or end of the line

::: python Coming from Python
Python's `input()` is similarly bare-bones. The Python equivalent of rustyline is the `readline` module (built into CPython on Unix) or third-party libraries like `prompt_toolkit`. The `rustyline` crate is inspired by GNU readline and provides the same experience: history, line editing, and signal handling.
:::

## Persisting History

You can save command history to a file so it persists across sessions:

```rust
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

fn main() {
    let mut editor = DefaultEditor::new().expect("Failed to create editor");

    // Load history from a file (ignore errors if file doesn't exist yet)
    let history_path = dirs::home_dir()
        .map(|h| h.join(".kodai_history"))
        .unwrap_or_else(|| ".kodai_history".into());

    let _ = editor.load_history(&history_path);

    println!("Kodai Agent (type /help for commands)");

    loop {
        match editor.readline("kodai> ") {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let _ = editor.add_history_entry(trimmed);
                println!("Echo: {trimmed}");
            }
            Err(ReadlineError::Interrupted) => {
                println!("Use /quit or Ctrl+D to exit");
            }
            Err(ReadlineError::Eof) => {
                println!("\nGoodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {err}");
                break;
            }
        }
    }

    // Save history when exiting
    let _ = editor.save_history(&history_path);
}
```

Note: this example uses the `dirs` crate to find the home directory. Add it with `cargo add dirs` if you want to run this code. Alternatively, you can hardcode a path like `.kodai_history` in the current directory.

## Key Takeaways

- `io::stdin().read_line(&mut buffer)` reads a line including the trailing newline. Always `.trim()` the result before processing.
- Always call `io::stdout().flush()` after `print!()` (without newline) to ensure the prompt appears before blocking on input.
- Detect EOF (`read_line` returns `Ok(0)` or rustyline returns `ReadlineError::Eof`) to handle Ctrl+D gracefully instead of crashing.
- The `rustyline` crate provides arrow-key navigation, command history, and signal handling — essential features for any interactive CLI tool.
- Separate input-reading logic from input-processing logic. Return `Option<String>` from your processor: `Some` for a response, `None` to signal exit.
