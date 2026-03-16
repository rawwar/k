---
title: Testing
description: Writing unit tests, integration tests, and doc tests in Rust compared to Python's pytest and unittest frameworks.
---

# Testing

> **What you'll learn:**
> - How to write unit tests with #[test] and organize them in mod tests blocks within the same file
> - How integration tests in the tests/ directory compare to Python's test files and pytest conventions
> - How to use assert!, assert_eq!, and #[should_panic] along with test utilities like mockall for mocking

Testing in Rust is built into the language and the toolchain. There is no need to install a testing framework — `cargo test` runs your tests out of the box. If you are coming from Python's pytest, you will find that Rust's testing story is simpler in some ways (built-in, zero setup) and different in others (no fixtures, different mocking story).

## Your first test

In Python with pytest:

```python
# test_math.py
def add(a: int, b: int) -> int:
    return a + b

def test_add():
    assert add(2, 3) == 5
    assert add(-1, 1) == 0
```

In Rust:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-1, 1), 0);
    }
}
```

Run with:

```bash
cargo test
```

Output:

```
running 1 test
test tests::test_add ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

::: python Coming from Python
Key differences from pytest:
- Tests live *inside the same file* as the code they test, in a `mod tests` block — not in a separate `test_*.py` file
- `#[test]` marks a function as a test (like pytest discovers `test_*` functions by name)
- `#[cfg(test)]` means the entire `mod tests` block is only compiled when running tests — it does not exist in your release binary
- `use super::*;` imports everything from the parent module so tests can access the functions being tested
:::

## Assert macros

Rust provides three main assertion macros:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_assertions() {
        // assert! — checks a boolean condition (like Python's assert)
        assert!(5 > 3);
        assert!("hello".contains("ell"));

        // assert_eq! — checks equality with a helpful diff on failure
        assert_eq!(2 + 2, 4);
        assert_eq!("hello".to_uppercase(), "HELLO");

        // assert_ne! — checks inequality
        assert_ne!(2 + 2, 5);
    }

    #[test]
    fn test_with_messages() {
        let result = 42;
        // Custom failure messages (like Python's assert x == y, "message")
        assert!(result > 0, "Result should be positive, got {}", result);
        assert_eq!(result, 42, "Expected 42 but got {}", result);
    }
}
```

::: python Coming from Python
Pytest automatically provides detailed failure messages by rewriting assert statements. Rust's `assert_eq!` and `assert_ne!` provide similar detail — they print both the expected and actual values on failure:
```
thread 'tests::test_add' panicked at 'assertion `left == right` failed
  left: 4
 right: 5'
```
For `assert!`, add a custom message to get useful output on failure.
:::

## Testing error conditions

### Testing panics

```rust
fn divide(a: i32, b: i32) -> i32 {
    if b == 0 {
        panic!("Division by zero!");
    }
    a / b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Division by zero")]
    fn test_divide_by_zero() {
        divide(10, 0);  // This should panic
    }

    #[test]
    fn test_divide_normal() {
        assert_eq!(divide(10, 2), 5);
    }
}
```

::: python Coming from Python
`#[should_panic]` is equivalent to pytest's `pytest.raises`:
```python
import pytest

def test_divide_by_zero():
    with pytest.raises(ZeroDivisionError):
        divide(10, 0)
```
The `expected` parameter in `#[should_panic(expected = "Division by zero")]` checks that the panic message contains the given string, similar to `pytest.raises(Exception, match="...")`.
:::

### Testing Result types

Most real-world Rust tests verify `Result` values:

```rust
use std::num::ParseIntError;

fn parse_port(s: &str) -> Result<u16, ParseIntError> {
    s.parse::<u16>()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests can return Result — ? works inside tests!
    #[test]
    fn test_valid_port() -> Result<(), ParseIntError> {
        let port = parse_port("8080")?;
        assert_eq!(port, 8080);
        Ok(())
    }

    #[test]
    fn test_invalid_port() {
        let result = parse_port("not_a_number");
        assert!(result.is_err());
    }

    #[test]
    fn test_port_range() {
        // u16 max is 65535
        assert!(parse_port("70000").is_err());
        assert!(parse_port("0").is_ok());
    }
}
```

## Test organization

### Unit tests — inside the source file

Unit tests live in the same file as the code they test. This is the convention for testing private functions and internal logic:

```rust
// src/tools/shell.rs
pub fn sanitize_command(cmd: &str) -> Result<String, String> {
    if cmd.contains("rm -rf /") {
        return Err(String::from("Dangerous command blocked"));
    }
    Ok(cmd.trim().to_string())
}

fn is_allowed_command(cmd: &str) -> bool {
    // Private function — only unit tests can test this
    !cmd.starts_with("sudo")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_removes_whitespace() {
        let result = sanitize_command("  ls -la  ").unwrap();
        assert_eq!(result, "ls -la");
    }

    #[test]
    fn test_sanitize_blocks_dangerous() {
        let result = sanitize_command("rm -rf /");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Dangerous command blocked");
    }

    #[test]
    fn test_private_is_allowed() {
        // Unit tests CAN access private functions
        assert!(is_allowed_command("ls"));
        assert!(!is_allowed_command("sudo rm"));
    }
}
```

