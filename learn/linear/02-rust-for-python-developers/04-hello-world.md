---
title: Hello World
description: Writing, compiling, and running your first Rust program while understanding the differences from Python's interpreted execution model.
---

# Hello World

> **What you'll learn:**
> - How to create a new Rust project with cargo new and understand the generated file structure
> - The difference between compiled binaries and Python's interpreted scripts
> - How fn main, println!, and basic Rust syntax compare to their Python equivalents

Time to write real Rust code. In this section, you will create a project, understand what every character in a Rust program means, and see how the compiled execution model differs from Python's interpreted approach.

## Creating your first project

```bash
cargo new hello-agent
cd hello-agent
```

Cargo generates this structure:

```
hello-agent/
  Cargo.toml
  src/
    main.rs
```

Let's look at each file.

**Cargo.toml:**

```toml
[package]
name = "hello-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
```

**src/main.rs:**

```rust
fn main() {
    println!("Hello, world!");
}
```

::: python Coming from Python
In Python, you might create a project with `mkdir my_project && touch my_project/main.py`. There is no standard project structure enforced by the language. In Rust, `cargo new` creates a standardized layout that every Rust developer recognizes. The `src/` directory is always where source code lives, and `main.rs` is always the entry point for binary projects. This consistency across the entire ecosystem makes navigating unfamiliar Rust projects immediately intuitive.
:::

## Anatomy of a Rust program

Let's compare the simplest possible program in both languages:

**Python:**

```python
print("Hello, world!")
```

**Rust:**

```rust
fn main() {
    println!("Hello, world!");
}
```

Several differences jump out:

### 1. An explicit `main` function is required

Python scripts execute top-level code directly. Rust requires a `fn main()` function as the entry point. Every Rust binary starts executing at `main`.

```python
# Python — top-level code just runs
name = "Agent"
print(f"Hello, {name}!")
```

```rust
// Rust — everything starts in main
fn main() {
    let name = "Agent";
    println!("Hello, {}!", name);
}
```

::: python Coming from Python
In Python, you might use `if __name__ == "__main__":` to guard your entry point. In Rust, the `fn main()` function serves this purpose unconditionally — it is always the entry point, no guard needed. There is no concept of a Rust file being "imported" or "run directly" like Python modules.
:::

### 2. `println!` is a macro, not a function

Notice the exclamation mark in `println!`. In Rust, the `!` indicates a *macro* call, not a regular function call. Macros are code that generates code at compile time. You do not need to understand how macros work internally yet — just know that `println!` is special.

```rust
fn main() {
    let language = "Rust";
    let year = 2015;

    // println! supports format strings with {} placeholders
    println!("Learning {} since {}!", language, year);

    // Debug printing with {:?} (like Python's repr())
    println!("Debug: {:?}", (language, year));
}
```

The formatting syntax is similar to Python's `.format()` method or f-strings:

| Python | Rust | Description |
|--------|------|-------------|
| `f"{name}"` | `format!("{}", name)` | String formatting |
| `print(f"{name}")` | `println!("{}", name)` | Print with format |
| `repr(obj)` | `format!("{:?}", obj)` | Debug representation |
| `f"{val:.2f}"` | `format!("{:.2}", val)` | Float precision |
| `f"{val:>10}"` | `format!("{:>10}", val)` | Right-align, width 10 |

### 3. Semicolons are required

Every statement in Rust ends with a semicolon. Forget one and the compiler will tell you exactly where:

```rust
fn main() {
    let x = 5;     // semicolons end statements
    let y = 10;    // they are not optional
    println!("{}", x + y);
}
```

### 4. Curly braces define scope

Python uses indentation to define blocks. Rust uses curly braces `{}`. Indentation in Rust is convention (enforced by `cargo fmt`), not syntax.

```python
# Python — indentation is syntax
if x > 0:
    print("positive")
else:
    print("non-positive")
```

```rust
// Rust — curly braces are syntax, indentation is convention
fn main() {
    let x = 5;
    if x > 0 {
        println!("positive");
    } else {
        println!("non-positive");
    }
}
```

## Variables and types

Let's look at variable declarations in both languages:

```python
# Python — no type annotations needed, mutable by default
name = "Agent"
count = 42
pi = 3.14
active = True
```

```rust
fn main() {
    // Rust — type is inferred, immutable by default
    let name = "Agent";      // &str (string slice)
    let count = 42;          // i32 (32-bit integer)
    let pi = 3.14;           // f64 (64-bit float)
    let active = true;       // bool

    // Explicit type annotations (optional when type can be inferred)
    let count: i32 = 42;
    let pi: f64 = 3.14;

    println!("{} - {} - {} - {}", name, count, pi, active);
}
```

