---
title: Choosing a Language
description: Why Rust is an excellent choice for building a coding agent, and how it compares to TypeScript, Python, and Go for this use case.
---

# Choosing a Language

> **What you'll learn:**
> - The specific properties that make a language well-suited for building coding agents
> - How Rust's type system, performance, and error handling benefit agent development
> - Trade-offs between Rust, TypeScript, Python, and Go for agent implementation

## The Language Matters More Than You Think

When you're building a coding agent, the implementation language isn't just a preference — it shapes the architecture, determines what guarantees you get at compile time versus runtime, affects performance characteristics, and influences how you think about error handling, concurrency, and system interaction.

Every major coding agent has made a deliberate language choice. Claude Code is built with TypeScript. OpenCode chose Go. Pi chose Rust. Codex uses a mix of TypeScript and Python. Each choice reflects the team's priorities and produces a different set of trade-offs.

We're choosing Rust. Let's examine why, starting with what properties matter most for this use case, and then comparing the leading candidates.

## What Properties Matter for Agent Development

Before comparing languages, let's identify the properties that matter most when building a coding agent:

**Reliable error handling.** An agent runs autonomously for extended periods, executing dozens of operations per task. A single unhandled error can crash the session, losing the entire conversation context. You need a language that makes error paths explicit and hard to ignore.

**Good async I/O.** Agents spend most of their time waiting — waiting for the LLM API to respond, waiting for shell commands to complete, waiting for file I/O. Async I/O lets the agent handle these waits efficiently, enabling streaming, timeouts, and concurrent tool execution.

**Strong type system.** The data flowing through an agent is complex — messages with multiple content types, tool calls with varying parameter schemas, provider-specific response formats. A strong type system catches data-handling bugs at compile time rather than in production.

**Process spawning and filesystem interaction.** Agents spawn child processes (shell commands) and read/write files constantly. The language needs robust, ergonomic APIs for these operations.

**Single binary distribution.** An agent is a CLI tool that developers install and run. A single binary with no runtime dependencies is dramatically easier to distribute than an application that requires Node.js, Python, or a Go runtime to be installed separately.

**Performance.** While agents are mostly I/O-bound (waiting for API responses), some operations — parsing large files, searching codebases, processing streaming responses — benefit from computational efficiency. A fast language means the agent feels responsive even during heavy processing.

## The Contenders

### TypeScript: The Web-First Choice

TypeScript is the language of Claude Code and many other AI-powered tools. It has excellent LLM SDK support (both Anthropic and OpenAI publish first-party TypeScript SDKs), a massive ecosystem of npm packages, and a huge developer community.

**Strengths for agents:**
- First-party SDK support from all major LLM providers.
- Excellent JSON handling — critical because LLM APIs are JSON-based.
- Large ecosystem of utility libraries.
- Familiar to many developers.

**Weaknesses for agents:**
- Runtime errors that the type system doesn't catch (TypeScript's type system is structural and has escape hatches like `any`).
- Requires Node.js runtime, which adds distribution complexity.
- Single-threaded event loop can become a bottleneck during CPU-intensive operations like large file parsing.
- Error handling relies on exceptions, which are easy to forget and invisible in function signatures.

### Python: The ML-Adjacent Choice

Python is the lingua franca of the AI/ML world. It has excellent LLM SDK support, the richest ecosystem of AI libraries, and it's probably the language you're most comfortable with if you're reading this tutorial.

**Strengths for agents:**
- The most extensive AI/ML ecosystem.
- First-party SDK support from all providers.
- Familiar and rapid to develop in.
- Excellent for prototyping.

**Weaknesses for agents:**
- The GIL limits true parallelism for CPU-bound tasks.
- No compile-time type checking (even with type hints, enforcement is optional and incomplete).
- Distribution is complex — managing Python versions, virtual environments, and dependencies across systems is notoriously difficult.
- Runtime errors that surface only in production. An entire class of bugs (misspelled attribute names, wrong argument types, None where a value was expected) pass silently until the code runs.
- Performance for CPU-bound operations like file parsing and pattern matching is significantly slower than compiled languages.

::: python Coming from Python
If you're a Python developer, you might feel defensive about these weaknesses. And it's true — Python's weaknesses are manageable, and millions of production systems run on Python perfectly well. But for a coding agent specifically, where an unhandled `AttributeError` can crash an autonomous session and lose thirty minutes of context, and where you want a single `pip install` or binary download to work on any system, Python's weaknesses are particularly acute. That said, building in Rust while knowing Python gives you the best of both worlds — you understand the problem domain (from Python) and the implementation tool gives you stronger guarantees (from Rust).
:::

### Go: The Pragmatic Choice

Go is OpenCode's language, and it's an excellent choice for CLI tools. It compiles to a single static binary, has good concurrency primitives (goroutines), and its standard library includes everything you need for HTTP, JSON, and process management.

**Strengths for agents:**
- Single binary compilation.
- Goroutines for lightweight concurrency.
- Excellent standard library for HTTP, JSON, and OS interaction.
- Fast compilation and execution.
- Simple language that's quick to learn.

