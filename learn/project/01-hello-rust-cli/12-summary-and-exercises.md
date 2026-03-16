---
title: Summary and Exercises
description: Review the core Rust and CLI concepts from Chapter 1 and solidify your understanding with hands-on exercises.
---

# Summary and Exercises

> **What you'll learn:**
> - How all the Chapter 1 concepts connect to form the foundation of your coding agent
> - How to extend the REPL with new commands and features as a self-directed exercise
> - How to diagnose common beginner mistakes using the Rust compiler's error messages

You have come a long way in this chapter. You started with zero Rust knowledge and now have a working interactive REPL — a real program you can compile, run, and interact with. Let's review what you learned, connect the pieces, and then challenge you with exercises that reinforce each concept.

## What You Built

Here is a bird's-eye view of the coding agent skeleton you built in this chapter:

```
kodai/
  Cargo.toml              # Project manifest with clap and rustyline
  Cargo.lock              # Pinned dependency versions
  src/
    main.rs               # CLI parsing, REPL loop, and command handlers
```

The code snapshot keeps everything in a single `main.rs` for clarity -- you can see the entire program at a glance. In a production codebase, you would split this into modules (`lib.rs`, `commands.rs`, etc.), but for learning, one file makes it easier to follow the flow from CLI parsing through command dispatch to user interaction.

This program:

1. **Parses CLI arguments** with `clap` — supports `--verbose`, `--model`, and an optional prompt
2. **Starts an interactive REPL** using `rustyline` for line editing and history
3. **Dispatches commands** — `/help`, `/quit`, `/clear`, and user messages
4. **Handles errors gracefully** — EOF, Ctrl+C, invalid commands all produce clean responses
5. **Persists history** — previous commands survive across sessions

## Concept Map

Here is how every concept from this chapter connects:

```
                    Chapter 1 Concept Map
  ┌──────────────────────────────────────────────────┐
  │                                                  │
  │  Cargo.toml          cargo run                   │
  │  (dependencies) ───→ (build + execute)           │
  │       │                    │                     │
  │       v                    v                     │
  │  clap, rustyline     src/main.rs                 │
  │  (crate ecosystem)   (entry point)               │
  │                            │                     │
  │              ┌─────────────┼─────────────┐       │
  │              v             v             v       │
  │         CLI Parsing    REPL Loop    Modules      │
  │         (clap derive)  (loop +      (mod, pub,   │
  │              │          match)       use)         │
  │              v             │                     │
  │         Struct types       v                     │
  │         (Variables    Command                    │
  │          & Types)     Dispatch                   │
  │                       (Functions                 │
  │                        + Enums)                  │
  │                            │                     │
  │                            v                     │
  │                      Error Handling              │
  │                      (Result, Option, ?)         │
  │                            │                     │
  │                            v                     │
  │                      User Input                  │
  │                      (stdin, rustyline)           │
  └──────────────────────────────────────────────────┘
```

Every topic builds on the ones before it. CLI parsing requires understanding structs and types. The REPL loop uses functions, pattern matching, and error handling. Modules organize everything into a maintainable structure.

## Key Rust Concepts Summary

### From Python to Rust: Quick Reference

| Python | Rust | Notes |
|--------|------|-------|
| `def func():` | `fn func()` | `fn` keyword, curly braces for body |
| `x = 5` | `let x = 5;` | Immutable by default |
| `x = 5` (mutable) | `let mut x = 5;` | Explicit mutability |
| `str` | `String` / `&str` | Owned vs. borrowed |
| `list` | `Vec<T>` | Type parameter required |
| `dict` | `HashMap<K, V>` | Type parameters required |
| `None` | `Option::None` | Part of the `Option<T>` enum |
| `try/except` | `Result<T, E>` + `?` | Compile-time error checking |
| `import module` | `mod module;` + `use` | Explicit declaration required |
| `pip install` | `cargo add` | Single tool for everything |
| `python script.py` | `cargo run` | Compile then execute |
| `pytest` | `cargo test` | Built into the toolchain |
| `black .` | `cargo fmt` | Built into the toolchain |
| `pylint` | `cargo clippy` | Built into the toolchain |

::: python Coming from Python
The biggest mindset shift is not any single syntax difference — it is that Rust's compiler is your collaborator. In Python, you run code to find bugs. In Rust, you talk to the compiler. When `cargo check` shows errors, read them carefully — they almost always tell you exactly what to fix. Over time, you will start thinking of the compiler as a pair programmer that catches your mistakes before they reach production.
:::

## Common Beginner Mistakes

Here are the errors you are most likely to encounter in your first week of Rust, with explanations and fixes.

### 1. Forgetting `mut`

```
error[E0384]: cannot assign twice to immutable variable `x`
 --> src/main.rs:3:5
  |
2 |     let x = 5;
  |         - first assignment
3 |     x = 10;
  |     ^^^^^^ cannot assign twice to immutable variable
  |
help: consider making this binding mutable
  |
2 |     let mut x = 5;
  |         +++
```

**Fix:** Add `mut` if you need to change the variable. If you do not, that is the compiler telling you the variable should not be changed.

### 2. Using a moved value

