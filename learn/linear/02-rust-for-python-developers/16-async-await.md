---
title: Async Await
description: Async programming in Rust with tokio, compared to Python's asyncio, covering futures, runtimes, and concurrent execution patterns.
---

# Async Await

> **What you'll learn:**
> - How Rust's async/await syntax compares to Python's and why Rust requires an explicit runtime like tokio
> - The difference between Rust's poll-based futures and Python's coroutine-based approach
> - How to use tokio::spawn, join!, and select! for concurrent agent operations like streaming and tool execution

A coding agent is inherently concurrent — it streams responses from an API while waiting for user input, executes shell commands with timeouts, and makes parallel network requests. Both Python and Rust support async programming, and the surface-level syntax is remarkably similar. But the underlying models differ in important ways.

## Python asyncio vs Rust tokio — a first look

**Python:**

```python
import asyncio
import httpx

async def fetch_response(url: str) -> str:
    async with httpx.AsyncClient() as client:
        response = await client.get(url)
        return response.text

async def main():
    result = await fetch_response("https://api.example.com/data")
    print(result)

asyncio.run(main())
```

**Rust:**

```rust
use reqwest;

async fn fetch_response(url: &str) -> Result<String, reqwest::Error> {
    let response = reqwest::get(url).await?;
    let text = response.text().await?;
    Ok(text)
}

#[tokio::main]
async fn main() {
    match fetch_response("https://api.example.com/data").await {
        Ok(text) => println!("{}", text),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

The syntax is almost identical: `async fn`, `await`, and a runtime to drive execution. But there are key differences beneath the surface.

::: python Coming from Python
The biggest conceptual difference: in Python, `asyncio` is built into the standard library. You call `asyncio.run(main())` and the event loop is created for you. In Rust, async is a language feature but the runtime is *not* included in `std`. You must choose a runtime — `tokio` is the standard choice for most applications, including everything we build in this course.

The `#[tokio::main]` attribute macro transforms your `async fn main()` into a regular `fn main()` that starts the tokio runtime. It is equivalent to Python's `asyncio.run(main())` wrapper.
:::

## Setting up tokio

Add tokio to your project:

```bash
cargo add tokio --features full
```

In your `Cargo.toml`:

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```

The `features = ["full"]` flag enables all tokio features (async I/O, timers, synchronization, process spawning). For production, you can enable only the features you need.

## How async works in Rust

When you write `async fn`, Rust transforms the function into a *state machine* that implements the `Future` trait. A future is a value that *will* produce a result at some point.

```rust
// This async function...
async fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

// ...is roughly equivalent to this (simplified):
// fn greet(name: &str) -> impl Future<Output = String> {
//     // returns a state machine that, when polled, produces a String
// }
```

The key insight: calling an async function **does not execute it**. It creates a future. The future only runs when you `.await` it or spawn it on the runtime.

```rust
#[tokio::main]
async fn main() {
    // This creates a future but does NOT execute the function
    let future = greet("Agent");

    // This actually runs the function
    let result = future.await;
    println!("{}", result);
}

async fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
```

::: python Coming from Python
This is identical to Python's behavior:
```python
async def greet(name):
    return f"Hello, {name}!"

# This creates a coroutine, does NOT run it
coro = greet("Agent")

# This runs it
result = await coro
```
Both languages use lazy evaluation for async functions — calling them produces a handle (future/coroutine) that must be driven to completion by `.await` or the runtime.
:::

## Concurrent execution with `join!`

`tokio::join!` runs multiple futures concurrently, waiting for all of them to complete:

```rust
use tokio::time::{sleep, Duration};

async fn fetch_model_response() -> String {
    sleep(Duration::from_millis(200)).await;  // simulate API call
    String::from("I can help with that!")
}

async fn load_context() -> String {
    sleep(Duration::from_millis(100)).await;  // simulate file read
    String::from("fn main() { ... }")
}

async fn check_permissions() -> bool {
    sleep(Duration::from_millis(50)).await;  // simulate check
    true
}

