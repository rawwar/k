---
title: CLI Argument Parsing
description: Parse command-line arguments and flags using the clap crate to configure your coding agent at launch.
---

# CLI Argument Parsing

> **What you'll learn:**
> - How to define CLI arguments and flags declaratively using clap's derive macros
> - How to handle required arguments, optional flags, and subcommands in a type-safe way
> - How to add `--help` and `--version` output automatically through clap's built-in support

Every serious CLI tool needs to accept arguments: configuration flags, file paths, mode selectors. You could parse `std::env::args()` by hand, but that gets tedious fast — you would need to handle missing arguments, invalid values, help text, and error messages manually. Instead, Rust has an excellent crate for this: **clap**.

`clap` lets you define your CLI interface as a Rust struct. It automatically generates argument parsing, validation, help output, and error messages. It is the most popular CLI argument parser in the Rust ecosystem and is used by tools like `ripgrep`, `bat`, and `fd`.

## Adding clap to Your Project

First, add `clap` with the `derive` feature:

```bash
cargo add clap --features derive
```

Your `Cargo.toml` now includes:

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
```

The `derive` feature enables clap's derive macros, which let you define your CLI as a regular Rust struct.

## Your First CLI Parser

Replace your `src/main.rs` with:

```rust
use clap::Parser;

/// Kodai — a CLI coding agent
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        println!("Verbose mode enabled");
    }

    println!("Kodai is ready.");
}
```

Run it:

```bash
cargo run
```

Output:

```
Kodai is ready.
```

Now try with the flag:

```bash
cargo run -- --verbose
```

Output:

```
Verbose mode enabled
Kodai is ready.
```

The `--` after `cargo run` separates Cargo's arguments from your program's arguments. Everything after `--` is passed to your binary.

### Free --help and --version

clap generates these automatically:

```bash
cargo run -- --help
```

Output:

```
Kodai — a CLI coding agent

Usage: kodai [OPTIONS]

Options:
  -v, --verbose  Enable verbose output
  -h, --help     Print help
  -V, --version  Print version
```

```bash
cargo run -- --version
```

Output:

```
kodai 0.1.0
```

The version comes from `Cargo.toml` automatically. The description comes from the doc comment (`///`) on the struct.

::: python Coming from Python
Python's equivalent is `argparse`:
```python
import argparse
parser = argparse.ArgumentParser(description="Kodai — a CLI coding agent")
parser.add_argument("-v", "--verbose", action="store_true", help="Enable verbose output")
args = parser.parse_args()
```
clap's derive approach is more concise and type-safe. The struct fields *are* the arguments — their types determine how clap parses and validates input. If someone passes `--verbose hello` where a bool is expected, clap rejects it before your code runs.
:::

## Adding More Arguments

Let's make the CLI more useful for a coding agent:

```rust
use clap::Parser;

/// Kodai — a CLI coding agent
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Model to use for the LLM
    #[arg(short, long, default_value = "claude-sonnet")]
    model: String,

    /// Maximum number of tokens in the response
    #[arg(long, default_value_t = 4096)]
    max_tokens: u32,

    /// Optional initial prompt to send to the agent
    prompt: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    println!("Model: {}", cli.model);
    println!("Max tokens: {}", cli.max_tokens);
    println!("Verbose: {}", cli.verbose);

    if let Some(prompt) = &cli.prompt {
        println!("Initial prompt: {prompt}");
    } else {
        println!("No initial prompt — entering interactive mode");
    }
}
```

Let's test different invocations:

```bash
# Default values
cargo run

# Override the model
cargo run -- --model gpt-4

# With an initial prompt (positional argument)
cargo run -- "Fix the bug in main.rs"

# Combine flags and positional arguments
cargo run -- --verbose --model claude-opus --max-tokens 8192 "Refactor this function"
```

Here is how the different argument types work:

| Field Type | clap Behavior | Example |
|-----------|---------------|---------|
| `bool` with `#[arg(long)]` | Flag (present = true) | `--verbose` |
| `String` with `default_value` | Optional with default | `--model claude-sonnet` |
| `u32` with `default_value_t` | Numeric with default | `--max-tokens 4096` |
| `Option<String>` | Truly optional, `None` if absent | `"some prompt"` |

