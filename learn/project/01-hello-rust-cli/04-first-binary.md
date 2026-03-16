---
title: First Binary
description: Write, compile, and run your first Rust program — explore the main function, println! macros, and the compile-run cycle.
---

# First Binary

> **What you'll learn:**
> - How the `main` function serves as the entry point for every Rust binary
> - How to use `println!` and format strings for terminal output
> - How to compile and run your binary in both debug and release modes

You have Rust installed. You have Cargo ready. Now it is time to write actual Rust code, compile it, and watch it run. This subchapter walks you through the anatomy of a Rust program, explains the `println!` macro you will use constantly, and shows you the compile-run cycle that replaces Python's "just run the script" workflow.

## The Anatomy of main()

Open the `src/main.rs` file in the `kodai` project you created in the previous section (or create a fresh one with `cargo new kodai`). You should see:

```rust
fn main() {
    println!("Hello, world!");
}
```

This is a complete, runnable Rust program. Let's break it down:

- **`fn`** — the keyword to define a function (Python uses `def`)
- **`main`** — the function name. Every Rust binary must have exactly one `main` function. This is where execution starts.
- **`()`** — an empty parameter list. `main` takes no arguments (we handle CLI arguments differently — see subchapter 9).
- **`{ ... }`** — curly braces delimit the function body. Rust uses braces, not indentation, for blocks.
- **`println!("Hello, world!");`** — print a line to stdout. The `!` means this is a *macro*, not a regular function (more on that in a moment).
- **`;`** — semicolons terminate statements. This is one of the first things that will feel different coming from Python.

::: python Coming from Python
In Python, the entry point is typically `if __name__ == "__main__": main()`. In Rust, the `main` function in `src/main.rs` is *always* the entry point — no guard clause needed. The Rust compiler looks for this exact function when building a binary.

Also notice: no `def`, no colon, no indentation-based blocks. Rust uses `fn`, curly braces, and semicolons. It feels different at first, but you get used to it within a day or two.
:::

## The println! Macro

`println!` is the Rust way to print formatted text to stdout. The `!` at the end tells you it is a macro — a piece of code that expands at compile time. You do not need to understand macros deeply right now. Just know that `println!` works much like Python's `print()` with f-strings.

Here is a more interesting version. Replace the contents of `src/main.rs` with:

```rust
fn main() {
    let name = "Kodai";
    let version = 1;
    println!("Welcome to {name} v{version}!");
    println!("You are building a coding agent in Rust.");
    println!("Type /help for commands or /quit to exit.");
}
```

Run it:

```bash
cargo run
```

Output:

```
Welcome to Kodai v1!
You are building a coding agent in Rust.
Type /help for commands or /quit to exit.
```

### Format String Syntax

`println!` supports several formatting patterns:

```rust
fn main() {
    let language = "Rust";
    let year = 2015;

    // Inline variable names (like Python f-strings)
    println!("{language} was released in {year}");

    // Positional arguments
    println!("{0} is fast. {0} is safe.", language);

    // Debug formatting with {:?}
    let numbers = vec![1, 2, 3];
    println!("Numbers: {:?}", numbers);

    // Padding and alignment
    println!("{:<15} | {:>5}", "Language", "Year");
    println!("{:<15} | {:>5}", language, year);
}
```

Output:

```
Rust was released in 2015
Rust is fast. Rust is safe.
Numbers: [1, 2, 3]
Language        |  Year
Rust            |  2015
```

The `{:?}` formatter is particularly useful during development — it prints a debug representation of any type that implements the `Debug` trait. Think of it as Rust's equivalent of Python's `repr()`.

::: python Coming from Python
Rust's `println!("{name}")` is very similar to Python's `print(f"{name}")`. The main differences: (1) Rust uses `println!` with curly braces, Python uses `print()` with f-string prefix. (2) Rust's `{:?}` is like Python's `repr()`. (3) Rust's formatting happens at compile time, so typos in format strings are caught before you run the program — no more runtime `KeyError` from a mistyped f-string variable.
:::

## Other Print Macros

Rust provides several printing macros beyond `println!`:

```rust
fn main() {
    // println! — print with a newline
    println!("This ends with a newline");

    // print! — print without a newline
    print!("Enter your name: ");

    // eprintln! — print to stderr with a newline
    eprintln!("Warning: this goes to stderr");

    // eprint! — print to stderr without a newline
    eprint!("Error: ");
    eprintln!("something went wrong");
}
```

