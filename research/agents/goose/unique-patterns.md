# Goose — Unique Patterns & Key Differentiators

## Overview

Goose makes several distinctive architectural choices that set it apart from other coding agents. The most significant is its wholesale adoption of MCP (Model Context Protocol) as the universal extension interface, combined with Block's enterprise-oriented perspective on agent safety and deployment.

## 1. MCP as the Universal Extension Interface

### The Pattern

While other agents (Claude Code, Codex CLI, Aider) build their tool systems as internal abstractions with MCP as an optional add-on, **Goose builds everything on MCP from the ground up**. There is no separate "built-in tool" layer — file editing, shell execution, code analysis, and memory are all MCP servers.

### Implementation

Every tool interaction flows through `McpClientTrait`:

```rust
#[async_trait]
pub trait McpClientTrait: Send + Sync {
    async fn list_tools(&self, ...) -> Result<ListToolsResult>;
    async fn call_tool(&self, ...) -> Result<CallToolResult>;
    async fn list_resources(...) -> Result<ListResourcesResult>;
    async fn read_resource(...) -> Result<ReadResourceResult>;
    async fn list_prompts(...) -> Result<ListPromptsResult>;
    async fn get_prompt(...) -> Result<GetPromptResult>;
    async fn subscribe(&self) -> mpsc::Receiver<ServerNotification>;
    async fn get_moim(&self, session_id) -> Option<String>;
}
```

This means:
- Built-in developer tools (shell, edit, write, tree) implement `McpClientTrait`
- Builtin MCP servers (computer controller, memory) run over DuplexStream
- External extensions (GitHub, Postgres) connect over stdio or HTTP
- All use the identical dispatch path in the agent loop

### Why This Matters

1. **Ecosystem leverage**: Any MCP server works with Goose immediately. The MCP ecosystem is growing rapidly.
2. **Consistent security**: The same inspection pipeline (security → adversary → permission → repetition) applies to ALL tools, regardless of origin.
3. **Extension composability**: Extensions can be mixed, matched, enabled/disabled without changing agent logic.
4. **Portability**: Goose's built-in MCP servers can run standalone with other MCP clients.

### Trade-offs

- **Performance overhead**: In-process tools go through MCP serialization even for simple operations. Platform extensions mitigate this by implementing `McpClientTrait` directly without transport.
- **Complexity**: The 7 transport types (Platform, Builtin, Stdio, StreamableHttp, InlinePython, Frontend, SSE) add complexity compared to a simple function-call interface.

## 2. Multi-Tier Extension Architecture

### The Pattern

Goose doesn't have a single extension type — it has a tiered hierarchy optimized for different use cases:

| Tier | Type | Transport | Access Level | Example |
|------|------|-----------|-------------|---------|
| 1 | Platform | Direct (in-process) | Full agent access | developer, analyze, summon |
| 2 | Builtin | DuplexStream (in-process MCP) | Standard MCP | computercontroller, memory |
| 3 | Stdio | Child process stdin/stdout | Standard MCP | npm/uvx packages |
| 4 | StreamableHttp | HTTP | Standard MCP | Remote services |
| 5 | InlinePython | Child process (uvx) | Standard MCP | User Python scripts |
| 6 | Frontend | UI channel | UI-only | Interactive widgets |

### Why This Matters

- **Platform extensions** get access to `PlatformExtensionContext`, enabling features like the Extension Manager (which can add/remove other extensions at runtime) and Summon (which can spawn subagents).
- **Builtin extensions** benefit from in-process efficiency while maintaining MCP protocol compliance.
- **Stdio/HTTP extensions** provide the plug-and-play ecosystem experience.
- **InlinePython** enables rapid prototyping of extensions without publishing packages.

## 3. MOIM (Model-Oriented Information Management)

### The Pattern

MOIM is a per-turn context injection mechanism. Before each LLM call, Goose queries all extensions for dynamic context via `get_moim()`, then injects it into the conversation.

### Implementation

```rust
// Each turn, before calling the LLM:
let conversation_with_moim = super::moim::inject_moim(
    &conversation, &extension_manager
).await;
```

Extensions implement:
```rust
async fn get_moim(&self, session_id: &str) -> Option<String>;
```

### Use Cases

- **Top of Mind** extension: Users set persistent instructions via environment variables that are injected every turn, ensuring the agent never "forgets" key requirements.
- Extensions can dynamically adjust their guidance based on session state.

### Why This Matters

This is a clean solution to the "context drift" problem — where important instructions get buried in long conversations. Instead of relying on the LLM to remember instructions from the beginning, MOIM re-injects them every turn.

## 4. Background Tool-Pair Summarization

### The Pattern

Most agents handle context overflow reactively (compact when you hit the limit). Goose additionally runs a **proactive background task** each turn that summarizes old tool request/response pairs.

### Implementation

```rust
// Runs as a background tokio task each turn
pub fn maybe_summarize_tool_pairs(
    provider: Arc<dyn Provider>,
    session_id: String,
    conversation: &Conversation,
    tool_call_cut_off: usize,
) -> JoinHandle<Vec<(usize, Message)>>
```

Old tool pairs are:
1. Summarized by the LLM into concise descriptions
2. Marked as `agent_invisible` (kept for UI, hidden from LLM)
3. Replaced with summary messages

### Why This Matters

Tool results are often the largest context consumers (think: full file contents, command outputs, search results). By progressively summarizing them, Goose maintains a much longer effective conversation history than agents that only compact reactively.

## 5. Recipe System with Retry Logic

### The Pattern

Goose supports "recipes" — automated task templates with success criteria, retry logic, and conversation reset. This enables unattended task execution.

### Implementation

