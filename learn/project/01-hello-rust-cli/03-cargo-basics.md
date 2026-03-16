---
title: Cargo Basics
description: Learn how Cargo manages dependencies, builds projects, and runs tests as the central tool in every Rust workflow.
---

# Cargo Basics

> **What you'll learn:**
> - How to create a new project with `cargo new` and understand the generated file layout
> - How to add, update, and manage crate dependencies in `Cargo.toml`
> - How to use `cargo build`, `cargo run`, and `cargo test` in your daily development cycle

Cargo is the tool you reach for every single day as a Rust developer. It creates projects, manages dependencies, compiles your code, runs tests, generates documentation, and publishes packages. In the Python world, you need separate tools for each of these tasks — `pip` for dependencies, `pytest` for tests, `build` for packaging, `black` for formatting, `pyproject.toml` for configuration. Cargo does all of it in one unified tool.

Let's get familiar with it by creating a project from scratch.

## Creating a New Project

Open your terminal and run:

```bash
cargo new kodai
cd kodai
```

The name `kodai` is what we call the coding agent you build throughout this book (you can choose any name, but the examples use `kodai`). Cargo creates a new directory with this structure:

```
kodai/
  Cargo.toml
  src/
    main.rs
  .gitignore
```

Cargo also initializes a Git repository automatically. Let's look at each file.

### Cargo.toml — The Project Manifest

```toml
[package]
name = "kodai"
version = "0.1.0"
edition = "2021"

[dependencies]
```

This is Rust's equivalent of `pyproject.toml`. The `[package]` section describes your project, and `[dependencies]` is where you list external crates (Rust's term for packages/libraries).

The `edition` field specifies which Rust edition your code uses. Editions are how Rust introduces breaking changes without breaking old code — think of it like Python 2 vs. Python 3, but backward-compatible. The `2021` edition is standard for all new projects.

### src/main.rs — The Entry Point

```rust
fn main() {
    println!("Hello, world!");
}
```

Every Rust binary needs a `main` function in `src/main.rs`. This is where execution begins — the equivalent of Python's `if __name__ == "__main__":` block.

::: python Coming from Python
In Python, `pyproject.toml` describes your project metadata and dependencies. Rust's `Cargo.toml` serves exactly the same purpose, but Cargo also replaces `pip`, `venv`, `build`, and `setuptools`. One file, one tool, everything works together.
:::

## Building and Running

The two commands you use most often:

```bash
# Compile the project (debug mode by default)
cargo build

# Compile AND run the project
cargo run
```

`cargo run` is what you want 90% of the time during development — it compiles your code and immediately executes the resulting binary. You should see:

```
   Compiling kodai v0.1.0 (/path/to/kodai)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.50s
     Running `target/debug/kodai`
Hello, world!
```

Notice the `[unoptimized + debuginfo]` tag. By default, `cargo build` produces a **debug build** — fast to compile but slower to run. When you are ready to ship, you create a **release build**:

```bash
cargo build --release
```

Release builds take longer to compile but produce a significantly faster binary. The output goes to `target/release/kodai` instead of `target/debug/kodai`.

## Checking Without Building

Sometimes you just want to know if your code compiles without producing a binary:

```bash
cargo check
```

This is faster than `cargo build` because it skips code generation. Use it as a quick feedback loop while developing: make a change, run `cargo check`, see if it compiles.

## Adding Dependencies

The Rust ecosystem publishes crates to [crates.io](https://crates.io), the official package registry (equivalent to PyPI). To add a dependency:

```bash
cargo add serde
```

This adds the `serde` crate to your `Cargo.toml`:

```toml
[dependencies]
serde = "1.0.219"
```

The version string `"1.0.219"` follows semantic versioning. By default, Cargo uses a caret requirement: `"1.0.219"` means "any version >= 1.0.219 and < 2.0.0". This gives you bug fixes and minor updates automatically while protecting you from breaking changes.

You can also add dependencies with specific features enabled:

```bash
cargo add serde --features derive
```

Features are Rust's way of enabling optional functionality in a crate. The `derive` feature of `serde`, for example, lets you automatically generate serialization code using annotations.

### Cargo.lock — The Exact Dependency Snapshot

When you build for the first time after adding a dependency, Cargo creates a `Cargo.lock` file. This records the exact version of every dependency (including transitive dependencies). It serves the same purpose as `pip freeze > requirements.txt` or Poetry's `poetry.lock`.

For binary projects (like our coding agent), you **should** commit `Cargo.lock` to version control. It ensures everyone building your project gets exactly the same dependency versions.

## Running Tests

Cargo has a built-in test runner:

```bash
cargo test
```

This compiles your code with the test harness enabled and runs all functions annotated with `#[test]`. Let's add a simple test. Open `src/main.rs` and replace its contents with:

```rust
fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

fn main() {
    println!("{}", greet("world"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("Rust"), "Hello, Rust!");
    }

    #[test]
    fn test_greet_empty() {
        assert_eq!(greet(""), "Hello, !");
    }
}
```

Now run `cargo test`:

```
   Compiling kodai v0.1.0 (/path/to/kodai)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.50s
     Running unittests src/main.rs (target/debug/deps/kodai-abc123)

running 2 tests
test tests::test_greet ... ok
test tests::test_greet_empty ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

::: python Coming from Python
In Python, you typically write tests in separate files and run them with `pytest`. In Rust, tests live *inside the same file* as the code they test, wrapped in a `#[cfg(test)]` module. This means tests are always right next to the implementation — no hunting for the matching test file. The `#[cfg(test)]` attribute ensures test code is not included in the release binary.
:::

## Other Useful Cargo Commands

| Command | What It Does | Python Equivalent |
|---------|-------------|-------------------|
| `cargo fmt` | Format all code | `black .` |
| `cargo clippy` | Run the linter | `flake8` or `pylint` |
| `cargo doc --open` | Generate and open documentation | `sphinx-build` |
| `cargo update` | Update dependencies to latest compatible versions | `pip install --upgrade` |
| `cargo clean` | Delete compiled artifacts (`target/`) | `rm -rf __pycache__/ .pyc` |

A typical development session looks like this:

1. Write code
2. `cargo run` to compile and test your changes
3. `cargo clippy` to catch common mistakes
4. `cargo fmt` to keep formatting consistent
5. `cargo test` to verify nothing is broken

You will use this workflow for the rest of the book.

## The target/ Directory

When you build, Cargo puts all compiled artifacts in the `target/` directory:

```
target/
  debug/           # Debug builds
    kodai          # Your compiled binary
    deps/          # Compiled dependencies
    build/         # Build script outputs
  release/         # Release builds (when using --release)
```

This directory can grow large (hundreds of megabytes for projects with many dependencies). It is already in your `.gitignore` — never commit it to version control. If you need to free disk space, `cargo clean` deletes the entire `target/` directory.

## Key Takeaways

- `cargo new <name>` creates a complete project with `Cargo.toml`, `src/main.rs`, and a git repository — ready to build immediately.
- `cargo run` is your primary command during development — it compiles and runs your binary in one step.
- Dependencies are added with `cargo add <crate>` and recorded in `Cargo.toml`. The `Cargo.lock` file pins exact versions for reproducible builds.
- Tests live alongside your code in `#[cfg(test)]` modules and run with `cargo test` — no separate test framework needed.
- `cargo fmt` and `cargo clippy` keep your code clean and idiomatic — use them early and often.
