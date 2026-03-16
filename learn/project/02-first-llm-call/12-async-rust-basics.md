---
title: Async Rust Basics
description: Understand async/await in Rust and use the tokio runtime to perform non-blocking HTTP requests.
---

# Async Rust Basics

> **What you'll learn:**
> - How `async fn` and `.await` work in Rust and why they require a runtime like tokio
> - How to annotate `main` with `#[tokio::main]` to run async code from your binary's entry point
> - How async differs from threads and why it is the right choice for I/O-bound API calls

You have been using `async` and `.await` throughout this chapter without a deep explanation. Every `reqwest` call is `async`, every response read is `async`, and your `main` function has a `#[tokio::main]` annotation. Now let's understand what all of that actually means and why Rust does async this way.

## The Problem: Waiting for I/O

When your agent sends a request to the Anthropic API, the network round trip takes somewhere between 1 and 30 seconds. During that time, your program is waiting -- waiting for bytes to travel across the internet, for the model to generate tokens, and for the response to come back.

In synchronous code, waiting means blocking. The thread sits idle, consuming memory but doing nothing useful. For a simple CLI tool, this is fine -- you have one user and one request at a time. But even for a CLI, blocking has a cost: you cannot do anything else while waiting. You cannot show a spinner, handle a Ctrl+C gracefully, or start a second request.

Async programming lets you write code that *yields* control while waiting, so the runtime can do other work (or just sleep efficiently) until the I/O completes.

## How async/await Works in Rust

When you write an `async fn`, the compiler transforms it into a state machine. Each `.await` point becomes a state transition. Here is a conceptual model:

```rust
// What you write:
async fn get_response(client: &reqwest::Client) -> String {
    let response = client.get("https://example.com").send().await.unwrap();
    let body = response.text().await.unwrap();
    body
}

// What the compiler roughly generates (simplified):
// A state machine with three states:
// State 0: About to call send()
// State 1: send() returned, about to call text()
// State 2: text() returned, ready to return the body
```

The function does not execute when you call it. Instead, calling `get_response(&client)` returns a `Future` -- a value that represents a computation that has not started yet. The computation only progresses when someone calls `.await` on the future, which tells the runtime "run this until it needs to wait, then let me know when it's ready."

This is fundamentally different from languages like JavaScript or Python where `async def` creates a coroutine that starts eagerly. In Rust, futures are *lazy* -- nothing happens until you explicitly drive them.

::: python Coming from Python
Python's `async/await` is similar in syntax but different in execution model:
```python
import asyncio
import aiohttp

async def get_response():
    async with aiohttp.ClientSession() as session:
        response = await session.get("https://example.com")
        body = await response.text()
        return body

# Must run within an event loop
asyncio.run(get_response())
```
Both languages require a runtime (Python's `asyncio` event loop, Rust's tokio runtime) to actually execute async code. Both use `await` to pause at I/O points. The key difference is that Rust's futures are lazy and zero-cost -- the compiler generates a state machine with no heap allocation, while Python's coroutines are heap-allocated objects managed by the garbage collector.
:::

## The Tokio Runtime

Rust does not have a built-in async runtime. The language provides the `async/await` syntax and the `Future` trait, but the machinery to actually execute futures is provided by a crate. Tokio is the most widely used runtime in the Rust ecosystem.

Tokio provides:
- **An executor** that drives futures to completion by polling them when I/O events occur.
- **A reactor** that monitors file descriptors (sockets, timers) using OS primitives like `epoll` (Linux) or `kqueue` (macOS).
- **Utilities** like `tokio::time::sleep`, `tokio::spawn`, and async-aware channels.

You have been using `#[tokio::main]` to start the runtime:

```rust
#[tokio::main]
async fn main() {
    println!("This runs inside the tokio runtime");
}
```

This attribute macro is syntactic sugar. It expands to:

```rust
fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("This runs inside the tokio runtime");
    });
}
```

`block_on` takes a future and blocks the current thread until it completes. This is the bridge between the synchronous world (your `main` function) and the async world (everything inside it).

## async fn in Practice

Let's look at how async functions compose in your agent:

