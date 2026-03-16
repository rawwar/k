---
title: Audit Trails
description: Build comprehensive logging and audit trail systems that record every agent action for debugging, compliance, and post-incident analysis.
---

# Audit Trails

> **What you'll learn:**
> - How to design structured audit logs that capture tool invocations, LLM decisions, permission checks, and user approvals
> - Techniques for correlating audit events across the agent loop to reconstruct the full decision chain for any action
> - How to implement tamper-resistant audit storage and retention policies suitable for compliance requirements

When something goes wrong with a coding agent -- and eventually something will -- the first question is always "what happened?" Audit trails answer that question by recording every significant event in the agent's lifecycle: every tool invocation, every permission check, every user approval, and every LLM decision. A good audit trail lets you reconstruct the complete causal chain from "the user typed a prompt" to "the agent deleted that file."

Audit trails serve three distinct purposes: real-time debugging (what is the agent doing right now?), post-incident analysis (what went wrong and how do we prevent it?), and compliance (can we prove that proper approvals were obtained?).

## Designing the Audit Event Schema

The foundation of any audit system is the event schema. Every event must be self-contained -- you should be able to understand what happened from a single event record without needing to look up context elsewhere:

```rust
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single audit event recording something that happened in the agent.
#[derive(Debug, Clone)]
struct AuditEvent {
    /// Unique identifier for this event
    event_id: String,
    /// When this event occurred (Unix timestamp in milliseconds)
    timestamp_ms: u64,
    /// Which session this event belongs to
    session_id: String,
    /// Which turn within the session (links events to conversation turns)
    turn_number: u32,
    /// Category of event for filtering
    event_type: EventType,
    /// The specific action or decision that occurred
    action: String,
    /// Outcome of the action
    outcome: EventOutcome,
    /// Structured metadata specific to this event type
    metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
enum EventType {
    /// A tool was invoked (or attempted)
    ToolInvocation,
    /// A permission check was performed
    PermissionCheck,
    /// User was prompted for approval
    ApprovalRequest,
    /// User responded to an approval prompt
    ApprovalResponse,
    /// LLM made a decision (tool call, reasoning step)
    LlmDecision,
    /// A checkpoint was created
    Checkpoint,
    /// A rollback was performed
    Rollback,
    /// An error occurred
    Error,
    /// Session lifecycle event (start, end)
    SessionLifecycle,
}

#[derive(Debug, Clone)]
enum EventOutcome {
    Success,
    Failure { reason: String },
    Denied { reason: String },
    Pending,
}

impl AuditEvent {
    fn new(
        session_id: &str,
        turn: u32,
        event_type: EventType,
        action: &str,
        outcome: EventOutcome,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            event_id: format!("{}-{}", timestamp, rand_suffix()),
            timestamp_ms: timestamp,
            session_id: session_id.to_string(),
            turn_number: turn,
            event_type,
            action: action.to_string(),
            outcome,
            metadata: HashMap::new(),
        }
    }

    fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Serialize the event to JSON for storage.
    fn to_json(&self) -> String {
        let metadata_str: Vec<String> = self.metadata
            .iter()
            .map(|(k, v)| format!("    \"{}\": \"{}\"", k, v))
            .collect();

        format!(
            r#"{{
  "event_id": "{}",
  "timestamp_ms": {},
  "session_id": "{}",
  "turn": {},
  "event_type": "{:?}",
  "action": "{}",
  "outcome": "{:?}",
  "metadata": {{
{}
  }}
}}"#,
            self.event_id,
            self.timestamp_ms,
            self.session_id,
            self.turn_number,
            self.event_type,
            self.action,
            self.outcome,
            metadata_str.join(",\n"),
        )
    }
}

/// Generate a simple random suffix for event IDs.
fn rand_suffix() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut hasher = DefaultHasher::new();
    now.hash(&mut hasher);
    format!("{:08x}", hasher.finish() & 0xFFFFFFFF)
}

fn main() {
    let session_id = "session-abc123";

    // Log a tool invocation
    let event = AuditEvent::new(
        session_id,
        1,
        EventType::ToolInvocation,
        "write_file(src/main.rs)",
        EventOutcome::Success,
    )
    .with_metadata("tool", "write_file")
    .with_metadata("path", "src/main.rs")
    .with_metadata("bytes_written", "1542");

    println!("{}\n", event.to_json());

    // Log a permission denial
    let event = AuditEvent::new(
        session_id,
        1,
        EventType::PermissionCheck,
        "shell(rm -rf /tmp/data)",
        EventOutcome::Denied {
            reason: "Command matches denylist pattern: rm -rf".into(),
        },
    )
    .with_metadata("tool", "shell")
    .with_metadata("command", "rm -rf /tmp/data")
    .with_metadata("denylist_rule", "rm -rf");

    println!("{}", event.to_json());
}
```

## The Audit Logger

The audit logger is the central component that receives events from all parts of the agent and writes them to storage. It must be lightweight enough to not slow down the agent, but reliable enough to never lose events:

```rust
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// The audit logger writes events to a structured log file.
struct AuditLogger {
    /// Path to the audit log file
    log_path: PathBuf,
    /// Buffer events before flushing (for performance)
    buffer: Mutex<Vec<String>>,
    /// Flush the buffer after this many events
    flush_threshold: usize,
    /// Session identifier for all events from this logger
    session_id: String,
}

impl AuditLogger {
    fn new(log_dir: &str, session_id: &str) -> Result<Self, String> {
        let log_dir = PathBuf::from(log_dir);
        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("Failed to create log directory: {}", e))?;

        let log_path = log_dir.join(format!("{}.jsonl", session_id));

        Ok(Self {
            log_path,
            buffer: Mutex::new(Vec::new()),
            flush_threshold: 10,
            session_id: session_id.to_string(),
        })
    }

    /// Log an event. The event is buffered and periodically flushed to disk.
    fn log(&self, event_json: &str) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push(event_json.to_string());

        if buffer.len() >= self.flush_threshold {
            self.flush_buffer(&mut buffer);
        }
    }

    /// Force all buffered events to be written to disk.
    fn flush(&self) {
        let mut buffer = self.buffer.lock().unwrap();
        self.flush_buffer(&mut buffer);
    }

    fn flush_buffer(&self, buffer: &mut Vec<String>) {
        if buffer.is_empty() {
            return;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .expect("Failed to open audit log");

        for event in buffer.drain(..) {
            // Write each event as a single line (JSONL format)
            let single_line = event.replace('\n', " ");
            writeln!(file, "{}", single_line).expect("Failed to write audit event");
        }
    }

    /// Read back all events from this session's log.
    fn read_events(&self) -> Result<Vec<String>, String> {
        let content = fs::read_to_string(&self.log_path)
            .map_err(|e| format!("Failed to read audit log: {}", e))?;

        Ok(content.lines().map(|l| l.to_string()).collect())
    }
}

impl Drop for AuditLogger {
    fn drop(&mut self) {
        // Ensure all events are flushed when the logger is dropped
        self.flush();
    }
}

fn main() {
    // In production, use a persistent directory like ~/.agent/audit/
    let logger = AuditLogger::new("/tmp/agent-audit", "session-demo")
        .expect("Failed to create logger");

    // Log some events
    logger.log(r#"{"event": "session_start", "timestamp": 1234567890}"#);
    logger.log(r#"{"event": "tool_invoke", "tool": "read_file", "path": "src/main.rs"}"#);
    logger.log(r#"{"event": "tool_invoke", "tool": "write_file", "path": "src/lib.rs"}"#);

    // Force flush
    logger.flush();

    println!("Audit log written to: /tmp/agent-audit/session-demo.jsonl");

    // Read back
    match logger.read_events() {
        Ok(events) => {
            println!("Logged {} events:", events.len());
            for event in &events {
                println!("  {}", event);
            }
        }
        Err(e) => println!("Error reading log: {}", e),
    }
}
```

## Correlating Events Across the Agent Loop

The real power of audit trails comes from correlation -- linking events together to understand the causal chain. Every tool invocation should link back to the LLM decision that triggered it, the permission check that authorized it, and the user approval (if any) that greenlit it:

```rust
use std::collections::HashMap;

/// A correlation context that threads through the entire processing pipeline.
#[derive(Debug, Clone)]
struct CorrelationContext {
    /// The session this belongs to
    session_id: String,
    /// The conversation turn
    turn_number: u32,
    /// A unique ID for this specific action chain
    trace_id: String,
    /// IDs of parent events in the causal chain
    parent_event_ids: Vec<String>,
}

impl CorrelationContext {
    fn new(session_id: &str, turn: u32) -> Self {
        Self {
            session_id: session_id.to_string(),
            turn_number: turn,
            trace_id: format!("trace-{}-{}", session_id, turn),
            parent_event_ids: Vec::new(),
        }
    }

    /// Create a child context linked to a parent event.
    fn child(&self, parent_event_id: &str) -> Self {
        let mut child = self.clone();
        child.parent_event_ids.push(parent_event_id.to_string());
        child
    }
}

/// Reconstruct the event chain for a specific action.
fn reconstruct_event_chain(
    events: &[HashMap<String, String>],
    target_trace_id: &str,
) -> Vec<String> {
    events
        .iter()
        .filter(|e| {
            e.get("trace_id")
                .map_or(false, |t| t == target_trace_id)
        })
        .map(|e| {
            format!(
                "[{}] {} -> {}",
                e.get("event_type").unwrap_or(&"?".to_string()),
                e.get("action").unwrap_or(&"?".to_string()),
                e.get("outcome").unwrap_or(&"?".to_string()),
            )
        })
        .collect()
}

fn main() {
    let ctx = CorrelationContext::new("session-123", 5);
    println!("Trace ID: {}", ctx.trace_id);

    // Simulate a chain of correlated events
    let events: Vec<HashMap<String, String>> = vec![
        HashMap::from([
            ("trace_id".into(), "trace-session-123-5".into()),
            ("event_type".into(), "LlmDecision".into()),
            ("action".into(), "Decided to write file src/main.rs".into()),
            ("outcome".into(), "tool_call_generated".into()),
        ]),
        HashMap::from([
            ("trace_id".into(), "trace-session-123-5".into()),
            ("event_type".into(), "PermissionCheck".into()),
            ("action".into(), "Check write permission for src/main.rs".into()),
            ("outcome".into(), "allowed".into()),
        ]),
        HashMap::from([
            ("trace_id".into(), "trace-session-123-5".into()),
            ("event_type".into(), "ApprovalRequest".into()),
            ("action".into(), "Prompt user for write approval".into()),
            ("outcome".into(), "approved_for_session".into()),
        ]),
        HashMap::from([
            ("trace_id".into(), "trace-session-123-5".into()),
            ("event_type".into(), "ToolInvocation".into()),
            ("action".into(), "write_file(src/main.rs)".into()),
            ("outcome".into(), "success".into()),
        ]),
    ];

    let chain = reconstruct_event_chain(&events, "trace-session-123-5");
    println!("\nEvent chain for trace-session-123-5:");
    for (i, event) in chain.iter().enumerate() {
        println!("  {}. {}", i + 1, event);
    }
}
```

