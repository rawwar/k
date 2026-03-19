# Goose — Core Architecture

## Overview

Goose is a Rust-based coding agent organized as a Cargo workspace. The architecture follows a layered design: UI → Server → Agent → Provider + ExtensionManager. The core insight is that **all tool functionality flows through MCP** — there is no separate "built-in tool" abstraction. Everything, from file editing to shell execution, is an MCP server.

## Workspace Structure

```
block/goose/
├── .cargo/                    # Cargo build config
├── .devcontainer/             # Dev container setup
├── crates/                    # Rust workspace crates
│   ├── goose/                 # Core library (agent, providers, config, MCP client)
│   ├── goose-cli/             # CLI binary
│   ├── goose-server/          # HTTP API server (Axum)
│   ├── goose-mcp/             # Built-in MCP server implementations
│   ├── goose-acp/             # Agent Communication Protocol
│   ├── goose-acp-macros/      # ACP proc macros
│   ├── goose-test/            # Test utilities
│   └── goose-test-support/    # Test infrastructure
├── documentation/             # Docs site (Docusaurus)
├── extensions/                # External extension examples/recipes
├── ui/                        # Frontend applications
│   ├── desktop/               # Electron/Tauri desktop app
│   ├── acp/                   # ACP UI
│   └── text/                  # Text-based TUI
├── vendor/v8/                 # Vendored V8 engine stub
└── Cargo.toml                 # Workspace root
```

## Key Dependencies

| Dependency | Version | Purpose |
|-----------|---------|---------|
| `rmcp` | 1.2.0 | Rust MCP SDK (client + server) |
| `sacp` | 10.1.0 | Agent Communication Protocol SDK |
| `tokio` | 1.x (full) | Async runtime |
| `axum` | 0.8 | HTTP server framework |
| `reqwest` | - | HTTP client for provider APIs |
| `serde` / `serde_json` / `serde_yaml` | - | Serialization |
| `tree-sitter` | - | Code analysis (Go, Java, JS, Kotlin, Python, Ruby, Rust, Swift, TypeScript) |
| `opentelemetry` / `tracing` | - | Observability |
| `schemars` | - | JSON Schema generation for tool definitions |

## Core Crate: `goose`

The `goose` crate (`crates/goose/src/`) contains the heart of the system:

```
crates/goose/src/
├── agents/
│   ├── agent.rs                    # Main Agent struct, reply loop (~97KB)
│   ├── extension_manager.rs        # MCP extension lifecycle & routing (~81KB)
│   ├── mcp_client.rs               # MCP client wrapper over rmcp
│   ├── extension.rs                # ExtensionConfig enum (7 transport types)
│   ├── tool_execution.rs           # Tool approval & dispatch context
│   ├── reply_parts.rs              # LLM streaming, tool argument coercion
│   ├── retry.rs                    # Retry/success-check manager
│   ├── types.rs                    # SessionConfig, RetryConfig, SharedProvider
│   ├── platform_extensions/        # In-process platform extensions
│   │   ├── developer/              # File editing, shell, tree (DEFAULT)
│   │   ├── analyze/                # Tree-sitter code analysis
│   │   ├── todo/                   # Task tracking
│   │   ├── apps/                   # HTML app management
│   │   ├── chatrecall/             # Conversation search
│   │   ├── extension_manager_ext/  # Runtime extension management
│   │   ├── summon/                 # Subagent delegation
│   │   └── tom/                    # Top of Mind context injection
│   └── mod.rs
├── config/
│   ├── base.rs                     # Config singleton (~/.config/goose/config.yaml)
│   ├── extensions.rs               # Extension config persistence
│   ├── goose_mode.rs               # Permission modes
│   ├── permission.rs               # Permission manager
│   └── mod.rs
├── context_mgmt/
│   └── mod.rs                      # Compaction, summarization, token management
├── conversation/
│   ├── message.rs                  # Message types (~53KB)
│   └── mod.rs                      # Conversation struct (~44KB)
├── model.rs                        # ModelConfig, context limits
├── providers/
│   ├── base.rs                     # Provider trait
│   ├── anthropic.rs                # Anthropic Claude
│   ├── openai.rs                   # OpenAI GPT
│   ├── google.rs                   # Google Gemini
│   ├── azure.rs                    # Azure OpenAI
│   ├── bedrock.rs                  # AWS Bedrock
│   ├── ollama.rs                   # Ollama local models
│   └── ... (30+ provider files)
├── prompt/                         # System prompt construction
├── session/                        # Session persistence
└── token_counter.rs                # Token estimation
```

## The Agent Struct

The `Agent` is the central coordinator:

```rust
pub struct Agent {
    provider: SharedProvider,                    // Arc<Mutex<Option<Arc<dyn Provider>>>>
    config: AgentConfig,                         // session, permissions, mode, platform
    current_goose_mode: Mutex<GooseMode>,        // auto/approve/smart_approve/chat
    extension_manager: Arc<ExtensionManager>,    // all MCP extensions
    final_output_tool: Arc<Mutex<Option<FinalOutputTool>>>,
    frontend_tools: Mutex<HashMap<String, FrontendTool>>,
    frontend_instructions: Mutex<Option<String>>,
    prompt_manager: Mutex<PromptManager>,
    tool_confirmation_router: ToolConfirmationRouter,
    tool_result_tx: mpsc::Sender<(String, ToolResult<CallToolResult>)>,
    tool_result_rx: ToolResultReceiver,
    retry_manager: RetryManager,
    tool_inspection_manager: ToolInspectionManager,
    container: Mutex<Option<Container>>,
}
```

### AgentConfig

