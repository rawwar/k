---
title: Serialization Formats
description: Compare JSON, MessagePack, and other serialization formats for conversation persistence considering size, speed, and human readability.
---

# Serialization Formats

> **What you'll learn:**
> - How to use serde with JSON for human-readable session files and MessagePack for compact storage
> - How to handle schema versioning and forward-compatible deserialization of session files
> - How to benchmark serialization performance for sessions with thousands of messages

In the previous subchapter, you used JSON for session persistence. JSON is a great default -- it is human-readable, widely supported, and easy to debug. But when sessions grow to thousands of messages with large tool results, JSON's verbosity becomes a problem. This subchapter explores your serialization options and helps you choose the right format for different scenarios.

## JSON: The Human-Readable Default

You already have JSON serialization thanks to serde. Let's look at what it produces and where its limitations show:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct SimpleMessage {
    role: String,
    content: String,
    token_count: usize,
}

fn main() {
    let messages = vec![
        SimpleMessage {
            role: "user".to_string(),
            content: "Read src/main.rs".to_string(),
            token_count: 6,
        },
        SimpleMessage {
            role: "assistant".to_string(),
            content: "Here is the content of src/main.rs:\n\nfn main() {\n    println!(\"hello world\");\n}".to_string(),
            token_count: 22,
        },
    ];

    // Pretty JSON for human readability
    let pretty = serde_json::to_string_pretty(&messages).unwrap();
    println!("Pretty JSON ({} bytes):", pretty.len());
    println!("{}\n", pretty);

    // Compact JSON for smaller files
    let compact = serde_json::to_string(&messages).unwrap();
    println!("Compact JSON ({} bytes):", compact.len());
    println!("{}", compact);
}
```

JSON's strengths are clear: you can open a session file in any text editor and read it. But JSON has costs:

- **Verbose**: Every key is repeated for every object. Field names like `"token_count"` appear thousands of times in a long session.
- **String escaping**: Code content requires escaping of quotes, backslashes, and newlines, bloating size by 10--30%.
- **Parse speed**: JSON parsing is slower than binary formats because it must handle string-to-number conversion, Unicode escaping, and whitespace.

## MessagePack: The Compact Binary Alternative

MessagePack (msgpack) is a binary serialization format that is structurally equivalent to JSON but much more compact. Thanks to serde, switching between JSON and MessagePack requires almost no code changes.

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
rmp-serde = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Now let's compare the two formats directly:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
    token_count: usize,
    priority: u8,
    is_summary: bool,
}

/// Generate a realistic-looking conversation for benchmarking.
fn generate_conversation(n: usize) -> Vec<Message> {
    let mut messages = Vec::with_capacity(n);
    for i in 0..n {
        let (role, content) = if i % 3 == 0 {
            ("user".to_string(), format!("Please help me with task {}. I need to modify the file src/handlers/request_{}.rs to add proper error handling for the database connection timeout case.", i, i))
        } else if i % 3 == 1 {
            ("assistant".to_string(), format!("I'll help you with that. Let me read the file first and then suggest the changes. The key issue is that the current code uses unwrap() on line {} which will panic on timeout.", i * 10))
        } else {
            ("tool".to_string(), format!("use std::io::Result;\n\nfn handle_request_{i}(req: Request) -> Result<Response> {{\n    let db = Database::connect()?;\n    let result = db.query(\"SELECT * FROM users WHERE id = $1\", &[&req.user_id])?;\n    Ok(Response::new(result))\n}}"))
        };
        messages.push(Message {
            role,
            content,
            token_count: 50 + (i % 20) * 10,
            priority: (i % 4) as u8,
            is_summary: false,
        });
    }
    messages
}

fn main() {
    let conversations = [10, 100, 1000];

    println!("{:<8} {:>12} {:>12} {:>8}", "Messages", "JSON bytes", "MsgPack", "Ratio");
    println!("{}", "-".repeat(44));

    for &n in &conversations {
        let msgs = generate_conversation(n);

        // JSON serialization
        let json_bytes = serde_json::to_vec(&msgs).unwrap();

        // MessagePack serialization
        let msgpack_bytes = rmp_serde::to_vec(&msgs).unwrap();

        let ratio = msgpack_bytes.len() as f64 / json_bytes.len() as f64;

        println!("{:<8} {:>12} {:>12} {:>7.1}%",
            n,
            json_bytes.len(),
            msgpack_bytes.len(),
            ratio * 100.0,
        );

        // Verify roundtrip
        let decoded: Vec<Message> = rmp_serde::from_slice(&msgpack_bytes).unwrap();
        assert_eq!(decoded.len(), msgs.len());
    }
}
```

