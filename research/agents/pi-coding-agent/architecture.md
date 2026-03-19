# Pi — Core Architecture

## High-Level Design

Pi's architecture is defined by its monorepo structure. Rather than a single monolithic application, Pi is composed of seven independently publishable npm packages, each with a clear responsibility boundary. The coding agent itself is just one package that composes the others. This separation means the LLM API, the agent runtime, and the terminal UI can each be used independently in other projects.

```
pi-mono/
├── packages/
│   ├── ai/           @mariozechner/pi-ai          — Unified multi-provider LLM API
│   ├── agent/        @mariozechner/pi-agent-core   — Agent runtime, tool calling, state
│   ├── coding-agent/ @mariozechner/pi-coding-agent — The CLI application
│   ├── tui/          @mariozechner/pi-tui          — Terminal UI framework
│   ├── web-ui/       @mariozechner/pi-web-ui       — Web components for chat UIs
│   ├── mom/          @mariozechner/pi-mom          — Slack bot delegating to pi
│   └── pods/         @mariozechner/pi-pods         — CLI for managing vLLM on GPU pods
├── package.json          (workspace root)
└── tsconfig.json
```

The dependency flow is strictly layered:

```
pi-ai (no internal deps)
  ↓
pi-agent-core (depends on pi-ai)
  ↓
pi-tui (no internal deps — standalone UI framework)
  ↓
pi-coding-agent (depends on pi-agent-core + pi-tui + pi-ai)
  ↓
pi-mom, pi-web-ui, pi-pods (independent utilities)
```

## Package Deep Dives

### 1. pi-ai — Unified Multi-Provider LLM API

`pi-ai` is the foundation layer that abstracts away LLM provider differences. It supports four underlying API protocols:

| Protocol | Providers |
|----------|-----------|
| OpenAI Completions | OpenAI, Azure OpenAI, Groq, Cerebras, xAI, OpenRouter, Hugging Face, Kimi, MiniMax |
| OpenAI Responses | OpenAI (newer API) |
| Anthropic Messages | Anthropic, Bedrock (Anthropic models) |
| Google Generative AI | Google (Gemini models) |

**Key capabilities:**

- **Cross-provider context handoff**: Convert conversation history between providers. This handles complex translations like Anthropic thinking traces → OpenAI system messages, signed content blobs that can't leave their provider, and different reasoning content field locations.
- **Token and cost tracking**: Best-effort tracking across providers with different reporting granularities.
- **Provider quirk handling**: Cerebras, xAI, and Mistral don't support the `store` field. Different providers use different `max_tokens` field names. Reasoning content lives in different response fields depending on the provider.
- **Browser support**: Some providers support CORS, so pi-ai works in browser contexts — enabling pi-web-ui.

The abstraction is intentionally thin. Pi-ai doesn't try to normalize all provider behavior into a lowest-common-denominator API. Instead, it handles the sharp edges (field naming, unsupported parameters, content format translation) while preserving provider-specific capabilities.

### 2. pi-agent-core — Agent Runtime

`pi-agent-core` provides the minimal scaffold for an agent loop:

- **Agent loop**: The core cycle of receiving user input, calling the LLM, executing tool calls, feeding results back.
- **Tool execution**: Dispatching tool calls to registered handlers, collecting results, handling errors.
- **State management**: Tracking conversation history, tool call state, and session metadata.
- **Event streaming**: Emitting events (message received, tool call started, tool call completed, response streaming) that consumers can subscribe to.
- **Validation**: Ensuring tool call parameters meet schemas, handling malformed LLM responses.

The design is deliberately minimal. There is no built-in concept of permissions, no planning layer, no sub-agent orchestration. These are concerns for the coding agent layer or extensions to handle.

### 3. pi-tui — Terminal UI Framework

`pi-tui` is a custom terminal UI framework built from scratch — not based on Ink, blessed, or any existing terminal UI library. Key design choices:

- **Differential rendering**: Only re-renders changed portions of the screen, eliminating flicker during streaming output. This is critical for a coding agent where LLM responses stream in token-by-token.
- **Retained-mode UI**: UI state is maintained in a tree of components. Updates modify the tree and trigger targeted re-renders rather than full-screen redraws.
- **Synchronized output**: Coordinates terminal output operations to prevent interleaving and visual corruption, especially important when multiple streams (agent output, user input, status indicators) are active simultaneously.

