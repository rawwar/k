---
title: Lessons from OpenCode
description: Analyze OpenCode's Go-based architecture and design choices, learning from its approach to provider abstraction and terminal UI.
---

# Lessons from OpenCode

> **What you'll learn:**
> - How OpenCode's Go implementation approaches the same problems (provider abstraction, tool execution, TUI) with different language-level tradeoffs
> - What OpenCode's provider switching and multi-model support reveals about building flexible provider abstraction layers
> - The design patterns OpenCode uses for its terminal UI, session management, and LSP integration that differ from Rust-based approaches

OpenCode is an open-source CLI coding agent written in Go. It solves many of the same problems as Claude Code — tool execution, provider abstraction, conversation management, terminal rendering — but makes different architectural choices that reflect both Go's language characteristics and a different philosophy about agent design. Studying OpenCode alongside Claude Code reveals which patterns are universal to coding agents and which are a function of language, team, or context.

## Lesson 1: Provider Abstraction with Maximum Flexibility

OpenCode's most distinctive architectural choice is its provider abstraction layer. Where Claude Code is tightly coupled to the Anthropic API (for obvious reasons), OpenCode is provider-agnostic by design. It supports Anthropic, OpenAI, Google, Groq, AWS Bedrock, Azure OpenAI, local models via Ollama, and OpenAI-compatible endpoints — all behind a single interface.

The lesson is about how far to take provider abstraction. OpenCode's interface must accommodate the *union* of all provider capabilities. Some providers support streaming. Some support tool use. Some have different message formats. The abstraction must handle all combinations:

```rust
// Inspired by OpenCode's provider model
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn model_name(&self) -> &str;

    /// Not all providers support tool use
    fn supports_tool_use(&self) -> bool;

    /// Not all providers support streaming
    fn supports_streaming(&self) -> bool;

    /// The universal completion method
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Streaming variant (optional capability)
    async fn stream_completion(
        &self,
        request: CompletionRequest,
    ) -> Result<Box<dyn Stream<Item = Result<StreamChunk>> + Send + Unpin>>;

    /// Context window size varies by model
    fn context_window_size(&self) -> usize;

    /// Maximum output tokens varies by model
    fn max_output_tokens(&self) -> usize;
}

/// A provider-agnostic completion request
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: usize,
    pub temperature: f32,
    pub system_prompt: Option<String>,
}
```

OpenCode maintains a model registry — a catalog of known models with their capabilities, token limits, and pricing information. When the user switches models, the agent adjusts its behavior automatically: it knows that `gpt-4o` supports tool use but a smaller local model might not, and it adapts accordingly.

```rust
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub supports_tool_use: bool,
    pub supports_streaming: bool,
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
}

pub struct ModelRegistry {
    models: HashMap<String, ModelInfo>,
}

impl ModelRegistry {
    pub fn get(&self, model_id: &str) -> Option<&ModelInfo> {
        self.models.get(model_id)
    }

    pub fn models_for_provider(&self, provider: &str) -> Vec<&ModelInfo> {
        self.models.values()
            .filter(|m| m.provider == provider)
            .collect()
    }
}
```

The lesson for your agent: even if you start with a single provider, design the abstraction to accommodate multiple providers from the beginning. The cost of abstraction is low, and the cost of retrofitting it later is high.

::: tip In the Wild
OpenCode's model registry includes pricing information for each model, enabling it to show users the cost of each conversation in real time. The status bar displays running costs alongside token counts, which helps users make informed decisions about when to use expensive frontier models versus cheaper alternatives. This is a feature that falls naturally out of a well-designed provider abstraction — once you have a model catalog, cost tracking is a simple multiplication.
:::

## Lesson 2: Go's Concurrency Model vs. Rust's

OpenCode is written in Go, which gives it a fundamentally different concurrency story than Rust. Go's goroutines and channels make concurrent programming straightforward but without compile-time safety guarantees. Rust's async/await with ownership checking is more complex but catches data races at compile time.

This difference manifests in how each language handles shared state:

```go
// Go approach (OpenCode-style): channels for communication
type Agent struct {
    events chan Event
    state  *State // Protected by a mutex, but the compiler won't catch misuse
}
```

```rust
// Rust approach: ownership and lifetimes enforce correctness
pub struct Agent {
    context: Arc<RwLock<ContextManager>>,  // Compiler enforces lock discipline
    events: mpsc::Sender<Event>,            // Ownership of sender is tracked
}
```

The lesson is not that one language is better than the other. It is that Go's approach requires discipline (you must remember to lock the mutex), while Rust's approach requires ceremony (you must satisfy the borrow checker). For a coding agent where data races could cause file corruption or lost conversation state, Rust's compile-time guarantees are a meaningful safety advantage.

::: python Coming from Python
As a Python developer, Go's approach might feel more familiar. Python's threading model also relies on discipline — you protect shared state with `threading.Lock` and hope you did not miss a spot. Rust's ownership model is the unfamiliar one, and its learning curve is the price you pay for the guarantee that, once the code compiles, there are no data races. This is a one-time cost that pays off every time you refactor or add a new feature.
:::

