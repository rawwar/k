# Gemini CLI — Architecture

> Deep dive into the TypeScript monorepo structure, core modules, sandbox system,
> and integration points.

## Monorepo Structure

Gemini CLI uses a TypeScript monorepo with two primary packages:

```
gemini-cli/
├── packages/
│   ├── cli/           # CLI entry point, REPL, slash commands, UI
│   │   └── src/
│   │       ├── cli.ts          # Main entry, argument parsing
│   │       ├── repl/           # Interactive REPL loop
│   │       ├── commands/       # Slash commands (/help, /memory, /restore, etc.)
│   │       ├── ui/             # Terminal UI rendering (ink-based)
│   │       └── headless/       # Non-interactive mode (text, JSON, stream-json)
│   │
│   └── core/          # Core agent engine — model-agnostic internals
│       └── src/
│           ├── agent/              # Agent orchestration (main loop driver)
│           ├── agents/             # Multi-agent / sub-agent support
│           ├── core/               # Core modules (see below)
│           ├── tools/              # Tool implementations (18+ built-in)
│           ├── sandbox/            # Sandbox backends (Seatbelt, Docker, etc.)
│           ├── config/             # Configuration management
│           ├── mcp/                # MCP server integration
│           ├── skills/             # Agent skills (progressive disclosure)
│           ├── policy/             # Security policy engine
│           ├── confirmation-bus/   # User confirmation system
│           ├── safety/             # Safety filters
│           ├── routing/            # Model/request routing
│           ├── scheduler/          # Tool execution scheduling
│           ├── hooks/              # Lifecycle hooks
│           ├── voice/              # Voice input support
│           ├── ide/                # IDE integration (VS Code companion)
│           ├── output/             # Output formatting
│           ├── telemetry/          # Usage tracking
│           ├── fallback/           # Fallback/error recovery
│           └── billing/            # Usage/billing tracking
├── .gemini/           # Project-level agent configuration
├── tools/             # Build and development tooling
└── docs/              # Documentation
```

## Core Modules (packages/core/src/core/)

The `core/` directory contains the fundamental building blocks of the agent engine.

### baseLlmClient.ts — LLM Client Abstraction

Provides the base interface for communicating with LLM backends. This abstraction
allows Gemini CLI to support multiple authentication methods (OAuth, API Key, Vertex AI)
through a unified client interface.

Key responsibilities:
- Request construction and serialization
- Response streaming and deserialization
- Error handling and retry logic
- Authentication header injection

### client.ts — Gemini API Client

The concrete implementation of baseLlmClient for Gemini models. Handles:
- API endpoint selection (generativelanguage.googleapis.com vs Vertex AI endpoints)
- Model-specific parameter configuration
- Token counting and limit enforcement
- Streaming response handling via Server-Sent Events

### contentGenerator.ts — Content Generation Pipeline

The content generator is the bridge between the agent loop and the LLM client. It:
1. Assembles the full prompt (system instructions + conversation history + tool results)
2. Manages token budgets using tokenLimits
3. Applies token caching for API key users
4. Submits requests and processes streaming responses
5. Extracts tool calls from model responses
6. Routes tool calls to the tool scheduler

This is the **heart of the agentic loop** — it converts a turn's state into an
API request and processes the response back into actions.

### geminiChat.ts — Chat Session Management

Manages the stateful chat session:
- Conversation history tracking
- Message role management (user, model, function)
- History truncation when approaching token limits
- Multi-turn context maintenance
- Integration with checkpointing system

### geminiRequest.ts — Request Construction

Handles the specifics of Gemini API request formatting:
- Tool declaration serialization
- Function calling configuration
- Safety settings
- Generation parameters (temperature, top-p, top-k)
- System instruction formatting
- Grounding configuration (Google Search)

### turn.ts — Turn State Machine

