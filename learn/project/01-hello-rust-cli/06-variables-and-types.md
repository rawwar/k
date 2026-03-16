---
title: Variables and Types
description: Master Rust's type system fundamentals including immutability by default, scalar types, strings, and a first look at ownership.
---

# Variables and Types

> **What you'll learn:**
> - How `let`, `let mut`, and `const` differ and why immutability is the default in Rust
> - The distinction between `String` and `&str` and when to use each
> - How Rust's ownership and borrowing rules prevent data races and dangling references

Rust's type system is your most powerful ally. Coming from Python, where variables can hold any value and types are checked at runtime, Rust feels strict at first. Every variable has a fixed type, immutability is the default, and the compiler tracks who owns each piece of data. These constraints are not obstacles — they are guardrails that prevent bugs before your code ever runs.

## Variables with let

In Rust, you declare variables with `let`:

```rust
fn main() {
    let name = "Kodai";
    let version = 1;
    let verbose = true;

    println!("Agent: {name}, version: {version}, verbose: {verbose}");
}
```

Rust **infers** the type of each variable from the value you assign. You can also annotate the type explicitly:

```rust
fn main() {
    let name: &str = "Kodai";
    let version: i32 = 1;
    let verbose: bool = true;

    println!("Agent: {name}, version: {version}, verbose: {verbose}");
}
```

Both forms are equivalent. Type annotations are optional when the compiler can infer the type, but they are useful for clarity — especially when reading code later.

## Immutability by Default

Here is the single biggest difference from Python: **variables in Rust are immutable by default**.

```rust
fn main() {
    let x = 5;
    x = 10;  // ERROR: cannot assign twice to immutable variable
    println!("{x}");
}
```

This does not compile. To make a variable mutable, you must explicitly say so with `mut`:

```rust
fn main() {
    let mut x = 5;
    println!("x is {x}");
    x = 10;
    println!("x is now {x}");
}
```

Output:

```
x is 5
x is now 10
```

Why does Rust do this? Immutable-by-default prevents accidental state changes. When you see `let x = ...`, you know that `x` never changes — you can reason about the code without tracking mutations. When you see `let mut x = ...`, it signals that this value *will* change, so you read that code more carefully.

::: python Coming from Python
In Python, all variables are mutable by default. There is no `const` or `final` keyword (the `Final` type hint exists but is not enforced at runtime). In Rust, immutability is the default and you opt into mutability with `mut`. This is a deliberate design choice: most variables do not need to change, so Rust makes the safe choice the easy one.
:::

## Constants

For values that are truly fixed at compile time, use `const`:

```rust
const MAX_RETRIES: u32 = 3;
const AGENT_NAME: &str = "Kodai";

fn main() {
    println!("{AGENT_NAME} will retry up to {MAX_RETRIES} times");
}
```

Constants differ from `let` bindings in two ways:
- They must have an explicit type annotation
- Their value must be computable at compile time (no function calls)

Use `const` for configuration values that never change. Use `let` for everything else.

## Scalar Types

Rust has four categories of scalar (single-value) types:

### Integers

| Type | Size | Range | Python equivalent |
|------|------|-------|-------------------|
| `i8` | 8-bit | -128 to 127 | `int` (bounded) |
| `i16` | 16-bit | -32,768 to 32,767 | `int` (bounded) |
| `i32` | 32-bit | -2B to 2B | `int` (bounded) |
| `i64` | 64-bit | very large | `int` (bounded) |
| `u8` | 8-bit | 0 to 255 | `int` (bounded) |
| `u32` | 32-bit | 0 to 4B | `int` (bounded) |
| `usize` | pointer-sized | platform-dependent | `int` for indexing |

The default integer type is `i32`. Use `usize` for collection indices and sizes.

### Floating point, boolean, character

```rust
fn main() {
    let pi: f64 = 3.14159;       // 64-bit float (default)
    let active: bool = true;      // boolean
    let emoji: char = '🦀';       // Unicode character (4 bytes)

    println!("pi={pi}, active={active}, emoji={emoji}");
}
```

Rust's `char` is a full Unicode scalar value (4 bytes), not a single byte like in C.

## Strings: String vs. &str

Strings are where most Python developers first feel Rust's complexity. Rust has two main string types:

| Type | What It Is | Equivalent |
|------|-----------|------------|
| `String` | Owned, heap-allocated, growable | Python's `str` (sort of) |
| `&str` | Borrowed reference to string data | A read-only view into a `String` |