## Lesson 3: The TUI as a Structured Interface

OpenCode invests heavily in its terminal user interface, built with the Bubble Tea framework (Go's equivalent of Ratatui). The TUI provides:

- **A message pane** showing the conversation with syntax-highlighted code blocks
- **A status bar** with model name, token usage, session info, and cost
- **A file change tracker** showing what files have been modified
- **Session management** with the ability to switch between conversations
- **Vim-like keybindings** for navigation

The lesson is that a well-designed TUI transforms the agent from a simple chat interface into a proper development tool. The status bar alone — showing token usage, active model, and session name — provides situational awareness that plain text output cannot match.

Here is how you might structure a similar status bar in Rust with `ratatui`:

```rust
use ratatui::{
    layout::{Constraint, Layout, Direction},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

fn render_status_bar(frame: &mut Frame, area: ratatui::layout::Rect, status: &AgentStatus) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .split(area);

    // Model name
    let model = Paragraph::new(format!(" {} ", status.model_name))
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(model, chunks[0]);

    // Token usage
    let tokens = Paragraph::new(format!(
        " Tokens: {} / {} ",
        status.tokens_used, status.max_tokens
    )).style(Style::default().fg(Color::Yellow));
    frame.render_widget(tokens, chunks[1]);

    // Session info
    let session = Paragraph::new(format!(" Session: {} ", status.session_name))
        .style(Style::default().fg(Color::Green));
    frame.render_widget(session, chunks[2]);

    // Cost (if available)
    let cost = Paragraph::new(format!(" ${:.4} ", status.estimated_cost))
        .style(Style::default().fg(Color::Magenta));
    frame.render_widget(cost, chunks[3]);
}
```

## Lesson 4: LSP Integration for Code Intelligence

OpenCode integrates with the Language Server Protocol (LSP) to provide code-aware features. Instead of relying solely on the LLM's understanding of code (which comes from the context window), OpenCode can query a language server for type information, definitions, and references.

This is a powerful pattern because it gives the agent structured knowledge about the codebase that complements the LLM's unstructured understanding. The model might hallucinate that a function exists, but the LSP can verify it.

```rust
// Hypothetical LSP integration for an agent
pub struct CodeIntelligence {
    lsp_client: Option<LspClient>,
}

impl CodeIntelligence {
    pub async fn find_definition(&self, file: &Path, position: Position) -> Option<Location> {
        self.lsp_client.as_ref()?.goto_definition(file, position).await.ok()
    }

    pub async fn find_references(&self, file: &Path, position: Position) -> Vec<Location> {
        self.lsp_client.as_ref()
            .map(|c| c.find_references(file, position))
            .unwrap_or_default()
    }

    pub async fn get_diagnostics(&self, file: &Path) -> Vec<Diagnostic> {
        self.lsp_client.as_ref()
            .map(|c| c.diagnostics(file))
            .unwrap_or_default()
    }
}
```

The lesson is that LSP integration is a force multiplier for code correctness. It enables tools that are impossible with text-only approaches: "find all callers of this function," "what type does this variable have," "are there any compile errors in this file." These become tools the agent can use, reducing hallucination and improving the quality of code modifications.

## Lesson 5: Session Persistence is Essential

OpenCode treats sessions as first-class entities. Every conversation is automatically saved. Users can list, resume, and branch sessions. This turns the agent from a ephemeral chat tool into a persistent workspace.

The lesson is that session persistence is not a nice-to-have — it is essential for real-world use. Developers work on tasks across multiple sessions (morning, afternoon, next day). Losing context between sessions forces them to re-explain the problem. With session persistence, they can pick up exactly where they left off.

```rust
pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            if entry.path().extension() == Some("json".as_ref()) {
                let summary = self.load_summary(&entry.path())?;
                sessions.push(summary);
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn resume_session(&self, id: &str) -> anyhow::Result<Session> {
        let path = self.sessions_dir.join(format!("{}.json", id));
        let content = std::fs::read_to_string(&path)?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }
}
```

## Lesson 6: Open Source Enables Customization

The most fundamental lesson from OpenCode is the value of being open source. Users can read the code, understand the architecture, modify the behavior, add tools, and contribute improvements. The agent becomes a framework, not just a product.

For your agent, this means designing with extensibility in mind. Even if you do not open-source the code, building it as if you might — with clean interfaces, documented extension points, and a plugin system — results in better architecture.

## Key Takeaways

- Design provider abstraction to accommodate the union of all provider capabilities from the start — OpenCode's multi-provider support shows that retrofitting flexibility is harder than building it in.
- A model registry with capabilities, token limits, and pricing enables runtime adaptation (adjusting behavior per model) and user-facing features like cost tracking.
- LSP integration gives the agent structured, verifiable knowledge about the codebase that complements the LLM's context-based understanding and reduces hallucination.
- Session persistence transforms a coding agent from an ephemeral chat tool into a persistent workspace where developers can resume complex tasks across multiple sittings.
- A well-designed TUI with status information (model, tokens, cost, session) provides situational awareness that makes the agent feel like a professional development tool rather than a chat interface.