The decision to build a custom TUI rather than use existing libraries reflects Pi's philosophy — existing solutions didn't meet the specific needs, and building from scratch meant full control over rendering behavior.

### 4. pi-coding-agent — The CLI

The coding agent itself composes the other packages and adds:

- **Default tool set**: Four tools — `read`, `write`, `edit`, `bash` (see [tool-system.md](tool-system.md))
- **Extension system**: TypeScript API for registering tools, commands, keyboard shortcuts, event handlers, UI components, and more (see [tool-system.md](tool-system.md))
- **Skills system**: On-demand capability packages via SKILL.md files (see [tool-system.md](tool-system.md))
- **Context management**: AGENTS.md, SYSTEM.md, compaction, prompt templates (see [context-management.md](context-management.md))
- **Session management**: Tree-structured JSONL persistence with branching and forking (see [context-management.md](context-management.md))
- **Package management**: Install/manage extensions and skills from npm or git
- **Message queue**: Steering and follow-up messages while agent works (see [agentic-loop.md](agentic-loop.md))

## The Four Modes of Operation

Pi runs in four distinct modes, each targeting a different use case:

### 1. Interactive Mode (Default)
The standard terminal experience. User types messages, sees streaming responses, can submit steering messages mid-generation. The full TUI is active with status lines, keyboard shortcuts, and extension-provided UI components.

### 2. Print / JSON Mode
Non-interactive output mode for scripting. Pi processes input and outputs results as plain text or structured JSON. Useful for CI pipelines, batch processing, and integration with other tools.

### 3. RPC Mode (stdin/stdout)
Pi acts as a JSON-RPC server over stdin/stdout. Other programs can drive pi programmatically — send messages, receive events, call tools. This is how pi-mom (the Slack bot) communicates with pi instances.

### 4. SDK Mode
Direct programmatic access via the TypeScript API. Import pi-coding-agent as a library in your own application. Full access to the agent loop, tool system, and extension API without the CLI wrapper.

```
┌──────────────────────────────────────────────────────────┐
│                    pi-coding-agent                         │
│                                                            │
│  ┌────────────┐  ┌────────────┐  ┌───────┐  ┌─────┐     │
│  │ Interactive │  │ Print/JSON │  │  RPC  │  │ SDK │     │
│  │   (TUI)    │  │  (stdout)  │  │(stdio)│  │(API)│     │
│  └─────┬──────┘  └─────┬──────┘  └───┬───┘  └──┬──┘     │
│        │               │             │          │         │
│        ▼               ▼             ▼          ▼         │
│  ┌────────────────────────────────────────────────────┐   │
│  │              Agent Core + Extensions                │   │
│  │         ┌──────────────────────────┐               │   │
│  │         │    4 Tools + Ext Tools   │               │   │
│  │         └──────────────────────────┘               │   │
│  └────────────────────────────────────────────────────┘   │
│                          │                                 │
│                          ▼                                 │
│  ┌────────────────────────────────────────────────────┐   │
│  │           pi-ai (15+ LLM Providers)                │   │
│  └────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

## Extension / Skills / Packages Architecture

Pi's extensibility forms a three-tier system:

### Extensions (TypeScript Modules)
The most powerful integration point. Extensions are TypeScript modules that hook into the agent lifecycle:

- **Tool registration**: Add new tools or replace built-in tools entirely
- **Command registration**: Add slash commands accessible via `/command`
- **Event handlers**: Subscribe to agent lifecycle events (message, tool call, response, etc.)
- **UI components**: Custom status lines, headers, footers, editors
- **Keyboard shortcuts**: Bind custom actions to key combinations
- **Context manipulation**: Inject messages, filter history, implement RAG

### Skills (Capability Packages)
Lighter-weight than extensions. A skill is a SKILL.md file that describes a capability:

- Stored in `~/.pi/agent/skills/` or project directories
- Loaded on-demand via `/skill:name` or auto-loaded by the agent
- Follows the Agent Skills standard (agentskills.io)
- Provides progressive disclosure — capabilities are described only when needed

### Packages (Distribution Format)
Bundles of extensions, skills, prompts, and themes distributed via npm or git:

- Install: `pi install npm:@foo/pi-tools` or `pi install git:github.com/user/repo`
- npm keyword: `pi-package` for discoverability
- Can contain any combination of extensions, skills, prompt templates, and themes

This three-tier system means pi itself never needs to grow. New capabilities are added by the community as packages, keeping the core stable and predictable.
