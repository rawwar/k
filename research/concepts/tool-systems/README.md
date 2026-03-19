---
title: Tool Systems
status: complete
---

# Tool Systems

How coding agents expose capabilities to LLMs, dispatch tool calls, execute them safely, and handle failures. This document synthesizes patterns from 17+ agent implementations.

---

## Design Patterns

Agents use fundamentally different architectures for structuring their tool systems. The choice shapes everything downstream — extensibility, safety, performance, and cognitive load on the model.

### Pattern 1: Trait / Interface-Based

A statically-typed interface that every tool must implement. The agent dispatches by iterating registered tools and matching by name.

**Go interface (OpenCode):**
```go
type BaseTool interface {
    Info() ToolInfo
    Run(ctx context.Context, params ToolCall) (ToolResponse, error)
}
```

**Rust trait (Codex):**
```rust
impl ToolRouter {
    pub fn build_tool_call(&self, item: &ResponseItem) -> ToolCall {
        match item {
            ResponseItem::FunctionCall { name, .. } => { /* registry lookup */ }
            ResponseItem::LocalShellCall { .. } => { /* shell handler */ }
            // ...
        }
    }
}
```

**Rust trait (Ante):**
```rust
trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn call(&self, input: Value) -> Result<Value, ToolError>;
}
```

**Rust trait (Goose — MCP tool router):**
```rust
#[async_trait]
pub trait McpClientTrait: Send + Sync {
    async fn list_tools(&self, ...) -> Result<ListToolsResult>;
    async fn call_tool(&self, ...) -> Result<CallToolResult>;
}
```

**Strengths:** Compile-time safety, clear contract, dependency injection via constructors. Tools are testable units.
**Weaknesses:** Adding a tool requires code changes and recompilation. Less accessible to non-core contributors.

### Pattern 2: Registry with Dispatch

A central registry maps tool names to handlers. The agent loop looks up the name and calls the handler. This is the most common pattern.

**OpenCode's linear dispatch:**
```go
for _, availableTool := range a.tools {
    if availableTool.Info().Name == toolCall.Name {
        tool = availableTool
        break
    }
}
```

**Goose's namespace-based dispatch:**
```rust
// Tools are namespaced as "extensionname__toolname"
pub async fn dispatch_tool_call(&self, ctx: ToolCallContext, ...) {
    let (extension, tool_name) = resolve_tool(name);  // split on "__"
    extension.client.call_tool(tool_name, arguments).await
}
```

**Gemini CLI's ToolRegistry:**
Built-in tools are statically registered at startup; MCP and custom discovery tools are dynamically added. All tools live in a single registry and appear uniformly to the model.

**Codex's ToolRouter + ToolRegistry:** Distinguishes tool call types at the enum level (`FunctionCall`, `LocalShellCall`, `CustomToolCall`, `ToolSearchCall`) before dispatching to the appropriate handler.

### Pattern 3: Action / Observation Model

OpenHands takes a unique approach: tools don't exist as executable objects. They are *schemas* sent to the LLM. The actual execution flows through a typed pipeline:

```
LLM tool_call → response_to_actions() → Action dataclass → Runtime → Observation
```

Each tool maps to a specific Action/Observation pair:

| Tool | Action | Observation |
|------|--------|-------------|
| `execute_bash` | `CmdRunAction` | `CmdOutputObservation` |
| `str_replace_editor` | `FileEditAction` | `FileEditObservation` |
| `browser` | `BrowseInteractiveAction` | `BrowserOutputObservation` |
| `think` | `AgentThinkAction` | `AgentThinkObservation` |

**Why this works:** Actions serialize to an event stream (append-only log), enabling full session replay, security interception, and runtime-agnostic execution. The same Action runs identically on Docker, local, or Kubernetes runtimes.

### Pattern 4: MCP-Native (Tools as External Services)

Goose's defining decision: *everything is an MCP server*. There is no separate "built-in tool" abstraction. The `ExtensionManager` connects to MCP servers (7 transport types) and presents a unified interface.

```
Platform extensions  → McpClientTrait (in-process, no transport)
Builtin extensions   → McpClientTrait (in-process DuplexStream)
Stdio extensions     → McpClientTrait (child process stdin/stdout)
StreamableHttp       → McpClientTrait (HTTP)
```

