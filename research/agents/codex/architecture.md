# Codex CLI — Core Architecture

## Overview

Codex CLI is structured as a **Cargo workspace with 70+ crates** following a strict
layered architecture. The design separates UI concerns from business logic, uses a
message-passing (SQ/EQ) pattern for communication, and delegates sandboxing to
OS-native mechanisms.

## Repository Structure

```
openai/codex/
├── codex-rs/                    # Rust implementation (production)
│   ├── core/                    # Business logic engine (~120 modules)
│   ├── protocol/                # Shared types and SQ/EQ protocol
│   ├── cli/                     # Binary entry point + subcommands
│   ├── tui/                     # Full-screen Ratatui terminal UI
│   ├── exec/                    # Non-interactive execution mode
│   ├── exec-server/             # JSON-RPC exec server
│   ├── config/                  # Layered TOML configuration
│   ├── execpolicy/              # Command approval policy engine
│   ├── execpolicy-legacy/       # Legacy allowlist-based policy
│   ├── linux-sandbox/           # Bubblewrap + seccomp + Landlock
│   ├── windows-sandbox-rs/      # Windows ACL + Firewall sandbox
│   ├── process-hardening/       # Pre-main security hardening
│   ├── shell-command/           # Shell command parsing (84KB parser!)
│   ├── shell-escalation/        # Privileged escalation server
│   ├── codex-api/               # Typed OpenAI API client
│   ├── backend-client/          # Codex Cloud backend client
│   ├── mcp-server/              # Codex as MCP server
│   ├── connectors/              # ChatGPT app/plugin directory
│   ├── skills/                  # Embedded skill templates
│   ├── hooks/                   # Hook system (session, prompt, stop)
│   ├── state/                   # SQLite state persistence
│   ├── responses-api-proxy/     # Security-hardening API key proxy
│   ├── ollama/                  # Ollama local model support
│   ├── lmstudio/                # LM Studio local model support
│   ├── apply-patch/             # Unified diff/patch application
│   ├── file-search/             # File search capabilities
│   ├── network-proxy/           # Managed network proxy
│   ├── otel/                    # OpenTelemetry instrumentation
│   ├── secrets/                 # Secret management
│   ├── login/                   # Authentication flows
│   ├── app-server/              # App server (IDE extension backend)
│   ├── app-server-protocol/     # App server JSON-RPC protocol
│   └── ...                      # 40+ more crates
├── codex-cli/                   # Legacy TypeScript CLI (deprecated)
├── sdk/                         # SDK packages
├── shell-tool-mcp/              # Shell tool MCP server
├── docs/                        # Documentation
└── scripts/                     # Build/release scripts
```

## Core Architecture Pattern: SQ/EQ (Submission Queue / Event Queue)

The foundational communication pattern across the entire system is the **SQ/EQ**
(Submission Queue / Event Queue) model. This is a unidirectional message-passing
system that cleanly separates the UI layer from the agent core.

### Submission Queue (SQ) — UI → Core

```rust
// codex-rs/protocol/src/protocol.rs
pub struct Submission {
    pub id: String,
    pub op: Op,
    pub trace: Option<W3cTraceContext>,
}

pub enum Op {
    Interrupt,
    UserInput { items: Vec<UserInput>, ... },
    UserTurn {
        items: Vec<UserInput>,
        cwd: PathBuf,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
        model: String,
        effort: Option<ReasoningEffort>,
        collaboration_mode: Option<CollaborationMode>,
        ...
    },
    OverrideTurnContext { cwd, approval_policy, model, ... },
    ExecApproval { id: String, decision: ReviewDecision },
    PatchApproval { id: String, decision: ReviewDecision },
    ResolveElicitation { ... },
    Compact { ... },
    Undo { ... },
    ThreadRollback { ... },
    Review { ... },
    ListModels {},
    Shutdown {},
    // ... RealtimeConversation*, McpServerRefresh, etc.
}
```

### Event Queue (EQ) — Core → UI

```rust
pub enum EventMsg {
    TurnStarted(TurnStartedEvent),
    TurnComplete(TurnCompleteEvent),
    TurnAborted(TurnAbortedEvent),
    AgentMessage(AgentMessageEvent),
    Reasoning(ReasoningEvent),
    ExecApprovalRequest(ExecApprovalRequestEvent),
    ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent),
    ContextCompacted(ContextCompactedEvent),
    TokenUsage(TokenUsageEvent),
    Error(ErrorEvent),
    ShutdownComplete,
    // ... dozens more
}
```

This pattern enables:
- **Multiple frontends**: TUI, exec mode, app-server, and MCP server all use
  the same core engine
- **Async processing**: The UI never blocks on model calls or tool execution
- **Testing**: Core logic can be tested without any UI
- **Distributed operation**: The exec-server extends this over JSON-RPC/WebSocket

## The `Codex` Struct — Heart of the System

