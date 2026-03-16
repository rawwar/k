---
title: Why Rust
description: The case for Rust as a systems language for building coding agents, focusing on safety, performance, and developer experience.
---

# Why Rust

> **What you'll learn:**
> - Why Rust's compile-time safety guarantees reduce runtime bugs in long-running agent processes
> - How Rust's performance characteristics benefit latency-sensitive agent operations like file I/O and streaming
> - The ecosystem advantages Rust offers for building CLI tools and networked applications

You already know Python. It is an extraordinary language — readable, productive, and backed by a massive ecosystem. So why learn Rust? Why not build the coding agent in Python?

The honest answer is: you *could* build a coding agent in Python. Several production agents are written in TypeScript or Go. But Rust offers a specific combination of properties that makes it uniquely well-suited for this kind of project, and learning it will fundamentally level up your understanding of how software really works.

## The problem with runtime errors

In Python, bugs show up when your code runs. A typo in a variable name? `NameError` at runtime. Passed the wrong type to a function? You might not find out until that specific code path executes in production. Python's flexibility — the same feature that makes it fast to prototype — means the interpreter trusts you to get things right.

```python
def process_message(message):
    # This typo won't be caught until this function actually runs
    return message.cotent  # AttributeError: 'str' has no attribute 'cotent'
```

For a coding agent that runs for hours, processes hundreds of LLM responses, and executes shell commands on a user's machine, runtime errors are not just inconvenient — they are dangerous. An unhandled exception could leave a subprocess running, corrupt file state, or silently drop a user's work.

Rust takes a different approach. The compiler checks your code *before* it runs. If it compiles, an entire class of bugs — null pointer dereferences, data races, type mismatches, use-after-free — simply cannot happen.

```rust
fn process_message(message: &str) -> &str {
    // This typo is a compile-time error — the program never runs with this bug
    message.cotent  // error[E0609]: no field `cotent` on type `&str`
}
```

::: python Coming from Python
In Python, you might use mypy or pyright for static type checking, and they catch many bugs. But they are optional, not enforced by the language, and they cannot catch memory safety issues. Rust's compiler is like mypy on steroids — it is not optional, covers far more ground, and the guarantees are absolute rather than best-effort.
:::

## Performance that matters

Python is slow. Not in a way that matters for most web applications — Django serves millions of requests behind a good web server. But for a coding agent, performance bottlenecks show up in specific places:

- **File I/O and directory traversal** — scanning large codebases to gather context
- **String processing** — parsing LLM responses, diffing files, building prompts
- **Concurrent operations** — streaming responses while watching for tool calls while waiting for subprocess output
- **Startup time** — a CLI tool that takes 500ms to start feels sluggish; one that starts in 5ms feels instant

Rust compiles to native machine code with no runtime overhead. There is no interpreter startup, no garbage collector pausing to clean up memory, no Global Interpreter Lock (GIL) preventing true parallelism. A Rust binary starts instantly and runs at speeds comparable to C.

::: python Coming from Python
Python's GIL means that even with multiple threads, only one thread executes Python bytecode at a time. You work around this with multiprocessing (expensive process spawning) or asyncio (cooperative concurrency, but still single-threaded execution). Rust gives you true parallelism with threads *and* async concurrency, with compile-time guarantees that you will not hit data races.
:::

## Memory safety without garbage collection

Python manages memory through reference counting plus a cycle-detecting garbage collector. You never think about memory — objects are created, used, and eventually cleaned up. This is enormously convenient, but it comes with costs: GC pauses, unpredictable memory usage, and no control over when resources are released.

Rust has no garbage collector. Instead, it uses an *ownership system* checked at compile time. Every value has exactly one owner. When the owner goes out of scope, the value is immediately dropped and its resources are freed. No GC pauses, no memory leaks, completely deterministic resource management.

This matters for a coding agent because:
- **Long-running sessions** accumulate state — conversation history, file contents, tool results. Predictable memory management prevents gradual memory bloat.
- **File handles and network connections** are released immediately when they go out of scope, rather than waiting for a garbage collection cycle.
- **No GC pauses** means consistent latency in streaming responses back to the user.