Each interaction cycle is modeled as a "turn." The turn module manages:
- Turn lifecycle: user_input -> model_response -> tool_execution -> model_response -> ...
- Turn completion detection (when the model doesn't request any tools)
- Turn state persistence for checkpointing
- Error recovery within a turn
- Maximum iteration limits to prevent infinite loops

### tokenLimits.ts — Token Budget Management

Critical for managing Gemini's 1M token context window:
- Calculates available token budget per request
- Reserves tokens for system instructions and tool declarations
- Manages conversation history pruning
- Token caching optimization (marks cacheable prefixes)
- Per-model limit configuration

### prompts.ts — System Prompts

Manages the system instruction assembly:
- Base system prompt with agent capabilities
- GEMINI.md content injection (global -> workspace -> JIT)
- Tool declarations and usage instructions
- Safety guidelines
- Context-dependent instructions

### coreToolScheduler.ts — Tool Scheduling

Coordinates tool execution within a turn:
- Parallel vs sequential tool execution
- Tool dependency resolution
- Timeout management
- Result aggregation
- Error handling and retry

### coreToolHookTriggers.ts — Hook System

Lifecycle hooks that fire at specific points:
- Pre/post tool execution hooks
- Turn completion hooks
- Error hooks
- Hooks for custom tool pipelines

## Tool Registry

The ToolRegistry is the central catalog of all available tools:

```
ToolRegistry
├── Built-in tools (18+)
│   ├── File system tools (glob, grep_search, read_file, write_file, etc.)
│   ├── Execution tools (run_shell_command)
│   ├── Planning tools (enter_plan_mode, exit_plan_mode)
│   ├── Memory tools (save_memory, activate_skill, get_internal_docs)
│   ├── Interaction tools (ask_user, write_todos)
│   ├── Web tools (google_web_search, web_fetch)
│   └── System tools (complete_task)
│
├── MCP tools (dynamically registered from MCP servers)
│
└── Custom tools (via tools.discoveryCommand)
```

Each tool registration includes:
- Name and description (used in model's tool declarations)
- Parameter schema (JSON Schema)
- Confirmation requirements (mutating vs read-only)
- Sandbox requirements (which tools need sandboxing)
- Execution function

## Confirmation Bus

The confirmation bus is a clean event-driven abstraction for user consent:

```
Tool Execution Request
       │
       v
┌──────────────────┐     ┌───────────────────┐
│ Confirmation Bus │────>│  Policy Engine     │
│                  │     │  (auto-approve     │
│                  │<────│   or require       │
│                  │     │   confirmation)    │
└──────┬───────────┘     └───────────────────┘
       │
       v
  User Prompt (if required)
       │
       v
  Tool Execution (in sandbox if configured)
```

The confirmation bus separates the **decision** of whether to confirm from the
**mechanism** of how confirmation happens (interactive prompt, auto-approve,
headless mode reject, etc.).

Key behaviors:
- **Read-only tools** (glob, grep_search, read_file): auto-approved, no confirmation
- **Mutating tools** (write_file, replace, run_shell_command): require confirmation
- **Policy overrides**: users can configure auto-approve patterns in settings
- **Headless mode**: configurable behavior (reject all, approve all, or specific patterns)

## Sandbox Architecture

Gemini CLI has the most sophisticated sandboxing system of any terminal coding agent,
with four distinct backends:

### 1. macOS Seatbelt (sandbox-exec)

- **Platform**: macOS only
- **Mechanism**: Apple's built-in sandboxing via `sandbox-exec` with SBPL profiles
- **Isolation**: Process-level; restricts file system, network, IPC
- **Profiles**: Multiple profiles for different tool types
- **Overhead**: Very low — native OS feature
- **Default on macOS**: Yes, for shell commands

### 2. Docker / Podman Containers

- **Platform**: Cross-platform (Linux, macOS, Windows)
- **Mechanism**: Full container isolation
- **Isolation**: File system, network, process namespace
- **Configuration**: Auto-detects Docker or Podman
- **Overhead**: Higher — container startup cost
- **Use case**: When stronger isolation than Seatbelt is needed

### 3. gVisor (runsc)

- **Platform**: Linux only
- **Mechanism**: User-space kernel intercepts syscalls
- **Isolation**: Strongest available — syscall-level filtering
- **Overhead**: Moderate — syscall interception cost
- **Use case**: Untrusted code execution, maximum security

### 4. LXC / LXD

- **Platform**: Linux only
- **Mechanism**: Full system containers
- **Isolation**: Complete system-level isolation
- **Overhead**: Higher — full system container
- **Use case**: When full OS-level isolation is required

### Sandbox Configuration

```bash
# Environment variable
export GEMINI_SANDBOX=docker  # Options: seatbelt, docker, podman, gvisor, lxc

# Or in settings.json
# { "sandbox": "docker", "sandboxFlags": ["--network=none"] }

# Custom flags via environment
export SANDBOX_FLAGS="--cpus=2 --memory=4g"
```

## Routing Module

The routing module handles model and request routing:
- **Model selection**: Based on configuration, task complexity, or fallback rules
- **Endpoint routing**: Google AI Studio vs Vertex AI endpoints
- **Fallback handling**: When primary model is unavailable
- **Rate limiting**: Respects per-model rate limits

## MCP Integration

Gemini CLI supports the Model Context Protocol for tool extensibility:

```
┌─────────────────┐     ┌──────────────────┐
│   Gemini CLI    │     │   MCP Server 1   │
│   (MCP Client)  │────>│   (e.g., GitHub) │
│                 │     └──────────────────┘
│                 │     ┌──────────────────┐
│                 │────>│   MCP Server 2   │
│                 │     │   (e.g., DB)     │
│                 │     └──────────────────┘
│                 │     ┌──────────────────┐
│                 │────>│   MCP Server N   │
│                 │     │   (custom)       │
└─────────────────┘     └──────────────────┘
```

Configuration in `.gemini/settings.json`:
```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "..." }
    }
  }
}
```

MCP tools are dynamically registered in the ToolRegistry alongside built-in tools,
making them available to the model as if they were native tools.

## IDE Integration

The `ide/` module supports VS Code companion integration:
- Bidirectional communication between terminal agent and VS Code
- File navigation commands
- Diagnostic sharing (linting errors, type errors)
- Editor state awareness

## Lifecycle Hooks

The hooks system provides extension points throughout the agent lifecycle:

```
Agent Start
  │
  ├── onAgentStart
  │
  ├── onTurnStart
  │   ├── onToolCall
  │   │   ├── onPreToolExecution
  │   │   └── onPostToolExecution
  │   ├── onModelResponse
  │   └── onTurnEnd
  │
  ├── onTurnStart (next turn)
  │   └── ...
  │
  └── onAgentEnd
```

Hooks can be registered by:
- Core modules (for internal coordination)
- MCP servers (for external integration)
- Custom configurations

## Configuration Hierarchy

```
Highest priority
    │
    ├── Command-line flags (--model, --sandbox, etc.)
    ├── Environment variables (GEMINI_SANDBOX, GEMINI_API_KEY, etc.)
    ├── Project settings (.gemini/settings.json)
    ├── User settings (~/.gemini/settings.json)
    └── Built-in defaults
    │
Lowest priority
```

## Build and Release

- **Build system**: TypeScript compilation, monorepo workspace management
- **Testing**: Unit tests per package, integration tests
- **Release cadence**: Weekly
  - **Nightly**: Continuous from main branch
  - **Preview**: Tuesday releases for early adopters
  - **Stable**: Tuesday releases for general use
- **Distribution**: npm, Homebrew, npx

## Key Architectural Decisions

1. **TypeScript monorepo**: Enables code sharing between CLI and core while maintaining
   clear package boundaries. Similar to Claude Code's approach.

2. **Separation of CLI and Core**: The core package is UI-agnostic, enabling reuse
   for IDE extensions (VS Code companion) and programmatic usage.

3. **Multi-tier sandboxing**: Rather than choosing one sandbox approach, Gemini CLI
   supports multiple backends. This adds complexity but provides flexibility across
   platforms and security requirements.

4. **Event-driven confirmation**: The confirmation bus pattern decouples tool execution
   from consent management, making it easy to add new confirmation strategies.

5. **Progressive skill disclosure**: Skills are loaded on-demand rather than included
   in every prompt, keeping the base token cost low and scaling expertise as needed.