```rust
// codex-rs/core/src/codex.rs
pub struct Codex {
    pub(crate) tx_sub: Sender<Submission>,    // Submit operations
    pub(crate) rx_event: Receiver<Event>,     // Receive events
    pub(crate) agent_status: watch::Receiver<AgentStatus>,
    pub(crate) session: Arc<Session>,
    pub(crate) session_loop_termination: SessionLoopTermination,
}
```

`Codex::spawn()` is the factory method that:
1. Creates submission/event channel pairs
2. Initializes the `Session` (model client, sandbox manager, tool registry)
3. Starts the background submission loop on a Tokio task
4. Returns `CodexSpawnOk { codex, thread_id }`

### Spawn Arguments

```rust
pub(crate) struct CodexSpawnArgs {
    pub config: Config,
    pub auth_manager: Arc<AuthManager>,
    pub models_manager: Arc<ModelsManager>,
    pub skills_manager: Arc<SkillsManager>,
    pub plugins_manager: Arc<PluginsManager>,
    pub mcp_manager: Arc<McpManager>,
    pub conversation_history: InitialHistory,
    pub session_source: SessionSource,
    pub agent_control: AgentControl,
    pub dynamic_tools: Vec<DynamicToolSpec>,
}
```

## CodexThread — Conversation Abstraction

A `CodexThread` wraps a `Codex` instance and adds conversation-level features:

```rust
pub struct CodexThread {
    pub(crate) codex: Codex,
    rollout_path: Option<PathBuf>,
    out_of_band_elicitation_count: Mutex<u64>,
}

pub struct ThreadConfigSnapshot {
    pub model: String,
    pub model_provider_id: String,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub cwd: PathBuf,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub personality: Option<Personality>,
}
```

Key methods:
- `submit(Op)` — Send an operation to the core
- `next_event()` — Receive the next event from the core
- `steer_input()` — Inject instructions into a running turn
- `shutdown_and_wait()` — Graceful shutdown

## Multi-Agent System

Codex supports spawning sub-agents for parallelizing complex tasks. The
multi-agent control plane lives in `core/src/agent/`:

### AgentControl

```rust
pub(crate) struct AgentControl {
    manager: Weak<ThreadManagerState>,  // Weak ref to avoid cycles
    state: Arc<Guards>,
}
```

Key operations:
- **`spawn_agent()`** — Creates a sub-agent with an initial prompt, reserves a
  spawn slot, assigns a nickname, optionally forks parent rollout
- **`send_input()`** — Sends user input to an existing agent
- **`interrupt_agent()`** / **`close_agent()`** / **`shutdown_agent_tree()`**
- **`resume_agent_from_rollout()`** — Resumes from persisted JSONL rollout,
  recursively resuming descendants

### Resource Guards

```rust
pub(crate) struct Guards {
    active_agents: Mutex<ActiveAgents>,
    total_count: AtomicUsize,  // enforces max_threads via CAS
}

pub(crate) struct SpawnReservation {
    state: Arc<Guards>,
    active: bool,
    reserved_agent_nickname: Option<String>,
}
```

`SpawnReservation` implements `Drop` to decrement the counter if spawn fails —
ensuring the guard is always released.

### Role System

Built-in roles configure agent behavior:

| Role | Purpose | Config Override |
|---|---|---|
| `default` | Standard agent | None |
| `explorer` | Codebase Q&A — "fast and authoritative" | May use faster model |
| `worker` | Execution tasks with file ownership semantics | Scoped permissions |

```rust
pub(crate) async fn apply_role_to_config(
    config: &mut Config,
    role_name: Option<&str>,
) -> Result<(), String>
```

Roles are applied as high-precedence config layers, preserving the caller's
model/provider unless explicitly overridden.

## Configuration System

Codex uses a **layered TOML configuration** system with strict precedence:

```
MDM (lowest) → System → User (~/.codex/config.toml) → Project → SessionFlags (highest)
```

### Config Layer Stack

```rust
pub struct ConfigLayerStack {
    layers: Vec<ConfigLayerEntry>,
}

pub struct ConfigLayerEntry {
    pub name: ConfigLayerSource,
    pub config: TomlValue,
    pub version: u64,
    pub disabled_reason: Option<String>,
}

pub enum ConfigLayerSource {
    Mdm,
    System,
    User,
    Project,
    SessionFlags,
}
```

`effective_config()` merges all enabled layers and enforces `ConfigRequirements`
constraints from enterprise management.

### Enterprise Requirements

```rust
pub struct ConfigRequirements {
    pub approval_policy: Option<...>,
    pub sandbox_policy: Option<...>,
    pub web_search_mode: Option<...>,
    pub mcp_servers: Vec<McpServerRequirement>,
    pub exec_policy: Option<...>,
    pub enforce_residency: Option<ResidencyRequirement>,
    pub network: Option<NetworkConstraints>,
}
```

