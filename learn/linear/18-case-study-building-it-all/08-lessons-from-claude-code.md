---
title: Lessons from Claude Code
description: Analyze Claude Code's architecture and design decisions, extracting lessons about tool design, safety, and developer experience.
---

# Lessons from Claude Code

> **What you'll learn:**
> - How Claude Code structures its tool system, permission model, and approval flows, and what design patterns are worth adopting
> - The architectural decisions behind Claude Code's streaming, context management, and multi-turn conversation handling
> - What Claude Code's approach to extensibility (MCP, skills, slash commands) reveals about building a platform vs building a tool

Claude Code is Anthropic's CLI-based coding agent. It runs in the terminal, reads and writes files, executes shell commands, and manages multi-turn conversations with the LLM. As the most visible production coding agent built by an AI lab, it embodies specific architectural opinions about how an agent should work. Some of those opinions are worth adopting wholesale. Others represent tradeoffs that make sense for Anthropic's context but may not be right for yours.

Let's analyze the key design decisions and extract the lessons.

## Lesson 1: The Tool System as a First-Class Citizen

Claude Code does not treat tools as an afterthought bolted onto a chat interface. Tools are the core mechanism through which the agent interacts with the world. Every meaningful action — reading a file, writing code, running tests, searching the codebase — goes through the tool system.

The tools in Claude Code include:

- **Read** — reads file contents at a given path
- **Write** — writes or creates files with specified content
- **Edit** — applies targeted edits to existing files (find and replace)
- **Bash** — executes shell commands
- **Glob** — finds files matching patterns
- **Grep** — searches file contents with regex
- **WebFetch** — retrieves web content
- **NotebookEdit** — modifies Jupyter notebooks

The lesson here is about *granularity*. Claude Code separates "write an entire file" (Write) from "edit a specific section" (Edit). This is not redundant — it reflects different use cases with different risk profiles. A full file write is appropriate for creating new files. A targeted edit is safer for modifying existing code because it changes only the specified section and fails if the target text is not found, preventing the model from accidentally overwriting unrelated code.

```rust
// Inspired by Claude Code's distinction between Write and Edit
pub enum FileOperation {
    /// Write the entire file (for new files or complete rewrites)
    WriteFile {
        path: PathBuf,
        content: String,
    },
    /// Edit a specific section (for modifications to existing files)
    EditFile {
        path: PathBuf,
        old_text: String,  // Must be found exactly once in the file
        new_text: String,
    },
}

impl FileOperation {
    pub fn execute(&self) -> anyhow::Result<String> {
        match self {
            Self::WriteFile { path, content } => {
                std::fs::write(path, content)?;
                Ok(format!("Wrote {} bytes to {}", content.len(), path.display()))
            }
            Self::EditFile { path, old_text, new_text } => {
                let content = std::fs::read_to_string(path)?;
                let count = content.matches(old_text).count();
                if count == 0 {
                    anyhow::bail!("old_text not found in {}", path.display());
                }
                if count > 1 {
                    anyhow::bail!(
                        "old_text found {} times in {} — must be unique",
                        count, path.display()
                    );
                }
                let new_content = content.replacen(old_text, new_text, 1);
                std::fs::write(path, &new_content)?;
                Ok(format!("Edited {}", path.display()))
            }
        }
    }
}
```

The uniqueness constraint on `EditFile`'s `old_text` is a safety mechanism. If the model provides text that matches multiple locations, the edit fails rather than making ambiguous changes. This is a subtle but important design decision.

::: wild In the Wild
Claude Code's Edit tool requires that the `old_string` parameter matches exactly once in the file. If it matches zero times, the edit fails — the model gets an error and can retry with corrected text. If it matches more than once, the edit also fails — the model must provide more surrounding context to make the match unique. This constraint prevents a large class of accidental edits where the model intends to change one occurrence but accidentally changes all of them.
:::

## Lesson 2: The Permission Model is Tiered

Claude Code's permission system is not a simple allow/deny gate. It operates on a spectrum of autonomy:

- **Always allowed**: Read-only operations like file reads, glob, grep, and web fetches are never blocked
- **Auto-approved with notification**: Some operations can be configured to auto-approve (e.g., writes within the project directory)
- **Requires approval**: Destructive operations prompt the user before execution
- **Always blocked**: Certain patterns are blocked entirely (e.g., modifying files outside the project root by default)

The lesson is that *binary permission models are too rigid for coding agents*. Users want maximum autonomy for safe operations and maximum control for dangerous ones. A one-size-fits-all "approve every tool call" model is exhausting. A "trust everything" model is dangerous. The tiered approach finds the practical middle ground.

