---
title: What We'll Build
description: A preview of the complete coding agent we will build throughout this tutorial track, including its architecture and capabilities.
---

# What We'll Build

> **What you'll learn:**
> - The high-level architecture of the coding agent we will implement step by step
> - The specific capabilities our agent will have: file operations, shell execution, search, and code editing
> - How each chapter of the tutorial maps to a component of the final system

## The Vision

Over the course of this tutorial track, you'll build a complete, working coding agent in Rust. Not a toy. Not a proof of concept. A real terminal-native agent that can read your codebase, write code, execute commands, run tests, and iterate on solutions — all orchestrated by a large language model.

By the end, you'll have a CLI tool that you type a natural language request into and watch as it reads files, thinks through the problem, writes a solution, tests it, and fixes any issues — the same fundamental experience that Claude Code, OpenCode, and Pi provide.

Let's walk through what this system looks like, starting from the outside and working our way in.

## The User Experience

You'll launch the agent from your terminal in a project directory:

```bash
$ my-agent
> Fix the failing test in tests/integration/test_users.rs
```

The agent begins working. You see streaming output as it thinks and acts:

```
Reading tests/integration/test_users.rs...
The test `test_create_user_with_duplicate_email` is failing because the
handler returns a 500 instead of a 409 Conflict.

Reading src/handlers/users.rs...
I see the issue. The `create_user` handler doesn't check for existing
users with the same email before inserting.

Writing src/handlers/users.rs...
Running cargo test...

1 test failed: test_list_users_pagination - the pagination offset is
now off by one because of the early return I added.

Writing src/handlers/users.rs...
Running cargo test...

All 23 tests passing. Here's what I changed:
- Added a duplicate email check in `create_user` that returns 409
- Fixed the pagination query to account for the new early return path
```

This is the experience we're building toward. The agent reads, thinks, acts, observes, and iterates — just like the production agents we studied.

## The Architecture

Our agent follows the five-component architecture we extracted from production agents:

### 1. The REPL Interface

The user-facing layer is a Read-Eval-Print Loop — a terminal interface where you type requests and see streaming responses. We'll build this with Rust's standard I/O initially, then upgrade to a richer terminal UI with Ratatui for features like syntax highlighting, scrollable history, and status indicators.

The REPL handles:
- Reading user input (with line editing and history).
- Rendering streaming text from the LLM.
- Displaying tool call notifications and results.
- Showing progress indicators during long operations.

### 2. The LLM Client

The LLM client communicates with the Anthropic API (Claude). It constructs messages in the format the API expects, sends them over HTTPS, and parses the streaming response. This is the reasoning engine of the agent — everything the agent "thinks" happens through this client.

The client handles:
- Message construction (system prompt, conversation history, tool definitions).
- HTTP request management with proper headers and authentication.
- Server-sent event (SSE) stream parsing for real-time output.
- Error handling for rate limits, network failures, and API errors.

```rust
// The core message types our client works with
struct Message {
    role: Role,
    content: Vec<ContentBlock>,
}

enum Role {
    User,
    Assistant,
}

enum ContentBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}
```

### 3. The Agentic Loop

The agentic loop is the orchestrator. It sits between the REPL, the LLM client, and the tool system, driving the perceive-reason-act cycle:

```rust
// Simplified view of our agentic loop
async fn agent_loop(
    client: &LlmClient,
    tools: &ToolDispatcher,
    messages: &mut Vec<Message>,
) -> Result<()> {
    loop {
        let response = client.send_message(messages).await?;

        // Display text content to the user
        display_text(&response);

        // Extract tool calls
        let tool_calls = extract_tool_calls(&response);

        if tool_calls.is_empty() {
            break; // Model is done — no more tool calls
        }

        // Execute each tool and collect results
        for call in tool_calls {
            let result = tools.dispatch(&call).await?;
            messages.push(tool_result_message(call.id, result));
        }

        // Add the assistant's response to history, then loop
        messages.push(assistant_message(response));
    }
    Ok(())
}
```

This simple loop structure drives the entire agent. The sophistication comes from the tools it dispatches and the model that decides what to do.

### 4. The Tool System

The tool system is the richest part of our architecture. We'll implement the tools that give the agent its capabilities:

**File Read (`read_file`):** Reads the contents of a file by path, with optional line range selection. This is the agent's primary perception tool — how it sees your code.

**File Write (`write_file`):** Creates or modifies files. We'll implement both full-file replacement (simpler) and diff-based editing (more precise). The agent uses this to make changes to your codebase.

**Shell Execute (`shell_exec`):** Runs a command in the user's shell and captures stdout, stderr, and the exit code. This is how the agent runs tests, builds projects, checks git status, and performs any operation that requires a command.

**Search (`search`):** Searches for patterns across files in the project, similar to `grep` or `ripgrep`. This lets the agent find relevant code without reading every file — crucial for navigating large codebases efficiently.

**List Directory (`list_dir`):** Lists the contents of a directory with file types and sizes. This gives the agent a map of the project structure.

Each tool implements a common trait:

```rust
#[async_trait]
trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    async fn execute(&self, params: Value) -> Result<ToolResult>;
}
```

