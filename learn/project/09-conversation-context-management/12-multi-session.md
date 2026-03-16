---
title: Multi Session
description: Manage multiple concurrent conversation sessions, each with independent context, for parallel workstreams.
---

# Multi Session

> **What you'll learn:**
> - How to implement a session manager that tracks multiple independent conversations
> - How to provide session naming, listing, and switching commands for the user
> - How to share common context like project configuration across sessions while keeping conversation state separate

A developer working on a complex project often juggles multiple tasks. They might have one conversation debugging a test failure, another exploring an API design, and a third drafting documentation. Multi-session support lets your agent handle all of these as separate conversations, each with its own context window and history, while sharing common project configuration.

## The Session Manager

The `SessionStore` from subchapter 5 handles persistence of individual sessions. The session manager sits on top of it, coordinating the lifecycle of multiple sessions and tracking which one is active:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Unique session identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

/// Lightweight handle for a session (full state stays on disk or in cache).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHandle {
    pub id: SessionId,
    pub name: String,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub message_count: usize,
    pub total_tokens: usize,
    pub model: String,
    pub working_directory: PathBuf,
}

/// Simplified session for the multi-session example.
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub messages: Vec<SessionMessage>,
    pub model: String,
    pub working_directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub token_count: usize,
}

impl Session {
    pub fn new(name: String, model: String, working_dir: PathBuf) -> Self {
        let now = SystemTime::now();
        let timestamp = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        Self {
            id: SessionId(format!("sess-{}", timestamp)),
            name,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            model,
            working_directory: working_dir,
        }
    }

    pub fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.token_count).sum()
    }

    pub fn to_handle(&self) -> SessionHandle {
        SessionHandle {
            id: self.id.clone(),
            name: self.name.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            message_count: self.messages.len(),
            total_tokens: self.total_tokens(),
            model: self.model.clone(),
            working_directory: self.working_directory.clone(),
        }
    }
}

/// Manages multiple sessions with one active at a time.
pub struct SessionManager {
    /// Currently loaded sessions (LRU cache in production)
    sessions: HashMap<SessionId, Session>,
    /// Which session is currently active
    active_session_id: Option<SessionId>,
    /// Default model for new sessions
    default_model: String,
    /// Working directory for new sessions
    working_directory: PathBuf,
}

impl SessionManager {
    pub fn new(model: String, working_dir: PathBuf) -> Self {
        Self {
            sessions: HashMap::new(),
            active_session_id: None,
            default_model: model,
            working_directory: working_dir,
        }
    }

    /// Create a new session and make it active.
    pub fn create_session(&mut self, name: &str) -> &Session {
        let session = Session::new(
            name.to_string(),
            self.default_model.clone(),
            self.working_directory.clone(),
        );
        let id = session.id.clone();
        self.sessions.insert(id.clone(), session);
        self.active_session_id = Some(id.clone());
        &self.sessions[&id]
    }

    /// Switch to an existing session by ID.
    pub fn switch_to(&mut self, id: &SessionId) -> Result<&Session, String> {
        if self.sessions.contains_key(id) {
            self.active_session_id = Some(id.clone());
            Ok(&self.sessions[id])
        } else {
            Err(format!("Session '{}' not found", id.0))
        }
    }

    /// Switch to a session by name (partial match).
    pub fn switch_by_name(&mut self, name: &str) -> Result<&Session, String> {
        let matches: Vec<SessionId> = self.sessions.values()
            .filter(|s| s.name.to_lowercase().contains(&name.to_lowercase()))
            .map(|s| s.id.clone())
            .collect();

        match matches.len() {
            0 => Err(format!("No session matching '{}'", name)),
            1 => {
                self.active_session_id = Some(matches[0].clone());
                Ok(&self.sessions[&matches[0]])
            }
            _ => {
                let names: Vec<&str> = matches.iter()
                    .map(|id| self.sessions[id].name.as_str())
                    .collect();
                Err(format!("Ambiguous: multiple sessions match '{}': {:?}", name, names))
            }
        }
    }

