---
title: "Tool System Design Patterns"
---

# Tool System Design Patterns

How coding agents structure their tool systems — from compile-time trait definitions to fully dynamic MCP ecosystems.

Every coding agent needs a way to give the LLM actions it can take in the world: read files, write code, run commands, search the web. The design of this "tool system" is one of the most consequential architectural decisions an agent makes. It determines who can extend the agent, how safe it is, how testable it is, and what happens when things go wrong.

After studying the internals of OpenCode, Codex, Goose, Claude Code, OpenHands, Aider, Gemini CLI, Amp, and others, six distinct patterns emerge. Most production agents use two or three in combination.

---

## Pattern 1: Trait/Interface-Based Tools

The most common pattern in compiled-language agents. Tools are defined as implementations of a shared interface or trait, giving compile-time guarantees that every tool provides the required metadata and execution logic.

### Go Interfaces — OpenCode

OpenCode defines a `BaseTool` interface that every tool must satisfy:

```go
// tool.go — the contract every tool implements
type BaseTool interface {
    Info() ToolInfo
    Run(ctx context.Context, call ToolCall) (ToolResult, error)
}

type ToolInfo struct {
    Name        string
    Description string
    Parameters  map[string]Parameter
    Required    []string
}

type ToolCall struct {
    ID        string
    Name      string
    Arguments json.RawMessage
}
```

Each tool is a standalone struct implementing this interface:

```go
// tools/bash.go
type BashTool struct {
    workDir string
    env     []string
}

func (b *BashTool) Info() ToolInfo {
    return ToolInfo{
        Name:        "bash",
        Description: "Execute a bash command in the working directory",
        Parameters: map[string]Parameter{
            "command": {Type: "string", Description: "The command to execute"},
        },
        Required: []string{"command"},
    }
}

func (b *BashTool) Run(ctx context.Context, call ToolCall) (ToolResult, error) {
    var args struct{ Command string }
    json.Unmarshal(call.Arguments, &args)
    // ... execute command, capture output ...
    return ToolResult{Content: output}, nil
}
```

The agent collects tools into a slice and iterates at dispatch time. The interface is minimal — just enough to describe and execute.

### Rust Traits — Codex