Claude Code and Gemini CLI adopt MCP as an *extension* mechanism alongside built-in tools. OpenHands and OpenCode also support MCP but treat MCP tools as second-class citizens (always requiring permission, prefixed with server name).

**MCP adoption spectrum:**

| Agent | MCP Role |
|-------|----------|
| Goose | **Core architecture** — everything is MCP |
| Claude Code | Extension mechanism + can serve as MCP server |
| Gemini CLI | Extension mechanism alongside built-in tools |
| Codex | Both client and server; MCP tools are first-class `FunctionCall` |
| OpenCode | Extension; tools prefixed `{server}_{tool}`, always need permission |
| OpenHands | Extension via microagents; first-class in action pipeline |
| Pi | No built-in MCP; community package `pi-mcp-adapter` |
| Ante | Custom Rust MCP SDK; bidirectional support |
| mini-SWE-agent | None |

### Pattern 5: No Tools At All (Bash-Only)

mini-SWE-agent's radical position: one tool — bash. Everything else (file editing, search, navigation, git) is done through shell commands the LLM already knows.

```python
BASH_TOOL = {
    "type": "function",
    "function": {
        "name": "bash",
        "description": "Execute a bash command",
        "parameters": {
            "properties": {
                "command": { "type": "string" }
            },
            "required": ["command"]
        }
    }
}
```

**Why it works:** Modern LLMs know `grep`, `sed`, `find`, `git`, and hundreds of other tools from training data. Custom agent tools are abstractions *on top of* things the LLM already knows. The SWE-agent team's own data shows the gap between custom tools and bash-only shrank dramatically as models improved from GPT-4 to Claude 3.5+.

**Trade-offs:** Zero setup in sandboxes, universal model compatibility. But: no safety guardrails beyond the sandbox, higher token cost per operation, and the model must know bash idioms for every operation.

### Pattern 6: Edit Format System (Prompt-Based Tools)

Aider doesn't use function calling at all. Its "tools" are *edit formats* — structured text protocols for expressing code changes:

| Format | How It Works | Best For |
|--------|-------------|----------|
| `whole` | LLM returns entire file | Weak models, small files |
| `diff` | SEARCH/REPLACE blocks | Most capable models |
| `diff-fenced` | Diff with filename inside fence | Gemini models specifically |
| `udiff` | Unified diff format | GPT-4 Turbo (anti-laziness) |
| `architect` | Two-model pipeline: reason then edit | Reasoning models (o1, o3, R1) |
| `whole-func` / `diff-func` | JSON function calling | **Worse** than plain text (historical) |

**Key insight from Aider's benchmarks:** Function-call formats (`whole-func`, `diff-func`) performed *worse* than plain text. The cognitive overhead of producing valid JSON hurt both code quality and format adherence. This finding shaped industry thinking about LLM tool interfaces.

### Pattern Comparison

| Pattern | Extensibility | Safety | Cognitive Load | Setup Cost |
|---------|--------------|--------|---------------|------------|
| Trait/interface | Moderate (code changes) | Compile-time checks | Low (typed) | High |
| Registry | High (dynamic registration) | Runtime validation | Low | Medium |
| Action/observation | High (new Action types) | Event-stream audit | Medium | High |
| MCP-native | Very high (any MCP server) | Depends on inspectors | Medium | Low |
| Bash-only | Infinite (any CLI tool) | None (sandbox only) | High for model | Zero |
| Edit formats | None (fixed formats) | N/A (no execution) | Varies by format | Zero |

---

## JSON Schema — How Tools Describe Themselves to LLMs

The schema sent to the model is the *entire interface* between agent and LLM. Schema design directly affects tool-calling reliability.

### Standard Formats

**OpenAI function calling (most agents):**
```json
{
  "type": "function",
  "function": {
    "name": "execute_bash",
    "description": "Execute a bash command in the terminal...",
    "parameters": {
      "type": "object",
      "properties": {
        "command": { "type": "string" },
        "timeout": { "type": "integer" }
      },
      "required": ["command"]
    }
  }
}
```
Used by: OpenHands, OpenCode, mini-SWE-agent, Gemini CLI, ForgeCode, Capy, TongAgents.

