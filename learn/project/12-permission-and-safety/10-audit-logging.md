---
title: Audit Logging
description: Recording every tool invocation, permission check, and approval decision in a structured audit log for debugging, compliance, and post-incident analysis.
---

# Audit Logging

> **What you'll learn:**
> - How to design a structured audit log schema that captures tool calls, parameters, and outcomes
> - Techniques for tamper-evident logging that ensures audit integrity
> - How to query audit logs for debugging failed operations and reviewing agent behavior

Every safety feature you have built so far is *preventive* — it stops bad things from happening. Audit logging is *detective* — it records what *did* happen so you can investigate when something goes wrong. A complete audit trail tells you exactly which tool calls the agent made, what parameters it used, whether they were approved, and what the outcome was.

This is not just for debugging. In a team environment, audit logs answer the question "what did the agent do to the codebase while I was away?" In a compliance context, they prove that safety checks were applied to every operation.

## The Audit Event Schema

A good audit log starts with a well-defined event schema. Each event should be self-contained — you should be able to understand what happened from a single event without reading the rest of the log:

```rust
use std::collections::HashMap;
use std::time::SystemTime;

/// A single event in the audit log.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Monotonically increasing event ID.
    pub id: u64,
    /// When the event occurred.
    pub timestamp: SystemTime,
    /// Category of the event.
    pub event_type: AuditEventType,
    /// The tool or system component that generated the event.
    pub source: String,
    /// Structured data specific to the event type.
    pub details: HashMap<String, String>,
    /// Whether the operation succeeded, failed, or was blocked.
    pub outcome: EventOutcome,
    /// The conversation turn that triggered this event.
    pub turn_id: Option<u64>,
    /// Session identifier for correlating events.
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEventType {
    /// A tool was invoked.
    ToolInvocation,
    /// A permission check was performed.
    PermissionCheck,
    /// User approval was requested.
    ApprovalRequest,
    /// User responded to an approval request.
    ApprovalResponse,
    /// A file checkpoint was created.
    CheckpointCreated,
    /// A checkpoint was restored (undo).
    CheckpointRestored,
    /// Permission level was changed.
    PermissionEscalation,
    /// A command was blocked by the safety filter.
    SafetyBlock,
    /// A sandbox violation was detected.
    SandboxViolation,
    /// Agent session started or ended.
    SessionLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventOutcome {
    Success,
    Failure(String),
    Blocked(String),
    Pending,
}

impl std::fmt::Display for AuditEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time_str = self.timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        write!(
            f,
            "[{}] #{} {:?} source={} outcome={:?}",
            time_str, self.id, self.event_type, self.source, self.outcome
        )?;

        if !self.details.is_empty() {
            let detail_str: Vec<String> = self
                .details
                .iter()
                .map(|(k, v)| {
                    let display_v = if v.len() > 100 {
                        format!("{}...", &v[..100])
                    } else {
                        v.clone()
                    };
                    format!("{}={}", k, display_v)
                })
                .collect();
            write!(f, " {}", detail_str.join(" "))?;
        }

        Ok(())
    }
}
```

::: python Coming from Python
In Python, you would typically use the `logging` module with a structured formatter:
```python
import logging
import json

logger = logging.getLogger("audit")
logger.info(json.dumps({
    "event": "tool_invocation",
    "tool": "write_file",
    "path": "src/main.rs",
    "outcome": "success",
}))
```
Rust does not have a built-in structured logging framework like Python's `logging`, but the `tracing` crate provides similar structured logging with better performance. For this chapter, we build a custom audit logger to make the design explicit, but in production you would likely integrate with `tracing`.
:::

## The Audit Logger

The `AuditLogger` collects events and provides methods to query them. It writes to both an in-memory buffer (for real-time queries) and optionally to a file (for persistence):

