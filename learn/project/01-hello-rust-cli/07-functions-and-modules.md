---
title: Functions and Modules
description: Define functions with explicit type signatures and organize them into modules for clean code separation.
---

# Functions and Modules

> **What you'll learn:**
> - How to write functions with typed parameters, return types, and Rust's expression-based return
> - How to split code into modules using `mod`, `pub`, and `use` for visibility control
> - How to create a multi-file module hierarchy that keeps your CLI project maintainable

As your coding agent grows, you need two things: functions to encapsulate behavior and modules to organize those functions into logical groups. Rust's function system will feel familiar coming from Python, with one twist — every parameter and return type must be explicit. Modules, on the other hand, are a new concept. They replace Python's file-based import system with something more explicit and powerful.

## Defining Functions

Here is a function in Rust:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    let result = add(3, 4);
    println!("3 + 4 = {result}");
}
```

Let's break down the syntax:

- **`fn`** — keyword to define a function (Python uses `def`)
- **`add`** — the function name, in `snake_case` (same convention as Python)
- **`a: i32, b: i32`** — parameters with explicit types. Unlike Python, you must declare the type of every parameter.
- **`-> i32`** — the return type. This is required if the function returns a value.
- **`a + b`** — the return value. Notice there is no `return` keyword and no semicolon.

That last point deserves emphasis. In Rust, the **last expression in a function is its return value** — as long as it does not end with a semicolon. Adding a semicolon turns an expression into a statement, which returns nothing.

```rust
fn add_with_return(a: i32, b: i32) -> i32 {
    return a + b;  // Also works — explicit return with semicolon
}

fn add_expression(a: i32, b: i32) -> i32 {
    a + b  // Preferred — implicit return, no semicolon
}

fn main() {
    println!("{}", add_with_return(1, 2));
    println!("{}", add_expression(1, 2));
}
```

Both forms are valid, but the expression-based style (without `return` and without a semicolon) is idiomatic Rust. Use `return` only for early returns — exiting a function before the last line.

::: python Coming from Python
Python functions use `def`, optional type hints, and explicit `return`. Rust functions use `fn`, mandatory type annotations, and implicit return (the last expression). The biggest adjustment: forgetting the semicolon on the last line is not a mistake in Rust — it is how you return a value. Adding a semicolon when you did not mean to is a common beginner error.

```python
# Python
def add(a: int, b: int) -> int:
    return a + b
```

```rust
// Rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```
:::

## Functions That Return Nothing

Functions that do not return a value have an implicit return type of `()` (the "unit type" — Rust's equivalent of Python's `None`):

```rust
fn print_banner() {
    println!("=========================");
    println!("  Kodai Coding Agent");
    println!("=========================");
}

fn main() {
    print_banner();
}
```

You do not need to write `-> ()` — it is implied when there is no return type annotation.

## Early Returns

Use `return` when you need to exit a function before the last line:

```rust
fn classify_command(input: &str) -> &str {
    if input.starts_with('/') {
        return "builtin";
    }

    if input.is_empty() {
        return "empty";
    }

    "user_message"
}

fn main() {
    println!("{}", classify_command("/help"));    // "builtin"
    println!("{}", classify_command(""));          // "empty"
    println!("{}", classify_command("hello"));     // "user_message"
}
```

The last line `"user_message"` does not need `return` because it is the final expression.

## Closures (Lambdas)

Rust closures are like Python's `lambda`, but more powerful — they can span multiple lines:

```rust
fn main() {
    let double = |x: i32| x * 2;
    let add = |a: i32, b: i32| -> i32 { a + b };

    println!("double(5) = {}", double(5));
    println!("add(3, 4) = {}", add(3, 4));

    // Closures can capture variables from the enclosing scope
    let prefix = "Agent";
    let make_greeting = |name: &str| format!("{prefix}: Hello, {name}!");

    println!("{}", make_greeting("developer"));
}
```

Closures are used heavily with iterators (`.map()`, `.filter()`, `.for_each()`) — the same patterns you know from Python's list comprehensions and `map()`/`filter()`.

## Introducing Modules

As your project grows, you need to split code across files. Rust uses **modules** to organize code into namespaces. A module is a container for functions, types, constants, and other modules.

### Inline Modules

The simplest module is defined inline, in the same file:

```rust
mod commands {
    pub fn help() -> String {
        String::from("Available commands: /help, /quit, /clear")
    }

    pub fn version() -> String {
        let version = env!("CARGO_PKG_VERSION");
        format!("Kodai v{version}")
    }
}

fn main() {
    println!("{}", commands::help());
    println!("{}", commands::version());
}
```

Key points:

- **`mod commands`** creates a module named `commands`.
- **`pub fn`** makes the function public. Without `pub`, it is private to the module.
- **`commands::help()`** uses the `::` path separator to call a function inside a module (like Python's dot notation).

### Visibility: pub and Private by Default

Everything in Rust is **private by default**. If you define a function without `pub`, it can only be called from within the same module:

```rust
mod internal {
    fn secret() -> &'static str {
        "you can't see me from outside"
    }

    pub fn public() -> &'static str {
        // Can call secret() here because we're inside the module
        secret();
        "this is visible"
    }
}

fn main() {
    println!("{}", internal::public());   // OK
    // println!("{}", internal::secret()); // ERROR: private function
}
```

::: python Coming from Python
Python uses the underscore convention (`_private_func`) to signal privacy, but it is not enforced — anyone can still call it. Rust enforces privacy at compile time. If a function is not marked `pub`, code outside the module literally cannot call it. This is a real boundary, not a suggestion.
:::

## File-Based Modules

For a real project, you want modules in separate files. Rust gives you two ways to do this.

### Single-File Module

Create `src/commands.rs`:

```rust
pub fn help() -> String {
    String::from("Available commands: /help, /quit, /clear")
}

pub fn version() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!("Kodai v{version}")
}
```

Then declare the module in `src/main.rs` (or `src/lib.rs`):

```rust
mod commands;

fn main() {
    println!("{}", commands::help());
    println!("{}", commands::version());
}
```

The `mod commands;` line tells Rust to look for either `src/commands.rs` or `src/commands/mod.rs`. Since you created `src/commands.rs`, it finds the file and includes it.

### Directory Module

When a module itself has submodules, use a directory with a `mod.rs` file. For example, to create a `tools` module with submodules:

```
src/
  main.rs
  lib.rs
  tools/
    mod.rs          # Module root — declares submodules
    shell.rs        # tools::shell
    file_read.rs    # tools::file_read
```

In `src/tools/mod.rs`:

```rust
pub mod shell;
pub mod file_read;
```

In `src/tools/shell.rs`:

```rust
pub fn execute(command: &str) -> String {
    format!("Executing: {command}")
}
```

In `src/tools/file_read.rs`:

```rust
pub fn read(path: &str) -> String {
    format!("Reading file: {path}")
}
```

In `src/lib.rs`:

```rust
pub mod tools;
```

In `src/main.rs`:

```rust
use kodai::tools;

fn main() {
    println!("{}", tools::shell::execute("ls -la"));
    println!("{}", tools::file_read::read("src/main.rs"));
}
```

### The use Statement

When paths get long, `use` creates shorter aliases:

```rust
use kodai::tools::shell;
use kodai::tools::file_read;

fn main() {
    println!("{}", shell::execute("ls -la"));
    println!("{}", file_read::read("src/main.rs"));
}
```

You can also import multiple items from the same path:

```rust
use kodai::tools::{shell, file_read};
```

Or bring a specific function into scope:

```rust
use kodai::tools::shell::execute;

fn main() {
    println!("{}", execute("ls -la"));
}
```

## Applying This to Your Project

Let's organize the coding agent project. Update your files:

In `src/lib.rs`:

```rust
pub mod commands;

pub fn create_prompt(username: &str) -> String {
    format!("{username}> ")
}
```

Create `src/commands.rs`:

```rust
pub fn handle(input: &str) -> String {
    match input.trim() {
        "/help" => help(),
        "/version" => version(),
        "/quit" => String::from("Goodbye!"),
        _ => format!("Unknown command: {input}"),
    }
}

fn help() -> String {
    let commands = ["/help", "/version", "/quit"];
    let list: Vec<&str> = commands.to_vec();
    format!("Available commands:\n{}", list.join("\n"))
}

fn version() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!("Kodai v{version}")
}
```

Update `src/main.rs`:

```rust
use kodai::commands;
use kodai::create_prompt;

fn main() {
    let prompt = create_prompt("dev");
    println!("Prompt: {prompt}");
    println!();
    println!("{}", commands::handle("/help"));
    println!();
    println!("{}", commands::handle("/version"));
}
```

Run `cargo run` and verify the output. You now have a cleanly organized project with separate modules for commands.

## Key Takeaways

- Rust functions require explicit type annotations for parameters and return types. The last expression (without a semicolon) is the return value.
- Everything is **private by default**. Use `pub` to make functions, types, and modules visible outside their parent module.
- Modules organize code into namespaces. Use `mod name;` to include a file-based module and `pub mod name;` to make it public.
- For file-based modules, create either `src/name.rs` or `src/name/mod.rs` — Rust finds both patterns automatically.
- Use `use` statements to shorten long module paths and keep your code readable.