::: python Coming from Python
In Python, tests typically live in separate files (`test_shell.py`), and testing private functions (prefixed with `_`) is discouraged but possible. In Rust, unit tests are *encouraged* to test private functions — that is the whole point of putting them in the same file. The `#[cfg(test)]` block has full access to the module's private items through `use super::*;`.
:::

### Integration tests — in the `tests/` directory

Integration tests live in a `tests/` directory at the project root and can only access your crate's *public* API:

```
my-project/
  Cargo.toml
  src/
    lib.rs
    tools/
      mod.rs
      shell.rs
  tests/
    test_tools.rs      # integration test
    test_agent.rs      # another integration test
```

**tests/test_tools.rs:**

```rust
// Integration tests import your crate as an external dependency
use my_project::tools::shell;

#[test]
fn test_shell_execute_safe_command() {
    let result = shell::sanitize_command("echo hello");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "echo hello");
}

#[test]
fn test_shell_blocks_dangerous_command() {
    let result = shell::sanitize_command("rm -rf /");
    assert!(result.is_err());
}
```

::: python Coming from Python
Integration tests in Rust map to Python's pattern of having a `tests/` directory with test files:
```
my_project/
  my_project/
    __init__.py
    tools/
      shell.py
  tests/
    test_tools.py    # imports from my_project
    test_agent.py
```
The key difference: Rust integration tests can *only* use your crate's public API (`pub` items). They test your code from the outside, as a user would. Unit tests inside the source file can test private internals.
:::

### Doc tests — tests in documentation

Rust can run code examples in your documentation as tests:

```rust
/// Adds two numbers together.
///
/// # Examples
///
/// ```
/// let result = my_project::add(2, 3);
/// assert_eq!(result, 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

When you run `cargo test`, these documentation examples are compiled and executed. This guarantees that your examples actually work — they can never go stale.

::: python Coming from Python
This is like Python's `doctest` module but much more widely used. In Python, doctests are often ignored or go stale. In Rust, doc tests are first-class — they run as part of `cargo test` by default, and the community convention is to include them in public API documentation.
:::

## Running specific tests

```bash
# Run all tests
cargo test

# Run tests matching a name pattern (like pytest -k)
cargo test test_shell

# Run a specific test
cargo test tests::test_sanitize_removes_whitespace

# Run only unit tests (skip integration and doc tests)
cargo test --lib

# Run only integration tests
cargo test --test test_tools

# Run tests with output printed (like pytest -s)
cargo test -- --nocapture

# Run tests in a single thread (like pytest -p no:xdist)
cargo test -- --test-threads=1

# List all tests without running them
cargo test -- --list
```

::: python Coming from Python
The flag mapping:

| pytest | cargo test |
|--------|-----------|
| `pytest` | `cargo test` |
| `pytest -k "shell"` | `cargo test shell` |
| `pytest tests/test_tools.py` | `cargo test --test test_tools` |
| `pytest -s` (show output) | `cargo test -- --nocapture` |
| `pytest -x` (stop on first failure) | `cargo test -- --fail-fast` |
| `pytest --co` (list tests) | `cargo test -- --list` |
:::

## Testing async code

Testing async functions requires the `#[tokio::test]` attribute:

```rust
async fn fetch_data() -> Result<String, String> {
    Ok(String::from("test data"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_data() {
        let result = fetch_data().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test data");
    }
}
```

::: python Coming from Python
In Python with pytest, you use `pytest-asyncio`:
```python
import pytest

@pytest.mark.asyncio
async def test_fetch_data():
    result = await fetch_data()
    assert result == "test data"
```
Rust's `#[tokio::test]` serves the same purpose — it sets up an async runtime for the test function.
:::

## Test helpers and shared setup

Rust does not have pytest-style fixtures, but you can use regular functions and helper modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Helper function — equivalent to a pytest fixture
    fn create_test_message(role: &str, content: &str) -> Message {
        Message {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn test_user_message() {
        let msg = create_test_message("user", "Hello");
        assert_eq!(msg.role, "user");
    }

    #[test]
    fn test_assistant_message() {
        let msg = create_test_message("assistant", "Hi there");
        assert_eq!(msg.role, "assistant");
    }
}

struct Message {
    role: String,
    content: String,
}
```

## Key Takeaways

- Rust testing is built into `cargo test` — no external framework needed, unlike Python's pytest
- Unit tests live *inside* the source file in a `#[cfg(test)] mod tests` block and can test private functions; integration tests live in `tests/` and can only access public API
- `assert_eq!` and `assert_ne!` provide detailed failure messages showing both expected and actual values, similar to pytest's assert rewriting
- Tests can return `Result`, allowing you to use `?` for ergonomic error handling in tests
- `#[tokio::test]` enables async test functions, equivalent to `@pytest.mark.asyncio` in Python