MessagePack is typically 50--70% the size of JSON for conversation data, because it uses binary encoding for numbers and does not repeat field names in the same way.

::: python Coming from Python
In Python, you would use the `msgpack` library:
```python
import msgpack, json

data = {"role": "user", "content": "hello", "token_count": 5}
json_bytes = json.dumps(data).encode()       # 54 bytes
msgpack_bytes = msgpack.packb(data)           # 36 bytes
```
The Rust `rmp-serde` crate works identically to `serde_json` -- you swap one
call for another. The `#[derive(Serialize, Deserialize)]` on your struct handles
both formats without modification. This is the power of serde's format-agnostic
design.
:::

## A Dual-Format Session Store

In practice, you want both formats available: JSON for debugging and MessagePack for production. Let's build a store that supports both:

```rust
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Supported serialization formats for session files.
#[derive(Debug, Clone, Copy)]
pub enum SerializationFormat {
    /// Human-readable JSON (.json)
    Json,
    /// Compact binary MessagePack (.msgpack)
    MessagePack,
}

impl SerializationFormat {
    /// File extension for this format.
    pub fn extension(&self) -> &str {
        match self {
            Self::Json => "json",
            Self::MessagePack => "msgpack",
        }
    }

    /// Detect format from file extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "json" => Some(Self::Json),
            "msgpack" | "mp" => Some(Self::MessagePack),
            _ => None,
        }
    }

    /// Serialize a value to bytes.
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, String> {
        match self {
            Self::Json => serde_json::to_vec_pretty(value)
                .map_err(|e| format!("JSON serialization failed: {}", e)),
            Self::MessagePack => rmp_serde::to_vec(value)
                .map_err(|e| format!("MessagePack serialization failed: {}", e)),
        }
    }

    /// Deserialize a value from bytes.
    pub fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, String> {
        match self {
            Self::Json => serde_json::from_slice(bytes)
                .map_err(|e| format!("JSON deserialization failed: {}", e)),
            Self::MessagePack => rmp_serde::from_slice(bytes)
                .map_err(|e| format!("MessagePack deserialization failed: {}", e)),
        }
    }
}

/// A session store that supports multiple serialization formats.
pub struct FormatAwareStore {
    base_dir: PathBuf,
    /// Format to use when writing new sessions
    write_format: SerializationFormat,
}

impl FormatAwareStore {
    pub fn new(base_dir: PathBuf, format: SerializationFormat) -> std::io::Result<Self> {
        fs::create_dir_all(&base_dir)?;
        Ok(Self {
            base_dir,
            write_format: format,
        })
    }

    /// Save a session using the configured write format.
    pub fn save<T: Serialize>(&self, id: &str, data: &T) -> Result<(), String> {
        let path = self.base_dir.join(format!("{}.{}", id, self.write_format.extension()));
        let temp_path = path.with_extension("tmp");

        let bytes = self.write_format.serialize(data)?;

        let mut file = fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;
        file.write_all(&bytes)
            .map_err(|e| format!("Failed to write: {}", e))?;
        file.sync_all()
            .map_err(|e| format!("Failed to sync: {}", e))?;

        fs::rename(&temp_path, &path)
            .map_err(|e| format!("Failed to rename: {}", e))?;

        Ok(())
    }

    /// Load a session, auto-detecting the format from the file extension.
    pub fn load<T: DeserializeOwned>(&self, id: &str) -> Result<T, String> {
        // Try each format in order of preference
        for format in &[SerializationFormat::MessagePack, SerializationFormat::Json] {
            let path = self.base_dir.join(format!("{}.{}", id, format.extension()));
            if path.exists() {
                let bytes = fs::read(&path)
                    .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
                return format.deserialize(&bytes);
            }
        }
        Err(format!("No session file found for id: {}", id))
    }

    /// Export a session to a different format (useful for debugging).
    pub fn export<T: Serialize + DeserializeOwned>(
        &self,
        id: &str,
        target_format: SerializationFormat,
    ) -> Result<PathBuf, String> {
        let data: T = self.load(id)?;
        let export_path = self.base_dir.join(
            format!("{}.export.{}", id, target_format.extension())
        );
        let bytes = target_format.serialize(&data)?;
        fs::write(&export_path, bytes)
            .map_err(|e| format!("Failed to write export: {}", e))?;
        Ok(export_path)
    }
}

fn main() -> Result<(), String> {
    let temp_dir = std::env::temp_dir().join("format-demo");
    let store = FormatAwareStore::new(temp_dir, SerializationFormat::MessagePack)
        .map_err(|e| e.to_string())?;

    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "Hello!".to_string(),
            token_count: 3,
            priority: 1,
            is_summary: false,
        },
    ];

    // Save as MessagePack
    store.save("demo-session", &messages)?;
    println!("Saved as MessagePack");

    // Load back (auto-detects format)
    let loaded: Vec<Message> = store.load("demo-session")?;
    println!("Loaded {} messages", loaded.len());

    // Export to JSON for debugging
    let json_path: PathBuf = store.export::<Vec<Message>>("demo-session", SerializationFormat::Json)?;
    println!("Exported to JSON: {:?}", json_path);

    Ok(())
}
```

