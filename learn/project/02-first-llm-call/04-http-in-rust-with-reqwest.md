---
title: HTTP in Rust with Reqwest
description: Use the reqwest crate to build an HTTP client capable of sending POST requests with custom headers and JSON bodies.
---

# HTTP in Rust with Reqwest

> **What you'll learn:**
> - How to add reqwest as a dependency and configure it with the JSON and rustls-tls features
> - How to construct a reusable HTTP client with default headers for the Anthropic API
> - How to send POST requests with JSON bodies and read the response status and body

You have an API key stored safely in an environment variable. You know the endpoint, the headers, and the request format. Now you need the machinery to actually send HTTP requests from Rust. That machinery is `reqwest` -- the most widely used HTTP client in the Rust ecosystem.

## Adding reqwest to Your Project

Open your `Cargo.toml` and add `reqwest` with the features you need:

```toml
[package]
name = "cli-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
dotenvy = "0.15"
```

Let's break down what each dependency does:

- **`reqwest`** with `"json"` feature -- enables the `.json()` method on request builders and response objects, so you can send and receive JSON without manual serialization.
- **`serde`** with `"derive"` feature -- the serialization framework. The `derive` feature lets you annotate structs with `#[derive(Serialize, Deserialize)]`.
- **`serde_json`** -- JSON-specific serialization and deserialization. Used by reqwest under the hood and also useful for manual JSON operations.
- **`tokio`** with `"full"` feature -- the async runtime. `reqwest` is an async library, so every HTTP call returns a `Future` that you `.await`.
- **`dotenvy`** -- loads `.env` files, as covered in the previous subchapter.

::: python Coming from Python
This is like adding entries to your `requirements.txt` or `pyproject.toml`:
```
requests
aiohttp
python-dotenv
```
The difference is that Rust's Cargo downloads, compiles, and statically links these dependencies. There is no separate `pip install` step -- `cargo build` handles everything. The first build takes a minute or two as it compiles reqwest and its dependencies; subsequent builds are fast because Cargo caches compiled artifacts.
:::

## Creating an HTTP Client

The `reqwest::Client` is a connection-pooling, reusable HTTP client. You should create **one** client and reuse it for all requests, rather than creating a new one per request. This matters because the client maintains a connection pool, TLS sessions, and cookie storage internally.

Here is the simplest way to create a client:

```rust
let client = reqwest::Client::new();
```

But for the Anthropic API, you want to pre-configure headers that go on every request. The `ClientBuilder` lets you do this:

```rust
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};

fn build_client(api_key: &str) -> Result<Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(api_key).unwrap());
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

    Client::builder()
        .default_headers(headers)
        .build()
}
```

The `default_headers` method attaches these headers to every request made through this client. You no longer need to set them manually on each request. The `Content-Type: application/json` header is handled automatically by reqwest when you use the `.json()` method on the request builder.

Note the two different ways to create `HeaderValue`:
- `HeaderValue::from_static("2023-06-01")` -- for compile-time string literals. Zero allocation, zero fallibility.
- `HeaderValue::from_str(api_key)` -- for runtime strings. This can fail if the string contains invalid header characters, but API keys are always ASCII, so `unwrap()` is safe here.

## Sending a POST Request

With the client ready, here is how you send a POST request with a JSON body:

```rust
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

async fn send_message(client: &reqwest::Client) -> Result<String, reqwest::Error> {
    let request_body = ChatRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 1024,
        messages: vec![
            Message {
                role: "user".to_string(),
                content: "Say hello in one sentence.".to_string(),
            },
        ],
    };

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .json(&request_body)
        .send()
        .await?;

    let body = response.text().await?;
    Ok(body)
}
```

Let's trace through this step by step:

1. **`client.post(url)`** -- creates a `RequestBuilder` for a POST request to the given URL.
2. **`.json(&request_body)`** -- serializes `request_body` to JSON and sets it as the request body. This also sets `Content-Type: application/json` automatically.
3. **`.send().await?`** -- sends the request and waits for the response. The `?` propagates any network error.
4. **`response.text().await?`** -- reads the full response body as a string.

Every step after `.send()` is async -- the function pauses at each `.await` and yields control to the runtime while waiting for I/O. This is why the function is `async fn` and why you need tokio.

## Reading the Response

The `response` object carries both metadata and the body:

```rust
async fn send_and_inspect(client: &reqwest::Client) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello!"}]
        }))
        .send()
        .await?;

    // Check the HTTP status code
    let status = response.status();
    println!("Status: {status}");

    // Read response headers
    if let Some(request_id) = response.headers().get("request-id") {
        println!("Request ID: {}", request_id.to_str().unwrap_or("unknown"));
    }

    // Read the body as text
    let body = response.text().await?;
    println!("Body: {body}");

    Ok(())
}
```

A few things to note:

- **`response.status()`** returns a `StatusCode` that you can compare: `status.is_success()`, `status == 200`, etc. You will use this extensively for error handling.
- **`response.headers()`** returns the response headers. The Anthropic API includes useful headers like `request-id` for debugging and rate limit headers like `retry-after`.
- **`response.text()`** consumes the response body and returns it as a `String`. You can only call this once -- the body is a stream that gets consumed. Alternatively, `response.json::<T>()` deserializes directly into a type (covered in the next subchapter on serde).

::: python Coming from Python
This maps directly to the `requests` library:
```python
response = requests.post(url, json=body, headers=headers)
print(response.status_code)  # response.status() in Rust
print(response.headers)      # response.headers() in Rust
print(response.text)          # response.text().await in Rust
print(response.json())        # response.json::<T>().await in Rust
```
The key difference is that every I/O operation in `reqwest` is async and requires `.await`. In Python's `requests`, the calls block the thread. The Rust approach is more like Python's `aiohttp` library, where you use `await` on every response operation.
:::

## A Complete Working Example

Let's put everything together into a program you can actually run:

```rust
use reqwest::header::{HeaderMap, HeaderValue};
use std::env;

fn build_client(api_key: &str) -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(api_key).unwrap());
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    let client = build_client(&api_key)?;

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "What is 2 + 2?"}]
        }))
        .send()
        .await?;

    println!("Status: {}", response.status());
    let body = response.text().await?;
    println!("Response: {body}");

    Ok(())
}
```

This program loads the API key, builds a client with the right headers, sends a simple message, and prints the raw response. You are not parsing the JSON into structs yet -- that comes in the next subchapter. But if you run this with a valid API key, you will see Claude's response in your terminal for the first time.

Run it:

```bash
export ANTHROPIC_API_KEY="sk-ant-api03-xxxxx"
cargo run
```

You should see output like:

```
Status: 200 OK
Response: {"id":"msg_01XFDUDYJgAACzvnptvVoYEL","type":"message","role":"assistant","content":[{"type":"text","text":"2 + 2 equals 4."}],"model":"claude-sonnet-4-20250514","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":14,"output_tokens":12}}
```

That raw JSON is your first successful LLM API call from Rust. In the next subchapter, you will turn that blob of JSON into clean Rust structs.

## Key Takeaways

- Use `reqwest` with the `json` feature as your HTTP client. Create one `Client` instance and reuse it across all requests.
- Pre-configure authentication and API version headers using `Client::builder().default_headers()` so you do not repeat them on every request.
- The `.json(&body)` method on the request builder serializes a Rust struct to JSON and sets the `Content-Type` header automatically.
- Every `reqwest` operation is async: `.send().await` for the request, `.text().await` or `.json().await` for reading the response body.
- Response bodies can only be consumed once -- choose between `.text()`, `.json()`, or `.bytes()` based on what you need.
