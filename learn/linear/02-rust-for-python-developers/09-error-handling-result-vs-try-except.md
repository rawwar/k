---
title: Error Handling Result vs Try Except
description: Replacing Python's try/except with Rust's Result type, the question mark operator, and explicit error propagation.
---

# Error Handling Result vs Try Except

> **What you'll learn:**
> - How `Result<T, E>` and `Option<T>` replace Python's exceptions with values that must be explicitly handled
> - How the ? operator provides ergonomic error propagation similar to Python's implicit exception bubbling
> - How to define custom error types and use the thiserror and anyhow crates for real-world error handling

Error handling is where Rust and Python differ most dramatically. Python uses exceptions — errors are thrown up the call stack until something catches them. If nothing catches them, the program crashes with a traceback. Rust has no exceptions. Instead, errors are *values* — regular data returned from functions that the compiler forces you to handle.

This sounds tedious, but Rust provides ergonomic tools that make it nearly as convenient as exceptions while being far more predictable.

## Python's exception model

In Python, errors are invisible in function signatures and can come from anywhere:

```python
def read_config(path: str) -> dict:
    with open(path) as f:          # Could raise FileNotFoundError, PermissionError
        content = f.read()         # Could raise UnicodeDecodeError, IOError
    return json.loads(content)     # Could raise JSONDecodeError

# Caller might handle errors, or might not
try:
    config = read_config("config.json")
except FileNotFoundError:
    config = {}
# But what about JSONDecodeError? PermissionError? We forgot those.
```

The problem is that nothing in the function signature tells you what errors it can raise. You have to read the documentation (if it exists) or the source code. And the compiler never tells you if you forgot to handle an error.

## Rust's Result type

In Rust, functions that can fail return a `Result<T, E>`:

```rust
use std::fs;
use std::io;

fn read_config(path: &str) -> Result<String, io::Error> {
    let content = fs::read_to_string(path)?;
    Ok(content)
}

fn main() {
    match read_config("config.json") {
        Ok(content) => println!("Config: {}", content),
        Err(e) => println!("Failed to read config: {}", e),
    }
}
```

The return type `Result<String, io::Error>` tells you everything:
- On success, you get a `String`
- On failure, you get an `io::Error`

The compiler *refuses* to let you ignore this. You must handle both cases.

::: python Coming from Python
Imagine if Python's type system enforced this:
```python
def read_config(path: str) -> str | FileNotFoundError | PermissionError:
    ...
```
And the type checker *refused to compile* if you accessed the return value without first checking if it was an error. That is what Rust does with `Result`. The error information is in the type signature, not hidden in documentation.
:::

## The `?` operator — ergonomic error propagation

The `?` operator is Rust's equivalent of letting exceptions bubble up. It unwraps an `Ok` value or returns the `Err` early from the current function:

```rust
use std::fs;
use std::io;

fn read_and_parse_config(path: &str) -> Result<Config, io::Error> {
    let content = fs::read_to_string(path)?;  // If this fails, return the error immediately
    let trimmed = content.trim().to_string();  // Only runs if read succeeded
    Ok(Config { data: trimmed })
}

struct Config {
    data: String,
}
```

Without `?`, you would write:

```rust
use std::fs;
use std::io;

struct Config {
    data: String,
}

fn read_and_parse_config(path: &str) -> Result<Config, io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return Err(e),
    };
    let trimmed = content.trim().to_string();
    Ok(Config { data: trimmed })
}
```

The `?` operator replaces that entire match block. You can chain multiple `?` calls, and the first error short-circuits the function:

```rust
use std::fs;
use std::io;

fn setup_agent(config_path: &str, log_path: &str) -> Result<String, io::Error> {
    let config = fs::read_to_string(config_path)?;   // may fail
    let _log = fs::read_to_string(log_path)?;         // may fail
    Ok(config)
}

fn main() {
    match setup_agent("config.json", "agent.log") {
        Ok(config) => println!("Started with config: {}", config),
        Err(e) => eprintln!("Setup failed: {}", e),
    }
}
```

::: python Coming from Python
The `?` operator is conceptually identical to Python's implicit exception propagation:
```python
def setup_agent(config_path: str, log_path: str) -> str:
    config = open(config_path).read()   # exception propagates automatically
    log = open(log_path).read()         # exception propagates automatically
    return config
```
In Python, errors propagate automatically unless you catch them. In Rust, errors propagate when you explicitly use `?`. The difference is that Rust makes it *visible* — every `?` in your code is a point where an error might cause early return. You can see the error flow by scanning for `?` marks.
:::

## `Option<T>` — the "might not exist" type

`Option<T>` handles the absence of values, replacing Python's `None`:

```rust
fn find_tool(name: &str) -> Option<String> {
    match name {
        "shell" => Some(String::from("Execute shell commands")),
        "read_file" => Some(String::from("Read file contents")),
        "write_file" => Some(String::from("Write file contents")),
        _ => None,
    }
}

fn main() {
    // Pattern matching
    match find_tool("shell") {
        Some(desc) => println!("Found: {}", desc),
        None => println!("Not found"),
    }

    // unwrap_or — provide a default (like dict.get(key, default))
    let desc = find_tool("unknown").unwrap_or(String::from("No description"));
    println!("{}", desc);

    // map — transform the inner value if present
    let upper = find_tool("shell").map(|d| d.to_uppercase());
    println!("{:?}", upper);  // Some("EXECUTE SHELL COMMANDS")

    // and_then — chain operations that return Option (like flatmap)
    let first_word = find_tool("shell")
        .and_then(|d| d.split_whitespace().next().map(String::from));
    println!("{:?}", first_word);  // Some("Execute")
}
```