**Anthropic tool_use format (Claude Code):**
Tools are defined in the API request and the model returns structured `tool_use` blocks. Claude Code uses Anthropic's native tool-use protocol — not prompt injection. The model returns blocks like:
```json
{ "type": "tool_use", "id": "toolu_01A...", "name": "Edit", "input": { "file_path": "...", "old_str": "...", "new_str": "..." } }
```

**MCP tool definitions (Goose, Codex as MCP server):**
```json
{
  "name": "codex",
  "description": "Run a Codex session",
  "inputSchema": {
    "properties": {
      "prompt": { "type": "string" },
      "approval-policy": { "enum": ["untrusted", "on-failure", "on-request", "never"] },
      "sandbox": { "enum": ["read-only", "workspace-write", "danger-full-access"] }
    }
  }
}
```

**Schema derivation from types (Goose):**
```rust
fn schema<T: JsonSchema>() -> JsonObject {
    serde_json::to_value(schema_for!(T))
        .expect("schema").as_object().expect("object").clone()
}
```
Goose derives JSON Schema directly from Rust struct definitions using `schemars::JsonSchema`, eliminating manual schema maintenance.

### Schema Engineering — Tricks That Matter

**ForgeCode's field ordering insight:** Putting `required` before `properties` in the JSON Schema improves model compliance. This is a training-data alignment trick — models have seen more schemas in that order.

**ForgeCode's naming alignment:** Using `old_string`/`new_string` instead of creative names aligns with the edit tool naming conventions most common in training data. Training-data-aligned naming reduces tool-calling errors.

**ForgeCode's flat schemas:** Nested object schemas cause more errors than flat ones. Flattening parameters into a single level of properties reduces model confusion.

**OpenHands' description length adaptation:** Different models have different tool-description token limits. OpenHands maintains two variants:
```python
def create_cmd_run_tool(cwd: str, short: bool = False):
    if short:
        description = CMD_RUN_TOOL_SHORT_DESCRIPTION  # < 1024 tokens
    else:
        description = CMD_RUN_TOOL_DESCRIPTION         # ~2000+ tokens
```
Short descriptions are used for GPT-4, o1, o3, o4 families.

**Goose's argument coercion:** Rather than failing on type mismatches, Goose coerces LLM arguments to match schemas. This handles the common case where LLMs send `"42"` instead of `42`.