```rust
pub struct RetryManager {
    attempts: Arc<Mutex<u32>>,
    repetition_inspector: Option<Arc<Mutex<Option<RepetitionInspector>>>>,
}
```

The retry flow:
1. Execute task according to recipe instructions
2. Run success checks (shell commands with timeouts)
3. If all pass → done
4. If any fail and attempts < max_retries:
   - Execute `on_failure` command if configured
   - **Reset conversation** to initial state
   - Increment attempts
   - Retry from scratch

### Why This Matters

Conversation reset is the key innovation. Instead of continuing a failing conversation (which often leads to deeper holes), Goose clears the slate and starts fresh. This is similar to how a human developer might "start over" when debugging gets too tangled.

This also enables CI/CD usage: Goose can be pointed at a task, left to retry autonomously, and report success/failure.

## 6. Subagent Delegation (Summon)

### The Pattern

The Summon extension enables the main agent to delegate tasks to subagents with isolated contexts. Each subagent can have its own set of extensions and instructions.

### Why This Matters

- **Context isolation**: Complex sub-tasks don't pollute the main conversation
- **Extension scoping**: A subagent for database work might only have the PostgreSQL extension
- **Skill/recipe loading**: Predefined task templates (called "skills") can be loaded on demand
- **Parallelism potential**: Multiple subagents could work on different aspects of a task

## 7. Enterprise Permission & Safety Model

### The Pattern

Goose implements a 4-level permission system designed for enterprise deployment:

```
Autonomous → Smart Approval → Manual Approval → Chat Only
(most permissive)                    (most restrictive)
```

Combined with:
- **Per-tool permissions**: Fine-grained allow/ask/deny per tool
- **`.gooseignore` files**: Restrict which files/directories Goose can access
- **4-inspector security pipeline**: Runs on every tool call
- **Malware detection**: Checks external extensions against known malware databases
- **Custom distributions**: Organizations can build branded Goose distributions with preconfigured providers, extensions, and security settings

### Why This Matters

Block (Square) is a financial technology company. Their enterprise perspective infuses Goose with security considerations that open-source-first agents often lack:

1. **Audit trail**: All tool calls are logged with inspection results
2. **Gradual trust**: Users can start in Manual Approval mode and relax as they gain confidence
3. **Mid-session switching**: Permission mode can change mid-conversation (`/mode approve`)
4. **Organizational control**: Custom distributions enable IT to pre-configure safety settings

## 8. Multi-Provider with Toolshim

### The Pattern

Goose supports 30+ LLM providers. For models that don't natively support tool calling (e.g., some Ollama models), it uses a "toolshim":

1. Tool definitions are converted to text instructions in the system prompt
2. The LLM's text output is post-processed to extract JSON tool calls from code blocks
3. These are then dispatched through the normal tool pipeline

### Why This Matters

This makes Goose truly provider-agnostic. Users can run local models via Ollama or LM Studio and still get tool-calling functionality, even if the model doesn't natively support function calling.

## 9. Agent Communication Protocol (ACP)

### The Pattern

Goose supports ACP (Agent Communication Protocol), enabling it to use other agents as providers:

```rust
// ACP providers in crates/goose-acp/
- Claude Code ACP → uses Claude Code as the "LLM"
- Codex ACP → uses OpenAI Codex as the "LLM"  
- Gemini ACP → uses Gemini CLI as the "LLM"
```

ACP providers pass Goose's extensions through to the underlying agent as MCP servers.

### Why This Matters

This creates an "agent-of-agents" pattern:
- Goose provides the extension/tool ecosystem
- The underlying agent (Claude Code, Codex, Gemini) provides the reasoning
- Extensions are shared, creating a unified experience regardless of which agent is doing the reasoning

## 10. Deep Link Extension Installation

### The Pattern

Extensions can be installed via URL deep links:

```
goose://extension?cmd=npx&arg=-y&arg=%40modelcontextprotocol/server-github&id=github&name=GitHub
```

### Why This Matters

This enables:
- **One-click installation** from documentation or web pages
- **Shareable configurations** via URLs
- **Extension directories** that provide install links
- **Onboarding flows** that set up entire development environments

## Comparison Matrix

| Pattern | Goose | Claude Code | Codex CLI | Aider |
|---------|-------|-------------|-----------|-------|
| MCP-native tools | ✅ Everything | ⚠️ Add-on | ⚠️ Limited | ❌ No |
| Multi-tier extensions | ✅ 7 types | ❌ 1 type | ❌ 1 type | ❌ N/A |
| MOIM (per-turn injection) | ✅ | ❌ | ❌ | ❌ |
| Background summarization | ✅ Tool-pairs | ❌ | ❌ | ❌ |
| Recipe + retry | ✅ With reset | ❌ | ❌ | ❌ |
| Subagent delegation | ✅ Summon | ❌ | ❌ | ❌ |
| Enterprise permissions | ✅ 4 modes | ⚠️ Basic | ⚠️ Basic | ❌ |
| Provider agnostic | ✅ 30+ | ❌ Anthropic only | ❌ OpenAI only | ✅ Many |
| Agent-as-provider (ACP) | ✅ | ❌ | ❌ | ❌ |
| Deep link install | ✅ | ❌ | ❌ | ❌ |
| Desktop app | ✅ Electron | ❌ CLI only | ❌ CLI only | ❌ CLI only |

## Summary

Goose's key differentiator is its **MCP-first architecture**. While other agents treat MCP as an integration layer bolted onto existing tool systems, Goose makes MCP the foundation. This creates a uniquely extensible and composable agent, at the cost of additional architectural complexity. Block's enterprise background adds a layer of safety and deployment sophistication that few open-source agents match.