```rust
pub struct AgentConfig {
    pub session_manager: Arc<SessionManager>,
    pub permission_manager: Arc<PermissionManager>,
    pub scheduler_service: Option<Arc<dyn SchedulerTrait>>,
    pub goose_mode: GooseMode,
    pub disable_session_naming: bool,
    pub goose_platform: GoosePlatform,
}
```

### Tool Inspection Pipeline

On construction, the Agent creates a `ToolInspectionManager` with 4 inspectors in order:

1. **SecurityInspector** — Checks for dangerous command patterns
2. **AdversaryInspector** — Detects prompt injection attempts in tool arguments
3. **PermissionInspector** — Applies per-tool permission rules (allow/ask/deny)
4. **RepetitionInspector** — Detects repeated failed tool calls

## Provider System

The `Provider` trait abstracts all LLM communication:

```rust
pub trait Provider: Send + Sync {
    fn get_name(&self) -> &str;
    async fn stream(
        &self, model_config: &ModelConfig, session_id: &str,
        system: &str, messages: &[Message], tools: &[Tool],
    ) -> Result<MessageStream, ProviderError>;
    async fn complete(...) -> Result<(Message, ProviderUsage)>;
    async fn complete_fast(...) -> Result<(Message, ProviderUsage)>;
    fn get_model_config(&self) -> ModelConfig;
    fn retry_config(&self) -> RetryConfig;
    async fn fetch_supported_models(&self) -> Result<Vec<String>>;
    async fn fetch_recommended_models(&self) -> Result<Vec<String>>;
}
```

### ModelConfig

```rust
pub struct ModelConfig {
    pub model_name: String,
    pub context_limit: Option<usize>,          // Default: 128,000 tokens
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub toolshim: bool,                        // For non-tool-calling models
    pub toolshim_model: Option<String>,
    pub fast_model_config: Option<Box<ModelConfig>>,
    pub request_params: Option<HashMap<String, Value>>,
    pub reasoning: Option<bool>,
}
```

### Provider Implementations (30+)

**Major providers:**
- Anthropic (Claude) — with prompt caching support
- OpenAI (GPT-4o, o1, etc.)
- Google (Gemini) — with configurable thinking levels
- Azure OpenAI
- AWS Bedrock
- Databricks

**Local model runners:**
- Ollama
- LM Studio
- Docker Model Runner
- Ramalama

**Gateway/proxy:**
- OpenRouter, LiteLLM, Tetrate, OpenAI-compatible

**Agent-as-provider (ACP):**
- Claude Code ACP, Codex ACP, Gemini ACP, Cursor Agent

### Toolshim

For models that don't natively support tool calling (e.g., some Ollama models), Goose uses a "toolshim" — it converts tool definitions into text instructions, then post-processes the LLM output to extract JSON tool calls from code blocks.

## Configuration System

Configuration is managed through a YAML file at `~/.config/goose/config.yaml`:

```rust
pub struct Config {
    values: Arc<Mutex<Mapping>>,
    config_path: PathBuf,
    defaults_path: Option<PathBuf>,
}

impl Config {
    pub fn global() -> &'static Config;  // Singleton
    pub fn get_param<T: Deserialize>(&self, key: &str) -> Result<T>;
    pub fn set_param<V: Serialize>(&self, key: &str, value: V) -> Result<()>;
}
```

Key config values:
- `GOOSE_PROVIDER` — Provider name (anthropic, openai, google, etc.)
- `GOOSE_MODEL` — Model identifier
- `GOOSE_MODE` — Permission mode (auto, approve, smart_approve, chat)
- `GOOSE_CONTEXT_LIMIT` — Override context window size
- `GOOSE_MAX_TURNS` — Max tool execution turns (default: 1000)

### Extension Configuration

Extensions are stored in the same YAML config:

```yaml
extensions:
  github:
    name: GitHub
    cmd: npx
    args: [-y, @modelcontextprotocol/server-github]
    enabled: true
    envs: { "GITHUB_PERSONAL_ACCESS_TOKEN": "<TOKEN>" }
    type: stdio
    timeout: 300
```

## Session Management

Sessions represent continuous conversations. The `SessionManager` handles:

- Persisting messages to disk
- Loading/resuming sessions
- Session metadata (name, timestamps, token counts)
- Auto-generating session names from conversation content

Sessions are shared between CLI and Desktop — changing provider/extension settings in one interface is reflected in the other.

## Permission System

Goose supports 4 permission modes:

| Mode | CLI Flag | Behavior |
|------|----------|----------|
| Autonomous | `auto` | No approval needed for any tool |
| Manual Approval | `approve` | Every tool call requires user approval |
| Smart Approval | `smart_approve` | AI decides what needs review |
| Chat Only | `chat` | All tools disabled, text conversation only |

Per-tool permissions can be configured as: Always allow, Ask before, Never allow.

## GooseHints System

Goose reads `.goosehints` files (or `AGENTS.md`, configurable via `CONTEXT_FILE_NAMES` env var) for persistent instructions:

- **Global hints**: `~/.config/goose/.goosehints`
- **Local hints**: `.goosehints` in project directories (hierarchical, nested)
- Loaded at session start, injected into system prompt
- Support `@filename` syntax for auto-including file contents

## Observability

Goose integrates OpenTelemetry for tracing:
- `tracing` + `tracing-opentelemetry` for structured logging
- Provider usage tracking (tokens in/out, costs)
- Session metrics

## Desktop Application

The desktop UI (`ui/desktop/`) is an Electron-based application that communicates with `goose-server` over HTTP. It provides:

- Session management with sidebar navigation
- Extension management UI (toggle, add, configure)
- Model/provider selection
- Permission mode switching mid-session
- Deep link protocol (`goose://extension?...`) for one-click extension installation