```rust
fn main() {
    // &str — a string literal, embedded in the binary
    let greeting: &str = "Hello";

    // String — a heap-allocated, owned string
    let mut name = String::from("world");

    // You can grow a String
    name.push_str("!");

    println!("{greeting}, {name}");
}
```

The critical distinction: a `String` owns its data. An `&str` borrows data from somewhere else (a `String`, a string literal, etc.). This matters because of Rust's ownership rules.

Here is a practical example for our coding agent:

```rust
fn format_prompt(username: &str) -> String {
    format!("{username}> ")
}

fn main() {
    let prompt = format_prompt("dev");
    println!("Prompt: {prompt}");
}
```

The function takes an `&str` (it only needs to *read* the name) and returns a `String` (it creates new, owned data). This pattern — borrow inputs, return owned data — is extremely common in Rust.

::: python Coming from Python
In Python, `str` is a single type that handles all string operations. You never think about who "owns" a string. In Rust, the split between `String` (owned) and `&str` (borrowed) reflects a real distinction: does this code own the string data, or is it just looking at someone else's data? This feels annoying at first, but it prevents a whole class of bugs where one part of your program frees memory that another part is still using.

A good rule of thumb for beginners: accept `&str` in function parameters (maximum flexibility) and return `String` from functions (caller owns the result).
:::

## Collections: Vec, HashMap

Rust's standard library includes the collections you know from Python:

```rust
use std::collections::HashMap;

fn main() {
    // Vec<T> — like Python's list
    let mut commands: Vec<String> = Vec::new();
    commands.push(String::from("/help"));
    commands.push(String::from("/quit"));
    commands.push(String::from("/clear"));

    println!("Available commands: {:?}", commands);
    println!("First command: {}", commands[0]);

    // HashMap<K, V> — like Python's dict
    let mut config: HashMap<&str, &str> = HashMap::new();
    config.insert("model", "claude-sonnet");
    config.insert("temperature", "0.7");

    println!("Config: {:?}", config);

    if let Some(model) = config.get("model") {
        println!("Using model: {model}");
    }
}
```

Output:

```
Available commands: ["/help", "/quit", "/clear"]
First command: /help
Config: {"temperature": "0.7", "model": "claude-sonnet"}
Using model: claude-sonnet
```

Notice that `HashMap::get` returns an `Option<&V>`, not the value directly. If the key does not exist, you get `None` instead of a `KeyError`. Rust forces you to handle the missing-key case at compile time.

## Ownership: The Big Idea

Ownership is Rust's core innovation. Here is the rule in one sentence: **every value has exactly one owner, and when the owner goes out of scope, the value is dropped**.

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = s1;  // s1's ownership MOVES to s2

    // println!("{s1}");  // ERROR: s1 is no longer valid
    println!("{s2}");     // OK: s2 is the owner now
}
```

When you assign `s1` to `s2`, Rust does not copy the string data. It *moves* ownership. After the move, `s1` is no longer valid — trying to use it is a compile error.

This prevents a dangerous class of bugs: two parts of your program thinking they own the same data and both trying to free it. In C, this is a double-free bug. In Rust, it is a compile error.

If you actually need a copy, use `.clone()`:

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = s1.clone();  // Deep copy

    println!("s1: {s1}");  // Both valid
    println!("s2: {s2}");
}
```

For simple types like integers and booleans, Rust copies automatically — no move:

```rust
fn main() {
    let x = 5;
    let y = x;  // x is copied, not moved
    println!("x={x}, y={y}");  // Both valid
}
```

You will deepen your understanding of ownership throughout this book. For now, remember: when the compiler says "value used here after move," it is protecting you from a real bug. Use `.clone()` when you need a copy, and prefer borrowing (`&`) when you only need to read.

## Key Takeaways

- Variables are **immutable by default** in Rust. Use `let mut` when you need to change a value — this signals intent clearly.
- Rust has two string types: `String` (owned, growable) and `&str` (borrowed reference). Accept `&str` in function parameters and return `String` from functions.
- Ownership means every value has one owner. Assigning a `String` to another variable *moves* it — the original variable becomes invalid. Use `.clone()` for copies.
- Scalar types (`i32`, `f64`, `bool`, `char`) are copied on assignment. Heap-allocated types (`String`, `Vec`) are moved.
- `HashMap::get` returns `Option`, not the value directly — Rust forces you to handle the missing-key case at compile time rather than crashing with a `KeyError` at runtime.
