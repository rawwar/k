---
title: Error Handling Basics
description: Use Result and Option to handle errors explicitly instead of relying on exceptions — Rust's approach to making failure paths visible and reliable.
---

# Error Handling Basics

> **What you'll learn:**
> - How `Result<T, E>` and `Option<T>` replace exceptions with compile-time checked error paths
> - How to use the `?` operator to propagate errors concisely through function chains
> - How to create custom error types and convert between different error kinds with `From`

If you have written Python for any length of time, you have dealt with exceptions — `try/except`, `raise`, catching `FileNotFoundError` deep in a call stack. Exceptions are powerful but invisible: nothing in a Python function's signature tells you which exceptions it might throw. You discover them at runtime, often in production.

Rust takes a radically different approach. There are no exceptions. Instead, functions that can fail return a `Result<T, E>` type that forces you to handle the error case *before the code compiles*. This is the single most important concept for building a reliable coding agent.

## The Result Type

`Result<T, E>` is an enum with two variants:

```rust
enum Result<T, E> {
    Ok(T),    // Success — contains the value
    Err(E),   // Failure — contains the error
}
```

A function that might fail returns `Result`. The caller must handle both cases:

```rust
use std::fs;

fn main() {
    let result = fs::read_to_string("config.toml");

    match result {
        Ok(contents) => println!("File contents:\n{contents}"),
        Err(error) => println!("Failed to read file: {error}"),
    }
}
```

The `match` expression is exhaustive — the compiler ensures you handle both `Ok` and `Err`. If you forget one, the code does not compile. Compare this to Python, where forgetting a `try/except` means the exception propagates silently until it crashes the program.

::: python Coming from Python
In Python, reading a file looks like this:
```python
try:
    contents = open("config.toml").read()
    print(contents)
except FileNotFoundError as e:
    print(f"Failed to read file: {e}")
```
The `try/except` is optional — without it, the program crashes with a traceback. In Rust, `fs::read_to_string` returns `Result<String, io::Error>`. The compiler *forces* you to handle the error case. You cannot accidentally ignore a file-not-found error.
:::

## The Option Type

`Option<T>` is for values that might not exist — Rust's replacement for `null`/`None`:

```rust
enum Option<T> {
    Some(T),  // A value exists
    None,     // No value
}
```

You already saw this with `HashMap::get`. Here is another example:

```rust
fn find_command(input: &str) -> Option<&str> {
    let commands = ["/help", "/quit", "/clear", "/version"];
    commands.iter().find(|&&cmd| cmd == input).copied()
}

fn main() {
    match find_command("/help") {
        Some(cmd) => println!("Found command: {cmd}"),
        None => println!("Unknown command"),
    }

    match find_command("/unknown") {
        Some(cmd) => println!("Found command: {cmd}"),
        None => println!("Unknown command"),
    }
}
```

Output:

```
Found command: /help
Unknown command
```

`Option` makes "absence" explicit. In Python, a function might return `None` silently, and you only discover the problem when you try to call a method on it (`AttributeError: 'NoneType' object has no attribute ...`). In Rust, `Option` forces you to check before using the value.

## Unwrapping: The Quick and Dangerous Path

When you are certain a `Result` or `Option` contains a value, you can use `.unwrap()`:

```rust
fn main() {
    let contents = std::fs::read_to_string("Cargo.toml").unwrap();
    println!("Read {} bytes", contents.len());
}
```

If the file exists, `.unwrap()` extracts the value. If it does not, your program **panics** — it crashes immediately with an error message. This is fine for quick prototyping and examples, but you should avoid `.unwrap()` in production code.

Better alternatives:

```rust
fn main() {
    // .expect() — like unwrap but with a custom error message
    let contents = std::fs::read_to_string("Cargo.toml")
        .expect("Cargo.toml must exist in project root");

    println!("Read {} bytes", contents.len());
}
```

`.expect("message")` is better than `.unwrap()` because it tells the reader *why* you believe this cannot fail.

## The ? Operator: Concise Error Propagation

In a real application, you do not want to handle every error at the call site. You want to propagate errors up to the caller. The `?` operator does this elegantly:

```rust
use std::fs;
use std::io;

fn read_config() -> Result<String, io::Error> {
    let contents = fs::read_to_string("config.toml")?;
    Ok(contents)
}

fn main() {
    match read_config() {
        Ok(config) => println!("Config loaded: {config}"),
        Err(e) => eprintln!("Error loading config: {e}"),
    }
}
```