**Discriminated union (OpenHands' str_replace_editor):** One tool with a `command` enum dispatches to different operations:
```json
"command": { "type": "string", "enum": ["view", "create", "str_replace", "insert", "undo_edit"] }
```
This reduces the number of tools the model must track while enabling 5 distinct operations through a single interface.

**Claude Code's MCP Tool Search:** When MCP tools exceed 10% of context window, tools are deferred rather than preloaded. The model uses a search tool to discover relevant MCP tools on demand — lazy-loading for tool schemas.

### Schema Design Impact on Performance

| Technique | Effect | Source |
|-----------|--------|--------|
| Field ordering (`required` first) | Improved compliance | ForgeCode |
| Flat vs nested schemas | Fewer argument errors | ForgeCode |
| Training-aligned naming | Lower error rate | ForgeCode |
| Short descriptions for constrained models | Fits within token limits | OpenHands |
| Argument coercion (string→int) | Fewer type mismatches | Goose |
| Discriminated unions (multi-command tools) | Fewer tools to track | OpenHands |
| Lazy tool loading (Tool Search) | Preserves context budget | Claude Code |
| Plain text > JSON function calling | Better code quality | Aider benchmarks |

---

## Execution Models

How tool calls actually run — from hermetically sealed sandboxes to unrestricted shell access.

### Sandboxed Execution

**Codex — 3-Layer Linux Sandbox (most sophisticated):**

| Layer | Mechanism | What It Does |
|-------|-----------|-------------|
| 1. Bubblewrap | Mount namespaces | Filesystem read-only by default, explicit writable dirs, PID/user/network isolation |
| 2. seccomp | Syscall filtering | Blocks ptrace, io_uring, network syscalls; allows only AF_UNIX sockets |
| 3. Landlock | LSM filesystem rules | Legacy fallback; path-based read/write restrictions |

Plus: macOS Seatbelt (`sandbox-exec -p`), Windows ACL Deny ACEs + firewall rules + restricted tokens + alternate desktops. Cross-platform coverage is unmatched.

Codex also implements proxy routing — a TCP→UDS→TCP bridge that allows sandboxed processes to reach only configured proxy endpoints while maintaining complete network isolation.

**OpenHands — Docker Container:**
Every Action executes inside a Docker container via `EventStreamRuntime`. The container runs an `ActionExecutionServer` that receives Actions over HTTP. Jupyter kernel, browser (Playwright), and shell all run inside the container. This provides strong isolation but with higher startup cost.

**Gemini CLI — Multi-Tier Sandboxing:**
Supports multiple sandbox backends depending on platform:
- macOS: Seatbelt profiles (similar to Codex)
- Linux: Docker containers
- Configurable per-tool: `run_shell_command` is always sandboxed; file tools may or may not be depending on configuration

**Capy — Full VM Per Task:**
The most heavyweight approach: each task runs in its own Ubuntu VM with sudo access. Complete isolation from other tasks/users. Network access for package managers only.

### Direct Execution (with Permission Gates)

**Claude Code — 5 Permission Modes:**

| Mode | Behavior |
|------|----------|
| `default` | Prompts for first use of each tool |
| `acceptEdits` | Auto-accepts file edits; asks for Bash |
| `plan` | Read-only — analyze but not modify |
| `dontAsk` | Auto-denies unless pre-approved |
| `bypassPermissions` | Skips all prompts (dangerous) |

Permission rules use glob patterns: `Bash(npm run *)`, `Read(src/**)`, `WebFetch(domain:example.com)`. Evaluation order: **deny → ask → allow**, first match wins. Managed settings cannot be overridden.

**OpenCode — Permission Service:**
Tools marked as requiring permission block the agent loop until the user responds. Options: allow once, allow for session, deny, or auto-approve (non-interactive mode). Bash tool has banned commands (`curl`, `wget`, `nc`) and safe read-only command whitelist.

**Warp — Category-Based:**
Read-only operations (read file, list dir, search) require no permission. Write operations (write, edit, delete, move) and Computer Use require explicit permission. Tool invocation follows a priority: built-in files → shell → LSP → web → MCP → Computer Use (last resort).

### Stateless Subprocess

**mini-SWE-agent — subprocess.run Per Action:**
Every command runs in a fresh subprocess. Directory and environment changes do NOT persist:
```python
# Each action is a new subprocess
subprocess.run(command, shell=True, capture_output=True, timeout=timeout)
```
The system prompt teaches the model to prefix commands with `cd /path && ...` to work around non-persistence. Environment variables configured once via YAML (`PAGER=cat`, `PIP_PROGRESS_BAR=off`) to prevent interactive hangs.

### Persistent Shell Session

Most production agents keep a bash session alive across tool calls:

| Agent | Shell Persistence | Env Vars Persist? | CWD Persists? |
|-------|------------------|-------------------|---------------|
| OpenCode | Persistent shell (`GetPersistentShell()`) | Yes | Yes |
| Claude Code | Separate process per command | No | Yes |
| Codex | Sandboxed process per command | Per-sandbox | Per-sandbox |
| Gemini CLI | Persistent | Yes | Yes |
| OpenHands | Persistent within Docker container | Yes | Yes |
| mini-SWE-agent | New subprocess each time | No | No |
| Pi | Full shell access, no persistence specified | Unknown | Unknown |

### Parallel Tool Execution

| Agent | Parallel Tool Calls? | How? |
|-------|---------------------|------|
| OpenHands | Yes | Multiple tool_calls in one LLM response; all processed, results returned together |
| Claude Code | Yes | Multiple tool_use blocks in one response |
| Codex | Yes | Multiple FunctionCall items in one response |
| Goose | Yes | Async dispatch via tokio |
| OpenCode | **No** | Sequential execution, one tool at a time |
| Gemini CLI | Yes | Multiple tool calls supported |
| mini-SWE-agent | **No** | Single command per step enforced by system prompt |
| Aider | N/A | No function calling; sequential edit application |

### Permission & Safety Comparison

| Agent | Sandbox | Permission Model | Banned Commands | Network Control |
|-------|---------|-----------------|-----------------|-----------------|
| Codex | 3-layer OS sandbox | 4-level approval policy + execution policy engine | Yes (policy-based allowlist) | seccomp + proxy routing |
| OpenHands | Docker container | SecurityAnalyzer (rule or LLM-based) + confirmation | No explicit list | Container network |
| Claude Code | None (direct) | 5 permission modes + glob rules | No explicit list | Permission required |
| Gemini CLI | Multi-tier (Seatbelt/Docker) | Confirmation model (read-only vs mutating) | No explicit list | Sandbox-dependent |
| OpenCode | None (direct) | Permission service + command filtering | Yes (`curl`, `wget`, `nc`, etc.) | Banned commands |
| Goose | None (direct) | 4-tier inspection pipeline | SecurityInspector patterns | No |
| Pi | None (direct) | None by default; extensions can add | None | None |
| mini-SWE-agent | External (Docker in benchmarks) | None | None | Sandbox-dependent |
| Capy | Full Ubuntu VM | Hard agent-tool boundaries (Captain can't write; Build can't ask user) | Platform-enforced | VM network |
| Junie CLI | Project scope enforcement | 4-level permission system (L0-L3) | Security pattern filtering | L3 required |
| ForgeCode | None | None (user's native shell) | None | None |

**Goose's 4-tier inspection pipeline** deserves special mention:
1. **SecurityInspector** — Pattern-matches dangerous commands
2. **AdversaryInspector** — Detects prompt injection in tool arguments
3. **PermissionInspector** — Per-tool rules (Autonomous/Manual/Smart/Chat-Only)
4. **RepetitionInspector** — Detects infinite retry loops

Every tool call passes through all four inspectors, even in autonomous mode.

**Codex's execution policy engine** is the most granular:
```rust
pub enum Decision { Allow, Prompt, Forbidden }
```
Rules match by program name + argument prefix patterns. A shell command parser (84KB, the largest single file in the codebase) decomposes complex shell expressions into individual invocations for per-command policy evaluation. Special argument types (`ARG_RFILES`, `ARG_WFILE`, `ARG_SED_COMMAND`) constrain what commands can do.

---

## Error Handling

How agents deal with tool failures — from simple error messages to sophisticated correction loops.

### Tool Validation Failures

Most agents return errors as tool results rather than crashing:

**OpenCode:** Go errors reserved for infrastructure failures; tool errors returned as `ToolResponse{IsError: true}`:
```go
if tool == nil {
    toolResults[i] = message.ToolResult{
        Content: fmt.Sprintf("Tool not found: %s", toolCall.Name),
        IsError: true,
    }
    continue  // Not fatal — reported back to LLM
}
```

**OpenHands:** `str_replace_editor` fails cleanly on ambiguous matches — if `old_str` matches zero or multiple locations, a descriptive error observation is returned. The model can then refine its search string.

**Ante:** Typed Rust errors (`InvalidArgs`, `Execution`, `Timeout`) via `Result<Value, ToolError>`. Compile-time type safety catches schema violations early through `serde::Deserialize`.

**Aider's fuzzy matching:** When the SEARCH block doesn't match exactly, Aider tries progressively looser matching:
1. Exact match
2. Strip trailing whitespace
3. Ignore blank lines
4. Normalized whitespace matching

This is a rare example of the tool system *repairing* model output before failing.

### Timeout Handling

| Agent | Default Timeout | Max Timeout | Behavior on Timeout |
|-------|----------------|-------------|-------------------|
| OpenCode | 60 seconds | 10 minutes | Kill process |
| OpenHands | Configurable per-call | Per `timeout` param | Force terminate |
| Codex | Configurable | Per sandbox policy | Kill + report |
| mini-SWE-agent | Configurable | Per config | `subprocess.run` timeout |
| Ante | 30 seconds | Configurable | `ToolError::Timeout` |
| ForgeCode | 300 seconds | `FORGE_TOOL_TIMEOUT` | Kill hung commands |
| Goose | Per-extension configurable | 300s default for stdio | MCP timeout |

### Output Truncation

Long outputs consume context budget. Agents handle this differently:

**OpenCode** — Keep first and last halves (30,000 char limit):
```go
func truncateOutput(content string) string {
    halfLength := MaxOutputLength / 2
    return fmt.Sprintf("%s\n\n... [%d lines truncated] ...\n\n%s",
        content[:halfLength], truncatedLinesCount, content[len(content)-halfLength:])
}
```

**mini-SWE-agent** — Head + tail with explicit elision (10,000 char limit):
```xml
<output_head>{{ output.output[:5000] }}</output_head>
<elided_chars>{{ elided_chars }} characters elided</elided_chars>
<output_tail>{{ output.output[-5000:] }}</output_tail>
```

**ForgeCode** — Explicit truncation signals in plain text (not just metadata), because some models miss metadata fields.

### Retry Strategies

**Codex — Sandbox escalation retry:**
```rust
match result {
    Err(SandboxErr::Denied { .. }) if tool.escalate_on_failure() => {
        // Request user approval to retry without sandbox
        Self::run_attempt(tool, req, escalated_sandbox, ..).await
    }
}
```
If a command fails due to sandbox restrictions, Codex can request escalation and retry with relaxed permissions.

**ForgeCode — Tool-call correction layer (highest-impact innovation):**
A programmatic interception layer validates and repairs LLM tool calls *before* execution. Catches three failure classes:
1. **Wrong tool selected** — redirects to correct tool
2. **Correct tool, wrong arguments** — pattern-matches common errors and auto-corrects
3. **Correct call, wrong sequencing** — enforces ordering constraints

Plus a `max_tool_failure_per_turn: 3` limit to prevent infinite retry loops. Per-tool micro-evaluations run in CI/CD to gate releases.

**mini-SWE-agent — Format error correction:**
```yaml
format_error_template: |
  Format error:
  <error>{{error}}</error>
  Please always provide EXACTLY ONE action in triple backticks.
```
Format errors (`FormatError` exception) add a corrective message to the conversation and let the model retry on the next step.

**Goose — RepetitionInspector:**
Detects tools called repeatedly without progress. Prevents infinite loops where the agent keeps retrying a failing tool. Returns `DECLINED_RESPONSE` to force the LLM to try an alternative approach.

### Error Feedback Loops

**OpenCode + OpenHands — LSP integration for immediate feedback:**
After file edits, both agents wait for LSP diagnostics and append type errors/warnings to the tool response. The model gets immediate feedback on its changes without needing a separate build/lint step:
```go
// After edit, wait briefly for LSP
func waitForLspDiagnostics(ctx context.Context, filePath string, lspClients map[string]*lsp.Client)
```

**Warp — Active AI error monitoring:**
Proactively monitors command exit codes and error output, suggests one-click fixes for common errors using pattern matching. This is the only agent where error handling is a *proactive* capability rather than reactive.

**Droid — GitHub Actions repair:**
The `github_action_repair` tool automatically analyzes CI failures, identifies root causes, and creates repair PRs. Error handling elevated to a first-class workflow.

**Junie CLI — Structured test result parsing:**
Extracts test name, file, line, status, error type, and expected/actual values from test output across 13+ test frameworks in 8 languages. Build output is parsed for error locations. This structured error information gives the model precise targets for fixes.

### Error Handling Comparison

| Agent | Validation Errors | Timeout | Retry Strategy | Feedback Loop |
|-------|------------------|---------|---------------|--------------|
| Codex | Error observation | Kill process | Sandbox escalation | Policy amendment |
| OpenHands | Descriptive error obs | Force terminate | Model retries naturally | LSP diagnostics |
| Claude Code | Error in tool result | Process kill | Model retries | LSP via plugin |
| OpenCode | `IsError: true` response | Kill (60s default) | Model retries | LSP diagnostics |
| Goose | Inspection pipeline | Per-extension timeout | RepetitionInspector | — |
| Gemini CLI | Error in result | Sandbox-dependent | Model retries | — |
| mini-SWE-agent | FormatError + retry | subprocess timeout | Corrective message | — |
| Aider | Fuzzy match fallback | N/A | Progressive matching | — |
| ForgeCode | Correction layer | 300s kill | Auto-correct + 3-failure limit | Micro-evaluations |
| Pi | Extension-dependent | Extension-dependent | Extension-dependent | Extension-dependent |
| Warp | Active AI monitoring | — | One-click fix suggestions | Proactive error detection |

---

## Tools & Projects

A curated map of the tools, libraries, and protocols that power the tool layer of modern coding agents. For deeper coverage, see the [full research compendium](../../.copilot/session-state/d0354d87-4da1-4c93-a043-006f84879a18/files/tool-systems-tools.md).

### MCP Protocol & SDKs

- **Model Context Protocol (MCP)** — Open protocol (Linux Foundation / Anthropic) standardizing how LLM apps connect to external tools via JSON-RPC 2.0. https://modelcontextprotocol.io. Eliminates the N×M integration problem: one server works with every MCP-compatible agent.
- **TypeScript SDK** — `@modelcontextprotocol/server` / `client`. Node, Bun, Deno. v1 stable; v2 pre-alpha. https://github.com/modelcontextprotocol/typescript-sdk
- **Python SDK** — `pip install "mcp[cli]"`. High-level `FastMCP` decorator API for defining tools, resources, and prompts in a few lines. https://github.com/modelcontextprotocol/python-sdk
- **Other official SDKs** — Go, Rust, Java, Kotlin, C#, Swift, Ruby, PHP — all under https://github.com/modelcontextprotocol
- **MCP Server Registry** — Browsable catalog of published servers (filesystem, Git, fetch, memory, Slack, Sentry, etc.). https://registry.modelcontextprotocol.io

### Sandboxing & Isolation

- **bubblewrap (bwrap)** — Lightweight, unprivileged Linux sandboxing via namespaces. C, nearly zero deps. Used by Flatpak. Ideal low-overhead primitive for custom agent sandboxes. https://github.com/containers/bubblewrap
- **Landlock LSM** — Kernel module (Linux 5.13+) letting unprivileged processes restrict their own filesystem access at runtime. No container overhead — just syscalls. https://landlock.io
- **seccomp-bpf** — Kernel feature filtering syscalls with BPF programs. Foundational layer used by Docker, bubblewrap, Chrome. Prevents dangerous syscalls (`mount`, `ptrace`). Linux kernel built-in.
- **gVisor** — Application kernel (Go) that re-implements Linux syscalls in userspace. OCI-compatible (`runsc`). Strongest isolation for containers; used by Google Cloud Run. ~5–15 % CPU overhead. https://github.com/google/gvisor
- **E2B** — Cloud sandboxes purpose-built for AI code execution. Instant creation via API, Python/JS SDKs. Self-hostable on GCP. https://e2b.dev
- **Daytona** — AGPL-3.0 sandbox infra with sub-90 ms creation, built-in LSP, and unlimited persistence. Python/TS/Go SDKs. https://daytona.io

| Solution | Isolation | Overhead | Root? | Best For |
|----------|-----------|----------|-------|----------|
| seccomp | Syscall filter | Negligible | No | Restricting dangerous syscalls |
| Landlock | Filesystem LSM | Negligible | No | Runtime filesystem restrictions |
| bubblewrap | Namespace-based | Low | No | Custom sandbox building blocks |
| gVisor | Userspace kernel | Medium (~5–15 %) | Yes | Untrusted code in containers |
| Docker | Container (ns + cgroups) | Low–Medium | Yes (daemon) | Standard agent sandboxing |
| E2B | Cloud sandbox | Medium (network) | N/A (managed) | SaaS agent code execution |
| Daytona | Cloud sandbox | Medium (network) | N/A (managed) | Full dev environments with LSP |

### Function Calling Libraries

- **Instructor** — Reliable structured JSON from any LLM, built on Pydantic. Automatic retries with validation feedback, streaming partial objects, multi-provider support. 10 K+ stars, 3 M+ monthly downloads. https://github.com/jxnl/instructor
- **Magentic** — `@prompt` decorator turns a Python function signature into an LLM call with structured output. `FunctionCall` type lets the model choose which function to invoke; `@prompt_chain` auto-resolves tool loops. https://github.com/jackmpcollins/magentic

### Code Editing Tools

- **diff-match-patch** — Google's battle-tested diff / match / patch library (2006). Myers' diff + bitap fuzzy matching + best-effort patch application. Critical for agents: patches still apply when code has shifted. C++, JS, Python, Java, and more. https://github.com/google/diff-match-patch
- **Tree-sitter** — Incremental parser generator producing concrete syntax trees for 200+ languages. Fast enough for per-keystroke parsing, robust with syntax errors. Enables syntax-aware edits, symbol extraction, and structural navigation. Used by Aider, Claude Code, Cursor. https://github.com/tree-sitter/tree-sitter
- **Language Server Protocol (LSP)** — Microsoft protocol (spec 3.17) giving tools IDE-grade intelligence: diagnostics, go-to-definition, references, rename, completions. JSON-RPC based — directly inspired MCP's design. Agents use LSP for pre/post-edit validation and safe refactoring. https://microsoft.github.io/language-server-protocol/

| Tool | Type | Speed | Languages | Incremental | Primary Agent Use |
|------|------|-------|-----------|-------------|-------------------|
| diff-match-patch | Diff / Patch | Very fast | Agnostic | N/A | Applying fuzzy code edits |
| Tree-sitter | Parser / CST | Very fast | 200+ grammars | Yes | Syntax-aware navigation & edits |
| LSP | Protocol | Varies | Per server | Yes | Diagnostics, refactoring, symbols |
| Universal Ctags | Indexer | Fast | 200+ | No (batch) | Symbol indexing for context |

### Safety & Guardrails

- **NeMo Guardrails** — NVIDIA's open-source toolkit (Apache 2.0) for adding programmable rails to LLM apps. Uses Colang DSL to define input/output/dialog/execution rails. Includes LLM vulnerability scanning against jailbreaks. Python 3.10–3.13. https://github.com/NVIDIA/NeMo-Guardrails
- **NVIDIA's layered approach** — Input rails (prompt-injection detection, PII redaction) → output rails (code validation, harmful-content filtering) → execution rails (tool invocation control, rate limiting). Complements OS-level sandboxing with application-level safety.

---

## Real-World Implementations

| Agent | Approach | Reference |
|-------|----------|-----------|
| **Codex** | 3-layer sandbox (bubblewrap + seccomp + Landlock), execution policy engine | [`../agents/codex/tool-system.md`](../agents/codex/tool-system.md) |
| **Goose** | MCP-native architecture, 7 extension types | [`../agents/goose/tool-system.md`](../agents/goose/tool-system.md) |
| **OpenHands** | Action/observation model, 9 tools | [`../agents/openhands/tool-system.md`](../agents/openhands/tool-system.md) |
| **OpenCode** | 14 tools with permission system | [`../agents/opencode/tool-system.md`](../agents/opencode/tool-system.md) |
| **Claude Code** | 27-tool catalog, 5 permission modes | [`../agents/claude-code/tool-system.md`](../agents/claude-code/tool-system.md) |
| **Gemini CLI** | 18+ tools, multi-tier sandboxing | [`../agents/gemini-cli/tool-system.md`](../agents/gemini-cli/tool-system.md) |
| **mini-SWE-agent** | Bash-only — no custom tool system | [`../agents/mini-swe-agent/tool-system.md`](../agents/mini-swe-agent/tool-system.md) |
| **Aider** | 6 edit formats (diff, whole, diff-fenced, architect) | [`../agents/aider/tool-system.md`](../agents/aider/tool-system.md) |
| **π-coding-agent** | Radically extensible 4-tool system | [`../agents/pi-coding-agent/tool-system.md`](../agents/pi-coding-agent/tool-system.md) |
| **Warp** | 20+ tools, PTY interaction, Computer Use, LSP, Active AI error detection | [`../agents/warp/tool-system.md`](../agents/warp/tool-system.md) |
| **Ante** | Rust-native, custom MCP SDK, bidirectional tool sharing | [`../agents/ante/tool-system.md`](../agents/ante/tool-system.md) |
| **Droid** | Enterprise tools, Factory Analytics, GitHub Actions repair, Skills system | [`../agents/droid/tool-system.md`](../agents/droid/tool-system.md) |
| **Junie CLI** | IDE+CLI dual-mode, 4-level permissions, 30+ tool functions | [`../agents/junie-cli/tool-system.md`](../agents/junie-cli/tool-system.md) |
| **ForgeCode** | 6 tools with tool-call correction layer, schema engineering | [`../agents/forgecode/tool-system.md`](../agents/forgecode/tool-system.md) |
| **Sage-Agent** | ToolManager with MCP integration (stdio + SSE) | [`../agents/sage-agent/tool-system.md`](../agents/sage-agent/tool-system.md) |
| **TongAgents** | Minimal confirmed tool set, multi-agent, benchmark-inferred | [`../agents/tongagents/tool-system.md`](../agents/tongagents/tool-system.md) |
| **Capy** | Hard agent-tool boundaries, Ubuntu VM per task | [`../agents/capy/tool-system.md`](../agents/capy/tool-system.md) |