    /// Get a mutable reference to the active session.
    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        let id = self.active_session_id.clone()?;
        self.sessions.get_mut(&id)
    }

    /// Get an immutable reference to the active session.
    pub fn active_session(&self) -> Option<&Session> {
        let id = self.active_session_id.as_ref()?;
        self.sessions.get(id)
    }

    /// Add a message to the active session.
    pub fn push_message(&mut self, role: String, content: String, tokens: usize) -> Result<(), String> {
        let session = self.active_session_mut()
            .ok_or("No active session")?;
        session.messages.push(SessionMessage {
            role,
            content,
            token_count: tokens,
        });
        session.updated_at = SystemTime::now();
        Ok(())
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Vec<SessionHandle> {
        let mut handles: Vec<SessionHandle> = self.sessions.values()
            .map(|s| s.to_handle())
            .collect();
        handles.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        handles
    }

    /// Delete a session. Cannot delete the active session.
    pub fn delete_session(&mut self, id: &SessionId) -> Result<(), String> {
        if self.active_session_id.as_ref() == Some(id) {
            return Err("Cannot delete the active session. Switch to another first.".to_string());
        }
        self.sessions.remove(id)
            .ok_or(format!("Session '{}' not found", id.0))?;
        Ok(())
    }

    /// Rename a session.
    pub fn rename_session(&mut self, id: &SessionId, new_name: &str) -> Result<(), String> {
        let session = self.sessions.get_mut(id)
            .ok_or(format!("Session '{}' not found", id.0))?;
        session.name = new_name.to_string();
        Ok(())
    }
}

fn main() {
    let mut manager = SessionManager::new(
        "claude-sonnet-4-20250514".to_string(),
        PathBuf::from("/home/user/project"),
    );

    // Create multiple sessions
    manager.create_session("Debug test failure");
    manager.push_message("user".into(), "Why is test_auth failing?".into(), 8).unwrap();
    manager.push_message("assistant".into(), "Let me check the test...".into(), 10).unwrap();

    manager.create_session("API design");
    manager.push_message("user".into(), "Design the REST API".into(), 6).unwrap();

    manager.create_session("Documentation");
    manager.push_message("user".into(), "Write README".into(), 4).unwrap();

    // List sessions
    println!("Sessions:");
    for handle in manager.list_sessions() {
        let active = if manager.active_session_id.as_ref() == Some(&handle.id) { "*" } else { " " };
        println!("  {} {} - {} ({} msgs, {} tokens)",
            active, handle.id.0, handle.name, handle.message_count, handle.total_tokens);
    }

    // Switch sessions
    println!("\nSwitching to 'debug'...");
    match manager.switch_by_name("debug") {
        Ok(session) => println!("Now on: {} ({} messages)",
            session.name, session.messages.len()),
        Err(e) => println!("Error: {}", e),
    }
}
```

::: python Coming from Python
In Python, a session manager might use a simple dictionary:
```python
class SessionManager:
    def __init__(self):
        self.sessions = {}
        self.active = None

    def create(self, name):
        s = Session(name)
        self.sessions[s.id] = s
        self.active = s.id
```
The Rust version enforces at compile time that you handle the `None` case when
there is no active session. In Python, `self.sessions[self.active]` would raise a
`KeyError` at runtime if `self.active` is `None`. In Rust, the `Option` type
forces you to handle this case explicitly every time you access the active session.
:::

## Sharing Context Across Sessions

While each session has independent conversation history, some context should be shared. Project configuration, for example, applies to all sessions in the same project:

```rust
use std::sync::Arc;

/// Context that is shared across all sessions in a project.
#[derive(Debug, Clone)]
pub struct SharedContext {
    /// Project configuration (loaded once, shared by reference)
    pub project_config: Arc<String>,
    /// System prompt base (shared across sessions)
    pub base_system_prompt: Arc<String>,
    /// Available tool definitions (shared across sessions)
    pub tool_definitions: Arc<Vec<String>>,
}

impl SharedContext {
    pub fn new(config: String, system_prompt: String, tools: Vec<String>) -> Self {
        Self {
            project_config: Arc::new(config),
            base_system_prompt: Arc::new(system_prompt),
            tool_definitions: Arc::new(tools),
        }
    }

    /// Token cost of the shared context (this is "free" per-session
    /// since it only exists once in memory).
    pub fn token_cost(&self) -> usize {
        let config_tokens = (self.project_config.len() as f64 * 0.3) as usize;
        let prompt_tokens = (self.base_system_prompt.len() as f64 * 0.3) as usize;
        let tool_tokens: usize = self.tool_definitions.iter()
            .map(|t| (t.len() as f64 * 0.3) as usize)
            .sum();
        config_tokens + prompt_tokens + tool_tokens
    }
}

/// A session-aware manager that shares common context.
pub struct ContextAwareSessionManager {
    manager: SessionManager,
    shared: SharedContext,
}

impl ContextAwareSessionManager {
    pub fn new(manager: SessionManager, shared: SharedContext) -> Self {
        Self { manager, shared }
    }