The `?` at the end of `fs::read_to_string("config.toml")?` means: "If this returns `Ok`, unwrap the value and continue. If this returns `Err`, return the error from the current function immediately."

Without `?`, you would have to write:

```rust
fn read_config() -> Result<String, io::Error> {
    let result = fs::read_to_string("config.toml");
    match result {
        Ok(contents) => Ok(contents),
        Err(e) => Err(e),
    }
}
```

The `?` operator saves you from this boilerplate. You can chain multiple fallible operations:

```rust
use std::fs;
use std::io;

fn read_and_count_lines(path: &str) -> Result<usize, io::Error> {
    let contents = fs::read_to_string(path)?;
    let line_count = contents.lines().count();
    Ok(line_count)
}

fn main() {
    match read_and_count_lines("Cargo.toml") {
        Ok(count) => println!("Cargo.toml has {count} lines"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

If `read_to_string` fails, the error is returned immediately. If it succeeds, execution continues to the next line. This pattern — chaining `?` calls — keeps your code flat and readable.

::: python Coming from Python
The `?` operator is like a one-character `try/except` that re-raises the exception. In Python:
```python
def read_and_count_lines(path):
    contents = open(path).read()  # raises if file missing
    return len(contents.splitlines())
```
Python propagates exceptions automatically. Rust makes you opt in with `?`. The benefit: by looking at a Rust function's signature (`-> Result<usize, io::Error>`), you *know* it can fail and you know what error type to expect. A Python function's signature tells you nothing about its failure modes.
:::

## Using ? in main()

You can use `?` in `main()` by changing its return type:

```rust
use std::fs;
use std::io;

fn main() -> Result<(), io::Error> {
    let contents = fs::read_to_string("Cargo.toml")?;
    println!("Read {} bytes from Cargo.toml", contents.len());
    Ok(())
}
```

If the `?` causes an error, Rust prints the error message and exits with a non-zero exit code. This is useful for simple programs, but for a CLI tool you usually want more control over error presentation.

## Handling Multiple Error Types

Real programs encounter different kinds of errors. What if your function reads a file *and* parses an integer from it?

```rust
use std::fs;
use std::num::ParseIntError;
use std::io;
use std::fmt;

#[derive(Debug)]
enum AppError {
    Io(io::Error),
    Parse(ParseIntError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "IO error: {e}"),
            AppError::Parse(e) => write!(f, "Parse error: {e}"),
        }
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<ParseIntError> for AppError {
    fn from(e: ParseIntError) -> Self {
        AppError::Parse(e)
    }
}

fn read_port_from_file(path: &str) -> Result<u16, AppError> {
    let contents = fs::read_to_string(path)?;  // io::Error -> AppError
    let port: u16 = contents.trim().parse()?;   // ParseIntError -> AppError
    Ok(port)
}

fn main() {
    match read_port_from_file("port.txt") {
        Ok(port) => println!("Port: {port}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

The `From` implementations tell Rust how to convert `io::Error` and `ParseIntError` into your `AppError` type. The `?` operator uses these conversions automatically.

This is a lot of boilerplate for two error types. In later chapters, you will use the `thiserror` crate to generate these implementations with a single `#[derive]` attribute. For now, understanding the manual version shows you what is happening under the hood.

## Practical Patterns for the Coding Agent

Here are patterns you will use throughout this book:

```rust
use std::io::{self, Write};

fn prompt_user(message: &str) -> Result<String, io::Error> {
    print!("{message}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

fn main() {
    match prompt_user("Enter a command: ") {
        Ok(input) => println!("You entered: {input}"),
        Err(e) => eprintln!("Failed to read input: {e}"),
    }
}
```

This function can fail in two places (flushing stdout, reading stdin), and both errors propagate cleanly with `?`. The caller decides how to handle the failure.

## Key Takeaways

- Rust has no exceptions. Functions that can fail return `Result<T, E>`, and the compiler forces you to handle both the success and error cases.
- `Option<T>` represents values that might not exist, replacing `null`/`None` with compile-time safety.
- The `?` operator propagates errors concisely — it unwraps `Ok` values and returns `Err` values to the caller automatically.
- Avoid `.unwrap()` in production code. Use `.expect("reason")` when you are certain a value exists, and `?` when errors should propagate.
- Custom error types with `From` implementations let you use `?` across functions that return different error types — the compiler handles conversion automatically.