#[tokio::main]
async fn main() {
    // Run all three concurrently — total time ~200ms, not 350ms
    let (response, context, permitted) = tokio::join!(
        fetch_model_response(),
        load_context(),
        check_permissions(),
    );

    println!("Response: {}", response);
    println!("Context: {}", context);
    println!("Permitted: {}", permitted);
}
```

::: python Coming from Python
This is equivalent to Python's `asyncio.gather`:
```python
response, context, permitted = await asyncio.gather(
    fetch_model_response(),
    load_context(),
    check_permissions(),
)
```
Both run the tasks concurrently on the event loop. The total time is the duration of the longest task, not the sum of all tasks.
:::

## Spawning tasks with `tokio::spawn`

`tokio::spawn` creates a new task that runs independently, like `asyncio.create_task` in Python:

```rust
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    // Spawn a background task
    let handle = tokio::spawn(async {
        sleep(Duration::from_millis(100)).await;
        String::from("Background task complete")
    });

    // Do other work while the task runs
    println!("Doing other work...");
    sleep(Duration::from_millis(50)).await;
    println!("Still working...");

    // Wait for the spawned task to finish
    let result = handle.await.unwrap();
    println!("{}", result);
}
```

::: python Coming from Python
Mapping `tokio::spawn` to Python:
```python
task = asyncio.create_task(background_work())
# ... do other things ...
result = await task
```
One important difference: `tokio::spawn` requires the future to be `Send` — meaning it can safely be moved to another thread. Tokio uses a multi-threaded runtime by default, so spawned tasks can run on any thread. Python's asyncio is single-threaded, so no such restriction exists.
:::

## Racing with `tokio::select!`

`select!` runs multiple futures and acts on whichever completes first. This is essential for timeouts and cancellation:

```rust
use tokio::time::{sleep, Duration};

async fn api_call() -> String {
    sleep(Duration::from_millis(500)).await;
    String::from("API response")
}

#[tokio::main]
async fn main() {
    tokio::select! {
        result = api_call() => {
            println!("Got response: {}", result);
        }
        _ = sleep(Duration::from_millis(200)) => {
            println!("Timeout! API took too long.");
        }
    }
}
```

::: python Coming from Python
`select!` is like `asyncio.wait` with `FIRST_COMPLETED`, or using `asyncio.wait_for` for timeouts:
```python
try:
    result = await asyncio.wait_for(api_call(), timeout=0.2)
    print(f"Got response: {result}")
except asyncio.TimeoutError:
    print("Timeout! API took too long.")
```
Rust's `select!` is more general — it can race any number of futures, not just add a timeout to one.
:::

## Async in the coding agent

For our coding agent, async enables:

```rust
use tokio::time::{sleep, Duration};

async fn stream_response() -> Vec<String> {
    // Simulate streaming chunks from the LLM API
    let chunks = vec!["Hello", ", ", "I can", " help", " with that!"];
    let mut result = Vec::new();
    for chunk in chunks {
        sleep(Duration::from_millis(50)).await;
        println!("Chunk: {}", chunk);
        result.push(chunk.to_string());
    }
    result
}

async fn execute_tool(name: &str, args: &str) -> String {
    sleep(Duration::from_millis(100)).await;
    format!("Tool {} executed with args: {}", name, args)
}

#[tokio::main]
async fn main() {
    // Stream a response
    let chunks = stream_response().await;
    let full_response: String = chunks.join("");
    println!("Full response: {}", full_response);

    // Execute a tool
    let result = execute_tool("shell", "ls -la").await;
    println!("{}", result);
}
```

## Async error handling

Async functions work naturally with `Result` and `?`:

```rust
use std::io;

async fn read_and_process(path: &str) -> Result<String, io::Error> {
    let content = tokio::fs::read_to_string(path).await?;
    let processed = content.to_uppercase();
    Ok(processed)
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    match read_and_process("example.txt").await {
        Ok(content) => println!("{}", content),
        Err(e) => eprintln!("Error: {}", e),
    }
    Ok(())
}
```

::: python Coming from Python
Error handling in async Rust uses the same `Result` + `?` pattern as synchronous Rust. There is no special async error handling. In Python, you use the same `try/except` in both sync and async code. The parallel is exact — async does not change how errors work in either language.
:::

## Key differences from Python's asyncio

| Aspect | Python asyncio | Rust tokio |
|--------|---------------|------------|
| Runtime | Built into stdlib | External crate (tokio) |
| Threading | Single-threaded event loop | Multi-threaded by default |
| Cancellation | `task.cancel()` raises `CancelledError` | Dropping a future cancels it |
| CPU-bound work | Blocks the event loop | Use `tokio::task::spawn_blocking` |
| Colored functions | Yes — async and sync don't mix | Yes — same limitation |

The "colored function" problem exists in both languages: you cannot directly `.await` in a synchronous function. In Python, you use `asyncio.run()` or `loop.run_until_complete()`. In Rust, you use `tokio::runtime::Runtime::block_on()`.

## Key Takeaways

- Rust's async/await syntax is nearly identical to Python's, but Rust requires an explicit runtime (tokio) instead of a built-in event loop
- `tokio::join!` (like `asyncio.gather`) runs futures concurrently; `tokio::select!` (like `asyncio.wait_for`) races futures and acts on the first to complete
- `tokio::spawn` creates independent tasks like `asyncio.create_task`, but requires the future to be `Send` because tokio uses multiple threads
- Async Rust uses the same `Result` + `?` error handling as synchronous Rust — async does not change the error model
- For the coding agent, async enables concurrent streaming, tool execution with timeouts, and non-blocking I/O — essential patterns for responsive agent behavior