## Benchmarking Serialization Performance

For sessions with thousands of messages, serialization performance matters. Let's measure it:

```rust
use std::time::Instant;

fn benchmark_format(name: &str, messages: &[Message]) {
    // Serialization
    let start = Instant::now();
    let json_bytes = serde_json::to_vec(messages).unwrap();
    let json_ser_time = start.elapsed();

    let start = Instant::now();
    let msgpack_bytes = rmp_serde::to_vec(messages).unwrap();
    let msgpack_ser_time = start.elapsed();

    // Deserialization
    let start = Instant::now();
    let _: Vec<Message> = serde_json::from_slice(&json_bytes).unwrap();
    let json_de_time = start.elapsed();

    let start = Instant::now();
    let _: Vec<Message> = rmp_serde::from_slice(&msgpack_bytes).unwrap();
    let msgpack_de_time = start.elapsed();

    println!("{}:", name);
    println!("  JSON:    {:>8} bytes | ser {:>8.2?} | de {:>8.2?}",
        json_bytes.len(), json_ser_time, json_de_time);
    println!("  MsgPack: {:>8} bytes | ser {:>8.2?} | de {:>8.2?}",
        msgpack_bytes.len(), msgpack_ser_time, msgpack_de_time);
    println!("  Savings: {:.0}% smaller, {:.1}x faster ser, {:.1}x faster de",
        (1.0 - msgpack_bytes.len() as f64 / json_bytes.len() as f64) * 100.0,
        json_ser_time.as_nanos() as f64 / msgpack_ser_time.as_nanos().max(1) as f64,
        json_de_time.as_nanos() as f64 / msgpack_de_time.as_nanos().max(1) as f64,
    );
    println!();
}

fn main() {
    for &size in &[100, 1000, 5000] {
        let msgs = generate_conversation(size);
        benchmark_format(&format!("{} messages", size), &msgs);
    }
}
```

Typical results show MessagePack being 30--50% smaller and 1.5--3x faster for serialization. The speed difference matters most when auto-saving during a conversation -- you do not want a 100ms pause every time the agent saves state.

::: wild In the Wild
Claude Code stores session data in a compact format that prioritizes fast writes over human readability. Session files can be exported to JSON for debugging. OpenCode uses a similar approach with its session store, keeping active sessions in a binary format and only converting to JSON when the user requests session inspection. Both agents prioritize write speed because auto-save happens on the hot path of every tool execution.
:::

## Choosing Your Format

Here is a practical decision matrix:

| Criterion | JSON | MessagePack |
|-----------|------|-------------|
| Debuggability | Excellent -- read in any editor | Poor -- binary, need a viewer |
| File size | Larger (baseline) | 30--50% smaller |
| Serialize speed | Slower (baseline) | 1.5--3x faster |
| Ecosystem support | Universal | Wide but not universal |
| Schema evolution | Easy to inspect diffs | Harder to debug migrations |
| Streaming writes | Possible (line-delimited JSON) | Requires framing |

The recommended approach: use **MessagePack for active session storage** (it is on the hot path) and **JSON for exports, debugging, and initial development**. The `FormatAwareStore` you built above supports both seamlessly.

## Key Takeaways

- serde's format-agnostic design means switching between JSON and MessagePack requires changing one function call, not your data structures
- MessagePack produces 30--50% smaller files and serializes 1.5--3x faster than JSON, which matters for auto-save performance
- Build a dual-format store that writes in the fast format but can read and export in either format for debugging
- Auto-detect the serialization format from the file extension so your agent can read sessions saved in any format
- Always benchmark with realistic data -- performance characteristics depend heavily on message content (code-heavy sessions behave differently from prose)