```rust
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Structured audit logger for agent operations.
pub struct AuditLogger {
    /// In-memory event buffer for real-time queries.
    events: Vec<AuditEvent>,
    /// Optional file path for persistent logging.
    log_file: Option<PathBuf>,
    /// Next event ID.
    next_id: AtomicU64,
    /// Current session ID.
    session_id: String,
}

impl AuditLogger {
    /// Create a new audit logger.
    pub fn new(session_id: &str, log_file: Option<PathBuf>) -> Self {
        Self {
            events: Vec::new(),
            log_file,
            next_id: AtomicU64::new(1),
            session_id: session_id.to_string(),
        }
    }

    /// Log an event. Writes to both memory and file (if configured).
    pub fn log(&mut self, event_type: AuditEventType, source: &str, details: HashMap<String, String>, outcome: EventOutcome, turn_id: Option<u64>) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let event = AuditEvent {
            id,
            timestamp: SystemTime::now(),
            event_type,
            source: source.to_string(),
            details,
            outcome,
            turn_id,
            session_id: self.session_id.clone(),
        };

        // Write to file if configured
        if let Some(ref path) = self.log_file {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(file, "{}", event);
            }
        }

        self.events.push(event);
    }

    /// Convenience method for logging a tool invocation.
    pub fn log_tool_call(
        &mut self,
        tool_name: &str,
        parameters: &[(String, String)],
        outcome: EventOutcome,
        turn_id: u64,
    ) {
        let mut details = HashMap::new();
        details.insert("tool".to_string(), tool_name.to_string());
        for (key, value) in parameters {
            details.insert(key.clone(), value.clone());
        }

        self.log(
            AuditEventType::ToolInvocation,
            tool_name,
            details,
            outcome,
            Some(turn_id),
        );
    }

    /// Convenience method for logging a permission check.
    pub fn log_permission_check(
        &mut self,
        tool_name: &str,
        required_level: &str,
        current_level: &str,
        allowed: bool,
    ) {
        let mut details = HashMap::new();
        details.insert("tool".to_string(), tool_name.to_string());
        details.insert("required_level".to_string(), required_level.to_string());
        details.insert("current_level".to_string(), current_level.to_string());

        let outcome = if allowed {
            EventOutcome::Success
        } else {
            EventOutcome::Blocked(format!(
                "Required {} but current level is {}",
                required_level, current_level
            ))
        };

        self.log(
            AuditEventType::PermissionCheck,
            "permission_gate",
            details,
            outcome,
            None,
        );
    }

    /// Convenience method for logging a safety block.
    pub fn log_safety_block(
        &mut self,
        command: &str,
        reason: &str,
        turn_id: u64,
    ) {
        let mut details = HashMap::new();
        details.insert("command".to_string(), command.to_string());
        details.insert("reason".to_string(), reason.to_string());

        self.log(
            AuditEventType::SafetyBlock,
            "safety_filter",
            details,
            EventOutcome::Blocked(reason.to_string()),
            Some(turn_id),
        );
    }

    /// Get all events.
    pub fn all_events(&self) -> &[AuditEvent] {
        &self.events
    }

    /// Get the total number of events logged.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}
```

## Querying the Audit Log

The audit log is only useful if you can search it effectively. Let's build a query interface:

```rust
/// Filter criteria for querying audit events.
#[derive(Debug, Default)]
pub struct AuditQuery {
    pub event_type: Option<AuditEventType>,
    pub source: Option<String>,
    pub outcome_type: Option<OutcomeFilter>,
    pub turn_id: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum OutcomeFilter {
    SuccessOnly,
    FailuresOnly,
    BlockedOnly,
}

impl AuditLogger {
    /// Query events matching the given criteria.
    pub fn query(&self, query: &AuditQuery) -> Vec<&AuditEvent> {
        let mut results: Vec<&AuditEvent> = self
            .events
            .iter()
            .filter(|event| {
                // Filter by event type
                if let Some(ref et) = query.event_type {
                    if event.event_type != *et {
                        return false;
                    }
                }

                // Filter by source
                if let Some(ref src) = query.source {
                    if event.source != *src {
                        return false;
                    }
                }

                // Filter by outcome
                if let Some(ref outcome_filter) = query.outcome_type {
                    match outcome_filter {
                        OutcomeFilter::SuccessOnly => {
                            if event.outcome != EventOutcome::Success {
                                return false;
                            }
                        }
                        OutcomeFilter::FailuresOnly => {
                            if !matches!(event.outcome, EventOutcome::Failure(_)) {
                                return false;
                            }
                        }
                        OutcomeFilter::BlockedOnly => {
                            if !matches!(event.outcome, EventOutcome::Blocked(_)) {
                                return false;
                            }
                        }
                    }
                }

                // Filter by turn
                if let Some(tid) = query.turn_id {
                    if event.turn_id != Some(tid) {
                        return false;
                    }
                }

                true
            })
            .collect();

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        results
    }

    /// Generate a summary of the session for review.
    pub fn session_summary(&self) -> String {
        let total = self.events.len();
        let tool_calls = self
            .events
            .iter()
            .filter(|e| e.event_type == AuditEventType::ToolInvocation)
            .count();
        let blocks = self
            .events
            .iter()
            .filter(|e| matches!(e.outcome, EventOutcome::Blocked(_)))
            .count();
        let approvals = self
            .events
            .iter()
            .filter(|e| e.event_type == AuditEventType::ApprovalResponse)
            .count();

        format!(
            "Session Summary:\n\
             Total events: {}\n\
             Tool invocations: {}\n\
             Blocked operations: {}\n\
             Approval decisions: {}",
            total, tool_calls, blocks, approvals
        )
    }
}
```

## Tamper-Evident Logging

For security-sensitive applications, you want assurance that the audit log has not been modified after the fact. A simple approach is to chain events with hashes, where each event includes the hash of the previous event:

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A tamper-evident wrapper around audit events.
#[derive(Debug, Clone)]
pub struct ChainedAuditEvent {
    pub event: AuditEvent,
    /// Hash of this event combined with the previous hash.
    pub chain_hash: u64,
    /// Hash of the previous event in the chain.
    pub previous_hash: u64,
}