Codex (OpenAI's open-source CLI agent) uses Rust's type system more aggressively. Rather than a single tool trait, it defines a `ToolRouter` that pattern-matches on response item variants:

```rust
// codex-rs/core/src/tool_router.rs
pub struct ToolRouter {
    registry: ToolRegistry,
}

impl ToolRouter {
    pub async fn build_tool_call(
        &self,
        item: &ResponseItem,
    ) -> Result<ToolCall> {
        match item {
            ResponseItem::FunctionCall { name, arguments, call_id, .. } => {
                // Dispatch to registered function tools
                self.registry.resolve_function(name, arguments, call_id)
            }
            ResponseItem::LocalShellCall { command, call_id, .. } => {
                // Direct shell execution — a first-class concept
                Ok(ToolCall::Shell { command: command.clone(), call_id: call_id.clone() })
            }
            ResponseItem::CustomToolCall { tool_type, arguments, call_id, .. } => {
                // MCP or user-defined tools
                self.registry.resolve_custom(tool_type, arguments, call_id)
            }
        }
    }
}
```

The key insight: Codex uses Rust enums to make tool *categories* a type-level concept. A `FunctionCall` is fundamentally different from a `LocalShellCall` — they have different security models, different execution paths, and different rollback semantics. The type system enforces this.

### Rust Traits — Ante

Ante (another Rust agent) takes the classic trait approach, closer to Go's interface pattern:

```rust
// ante/src/tools/mod.rs
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn call(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput>;
}
```

The `Send + Sync` bounds are crucial — they guarantee tools can be shared across async tasks and threads. This enables concurrent tool execution without runtime locks.

### Rust Traits — Goose

Goose wraps MCP client interactions behind a trait:

```rust
// goose/crates/goose/src/agents/extension.rs
#[async_trait]
pub trait McpClientTrait: Send + Sync {
    async fn list_tools(&self) -> Result<Vec<Tool>>;
    async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<Vec<Content>>;
    async fn list_resource_templates(&self) -> Result<Vec<ResourceTemplate>>;
}
```

This trait abstracts over different MCP transport mechanisms. Whether a tool runs in-process, over stdio, or via HTTP — the calling code sees the same interface.

### Strengths of Trait/Interface-Based Tools

| Strength | Why It Matters |
|----------|---------------|
| **Compile-time safety** | Missing methods are caught before deployment, not at runtime |
| **Clear contracts** | New contributors know exactly what to implement |
| **Testability** | Mock the interface, test dispatch logic independently |
| **Dependency injection** | Swap implementations for testing, sandboxing, or A/B tests |
| **IDE support** | Go-to-definition, find-all-references work perfectly |
| **Performance** | Static dispatch (Go interfaces, Rust trait objects) avoids hash lookups |

### Weaknesses

| Weakness | Impact |
|----------|--------|
| **Adding tools requires code changes** | Can't add a tool without modifying the agent's source |
| **Recompilation required** | Rust agents need a full rebuild for any tool change |
| **Less accessible** | Non-core contributors face a high barrier to adding tools |
| **Monolithic growth** | Tool count bloats the binary and compilation time |
| **Language lock-in** | Tools must be written in the agent's implementation language |

---

## Pattern 2: Registry with Dispatch

A registry pattern decouples tool *registration* from tool *invocation*. Tools are collected into a central data structure, and a dispatcher routes calls by name. This is the most universal pattern — nearly every agent uses some form of it.

### OpenCode's Linear Dispatch

The simplest form. OpenCode stores tools in a flat slice and dispatches via linear scan:

```go
// agent.go — tool dispatch
func (a *Agent) executeTool(call ToolCall) (ToolResult, error) {
    for _, tool := range a.tools {
        if tool.Info().Name == call.Name {
            return tool.Run(a.ctx, call)
        }
    }
    return ToolResult{}, fmt.Errorf("unknown tool: %s", call.Name)
}
```

With 10-20 tools, this is perfectly fast. The simplicity makes debugging trivial — add a log line and you see every dispatch attempt.

### Goose's Namespace-Based Dispatch

Goose has a more sophisticated problem: tools come from multiple MCP extensions, and tool names can collide. Solution — namespace them:

```rust
// When registering tools from an extension named "developer":
// tool "bash" becomes "developer__bash"
// tool "read_file" becomes "developer__read_file"

fn dispatch_tool_call(&self, tool_name: &str, args: Value) -> Result<Vec<Content>> {
    let parts: Vec<&str> = tool_name.splitn(2, "__").collect();
    match parts.as_slice() {
        [extension_name, local_tool_name] => {
            let ext = self.extensions.get(*extension_name)
                .ok_or_else(|| anyhow!("Unknown extension: {}", extension_name))?;
            ext.call_tool(local_tool_name, args).await
        }
        _ => Err(anyhow!("Invalid tool name format: {}", tool_name)),
    }
}
```

The `__` separator is a convention that prevents collisions while preserving readability. The LLM sees `developer__bash` and understands both the source and the action.

### Gemini CLI's ToolRegistry

Gemini CLI takes the registry pattern further with a distinction between static and dynamic tools:

```typescript
// src/core/tool-registry.ts
class ToolRegistry {
    private builtinTools: Map<string, ToolDefinition>;
    private mcpTools: Map<string, McpToolProxy>;
    private customTools: Map<string, CustomToolDefinition>;

    getAllTools(): ToolDefinition[] {
        return [
            ...this.builtinTools.values(),
            ...this.mcpTools.values(),
            ...this.customTools.values(),
        ];
    }

    resolve(name: string): ToolExecutor | undefined {
        return this.builtinTools.get(name)
            ?? this.mcpTools.get(name)
            ?? this.customTools.get(name);
    }
}
```

The resolution order matters: built-in tools shadow MCP tools which shadow custom tools. This prevents an MCP server from overriding `read_file` with a malicious implementation.

### Codex's Enum-Level Type Distinction

Codex combines the trait pattern with registry-level type categorization. Before a tool even reaches the registry, its *category* is determined by the response item variant:

```
ResponseItem::FunctionCall    → ToolRegistry (registered functions)
ResponseItem::LocalShellCall  → Direct shell execution
ResponseItem::CustomToolCall  → MCP / external tools
ResponseItem::ToolSearchCall  → Dynamic tool discovery
```

This is a two-tier dispatch: first by category (enum match), then by name (registry lookup). The category determines the security model — a `LocalShellCall` goes through sandboxing, while a `FunctionCall` runs in-process.

### Claude Code's Tool Categorization

Claude Code organizes tools into functional categories with different permission models:

| Category | Examples | Permission Model |
|----------|----------|-----------------|
| **Read** | `read_file`, `list_files`, `search_files` | Always allowed |
| **Write** | `write_file`, `edit_file` | Requires confirmation or allowlist |
| **Execute** | `bash`, `execute_command` | Requires confirmation, shows command |
| **Agent** | `dispatch_agent`, `task` | Sub-agent spawning, inherits permissions |
| **MCP** | Dynamic from MCP servers | Server-specific trust levels |

The categorization isn't just organizational — it drives the permission system. Read tools never trigger confirmation dialogs. Write tools check against path allowlists. Execute tools show the exact command for user approval.

### Registry Pattern Comparison

| Agent | Registry Type | Dispatch | Namespace | Priority |
|-------|--------------|----------|-----------|----------|
| OpenCode | Flat slice | Linear scan | None | First match |
| Goose | Extension map | `__` split | Extension name | Explicit |
| Gemini CLI | Layered maps | Fallthrough | Source type | Built-in > MCP > Custom |
| Codex | Enum + registry | Two-tier | Category + name | Category first |
| Claude Code | Categorized map | Category + name | Category | Category-based ACL |

---

## Pattern 3: Action/Observation Model

OpenHands takes a fundamentally different approach. Tools aren't executable objects at all — they're schemas that the LLM uses to generate structured output. Execution happens in a completely separate layer.

### The Pipeline

```
LLM generates tool_call
       ↓
response_to_actions()    ← Converts tool calls to Action dataclasses
       ↓
Action (e.g., CmdRunAction)
       ↓
Runtime.execute(action)  ← Could be Docker, local, Kubernetes, E2B...
       ↓
Observation (e.g., CmdOutputObservation)
       ↓
Event Stream             ← Serialized, replayable, auditable
       ↓
Back to LLM as context
```

### Action/Observation Pairs

Each tool maps to a specific Action class, and each Action produces a specific Observation:

```python
# openhands/events/action/commands.py
@dataclass
class CmdRunAction(Action):
    command: str
    thought: str = ""
    blocking: bool = False
    keep_prompt: bool = True

    @property
    def message(self) -> str:
        return f"Running command: {self.command}"

# openhands/events/observation/commands.py
@dataclass
class CmdOutputObservation(Observation):
    command: str
    exit_code: int = 0
    command_id: int = -1

    @property
    def message(self) -> str:
        return f"Command `{self.command}` exited with code {self.exit_code}"
```

The mapping is comprehensive:

| Tool Schema | Action | Observation |
|-------------|--------|-------------|
| `execute_bash` | `CmdRunAction` | `CmdOutputObservation` |
| `str_replace_editor` | `FileEditAction` | `FileEditObservation` |
| `browser` | `BrowseInteractiveAction` | `BrowserOutputObservation` |
| `finish` | `AgentFinishAction` | `AgentStateChangedObservation` |
| `message_user` | `MessageAction` | `UserMessageObservation` |

### Why This Works

The Action/Observation pattern enables capabilities that other patterns struggle with:

**1. Event Stream Serialization**
Every action and observation is serialized to an event stream. Sessions can be paused, persisted, and resumed. You can replay an entire agent session from the event log.

**2. Session Replay**
Because actions are data (not code), you can replay a session against a different runtime, a different model, or with modified actions. This is invaluable for debugging and evaluation.

**3. Security Interception**
A security layer can inspect every Action before it reaches the runtime. The `SecurityAnalyzer` checks actions against policies without modifying the tool schema or execution logic:

```python
class SecurityAnalyzer:
    async def check(self, action: Action) -> SecurityResult:
        if isinstance(action, CmdRunAction):
            if "rm -rf /" in action.command:
                return SecurityResult.BLOCK
        return SecurityResult.ALLOW
```

**4. Runtime-Agnostic Execution**
The same `CmdRunAction` executes identically whether the runtime is:
- A local Docker container
- A remote Kubernetes pod
- A cloud sandbox (E2B, Modal)
- The user's local machine (with appropriate guards)

The Action is the abstraction boundary. The Runtime is the strategy.

### Trade-offs

The indirection has costs. Adding a new tool requires:
1. Define the tool schema (for the LLM)
2. Create an Action dataclass
3. Create an Observation dataclass
4. Implement `response_to_actions()` mapping
5. Implement runtime execution for the action
6. Handle the observation in the agent loop

That's six touch points vs. one for a simple trait implementation. The overhead is justified when you need the serialization, replay, and runtime-agnostic properties.

---

## Pattern 4: MCP-Native Architecture

The Model Context Protocol (MCP) standardizes how tools are discovered, described, and invoked. Some agents treat MCP as an add-on; Goose treats it as the foundation.

### Goose's Defining Decision

In Goose, *everything* is an MCP server — even built-in tools:

```rust
// goose/crates/goose/src/agents/extension.rs
pub struct ExtensionManager {
    extensions: HashMap<String, Box<dyn McpClientTrait>>,
}

impl ExtensionManager {
    pub async fn add_extension(
        &mut self,
        name: &str,
        config: ExtensionConfig,
    ) -> Result<()> {
        let client: Box<dyn McpClientTrait> = match config.transport {
            Transport::Platform => {
                // In-process, no IPC overhead
                Box::new(PlatformClient::new(config.module))
            }
            Transport::Builtin => {
                // DuplexStream — in-process but protocol-compliant
                Box::new(BuiltinClient::new(config.entry_point))
            }
            Transport::Stdio => {
                // Subprocess with stdin/stdout
                Box::new(StdioClient::spawn(config.command, config.args))
            }
            Transport::StreamableHttp => {
                // Remote HTTP server
                Box::new(HttpClient::new(config.url))
            }
            Transport::Sse => {
                // Server-sent events (legacy)
                Box::new(SseClient::new(config.url))
            }
            // ... additional transport types
        };
        self.extensions.insert(name.to_string(), client);
        Ok(())
    }
}
```

Seven transport types, one interface. The built-in "developer" extension (file ops, bash, etc.) runs as a `Platform` transport with zero IPC overhead. A community extension might run over `Stdio`. A cloud service runs over `StreamableHttp`. The agent doesn't care.

### The MCP Adoption Spectrum

Not all agents embrace MCP equally:

| Agent | MCP Role | Details |
|-------|----------|---------|
| **Goose** | Core architecture | Everything is MCP. Built-in tools are MCP servers. |
| **Claude Code** | Extension mechanism | Built-in tools are native; MCP adds external tools |
| **Codex** | Client + Server | Can consume MCP tools AND expose itself as an MCP server |
| **Gemini CLI** | Extension layer | MCP tools are discovered and added to the registry |
| **Amp** | First-class integration | MCP tools sit alongside built-in tools with same permissions |
| **OpenCode** | Planned/partial | MCP support is being added incrementally |
| **Aider** | Not adopted | Edit-format approach doesn't map well to MCP |
| **OpenHands** | Experimental | MCP explored but Action/Observation model is primary |

### How MCP Blurs Built-in vs. External

The traditional distinction between "built-in" and "plugin" tools breaks down with MCP:

```
Traditional:
  Built-in tools → Fast, trusted, maintained by core team
  Plugin tools   → Slower, untrusted, maintained by community

MCP-native (Goose):
  Platform transport → Fast, trusted, maintained by core team
  Builtin transport  → Fast, trusted, maintained by core team
  Stdio transport    → Medium speed, configurable trust, anyone
  HTTP transport     → Network latency, configurable trust, anyone
```

The *trust model* and *performance profile* become properties of the transport, not the tool. A community-contributed tool running as a Platform transport is just as fast and trusted as a "built-in" tool.

### MCP Discovery

MCP's `tools/list` capability means agents can discover tools at runtime:

```json
// MCP tools/list response
{
  "tools": [
    {
      "name": "query_database",
      "description": "Execute a read-only SQL query",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query": { "type": "string", "description": "SQL query to execute" }
        },
        "required": ["query"]
      }
    }
  ]
}
```

This enables a fundamentally different workflow: instead of coding new tools into the agent, users *configure* tool servers. The agent discovers what's available and adapts.

---

## Pattern 5: No Tools (Bash-Only)

The most radical pattern: give the LLM exactly one tool — a bash shell — and let it figure out the rest.

### mini-SWE-agent's Approach

mini-SWE-agent (from the SWE-agent team at Princeton) demonstrates that a single bash tool is surprisingly effective:

```json
{
    "type": "function",
    "function": {
        "name": "bash",
        "description": "Execute a bash command in the terminal. All changes to files should be made through the terminal using bash commands. Do not use the 'exit' command.",
        "parameters": {
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute. Can be multiple lines."
                }
            },
            "required": ["command"]
        }
    }
}
```

That's it. One tool. The LLM uses `cat` to read files, `sed`/`awk` to edit them, `find`/`grep` to search, `git` for version control, `python` to run tests. Every operation maps to a bash command the model has seen thousands of times in training data.

### Why It Works

**Training data alignment.** Modern LLMs have ingested millions of shell sessions, Stack Overflow answers, and README files. They know `grep -rn "pattern" .` better than they know any agent's custom `search_files` tool schema.

**Zero abstraction overhead.** There's no `FileEditAction` → `Runtime` → `Docker exec` pipeline. It's just: run this command, return the output.

**Universal model compatibility.** Any model that supports function calling can use a single bash tool. No need to fine-tune for custom tool schemas or worry about schema compliance across different providers.

### The SWE-agent Team's Data

The SWE-agent team published findings showing that the gap between custom-tool agents and bash-only agents has been *shrinking dramatically* as models improve:

- With GPT-3.5: Custom tools outperformed bash-only by ~15% on SWE-bench
- With GPT-4: Gap narrowed to ~8%
- With Claude 3.5 Sonnet: Gap narrowed to ~3-4%
- With frontier models (late 2024+): Bash-only approaches competitive or superior

The hypothesis: as models get better at reasoning, the marginal value of structured tool schemas decreases. The model can figure out how to compose shell commands to achieve any goal.

### Trade-offs

| Advantage | Disadvantage |
|-----------|-------------|
| Zero setup complexity | No safety guardrails — `rm -rf /` is valid bash |
| Universal model compatibility | Higher token cost (verbose command output) |
| No schema maintenance | No structured error handling |
| Matches training distribution | Hard to intercept specific operations |
| Infinitely extensible (any CLI tool) | No permission model beyond OS-level |
| Easy to debug (just bash history) | Session state management is manual |

### When Bash-Only Shines

- **Prototyping**: Get an agent running in 50 lines of code
- **Evaluation**: Minimal confounding variables when benchmarking models
- **Simple tasks**: File edits, git operations, running tests
- **Trusted environments**: When sandboxing is handled at the container level

---

## Pattern 6: Edit Format System

Aider takes the most unconventional approach of any major agent. It doesn't use function calling at all for its core operation. Instead, "tools" are structured text formats that the LLM produces in its regular text output.

### The Formats

| Format | Description | Best For |
|--------|-------------|----------|
| `whole` | LLM outputs the entire file content | Small files, complete rewrites |
| `diff` | Search/replace blocks in fenced sections | Targeted edits to large files |
| `diff-fenced` | Like diff but with additional fencing | Models that struggle with delimiter parsing |
| `udiff` | Unified diff format (`---`/`+++`/`@@` hunks) | Models trained on git diff output |
| `architect` | High-level description, then a coder model applies | Complex refactors, planning-heavy tasks |
| `whole-func` | Output entire functions, not entire files | Medium-sized targeted edits |
| `diff-func` | Search/replace within function boundaries | Function-level precision |

### Example: The `diff` Format

```
Here's the change to fix the bug:

src/utils.py
<<<<<<< SEARCH
def calculate_total(items):
    total = 0
    for item in items:
        total += item.price
    return total
=======
def calculate_total(items):
    total = 0
    for item in items:
        total += item.price * item.quantity
    return total
>>>>>>> REPLACE
```

The LLM writes this as plain text. Aider parses the SEARCH/REPLACE blocks and applies them. No JSON, no function calling, no tool schemas.

### The Key Insight: Function Calling Performed Worse

Aider's creator (Paul Gauthier) extensively benchmarked function-calling-based edit tools against plain-text edit formats. The finding was surprising: **function-call formats consistently performed worse** than structured plain text.

The hypothesis for why:

1. **Cognitive overhead**: Producing valid JSON while simultaneously reasoning about code changes splits the model's attention
2. **Escaping hell**: Code contains quotes, backslashes, and special characters that must be escaped in JSON strings — the model makes mistakes
3. **Training distribution**: Models have seen far more diff/patch output in training than they've seen function-call JSON
4. **Token efficiency**: JSON wrappers, property names, and escaping waste tokens that could be used for reasoning

### Aider's Benchmark Results

Aider publishes an ongoing benchmark tracking edit format performance across models. The pattern is consistent:

- `diff` and `udiff` formats outperform function-call equivalents by 5-15% on code editing benchmarks
- `whole` format works best for small files where context fits easily
- `architect` format (two-pass: plan in text, execute with a coder) achieves highest scores on complex tasks
- The performance gap is largest on models with smaller context windows

### Trade-offs

| Advantage | Disadvantage |
|-----------|-------------|
| Matches training distribution | Requires custom parsing logic |
| No JSON escaping issues | Fragile — depends on delimiter parsing |
| Better benchmark performance | Can't use standard tool-calling infrastructure |
| Lower token overhead | No standardized schema (every format is custom) |
| Works with any model (no function calling needed) | Hard to compose with other tool systems |

---

## When to Use Each Pattern

### Decision Matrix

| Factor | Trait/Interface | Registry | Action/Obs | MCP-Native | Bash-Only | Edit Format |
|--------|----------------|----------|------------|------------|-----------|-------------|
| **Type safety** | ★★★★★ | ★★★☆☆ | ★★★★☆ | ★★☆☆☆ | ★☆☆☆☆ | ★☆☆☆☆ |
| **Extensibility** | ★★☆☆☆ | ★★★☆☆ | ★★☆☆☆ | ★★★★★ | ★★★★★ | ★★☆☆☆ |
| **Safety/sandboxing** | ★★★★☆ | ★★★☆☆ | ★★★★★ | ★★★☆☆ | ★☆☆☆☆ | ★★★☆☆ |
| **Setup complexity** | ★★★☆☆ | ★★★☆☆ | ★★☆☆☆ | ★★☆☆☆ | ★★★★★ | ★★★★☆ |
| **Debugging ease** | ★★★★☆ | ★★★☆☆ | ★★★★★ | ★★☆☆☆ | ★★★★★ | ★★★☆☆ |
| **Model compatibility** | ★★★☆☆ | ★★★☆☆ | ★★★☆☆ | ★★★☆☆ | ★★★★★ | ★★★★★ |
| **Session replay** | ★☆☆☆☆ | ★☆☆☆☆ | ★★★★★ | ★★☆☆☆ | ★★☆☆☆ | ★★☆☆☆ |
| **Community contribution** | ★★☆☆☆ | ★★★☆☆ | ★★☆☆☆ | ★★★★★ | ★★★★★ | ★★☆☆☆ |

### Guidelines by Context

**Small team, compiled language, safety-critical:**
→ Trait/Interface (Pattern 1) + Registry (Pattern 2)
→ Example: Codex (Rust traits + enum dispatch + sandbox)

**Platform play, ecosystem growth priority:**
→ MCP-Native (Pattern 4) + Registry (Pattern 2)
→ Example: Goose (everything is MCP, community extensions)

**Research/evaluation, need reproducibility:**
→ Action/Observation (Pattern 3)
→ Example: OpenHands (event streams, session replay, runtime-agnostic)

**Quick prototype, single-model target:**
→ Bash-Only (Pattern 5)
→ Example: mini-SWE-agent (one tool, 50 lines of agent code)

**Code editing focus, model-agnostic:**
→ Edit Format (Pattern 6)
→ Example: Aider (structured text protocols, no function calling dependency)

**Production agent, broad model support, extensible:**
→ Registry (Pattern 2) + MCP (Pattern 4) + Trait (Pattern 1) for core tools
→ Example: Claude Code, Gemini CLI (native core + MCP extensions)

---

## Pattern Evolution Timeline

The history of tool system design in coding agents tracks a clear evolution toward dynamism and standardization.

### Era 1: Hardcoded Tool Lists (2023)

The earliest agents — AutoGPT, early LangChain agents — had tools defined as Python functions in a list:

```python
tools = [
    {"name": "read_file", "fn": read_file, "description": "..."},
    {"name": "write_file", "fn": write_file, "description": "..."},
    {"name": "run_command", "fn": run_command, "description": "..."},
]
```

No interfaces, no registries, no dispatch logic. Just a list. This worked for demos but collapsed under the weight of real-world requirements: permissions, namespacing, error handling, timeouts.

### Era 2: Registry Patterns (2023-2024)

As agents matured, tool registries emerged. LangChain introduced `BaseTool` and `ToolRegistry`. CrewAI added role-based tool access. The pattern became: define tools against an interface, register them in a central store, dispatch by name.

This era also saw the rise of the trait-based pattern in compiled languages as agents like Codex and Goose chose Rust for performance and safety.

### Era 3: MCP and Hybrid Approaches (2024-2025)

Anthropic's release of MCP created a standardization moment. Agents that were already using registry patterns could add MCP as another tool source. Agents built from scratch (Goose) could make MCP the foundation.

The current state: most production agents use 2-3 patterns simultaneously:
- Native traits for core tools (performance, safety)
- Registry for dispatch (flexibility, categorization)
- MCP for extensibility (community, ecosystem)

### Era 4: Fully Dynamic Discovery (Emerging)

The next frontier is agents that discover tools at runtime based on the task:

```
User: "Help me optimize this database query"

Agent thinking:
  1. I need SQL tools → check MCP registry → found "postgres-mcp"
  2. I need query analysis → check MCP registry → found "explain-analyze-mcp"
  3. I need benchmarking → no MCP server found → fall back to bash + pgbench
```

Codex's `ToolSearchCall` variant hints at this direction. Instead of pre-loading all tools, the agent searches for relevant tools mid-conversation based on what it needs.

This shifts the design question from "what tools should the agent have?" to "how does the agent find the right tools for this specific task?"

---

## Key Takeaways

1. **No single pattern dominates.** Production agents combine 2-3 patterns. The trait pattern provides safety for core tools, the registry pattern provides dispatch flexibility, and MCP provides ecosystem extensibility.

2. **The trend is toward dynamism.** Static tool lists → registries → MCP discovery → runtime search. Each generation gives the agent more ability to adapt its toolset to the task.

3. **Tool design reflects trust models.** Codex's enum variants, Claude Code's categories, and Goose's transport types all encode trust levels. The tool system *is* the security model.

4. **Simpler can be better.** Aider's edit formats outperform function calling. Bash-only agents keep pace with multi-tool agents. More tools ≠ better performance. The best tool system is the one that aligns with how the model was trained.

5. **MCP is the convergence point.** Even agents that don't use MCP natively are adding MCP support. It's becoming the lingua franca of tool interoperability, much like LSP did for editor-language integration.

6. **The Action/Observation pattern is underrated.** OpenHands' approach sacrifices simplicity for properties (serialization, replay, runtime-agnosticism) that matter enormously at scale. If you're building an agent platform (not just an agent), this pattern deserves serious consideration.
