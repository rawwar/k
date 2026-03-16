---
title: Why Rust
description: Understand why Rust is an excellent choice for building reliable, fast command-line tools and coding agents — especially if you're coming from Python.
---

# Why Rust

> **What you'll learn:**
> - The key advantages Rust offers over Python, Go, and C++ for CLI tool development
> - How Rust's ownership model eliminates entire categories of bugs at compile time
> - Why Rust's performance characteristics matter for an interactive coding agent

You already know Python. It is a fantastic language for scripting, data science, and rapid prototyping. So why would you invest months learning Rust to build a coding agent? The short answer: Rust gives you the speed of C, the safety of a garbage-collected language, and the developer experience of a modern toolchain — all at once. For a CLI tool that spawns processes, streams LLM responses, and manages file I/O concurrently, those properties are not luxuries. They are necessities.

Let's unpack that.

## Speed Without Sacrifice

Python is an interpreted language. Every time you run a Python script, the interpreter reads your source code, compiles it to bytecode, and executes it on a virtual machine. This is convenient — you get a fast edit-run cycle — but it comes with a performance tax. CPU-intensive operations in Python can be 10x to 100x slower than their compiled equivalents.

Rust compiles directly to native machine code, just like C and C++. When you build your coding agent in Rust and run it, there is no interpreter in the middle. The binary talks directly to your operating system.

Why does this matter for a coding agent? Consider what happens in a single iteration of the agent loop:

1. Read user input from the terminal
2. Send an HTTP request to an LLM API
3. Stream the response tokens back in real-time
4. Parse tool-call JSON from the response
5. Spawn a child process to execute a shell command
6. Capture stdout and stderr
7. Feed the results back into the next API call

Steps 3 and 5 are particularly performance-sensitive. Streaming tokens to the terminal must feel instantaneous. Spawning processes must have minimal overhead. In Python, you can accomplish all of this — but you end up fighting the GIL (Global Interpreter Lock) when you try to do multiple things concurrently. In Rust, true parallelism is the default, and the overhead is negligible.

::: python Coming from Python
In Python, you might use `asyncio` to handle concurrent I/O and `subprocess` to spawn processes. This works, but the GIL means only one thread runs Python code at a time. Rust has no GIL — its `async`/`await` model (powered by runtimes like Tokio) gives you genuine concurrency without that bottleneck. Your agent can stream LLM output, watch for file changes, and run shell commands truly in parallel.
:::

## Safety as a Compile-Time Guarantee

Here is a Python function with a subtle bug:

```python
def get_first_word(text):
    words = text.split()
    return words[0]  # IndexError if text is empty
```

This code works perfectly — until someone passes an empty string. In Python, you discover this bug at runtime, possibly in production. The fix is simple (add an `if` check or use `try/except`), but the language does not *force* you to handle the edge case.

Rust takes a fundamentally different approach. The type system and ownership model turn large categories of runtime errors into compile-time errors. You literally cannot build the program until you handle the edge cases.

Here is the Rust equivalent:

```rust
fn get_first_word(text: &str) -> Option<&str> {
    text.split_whitespace().next()
}

fn main() {
    let result = get_first_word("hello world");
    match result {
        Some(word) => println!("First word: {word}"),
        None => println!("No words found"),
    }
}
```

The return type `Option<&str>` explicitly communicates that this function might not return a value. The compiler forces you to handle both cases (`Some` and `None`) before the code will compile. There is no equivalent of Python's `IndexError` lurking at runtime.

For a coding agent, this matters enormously. Your agent executes shell commands, reads and writes files, makes network requests — each of these can fail in dozens of ways. Rust's type system ensures you think about every failure mode before the binary ships.

## The Ownership Model — Your New Superpower

Rust's most distinctive feature is its ownership system. Every value in Rust has exactly one owner, and when that owner goes out of scope, the value is dropped (freed). This eliminates:

- **Memory leaks** — values are always freed when no longer needed
- **Use-after-free bugs** — the compiler prevents accessing freed memory
- **Data races** — the borrow checker prevents multiple mutable references to the same data

These guarantees come with zero runtime cost. There is no garbage collector pausing your program to sweep memory. The compiler does all the work at compile time.

You do not need to understand ownership deeply right now — we cover it progressively throughout the book. The important thing is that ownership is *why* Rust can be both fast and safe simultaneously.

::: python Coming from Python
Python manages memory with reference counting plus a cycle-detecting garbage collector. You never think about memory — which is great for scripting, but the GC can introduce unpredictable pauses. Rust's ownership model means memory is freed deterministically, exactly when it goes out of scope. No GC pauses, no memory leaks, no surprises.
:::

## Why Not Go or C++?

**Go** is a solid choice for CLI tools — it compiles to native code, has a great standard library, and its goroutine model makes concurrency easy. However, Go lacks Rust's compile-time safety guarantees. Nil pointer dereferences are a runtime panic in Go. Error handling is idiomatic but not enforced by the type system — you can silently ignore an error by assigning it to `_`. Go is simpler than Rust, but that simplicity comes at the cost of the guarantees that make Rust so reliable for long-running tools.

**C++** gives you similar performance to Rust, but memory safety is entirely your responsibility. Modern C++ (C++17, C++20) has smart pointers and RAII that help, but the language does not prevent you from writing unsafe code. A coding agent that spawns processes and manages file I/O is exactly the kind of program where C++ memory bugs thrive.

Rust sits in a unique position: it gives you the performance of C++, stronger safety than Go, and a modern toolchain that rivals either.

## Real-World Validation

This is not a theoretical argument. Production coding agents are being built in compiled languages for exactly these reasons:

::: wild In the Wild
Claude Code, Anthropic's coding agent, is built as a CLI tool that manages concurrent streams, process execution, and complex state. OpenCode, an open-source coding agent, is implemented in Go and leverages compiled-language performance for responsive terminal UI. The trend across the industry is clear: coding agents that need to feel fast, handle errors reliably, and manage system resources benefit enormously from compiled languages. Rust takes this a step further by catching entire categories of bugs at compile time.
:::

## The Developer Experience

One more thing that might surprise you: Rust has one of the best developer experiences of any compiled language.

- **The compiler's error messages are legendary.** When your code does not compile, `rustc` tells you exactly what went wrong, why, and often suggests a fix. Coming from Python's runtime tracebacks, you will find Rust's compile-time feedback refreshing.
- **Cargo** — the build tool and package manager — handles dependency management, building, testing, formatting, and linting in a single tool. It is like having `pip`, `pytest`, `black`, `mypy`, and `make` rolled into one.
- **The ecosystem is mature.** Crates (Rust's packages) exist for HTTP clients, CLI argument parsing, async runtimes, terminal UI, and everything else you need for a coding agent.

You will meet all of these tools in this chapter.

## Key Takeaways

- Rust compiles to native machine code, giving you C-level performance without a garbage collector or interpreter overhead — critical for a responsive coding agent.
- The ownership model and type system turn memory bugs, null pointer errors, and unhandled failures into compile-time errors, making your agent more reliable before it ever runs.
- Rust's tooling (Cargo, rust-analyzer, the compiler's error messages) provides a developer experience that rivals Python's — you are not giving up convenience for performance.
- For CLI tools that manage concurrent I/O, spawn processes, and handle complex error paths, Rust's safety guarantees are not academic — they prevent the bugs that matter most in production.