```rust
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    content: Vec<ContentBlock>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

// Each async fn returns a Future that must be .awaited
async fn build_client(api_key: &str) -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(api_key).unwrap());
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
}

async fn send_message(
    client: &reqwest::Client,
    user_input: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let request = ChatRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 1024,
        messages: vec![Message {
            role: "user".to_string(),
            content: user_input.to_string(),
        }],
    };

    // First await: send the HTTP request
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .json(&request)
        .send()
        .await?;

    // Second await: read and parse the response body
    let chat_response: ChatResponse = response.json().await?;

    let text = chat_response
        .content
        .iter()
        .filter_map(|block| block.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(text)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    // Await the client build (not strictly necessary here since it's sync,
    // but illustrates async composition)
    let client = build_client(&api_key).await?;

    // Await the API call
    let response = send_message(&client, "What is Rust?").await?;
    println!("{response}");

    Ok(())
}
```

Notice the chain: `main` awaits `send_message`, which internally awaits `client.send()` and `response.json()`. Each `.await` is a point where the function can pause and resume. If you had multiple concurrent operations, the runtime could interleave their execution at these points.

## Async vs. Threads

You might wonder: why not just use threads? Spawn a thread for each API call and join it when the response arrives.

Threads work, but they have overhead:
- **Memory.** Each thread has its own stack (typically 2-8 MB). A thousand threads consume gigabytes of RAM.
- **Context switching.** The OS scheduler switches between threads, which involves saving and restoring CPU registers.
- **Synchronization.** Sharing data between threads requires mutexes, which adds complexity and can cause deadlocks.

Async tasks are lightweight:
- **Memory.** An async task is just a state machine struct, typically a few hundred bytes.
- **Switching.** The runtime switches between tasks in user space, which is much faster than an OS context switch.
- **Cooperation.** Tasks yield at `.await` points, so you always know where context switches happen.

For your CLI agent, the difference is academic -- you are making one API call at a time. But as you add capabilities (tool use, parallel operations, streaming), async becomes genuinely important. And since `reqwest` is async-only, you need it from day one.

## Common Async Pitfalls

### Forgetting .await

If you forget `.await`, the code compiles but the future never runs:

```rust
async fn broken() {
    let client = reqwest::Client::new();
    // This creates a future but never executes it!
    client.get("https://example.com").send();
    // The compiler warns: "unused implementor of `Future` that must be used"
}
```

The Rust compiler warns you about unused futures, so pay attention to warnings.

### Blocking Inside Async

Never run expensive synchronous code inside an async function without yielding:

```rust
async fn bad_idea() {
    // This blocks the async runtime's thread!
    std::thread::sleep(std::time::Duration::from_secs(5));
}

async fn good_idea() {
    // This yields control to the runtime while waiting
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
}
```

Use `tokio::time::sleep` instead of `std::thread::sleep`. The async version yields control so the runtime can do other work; the synchronous version blocks the entire runtime thread.

### Error Handling with ?

The `?` operator works normally in async functions. Just make sure your async function's return type is `Result`:

```rust
async fn might_fail() -> Result<String, Box<dyn std::error::Error>> {
    let resp = reqwest::get("https://example.com").await?; // ? works here
    let body = resp.text().await?; // and here
    Ok(body)
}
```

::: details What about Send and 'static bounds?
If you try to `tokio::spawn` a future, you will encounter `Send` and `'static` bounds. These are required because spawned tasks can be moved between threads. For now, you do not need `tokio::spawn` -- your REPL calls async functions sequentially. You will encounter spawn bounds when you add concurrent tool execution in a later chapter.
:::

## Key Takeaways

- `async fn` in Rust returns a lazy `Future` that only executes when `.await`ed. Forgetting `.await` means the code never runs (but the compiler warns you).
- Tokio provides the runtime (executor + reactor) that drives futures to completion. The `#[tokio::main]` attribute sets it up automatically.
- Use `tokio::time::sleep` instead of `std::thread::sleep` inside async functions to avoid blocking the runtime thread.
- Async tasks are far lighter than OS threads -- they are small state machines rather than full thread stacks, making them ideal for I/O-bound work like API calls.
- The `?` operator works in async functions exactly as in synchronous ones, as long as the return type is a `Result`.