/// Audit logger with hash chaining for tamper evidence.
pub struct ChainedAuditLogger {
    inner: AuditLogger,
    chain: Vec<ChainedAuditEvent>,
    last_hash: u64,
}

impl ChainedAuditLogger {
    pub fn new(session_id: &str, log_file: Option<PathBuf>) -> Self {
        Self {
            inner: AuditLogger::new(session_id, log_file),
            chain: Vec::new(),
            last_hash: 0, // Genesis hash
        }
    }

    /// Log an event with hash chaining.
    pub fn log_chained(
        &mut self,
        event_type: AuditEventType,
        source: &str,
        details: HashMap<String, String>,
        outcome: EventOutcome,
        turn_id: Option<u64>,
    ) {
        // Log to the inner logger first
        self.inner.log(
            event_type.clone(),
            source,
            details.clone(),
            outcome.clone(),
            turn_id,
        );

        // Get the event that was just logged
        let event = self.inner.events.last().unwrap().clone();

        // Compute chain hash
        let mut hasher = DefaultHasher::new();
        self.last_hash.hash(&mut hasher);
        event.id.hash(&mut hasher);
        event.source.hash(&mut hasher);
        let chain_hash = hasher.finish();

        let chained = ChainedAuditEvent {
            event,
            chain_hash,
            previous_hash: self.last_hash,
        };

        self.last_hash = chain_hash;
        self.chain.push(chained);
    }

    /// Verify the integrity of the audit chain.
    pub fn verify_chain(&self) -> bool {
        let mut expected_previous = 0u64;

        for entry in &self.chain {
            if entry.previous_hash != expected_previous {
                return false;
            }

            // Recompute the hash
            let mut hasher = DefaultHasher::new();
            expected_previous.hash(&mut hasher);
            entry.event.id.hash(&mut hasher);
            entry.event.source.hash(&mut hasher);
            let computed = hasher.finish();

            if entry.chain_hash != computed {
                return false;
            }

            expected_previous = entry.chain_hash;
        }

        true
    }
}
```

## Putting It All Together

Here is how the audit logger integrates with the safety pipeline you built in previous subchapters:

```rust
fn main() {
    let mut logger = AuditLogger::new(
        "session-001",
        Some(PathBuf::from("/tmp/agent-audit.log")),
    );

    // Simulate a session with various events

    // Turn 1: Read a file (succeeds)
    logger.log_tool_call(
        "read_file",
        &[("path".to_string(), "src/main.rs".to_string())],
        EventOutcome::Success,
        1,
    );

    // Turn 1: Write a file (succeeds after approval)
    logger.log_permission_check("write_file", "standard", "standard", true);
    logger.log_tool_call(
        "write_file",
        &[
            ("path".to_string(), "src/main.rs".to_string()),
            ("size".to_string(), "1024".to_string()),
        ],
        EventOutcome::Success,
        1,
    );

    // Turn 2: Dangerous command blocked
    logger.log_safety_block(
        "rm -rf /",
        "Recursive delete from root directory",
        2,
    );

    // Turn 3: Network command attempted
    logger.log_tool_call(
        "shell",
        &[("command".to_string(), "curl https://api.example.com".to_string())],
        EventOutcome::Blocked("Network command requires approval".to_string()),
        3,
    );

    // Print summary
    println!("{}\n", logger.session_summary());

    // Query for blocked operations
    let blocked = logger.query(&AuditQuery {
        outcome_type: Some(OutcomeFilter::BlockedOnly),
        ..Default::default()
    });

    println!("Blocked operations:");
    for event in blocked {
        println!("  {}", event);
    }

    // Query for all tool calls in turn 1
    let turn1 = logger.query(&AuditQuery {
        event_type: Some(AuditEventType::ToolInvocation),
        turn_id: Some(1),
        ..Default::default()
    });

    println!("\nTurn 1 tool calls:");
    for event in turn1 {
        println!("  {}", event);
    }
}
```

::: wild In the Wild
Claude Code logs every tool invocation with its parameters and outcome. The logs are stored locally and can be reviewed after a session to understand what the agent did. This is particularly useful for debugging when the agent made incorrect changes — you can trace back through the log to find the exact tool call that introduced the problem. Codex includes audit logging as part of its container-based execution model, where all filesystem changes are tracked at the OS level through copy-on-write filesystem layers.
:::

## Key Takeaways

- A structured audit log schema (event type, source, details, outcome, turn ID) makes events self-contained and queryable — you should be able to understand any event without reading the full log.
- Convenience methods like `log_tool_call` and `log_safety_block` reduce boilerplate at call sites, making it easy to instrument every safety-relevant operation.
- Hash chaining provides tamper evidence: if any event in the chain is modified, the chain verification fails, revealing the tampering.
- Query capabilities (by event type, outcome, source, turn) enable focused investigation during debugging and post-incident analysis.
- Audit logging is the final safety layer — it does not prevent damage, but it ensures you always know what happened, which is essential for learning from failures and improving the safety system over time.