Sources: MDM managed preferences, cloud requirements, system requirements TOML.

## Model Provider Architecture

### Provider Info

```rust
struct ModelProviderInfo {
    name: String,
    base_url: Option<String>,
    env_key: Option<String>,             // e.g. "OPENAI_API_KEY"
    wire_api: WireApi,                   // Only Responses API now
    query_params: Option<HashMap<String, String>>,
    http_headers: Option<HashMap<String, String>>,
    request_max_retries: Option<u64>,    // default 4
    stream_max_retries: Option<u64>,     // default 5
    stream_idle_timeout_ms: Option<u64>, // default 300,000
    requires_openai_auth: bool,
    supports_websockets: bool,
}
```

### Built-in Providers

| Provider | Base URL | Auth |
|---|---|---|
| `openai` | `api.openai.com/v1` | OpenAI auth (OAuth / API key) |
| `ollama` | `localhost:11434/v1` | None |
| `lmstudio` | `localhost:1234/v1` | None |

Custom providers can be defined in `config.toml` with arbitrary base URLs,
headers, and query parameters.

### Wire API

Only the **Responses API** (`/v1/responses`) is supported. The legacy Chat
Completions API has been removed entirely — `wire_api = "chat"` produces an
error with migration instructions.

## Session Persistence

### Rollout Files

Sessions are persisted as JSONL rollout files under `~/.codex/sessions/`.
Each line contains a `ResponseItem` or `Op` that can be replayed to resume a
conversation. Sub-agent rollouts are stored alongside parent rollouts.

### SQLite State DB

```rust
// codex-rs/state/
pub const STATE_DB_VERSION: i32 = 5;
pub const LOGS_DB_VERSION: i32 = 1;
```

Stores thread metadata, agent job state, and structured logs. Location
controlled by `sqlite_home` config or `CODEX_SQLITE_HOME` env var.

## Responses API Proxy

A dedicated security-hardening proxy (`codex-rs/responses-api-proxy/`) that:
- Only forwards `POST /v1/responses` to upstream
- Reads API key from stdin (never environment)
- Uses `mlock(2)` to prevent key from being swapped to disk
- Leaks key to `'static` lifetime for `HeaderValue::from_static()`
- Marks header as sensitive (`.set_sensitive(true)`)
- Optional `GET /shutdown` for unprivileged teardown

## Hook System

Three hook points with defined JSON Schema contracts:

| Hook | When | Can Do |
|---|---|---|
| `SessionStart` | Session begins | Continue/stop, inject system message |
| `UserPromptSubmit` | User sends prompt | Block with reason |
| `Stop` | Turn ends | Block continuation, provide reason |

```rust
enum HookEvent {
    AfterAgent { thread_id, turn_id, input_messages, last_assistant_message },
    AfterToolUse { turn_id, call_id, tool_name, tool_kind, tool_input,
                   executed, success, duration_ms, mutating, sandbox,
                   sandbox_policy, output_preview },
}
```

Hooks can be shell scripts or programmatic functions registered via the config.

## Skills System

Skills are embedded at compile time via `include_dir!()` and installed to
`~/.codex/skills/.system/` on first run. A fingerprint check avoids redundant
reinstallation. Includes a `skill-creator` skill for generating new skills.

## OpenTelemetry Integration

Opt-in OTel export for enterprise auditing:

```toml
[otel]
environment = "staging"
exporter = "none"  # or "otlp-http" / "otlp-grpc"
log_user_prompt = false  # redact by default
```

Event categories: `codex.conversation_starts`, `codex.api_request`,
`codex.sse_event`, `codex.user_prompt`, `codex.tool_decision`,
`codex.tool_result`. Corresponding counter + histogram metrics for each.

## Build System

Dual build support:
- **Cargo**: Primary development build
- **Bazel**: CI/release builds with `MODULE.bazel.lock` lockfile tracking

The `justfile` in `codex-rs/` provides common commands: `just fmt`, `just fix`,
`just test`, `just write-config-schema`, `just bazel-lock-update`.

## Key Design Decisions

1. **Rust over TypeScript**: Zero-dependency binary, native performance, memory safety,
   direct syscall access for sandboxing
2. **SQ/EQ pattern**: Enables multiple frontends (TUI, exec, IDE, MCP) sharing one core
3. **OS-native sandboxing**: No containers/VMs — uses Seatbelt (macOS), bubblewrap+seccomp
   (Linux), ACLs+Firewall (Windows) for zero-overhead isolation
4. **Layered config**: Enterprise-friendly with MDM support and managed requirements
5. **Process hardening at ctor**: Security applied before `main()` even runs
6. **No tokenizer**: Uses byte-based heuristics (~4 bytes/token) for speed
7. **Vendored bubblewrap**: Falls back to embedded bwrap if system one absent
8. **Responses API only**: Dropped Chat Completions — fully committed to Responses wire API