```
error[E0382]: borrow of moved value: `s1`
 --> src/main.rs:4:20
  |
2 |     let s1 = String::from("hello");
  |         -- move occurs because `s1` has type `String`
3 |     let s2 = s1;
  |              -- value moved here
4 |     println!("{s1}");
  |               ^^ value borrowed here after move
```

**Fix:** Use `.clone()` to create a copy, or use a reference (`&s1`) instead of moving ownership.

### 3. Missing semicolon (or extra semicolon)

```
error[E0308]: mismatched types
 --> src/main.rs:2:5
  |
1 | fn add(a: i32, b: i32) -> i32 {
  |                            --- expected `i32` because of return type
2 |     a + b;
  |          ^ expected `i32`, found `()`
```

**Fix:** Remove the semicolon from the last expression to return it. `a + b;` is a statement (returns nothing). `a + b` is an expression (returns the sum).

### 4. Type mismatch with &str and String

```
error[E0308]: mismatched types
 --> src/main.rs:5:16
  |
5 |     takes_string(name);
  |                  ^^^^
  |                  expected `String`, found `&str`
```

**Fix:** Use `.to_string()` or `String::from()` to convert `&str` to `String`. Use `&` to borrow a `String` as `&str`.

### 5. Unused Result warning

```
warning: unused `Result` that must be used
 --> src/main.rs:3:5
  |
3 |     fs::write("output.txt", "hello");
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: this `Result` may be an `Err` variant, which should be handled
```

**Fix:** Handle the `Result` with `?`, `.expect()`, or `match`. Do not silently ignore it.

## Exercises

Practice each concept with these exercises. They are ordered by difficulty and build on the REPL you created.

### Exercise 1: Add a /history Command (Easy)

Add a `/history` built-in command that prints the last 10 commands the user entered. You need to store commands in a `Vec<String>` and pass it to the command handler.

**Hints:**
- Declare `let mut history: Vec<String> = Vec::new();` before the REPL loop
- Push each command into `history` before processing it
- In the handler, use `.iter().rev().take(10)` to get the last 10 entries

### Exercise 2: Add a /echo Command with Arguments (Easy)

Add a `/echo` command that echoes back everything after `/echo`:

```
kodai> /echo Hello, world!
Hello, world!
```

**Hints:**
- In `handle_builtin`, match on `cmd` that starts with `"echo "` using `cmd.strip_prefix("echo ")`
- Return the remaining text as the response

### Exercise 3: Add a /count Command (Medium)

Add a `/count` command that shows how many messages the user has sent (not counting built-in commands). Track this with a `u32` counter.

**Hints:**
- Declare `let mut message_count: u32 = 0;` before the loop
- Increment it only when `handle_command` processes a user message (not a `/`-prefixed command)
- You may need to change your function signatures or use a return value to signal whether the input was a user message

### Exercise 4: Multi-Line Input (Medium)

Add support for multi-line input. When a line ends with `\`, continue reading the next line and concatenate them:

```
kodai> This is a long \
...>   multi-line message
You said: This is a long multi-line message
```

**Hints:**
- After reading a line, check if it ends with `\` using `.ends_with('\\')`
- If it does, change the prompt to `"...> "` and keep reading
- Accumulate lines in a `String` using `.push_str()`
- Trim the trailing `\` from each continuation line

### Exercise 5: Command Aliases with a HashMap (Hard)

Implement a `/alias` command that lets users create shortcuts:

```
kodai> /alias h /help
Alias created: h -> /help
kodai> h
Available commands: ...
```

**Hints:**
- Store aliases in a `HashMap<String, String>`
- Before processing input, check if it matches an alias key
- If it does, replace the input with the alias value and process that instead
- Handle edge cases: aliasing to another alias, aliasing a built-in command name

### Exercise 6: Input Validation and Error Types (Hard)

Create a custom `InputError` enum with variants for different invalid input scenarios (empty input, input too long, invalid UTF-8). Use `Result<CommandResult, InputError>` as the return type for your command handler instead of returning `CommandResult` directly.

**Hints:**
- Define `enum InputError { Empty, TooLong(usize), InvalidCommand(String) }`
- Implement `Display` for `InputError`
- Change `handle_command` to return `Result<CommandResult, InputError>`
- Handle the `Result` in the REPL loop with a `match`

## What Comes Next

In Chapter 2, you replace the echo response in `handle_user_message()` with a real LLM API call. You learn how to:

- Make HTTP requests with the `reqwest` crate
- Serialize and deserialize JSON with `serde`
- Handle API errors and rate limits
- Stream responses token by token to the terminal

The REPL you built here is the foundation. Every new feature plugs into this structure — the loop stays the same, the command handlers grow, and the agent becomes more capable with each chapter.

## Key Takeaways

- You built a complete, working CLI application in Rust: a REPL with argument parsing, command dispatch, error handling, line editing, and persistent history.
- The Rust compiler is your best debugging tool. Read its error messages carefully — they point directly to the problem and usually suggest the fix.
- The key Rust concepts you learned — immutability, ownership, `Result`/`Option`, modules, and enums — are the building blocks for everything that follows.
- Your REPL's architecture (loop + command dispatch + separated handlers) is the same architecture used by production coding agents. You are building on a real foundation.
- Practice the exercises to internalize these concepts. Each one isolates a specific skill (collections, string processing, error handling, HashMap) that you need in upcoming chapters.