::: python Coming from Python
The biggest surprise: Rust variables are **immutable by default**. In Python, every variable is mutable — you can reassign `x = 5` then `x = "hello"` without any complaint. In Rust, `let x = 5;` means `x` can never change. If you want mutability, you must explicitly ask for it with `let mut x = 5;`. This default forces you to think about which values actually need to change, leading to clearer, more predictable code.
:::

```rust
fn main() {
    let x = 5;
    // x = 10;  // ERROR: cannot assign twice to immutable variable

    let mut y = 5;
    y = 10;  // OK — y is declared mutable
    println!("y = {}", y);
}
```

## Rust's basic types

Here is a quick mapping of Python types to Rust types:

| Python | Rust | Notes |
|--------|------|-------|
| `int` | `i32`, `i64`, `u32`, `u64` | Rust has fixed-size integers; Python has arbitrary precision |
| `float` | `f32`, `f64` | `f64` is the default, like Python's `float` |
| `bool` | `bool` | `true`/`false` (lowercase, unlike Python's `True`/`False`) |
| `str` | `String`, `&str` | Two string types — we will cover this in detail later |
| `None` | `()` (unit type) | Rust has no null/None; `()` is the empty value |
| `tuple` | `(i32, f64, bool)` | Fixed-size, typed tuples |

```rust
fn main() {
    // Integer types with explicit sizes
    let small: i8 = 127;          // 8-bit signed: -128 to 127
    let medium: i32 = 2_000_000;  // 32-bit signed (default integer type)
    let big: i64 = 9_000_000_000; // 64-bit signed
    let positive: u32 = 42;       // 32-bit unsigned (no negatives)

    // Underscores in numbers for readability (like Python's 1_000_000)
    let million = 1_000_000;

    // Tuples
    let point: (f64, f64) = (3.0, 4.0);
    println!("x={}, y={}", point.0, point.1);  // access by index with .N

    println!("{} {} {} {} {}", small, medium, big, positive, million);
}
```

## Compile, then run

Here is the fundamental difference from Python. In Python, you type `python script.py` and your code runs immediately. The interpreter reads your code line by line, parsing and executing as it goes.

In Rust, there are two distinct steps:

```bash
# Step 1: Compile — the compiler checks your code and produces a binary
cargo build

# Step 2: Run — execute the compiled binary
./target/debug/hello-agent

# Or do both in one command:
cargo run
```

During compilation, the Rust compiler:
1. **Parses** your source code
2. **Type checks** every expression
3. **Borrow checks** all references (we will cover this soon)
4. **Optimizes** the code
5. **Generates** a native machine code binary

If any check fails, no binary is produced. This means that if your code compiles, it is free of an entire class of bugs.

::: python Coming from Python
Think of it this way: in Python, `mypy script.py` checks types but `python script.py` runs regardless of type errors. In Rust, the type checker *is* the compiler — you cannot run code that fails type checking. This feels restrictive at first, but it means you spend less time debugging runtime errors and more time building features.
:::

## A slightly more interesting example

Let's write something that actually does something. Here is a program that takes a command-line argument and greets the user:

```rust
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let name = &args[1];
        println!("Hello, {}! Welcome to the coding agent.", name);
    } else {
        println!("Usage: hello-agent <name>");
    }
}
```

```bash
cargo run -- Alice
# Hello, Alice! Welcome to the coding agent.

cargo run
# Usage: hello-agent <name>
```

The `--` separates Cargo's arguments from your program's arguments. Everything after `--` is passed to your binary.

::: python Coming from Python
The equivalent Python would use `sys.argv`:
```python
import sys
if len(sys.argv) > 1:
    name = sys.argv[1]
    print(f"Hello, {name}! Welcome to the coding agent.")
else:
    print("Usage: hello-agent <name>")
```
The Rust version looks similar, but notice we had to specify the type `Vec<String>` for the collected arguments, and we used `&args[1]` (a reference) instead of just `args[1]`. These are concepts you will fully understand by the end of this chapter.
:::

## Key Takeaways

- Every Rust program starts at `fn main()` — there is no top-level code execution like Python
- `println!` is a macro (note the `!`), and its format syntax `{}` is similar to Python's `.format()` or f-strings
- Rust variables are immutable by default — use `let mut` when you need to reassign a value
- Rust compiles to a native binary before execution, catching type errors and other bugs at compile time rather than runtime
- `cargo run` combines compilation and execution into one command — use it during development for the fastest feedback loop