**Weaknesses for agents:**
- Less expressive type system — no enums with data, no generics (until recently), no trait-based dispatch.
- Error handling is explicit but verbose and easy to ignore (the `if err != nil` pattern).
- No pattern matching on response types — you end up with runtime type assertions.
- Garbage collector can introduce latency pauses (usually minor, but noticeable in real-time streaming).

### Rust: Our Choice

Rust gives us the strongest combination of the properties we identified:

**Compile-time error prevention.** Rust's type system catches an enormous range of bugs at compile time — null pointer dereferences (impossible — there's no null), use-after-free (impossible — the borrow checker prevents it), data races (impossible — the type system enforces thread safety), unhandled errors (the `Result` type must be explicitly handled or the compiler warns you).

**Explicit error handling.** Every function that can fail returns a `Result<T, E>`. You must handle the error case — you can't accidentally ignore it. The `?` operator makes error propagation ergonomic without hiding it.

**Powerful enums and pattern matching.** Rust's enums can carry data, which means you can represent complex message types, tool call variants, and response formats as types that the compiler verifies. Pattern matching ensures you handle every case.

**Async with Tokio.** Rust's async ecosystem, centered on Tokio, provides high-performance non-blocking I/O with compile-time safety guarantees. You get the performance of an event loop with the readability of sequential code.

**Single binary.** `cargo build --release` produces a single static binary with no runtime dependencies. Users download one file and run it.

**Performance.** Rust is consistently among the fastest languages in benchmarks. For an agent, this means responsive file searching, fast streaming response processing, and minimal overhead during I/O operations.

```rust
// Rust's type system catches agent-specific bugs at compile time
enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: serde_json::Value },
}

// The compiler ensures you handle every variant
fn process_block(block: &ContentBlock) {
    match block {
        ContentBlock::Text(text) => println!("{}", text),
        ContentBlock::ToolUse { id, name, input } => {
            // dispatch tool call
            println!("Tool: {} ({})", name, id);
        }
    }
    // If you add a new variant later, the compiler tells you
    // every place that needs to handle it. No runtime surprises.
}
```

::: wild In the Wild
The language choices of real agents reflect their priorities. Claude Code chose TypeScript for ecosystem and SDK access. OpenCode chose Go for simplicity and binary distribution. Pi chose Rust for type safety and performance. There's no wrong answer — but each choice produces a different set of trade-offs. Our choice of Rust means we get the strongest compile-time guarantees and the best performance, at the cost of a steeper learning curve and more verbose code.
:::

## The Trade-Offs We Accept

Rust isn't perfect for this use case. Here's what we're accepting:

**Steeper learning curve.** If you're coming from Python, Rust's ownership model, lifetime annotations, and borrow checker will feel unfamiliar. The learning curve is real, but it's front-loaded — once you internalize the concepts, you move quickly. And the concepts you learn (ownership, lifetimes, explicit error handling) make you a better programmer in every language.

**More verbose code.** Rust code is typically more verbose than equivalent Python or TypeScript. You'll write more type definitions, more error handling, and more trait implementations. This verbosity is the cost of explicitness — every type annotation is a piece of documentation, every `Result` type is an explicit error path.

**Smaller LLM SDK ecosystem.** The Anthropic and OpenAI Rust SDKs exist but are less mature than their TypeScript and Python counterparts. We'll work with the HTTP API directly where needed, which actually teaches you more about how the API works.

**Longer compilation times.** Rust's compile times are slower than Go's (though faster than many C++ projects). During development, `cargo check` (type checking without code generation) is fast enough for iterative development, and incremental compilation means only changed code gets recompiled.

## Why Rust for a Python Developer

If you're a Python developer, Rust might seem like a strange choice. Why not build the agent in Python — a language you already know?

The answer is that building in Rust teaches you two things simultaneously: agent architecture and a new programming paradigm. You already know how to think about code in Python's dynamic, interpreted world. Rust shows you a fundamentally different approach — one where the compiler is your pair-programming partner, catching bugs before you run the code.

More practically, the skills you gain from Rust — thinking about memory ownership, designing with types, handling errors explicitly — transfer back to your Python work. You'll start writing more defensive Python, using type hints more diligently, and handling edge cases more carefully. Rust doesn't replace Python in your toolkit; it enhances how you use Python.

And if you ever want to contribute to tools like Pi, build high-performance developer tools, or work on systems-level software, Rust proficiency opens doors that Python alone doesn't.

## Key Takeaways

- The key properties for an agent implementation language are reliable error handling, good async I/O, a strong type system, robust process/filesystem APIs, single binary distribution, and performance.
- Rust provides the strongest combination of these properties: compile-time error prevention, explicit error handling with `Result`, powerful enums with pattern matching, async I/O via Tokio, single binary compilation, and near-C performance.
- The trade-offs we accept with Rust are a steeper learning curve, more verbose code, a smaller LLM SDK ecosystem, and longer compilation times — costs that are front-loaded and diminish with experience.
- Building in Rust rather than Python teaches you two things at once: agent architecture and a new programming paradigm whose lessons transfer back to improve your Python code.
- Every language choice involves trade-offs; our choice of Rust prioritizes compile-time safety and performance, which are especially valuable for autonomous systems that run without constant human supervision.