    /// Get the effective context for the active session's next API call.
    pub fn build_api_context(&self) -> Option<ApiContext> {
        let session = self.manager.active_session()?;

        Some(ApiContext {
            system_prompt: self.shared.base_system_prompt.to_string(),
            tool_definitions: self.shared.tool_definitions.as_ref().clone(),
            messages: session.messages.clone(),
            shared_token_cost: self.shared.token_cost(),
            session_token_cost: session.total_tokens(),
        })
    }
}

#[derive(Debug)]
pub struct ApiContext {
    pub system_prompt: String,
    pub tool_definitions: Vec<String>,
    pub messages: Vec<SessionMessage>,
    pub shared_token_cost: usize,
    pub session_token_cost: usize,
}

fn main() {
    let shared = SharedContext::new(
        "Project: my-agent\nLanguage: Rust".to_string(),
        "You are a coding assistant.".to_string(),
        vec!["read_file".to_string(), "write_file".to_string()],
    );
    println!("Shared context: {} tokens (loaded once)", shared.token_cost());

    let mut manager = SessionManager::new(
        "claude-sonnet-4-20250514".to_string(),
        PathBuf::from("/home/user/project"),
    );
    manager.create_session("Session 1");
    manager.push_message("user".into(), "Hello".into(), 3).unwrap();

    let ctx_manager = ContextAwareSessionManager::new(manager, shared);

    if let Some(ctx) = ctx_manager.build_api_context() {
        println!("API context: {} shared + {} session = {} total tokens",
            ctx.shared_token_cost, ctx.session_token_cost,
            ctx.shared_token_cost + ctx.session_token_cost);
    }
}
```

Using `Arc` (Atomic Reference Counted) for shared context means the project configuration and tool definitions exist once in memory regardless of how many sessions are active. Each session gets a cheap clone of the `Arc` pointer, not a deep copy of the data.

## Session Commands

Add session management commands to your TUI:

```rust
/// Commands for session management.
pub enum SessionCommand {
    New { name: String },
    Switch { name: String },
    List,
    Rename { new_name: String },
    Delete { name: String },
}

pub fn parse_session_command(input: &str) -> Option<SessionCommand> {
    let parts: Vec<&str> = input.splitn(3, ' ').collect();

    match parts.get(0).map(|s| *s) {
        Some("/new") => {
            let name = parts.get(1..).map(|p| p.join(" "))
                .unwrap_or_else(|| "Untitled".to_string());
            Some(SessionCommand::New { name })
        }
        Some("/switch") => {
            let name = parts.get(1)?.to_string();
            Some(SessionCommand::Switch { name })
        }
        Some("/sessions") => Some(SessionCommand::List),
        Some("/rename") => {
            let new_name = parts.get(1..).map(|p| p.join(" "))?;
            Some(SessionCommand::Rename { new_name })
        }
        Some("/delete") => {
            let name = parts.get(1)?.to_string();
            Some(SessionCommand::Delete { name })
        }
        _ => None,
    }
}

fn main() {
    let commands = [
        "/new Debug authentication",
        "/sessions",
        "/switch debug",
        "/rename Fix login bug",
        "/new API design review",
        "/sessions",
    ];

    for cmd_str in &commands {
        println!("> {}", cmd_str);
        match parse_session_command(cmd_str) {
            Some(cmd) => println!("  Parsed: {:?}\n", match cmd {
                SessionCommand::New { ref name } => format!("New({})", name),
                SessionCommand::Switch { ref name } => format!("Switch({})", name),
                SessionCommand::List => "List".to_string(),
                SessionCommand::Rename { ref new_name } => format!("Rename({})", new_name),
                SessionCommand::Delete { ref name } => format!("Delete({})", name),
            }),
            None => println!("  Not a session command\n"),
        }
    }
}
```

::: wild In the Wild
Claude Code supports session resumption with the `--resume` flag, which lists recent sessions and lets the user pick one to continue. Each session is independent with its own conversation history and context window. The session list shows the project directory, time of last activity, and a preview of the last message. OpenCode provides a similar TUI for session browsing with fuzzy search by session name.
:::

## Key Takeaways

- Each session should have fully independent conversation state -- messages, token counts, and compaction status are per-session
- Use `Arc` (Atomic Reference Counted pointers) to share expensive context like project configuration and tool definitions across sessions without duplicating memory
- Support session operations through TUI commands: `/new`, `/switch`, `/sessions`, `/rename`, `/delete`
- Implement fuzzy name matching for `/switch` so users do not need to type exact session IDs
- Separate the session manager (in-memory coordination) from the session store (disk persistence) so each can be tested and evolved independently