::: python Coming from Python
Here are the common Python `None` patterns and their Rust `Option` equivalents:

| Python | Rust |
|--------|------|
| `x = None` | `let x: Option<T> = None` |
| `if x is not None:` | `if let Some(val) = x {` |
| `x or default` | `x.unwrap_or(default)` |
| `x if x is not None else compute()` | `x.unwrap_or_else(\|\| compute())` |
| `transform(x) if x else None` | `x.map(\|v\| transform(v))` |

The critical difference: Python lets you call `.method()` on a `None` value and crash at runtime. Rust makes it a compile error. You cannot access the value inside `Option` without handling the `None` case first.
:::

## Custom error types

For real applications, you will want your own error types that combine multiple error sources. Here is the manual approach:

```rust
use std::fmt;
use std::io;
use std::num;

#[derive(Debug)]
enum AgentError {
    ConfigNotFound(String),
    InvalidPort(num::ParseIntError),
    IoError(io::Error),
    ApiError(String),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::ConfigNotFound(path) => write!(f, "Config not found: {}", path),
            AgentError::InvalidPort(e) => write!(f, "Invalid port number: {}", e),
            AgentError::IoError(e) => write!(f, "I/O error: {}", e),
            AgentError::ApiError(msg) => write!(f, "API error: {}", msg),
        }
    }
}

// Allow automatic conversion from io::Error to AgentError
impl From<io::Error> for AgentError {
    fn from(e: io::Error) -> Self {
        AgentError::IoError(e)
    }
}

fn read_port_from_config(path: &str) -> Result<u16, AgentError> {
    let content = std::fs::read_to_string(path)?;  // io::Error auto-converts to AgentError
    let port: u16 = content.trim().parse().map_err(AgentError::InvalidPort)?;
    Ok(port)
}

fn main() {
    match read_port_from_config("port.txt") {
        Ok(port) => println!("Listening on port {}", port),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## The thiserror crate — less boilerplate

Writing all those `Display` and `From` implementations is tedious. The `thiserror` crate generates them for you:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
enum AgentError {
    #[error("Config not found: {0}")]
    ConfigNotFound(String),

    #[error("Invalid port number: {0}")]
    InvalidPort(#[from] std::num::ParseIntError),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("API error: {0}")]
    ApiError(String),
}
```

The `#[error("...")]` attribute generates `Display`, and `#[from]` generates the `From` implementation. This is the approach we will use in the coding agent.

::: python Coming from Python
This is like defining custom exception classes in Python:
```python
class AgentError(Exception):
    pass

class ConfigNotFound(AgentError):
    def __init__(self, path: str):
        super().__init__(f"Config not found: {path}")

class ApiError(AgentError):
    def __init__(self, message: str):
        super().__init__(f"API error: {message}")
```
The difference is that Python exceptions form a class hierarchy (you catch parent classes to handle groups of errors), while Rust errors are enum variants (you match on them). Both approaches organize errors into named categories — the mechanism is different but the intent is the same.
:::

## The anyhow crate — for applications

When you do not need callers to match on specific error variants — common in application code as opposed to library code — `anyhow` provides a catch-all error type:

```rust
use anyhow::{Context, Result};

fn read_config(path: &str) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .context(format!("Failed to read config from {}", path))?;
    Ok(content)
}

fn main() -> Result<()> {
    let config = read_config("config.json")?;
    println!("Config: {}", config);
    Ok(())
}
```

`anyhow::Result<T>` is shorthand for `Result<T, anyhow::Error>`, where `anyhow::Error` can hold any error type. The `.context()` method adds human-readable context to error messages, producing output like:

```
Error: Failed to read config from config.json

Caused by:
    No such file or directory (os error 2)
```

The rule of thumb:
- **Libraries**: Use `thiserror` with specific error enums — callers need to match on error variants
- **Applications**: Use `anyhow` — you want good error messages, not programmatic error handling

::: python Coming from Python
`anyhow` is the closest thing Rust has to Python's bare `except Exception as e:` — it catches any error and lets you add context. The difference is that in Rust, even with `anyhow`, you still must explicitly propagate errors with `?`. There is no invisible exception propagation.
:::

## Combining Result and Option

You will often convert between `Result` and `Option`:

```rust
fn parse_port(s: &str) -> Option<u16> {
    s.parse::<u16>().ok()  // Convert Result to Option, discarding the error
}

fn require_port(s: &str) -> Result<u16, String> {
    s.parse::<u16>().map_err(|e| format!("Bad port '{}': {}", s, e))
}

fn main() {
    let port = parse_port("8080");
    println!("{:?}", port);  // Some(8080)

    let port = parse_port("abc");
    println!("{:?}", port);  // None

    let port = require_port("abc");
    println!("{:?}", port);  // Err("Bad port 'abc': invalid digit found in string")
}
```

## Key Takeaways

- Rust uses `Result<T, E>` instead of exceptions — errors are values in the return type, making error paths visible and enforced by the compiler
- The `?` operator provides ergonomic error propagation similar to Python's implicit exception bubbling, but every error path is explicitly marked
- `Option<T>` replaces `None` with compile-time safety — you cannot use a value without handling the absence case
- Use `thiserror` for library error types (callers need to match variants) and `anyhow` for application error handling (good messages, less boilerplate)
- The fundamental shift: in Python, errors are invisible and you opt *in* to handling them with `try/except`; in Rust, errors are visible and you must explicitly handle them or propagate them with `?`