```rust
pub struct PermissionPolicy {
    /// Operations that never need approval
    pub always_allow: Vec<PermissionRule>,
    /// Operations that auto-approve but log
    pub auto_approve: Vec<PermissionRule>,
    /// Operations that always need user approval
    pub require_approval: Vec<PermissionRule>,
    /// Operations that are always blocked
    pub always_deny: Vec<PermissionRule>,
}

impl PermissionPolicy {
    pub fn evaluate(&self, tool_call: &ToolCall) -> Permission {
        // Check deny rules first (highest priority)
        if self.always_deny.iter().any(|r| r.matches(tool_call)) {
            return Permission::Denied("Blocked by policy".into());
        }
        // Then allow rules
        if self.always_allow.iter().any(|r| r.matches(tool_call)) {
            return Permission::Allowed;
        }
        // Then auto-approve rules
        if self.auto_approve.iter().any(|r| r.matches(tool_call)) {
            return Permission::Allowed; // But log it
        }
        // Default: require approval
        Permission::NeedsApproval
    }
}
```

## Lesson 3: Context Management is Invisible to the User

Claude Code handles context window limits transparently. When the conversation approaches the context limit, it compacts the history automatically. The user never sees a "context window exceeded" error. They might notice that the agent seems to have forgotten something from early in the conversation, but the interaction continues without interruption.

The lesson is that context management should be *infrastructure, not interface*. Users should not need to think about token counts, context windows, or compaction strategies. These are implementation details of the LLM that the agent abstracts away. When compaction happens, a subtle notification is appropriate ("Context compacted to continue the conversation"), but it should never require user action.

## Lesson 4: Extensibility Through MCP

Claude Code supports the Model Context Protocol (MCP), which allows external servers to provide additional tools. This transforms Claude Code from a fixed-capability tool into an extensible platform. Users can connect MCP servers that provide database access, cloud deployment, issue tracking, or any other capability — without modifying Claude Code's source code.

The lesson is about the *build vs. extend* decision. Claude Code's built-in tools cover the essential file and shell operations that every coding task needs. Everything else is extensible. This keeps the core agent focused while allowing unlimited expansion.

```rust
// The extensibility boundary — MCP tools implement the same trait as built-in tools
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    async fn execute(&self, params: serde_json::Value) -> anyhow::Result<String>;
}

// Built-in tool
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    // ...
}

// MCP-provided tool (same trait, different source)
pub struct McpTool {
    server: McpConnection,
    definition: ToolDefinition,
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str { &self.definition.name }
    // Delegates execution to the MCP server
    async fn execute(&self, params: serde_json::Value) -> anyhow::Result<String> {
        self.server.call_tool(&self.definition.name, params).await
    }
}
```

From the agentic loop's perspective, built-in tools and MCP tools are indistinguishable. This is the power of trait-based abstraction.

::: python Coming from Python
MCP is conceptually similar to Python's plugin systems (like `setuptools` entry points or `pluggy`), but it operates over a protocol boundary rather than in-process. An MCP server is a separate process (potentially in any language) that communicates with the agent via JSON-RPC over stdio or HTTP. This means you can extend a Rust agent with a Python MCP server, combining the strengths of both languages.
:::

## Lesson 5: The CLI is the Interface

Claude Code chose the terminal as its primary interface, not an IDE plugin or a web app. This is a deliberate architectural decision with important implications:

- **Universal compatibility** — works in any terminal, on any OS, with any editor
- **Composability** — can be invoked from scripts, CI pipelines, and other tools
- **Low overhead** — no GUI framework, no browser, no extension runtime
- **Developer familiarity** — developers already live in the terminal

The lesson for your agent is that the terminal is a surprisingly powerful interface. With streaming, syntax highlighting, and status indicators, a CLI agent can feel responsive and informative without the complexity of a GUI. The TUI mode (built with `ratatui`) adds structure when needed without abandoning the terminal.

## Lesson 6: The System Prompt is Architecture

Claude Code's system prompt is not a throwaway instruction — it is a carefully engineered document that shapes the agent's behavior in ways that code alone cannot. It defines the agent's persona, establishes rules about file operations, sets expectations for tool usage, and provides context about the user's environment.

The lesson is that the system prompt is as much a part of your agent's architecture as the code. Treat it as a versioned, tested artifact. Changes to the system prompt can dramatically alter agent behavior — for better or worse — and should go through the same review process as code changes.

```rust
fn build_system_prompt(config: &Config, project_context: &ProjectContext) -> String {
    format!(
        "{base}\n\n{environment}\n\n{rules}",
        base = include_str!("prompts/system.md"),
        environment = format!(
            "You are working in: {}\nPlatform: {}\nShell: {}",
            project_context.root_dir.display(),
            std::env::consts::OS,
            project_context.shell,
        ),
        rules = include_str!("prompts/safety_rules.md"),
    )
}
```

## Key Takeaways

- Design tools with appropriate granularity — Claude Code's separation of Write (full file) and Edit (targeted replacement with uniqueness constraints) prevents a class of accidental overwrites that a single "write file" tool would allow.
- Implement a tiered permission model (always allow, auto-approve, require approval, always deny) rather than a binary allow/deny gate — this balances user convenience for safe operations with safety for dangerous ones.
- Make context management invisible to the user — handle compaction automatically and transparently, notifying but never requiring action when the context window is managed.
- Use trait-based abstraction so that MCP-provided tools and built-in tools implement the same interface, making the extensibility boundary invisible to the agentic loop.
- Treat the system prompt as a versioned architectural artifact, not a casual instruction — changes to the prompt affect agent behavior as much as code changes and deserve the same rigor.