Note the difference between `default_value` (for strings, quoted) and `default_value_t` (for types that implement `Display`, unquoted).

## Subcommands

For more complex CLIs, you might want subcommands like `kodai chat`, `kodai run`, or `kodai config`. clap handles this with enums:

```rust
use clap::{Parser, Subcommand};

/// Kodai — a CLI coding agent
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start an interactive chat session
    Chat {
        /// Model to use
        #[arg(short, long, default_value = "claude-sonnet")]
        model: String,
    },
    /// Run a single prompt and exit
    Run {
        /// The prompt to execute
        prompt: String,

        /// Model to use
        #[arg(short, long, default_value = "claude-sonnet")]
        model: String,
    },
    /// Show current configuration
    Config,
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        println!("Verbose mode enabled");
    }

    match cli.command {
        Some(Commands::Chat { model }) => {
            println!("Starting chat with model: {model}");
        }
        Some(Commands::Run { prompt, model }) => {
            println!("Running prompt with {model}: {prompt}");
        }
        Some(Commands::Config) => {
            println!("Showing configuration...");
        }
        None => {
            println!("No subcommand given. Use --help for usage.");
        }
    }
}
```

Test it:

```bash
cargo run -- chat --model claude-opus
cargo run -- run "Fix the typo in README.md"
cargo run -- config
cargo run -- --help
```

Each subcommand gets its own `--help` as well:

```bash
cargo run -- chat --help
```

## Validation and Custom Errors

clap validates input types automatically. If someone passes a string where a number is expected, they get a clear error:

```bash
cargo run -- --max-tokens not_a_number
```

```
error: invalid value 'not_a_number' for '--max-tokens <MAX_TOKENS>':
  invalid digit found in string
```

For custom validation, you can use `value_parser`:

```rust
use clap::Parser;

/// Kodai — a CLI coding agent
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Maximum tokens (must be between 1 and 100000)
    #[arg(long, default_value_t = 4096, value_parser = clap::value_parser!(u32).range(1..=100000))]
    max_tokens: u32,
}

fn main() {
    let cli = Cli::parse();
    println!("Max tokens: {}", cli.max_tokens);
}
```

Now if someone passes `--max-tokens 0` or `--max-tokens 999999`, clap rejects it with a helpful error message.

## The CLI for Our Agent

Here is the CLI definition we carry forward for the coding agent:

```rust
use clap::Parser;

/// Kodai — a CLI coding agent built in Rust
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Enable verbose/debug output
    #[arg(short, long)]
    pub verbose: bool,

    /// Model to use for LLM calls
    #[arg(short, long, default_value = "claude-sonnet")]
    pub model: String,

    /// Optional initial prompt (skip interactive mode)
    pub prompt: Option<String>,
}
```

Put this in `src/cli.rs`, add `pub mod cli;` to `src/lib.rs`, and update `src/main.rs`:

```rust
use clap::Parser;
use kodai::cli::Cli;

fn main() {
    let cli = Cli::parse();

    let mode = if cli.prompt.is_some() { "single-shot" } else { "interactive" };
    println!("Kodai v{}", env!("CARGO_PKG_VERSION"));
    println!("Model: {}", cli.model);
    println!("Mode: {mode}");

    if cli.verbose {
        println!("Debug output enabled");
    }
}
```

Run `cargo run -- --help` and verify that your agent has a clean, professional CLI interface.

## Key Takeaways

- Use the `clap` crate with derive macros to define your CLI as a Rust struct — fields become arguments, types determine validation.
- clap generates `--help` and `--version` automatically from doc comments and `Cargo.toml` metadata.
- Use `bool` fields for flags, `String` with `default_value` for optional strings, `Option<T>` for truly optional arguments, and enums for subcommands.
- The `--` separator after `cargo run` passes arguments to your binary instead of to Cargo.
- clap validates input types at parse time — invalid values produce clear error messages before your code runs.