You will learn exactly how ownership works in the [Variables and Ownership](/linear/02-rust-for-python-developers/05-variables-ownership-vs-python-gc) section. For now, just understand that Rust gives you the safety of a garbage-collected language with the performance of manual memory management.

## A type system that helps instead of hinders

If your experience with type systems is limited to Python's type hints, you might associate types with extra ceremony that slows you down. Rust's type system is different — it actively *helps* you write correct code.

Rust has no `null`. Instead, values that might be absent use `Option<T>`, and the compiler forces you to handle both the `Some` and `None` cases. Rust has no exceptions. Instead, fallible operations return `Result<T, E>`, and the compiler forces you to handle both success and failure. This means entire categories of bugs that plague Python applications — `None` dereferences and unhandled exceptions — are structurally impossible in Rust.

```rust
// The compiler won't let you use this value without handling the None case
fn find_user(id: u64) -> Option<User> {
    // ...
}

// You must explicitly handle both possibilities
match find_user(42) {
    Some(user) => println!("Found: {}", user.name),
    None => println!("User not found"),
}
```

::: python Coming from Python
This is like having every function annotated with `Optional[User]` and having the type checker *refuse to compile* if you try to access `.name` without first checking for `None`. Imagine if Python enforced that — how many `AttributeError: 'NoneType' has no attribute` bugs would vanish from your codebase?
:::

## The Rust ecosystem for CLI and networking

Rust has an excellent ecosystem for exactly the kind of work a coding agent does:

- **clap** — command-line argument parsing (think argparse, but with compile-time validation)
- **tokio** — async runtime for networking and I/O (think asyncio, but with true concurrency)
- **reqwest** — HTTP client (think requests, but async-native)
- **serde** — serialization framework (think json module, but for any format with compile-time type checking)
- **ratatui** — terminal UI framework (think rich/textual, but zero-overhead)

These are not niche libraries. They are battle-tested by thousands of production applications. The Rust ecosystem is smaller than Python's, but for CLI tools, networking, and systems programming, it is mature and well-maintained.

::: wild In the Wild
Several production coding agents are built on these exact crates. Claude Code uses a TypeScript stack, but Rust-based CLI tools like ripgrep, bat, and fd demonstrate that the Rust ecosystem excels at building fast, reliable command-line applications. OpenCode is written in Go, another compiled language that values similar properties — the trend in agent development is clearly toward compiled, type-safe languages.
:::

## Why not just C or C++?

If performance and control are the goal, why not C or C++? Because they give you the power to shoot yourself in the foot. Buffer overflows, use-after-free, dangling pointers, data races — these bugs cause security vulnerabilities and crashes that are notoriously hard to track down.

Rust gives you the same performance as C/C++ but prevents these bugs at compile time. You get the control of a systems language with the safety guarantees you are used to from Python (and more). This is why Rust has been voted the "most admired" programming language in the Stack Overflow survey for years running.

## What you are signing up for

Rust has a learning curve. The compiler will reject code that Python would happily run. You will spend time fighting the borrow checker. Concepts like lifetimes and trait bounds will feel alien at first.

But here is the thing: the Rust compiler is the most helpful compiler you will ever use. Its error messages explain *what* went wrong, *why*, and often *how to fix it*. Every fight with the compiler teaches you something about memory safety, concurrency, or software design that you will carry back to every language you write.

This chapter exists to make that learning curve as gentle as possible by always connecting back to what you already know from Python. Let's start by getting Rust installed.

## Key Takeaways

- Rust catches bugs at compile time that Python only reveals at runtime, including null dereferences, type mismatches, and data races
- Rust compiles to native code with no garbage collector, giving consistent performance and predictable memory usage — critical for long-running agent processes
- The ownership system replaces garbage collection with compile-time checks, giving you safety without runtime overhead
- Rust's type system with `Option` and `Result` eliminates entire categories of bugs by forcing explicit handling of absence and failure
- The Rust ecosystem (tokio, serde, clap, reqwest) is mature and well-suited for building CLI tools and networked applications