::: tip In the Wild
Claude Code maintains a conversation log that records every message exchange and tool invocation. This serves as both a debugging aid and an audit trail -- if the agent produces unexpected results, the user can review the full conversation history to understand the reasoning chain. The logs include tool inputs and outputs, making it possible to replay the agent's decision process. Codex captures similar information and additionally logs the full diff of every file modification, providing a complete record of what changed and when.
:::

::: python Coming from Python
Python developers often use the `logging` module with handlers for different outputs (file, console, syslog). Rust's `tracing` crate provides a similar but more structured approach -- instead of formatted string messages, you emit structured events with typed fields. The `Mutex<Vec<String>>` pattern shown here is a simplified version of what `tracing` does internally. In production Rust code, you would use `tracing::info!()` with span contexts rather than building the buffering by hand. The advantage over Python's logging is that structured fields are preserved as data, not interpolated into strings, making them queryable.
:::

## Tamper-Resistant Storage

For audit logs to be trustworthy, they must be difficult to modify after the fact. While full tamper-proofing requires cryptographic solutions (like hash chains), there are simpler steps that raise the bar significantly:

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// An append-only audit log with integrity checking.
struct IntegrityCheckedLog {
    entries: Vec<LogEntry>,
}

#[derive(Debug, Clone)]
struct LogEntry {
    sequence: u64,
    content: String,
    /// Hash of (previous_hash + content) -- forms a chain
    hash: u64,
    previous_hash: u64,
}

impl IntegrityCheckedLog {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn append(&mut self, content: &str) {
        let previous_hash = self.entries.last().map_or(0, |e| e.hash);
        let sequence = self.entries.len() as u64;

        // Hash the chain: previous_hash + sequence + content
        let mut hasher = DefaultHasher::new();
        previous_hash.hash(&mut hasher);
        sequence.hash(&mut hasher);
        content.hash(&mut hasher);
        let hash = hasher.finish();

        self.entries.push(LogEntry {
            sequence,
            content: content.to_string(),
            hash,
            previous_hash,
        });
    }

    /// Verify the integrity of the entire log chain.
    fn verify_integrity(&self) -> bool {
        for (i, entry) in self.entries.iter().enumerate() {
            let expected_prev = if i == 0 { 0 } else { self.entries[i - 1].hash };

            if entry.previous_hash != expected_prev {
                println!("Integrity check failed at entry {}: previous hash mismatch", i);
                return false;
            }

            // Recompute the hash to verify
            let mut hasher = DefaultHasher::new();
            entry.previous_hash.hash(&mut hasher);
            entry.sequence.hash(&mut hasher);
            entry.content.hash(&mut hasher);
            let recomputed = hasher.finish();

            if recomputed != entry.hash {
                println!("Integrity check failed at entry {}: hash mismatch", i);
                return false;
            }
        }
        true
    }
}

fn main() {
    let mut log = IntegrityCheckedLog::new();

    log.append("session started");
    log.append("read_file(src/main.rs) -> success");
    log.append("write_file(src/lib.rs) -> success");
    log.append("shell(cargo test) -> success");

    println!("Log entries: {}", log.entries.len());
    println!("Integrity check: {}", log.verify_integrity());

    // Show the hash chain
    for entry in &log.entries {
        println!(
            "  #{}: hash={:016x} prev={:016x} \"{}\"",
            entry.sequence, entry.hash, entry.previous_hash, entry.content
        );
    }
}
```

## Key Takeaways

- Every audit event should be self-contained with a timestamp, session ID, turn number, event type, action description, outcome, and structured metadata
- JSONL (one JSON object per line) is the ideal format for audit logs because it supports append-only writes and is easy to parse, filter, and stream
- Correlation contexts (trace IDs and parent event IDs) link related events together, enabling reconstruction of the complete decision chain from LLM reasoning through permission checks to tool execution
- Hash-chained entries provide lightweight tamper detection -- if any entry is modified or deleted, the chain breaks and integrity verification fails
- Audit logs serve three purposes: real-time debugging, post-incident analysis, and compliance proof -- design the schema to support all three use cases from the start