::: python Coming from Python
This trait-based tool design is analogous to defining an abstract base class in Python:

```python
from abc import ABC, abstractmethod

class Tool(ABC):
    @abstractmethod
    def name(self) -> str: ...

    @abstractmethod
    def description(self) -> str: ...

    @abstractmethod
    def parameters_schema(self) -> dict: ...

    @abstractmethod
    async def execute(self, params: dict) -> ToolResult: ...
```

The Rust version provides stronger guarantees — if you forget to implement a method, the compiler tells you. The `Send + Sync` bounds ensure tools are safe to use across async tasks.
:::

### 5. Context Management

As the conversation grows, we need to manage the context window. Our agent will implement:

- **Token counting:** Estimating the token count of the current conversation to track context usage.
- **Context compaction:** When the conversation exceeds a threshold, summarizing earlier messages to free up space.
- **Smart truncation:** Handling oversized tool results (like the output of `cat` on a huge file) by truncating with a notice.

### Supporting Components

Beyond the five core components, we'll also build:

- **Configuration system:** TOML-based config for API keys, model selection, and preferences.
- **Permission system:** Tiered permissions for read, write, and execute operations.
- **Error handling infrastructure:** Custom error types with the `thiserror` crate for clear, recoverable error paths.
- **Logging:** Structured logging for debugging and session analysis.

## The Chapter Roadmap

Here's how the tutorial maps to the architecture:

| Chapter | Topic | What You Build |
|---------|-------|---------------|
| 1 | What Is a Coding Agent? | Mental model and architectural blueprint (this chapter) |
| 2 | Rust Fundamentals for Agents | Rust basics tailored for agent development |
| 3 | LLM Integration | HTTP client, API calls, streaming response parsing |
| 4 | The Agentic Loop | Core loop, message types, turn management |
| 5 | File Operations | Read, write, and list directory tools |
| 6 | Shell Execution | Process spawning, output capture, timeouts |
| 7 | Code Search | Pattern matching across files, search tool |
| 8 | Tool Dispatch | Trait-based dispatch, tool registry, parameter validation |
| 9 | Context Management | Token counting, compaction, smart truncation |
| 10 | Streaming UI | Ratatui-based terminal interface with streaming |
| 11 | Conversation Memory | Session persistence, conversation history |
| 12 | Permissions and Safety | Permission system, command classification, approval flow |
| 13 | Testing Your Agent | Integration tests, mock LLM, tool testing |
| 14 | Advanced Patterns | Multi-tool calls, error recovery, planning |

Each chapter builds on the previous ones. By Chapter 4, you'll have a working (if limited) agent that can chat. By Chapter 6, it can read files, write code, and run commands. By Chapter 10, it has a polished terminal interface. By Chapter 14, it handles edge cases and advanced scenarios that make it genuinely useful for real work.

::: wild In the Wild
The progression of our build mirrors how production agents were developed. Claude Code didn't ship with every feature on day one — it started as a basic REPL with API connectivity, then added tools, then streaming, then permissions, then polish. OpenCode's git history shows the same pattern: core loop first, tools second, UI third, refinements last. This layered approach isn't just pedagogically convenient — it's how real agents are built.
:::

## What We Won't Build

Setting expectations is as important as setting ambitions. Here's what falls outside our scope:

- **Multi-model support.** We'll target the Anthropic API specifically. Adding other providers is a great exercise once you have the core working, but starting provider-agnostic adds complexity without teaching new concepts.
- **Web browsing.** Some agents can open web pages and read documentation. We'll focus on filesystem and shell tools.
- **Visual output.** Image generation, diagram rendering, and rich media are out of scope. Our agent is text-focused.
- **Multi-agent orchestration.** Coordinating multiple agents is fascinating but requires a solid single-agent foundation first.

These aren't shortcomings — they're scope decisions. Each of these features could be added to the system we build, and you'll have the architectural understanding to do so. The foundation is what matters.

## The End Goal

When you finish this tutorial, you'll have:

1. **A working coding agent** — a real CLI tool that you can use for everyday development tasks.
2. **Deep architectural understanding** — knowledge of every component in the system and how they interact.
3. **A platform for experimentation** — a codebase you own completely, ready for customization and extension.
4. **Rust proficiency** — practical experience with async Rust, traits, enums, error handling, and systems programming.

You won't just understand how coding agents work. You'll have built one with your own hands.

Let's get started.

## Key Takeaways

- Our agent follows the five-component architecture shared by all production agents: REPL interface, LLM client, agentic loop, tool system, and context management.
- The tool system provides five core capabilities: file reading, file writing, shell execution, code search, and directory listing — sufficient for a wide range of real-world development tasks.
- The tutorial follows a layered build progression: core loop first, then tools, then UI, then safety, then polish — the same order that production agents were developed in.
- The agent targets the Anthropic API specifically, keeping the LLM integration focused while the architecture remains general enough to support other providers later.
- By the end, you'll have a working CLI agent, deep architectural knowledge, a customization platform, and practical Rust experience — four concrete outcomes from a single project.