For our coding agent, `println!` handles normal output and `eprintln!` handles errors and diagnostics. This distinction matters: when someone pipes your agent's output to a file, error messages still appear on the terminal because they go to stderr.

## The Compile-Run Cycle

Unlike Python, where you type `python script.py` and see results immediately, Rust has a compile step. Here is what happens when you run `cargo run`:

```
Source code (main.rs)
      |
      v
   Compiler (rustc)  <-- checks types, ownership, lifetimes
      |
      v
   Machine code (target/debug/kodai)
      |
      v
   Execution
```

This extra step means you wait a moment for compilation, but you gain something powerful: the compiler catches bugs *before* your code runs. Type mismatches, unused variables, missing error handling, ownership violations — all caught at compile time.

During development, the debug build is optimized for compile speed, not runtime speed. It compiles quickly but includes extra debug information and skips performance optimizations. For production:

```bash
cargo build --release
./target/release/kodai
```

The release binary is significantly faster — often 10x or more for computation-heavy code — but takes longer to compile.

## Adding a Timestamp

Let's make the program slightly more interesting by adding a compile-time feature. Update `src/main.rs`:

```rust
fn main() {
    let agent_name = "Kodai";
    let version = env!("CARGO_PKG_VERSION");

    println!("  _  __            _       _ ");
    println!(" | |/ /  ___    __| | __ _ (_)");
    println!(" | ' /  / _ \\  / _` |/ _` || |");
    println!(" | . \\ | (_) || (_| | (_| || |");
    println!(" |_|\\_\\ \\___/  \\__,_|\\__,_||_|");
    println!();
    println!("{agent_name} v{version} — Your CLI Coding Agent");
    println!("Type /help for commands or /quit to exit.");
}
```

Run it with `cargo run`. The `env!("CARGO_PKG_VERSION")` macro reads the version from your `Cargo.toml` at compile time. This is a common Rust pattern — you embed metadata into the binary during compilation.

## Experimenting with Compiler Errors

One of Rust's greatest strengths is its compiler. Let's see it in action. Try this intentionally broken code:

```rust
fn main() {
    let x = 5;
    let y = "hello";
    let z = x + y;
    println!("{z}");
}
```

`cargo run` produces:

```
error[E0277]: cannot add `&str` to `{integer}`
 --> src/main.rs:4:15
  |
4 |     let z = x + y;
  |               ^ no implementation for `{integer} + &str`
  |
  = help: the trait `Add<&str>` is not implemented for `{integer}`
```

The compiler tells you exactly where the error is, what went wrong, and why. In Python, you would get a `TypeError` at runtime. In Rust, this is caught before the program ever runs.

Try another one — an unused variable:

```rust
fn main() {
    let x = 42;
    println!("Hello!");
}
```

This compiles, but you get a warning:

```
warning: unused variable: `x`
 --> src/main.rs:2:9
  |
2 |     let x = 42;
  |         ^ help: if this is intentional, prefix it with an underscore: `_x`
```

Rust warns you about unused code and even suggests a fix. These warnings might feel noisy at first, but they catch real bugs in larger programs. A variable you declared but never used is often a sign that you forgot to finish implementing something.

## Putting It Together

Here is the `src/main.rs` we will carry forward:

```rust
fn main() {
    let agent_name = "Kodai";
    let version = env!("CARGO_PKG_VERSION");

    println!("{agent_name} v{version}");
    println!("Type /help for commands or /quit to exit.");
    println!();
    println!("Ready.");
}
```

Run `cargo run` one more time and verify it works. You now have a Rust binary that compiles and runs. In the next subchapter, you will learn how to organize this project as it grows.

## Key Takeaways

- Every Rust binary has a `main` function in `src/main.rs` — this is the entry point, no guard clauses needed.
- `println!` with `{}` placeholders is Rust's equivalent of Python's `print()` with f-strings, but formatting errors are caught at compile time.
- The compile-run cycle (`cargo run`) adds a step compared to Python, but in exchange the compiler catches type errors, unused variables, and many other bugs before your code ever executes.
- `cargo run` uses a debug build (fast to compile, slow to run). `cargo build --release` produces an optimized binary for production.
- Lean into compiler errors — they are detailed, helpful, and often suggest the exact fix you need.
