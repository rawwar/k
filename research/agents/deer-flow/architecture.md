---
title: DeerFlow Architecture
status: complete
---

# DeerFlow Architecture

## Tech Stack

| Layer | Technology | Notes |
|-------|------------|-------|
| Orchestration | LangGraph | Graph-based state machines; checkpointing, time-travel |
| LLM Abstraction | LangChain | Any OpenAI-compatible endpoint |
| Backend | Python 3.12+ | FastAPI + LangGraph Server |
| Frontend | Next.js / Node.js 22+ | pnpm, served on port 3000 |
| Package Manager (backend) | uv | Fast Python package management |
| Package Manager (frontend) | pnpm | Node.js |
| Container / Sandbox | Docker | Isolated per-task execution environment |
| Orchestration (advanced) | Kubernetes | Via provisioner service for K8s sandbox mode |

---

## Service Architecture

DeerFlow runs as multiple services, typically via Docker Compose or local dev scripts. A unified reverse proxy (nginx, port 2026) routes traffic:

```
Port 2026 (nginx unified proxy)
├── /                    → Next.js frontend (port 3000)
├── /api/langgraph/      → LangGraph Server (port 2024)
└── /api/gateway/        → Gateway API (port 8001)

Port 2024 — LangGraph Server
  └── Hosts the agent graph (lead_agent, mobile_agent, etc.)
  └── Runs via `langgraph dev` (open-source LangGraph CLI server)

Port 8001 — Gateway API
  └── Skill installation endpoint
  └── Follow-up suggestion generation
  └── IM channel dispatch (Telegram, Slack, Feishu)
  └── Health checks, model listing, thread management
```

---

## Agent Graph (LangGraph)

The lead agent is a **typed state graph** implemented in LangGraph. Each node is a Python function operating on the shared state:

```
                  ┌─────────────────┐
    ┌────────────►│  coordinator    │◄──────────────┐
    │             └────────┬────────┘               │
    │                      │ (delegate task)         │
    │             ┌────────▼────────┐               │
    │             │  planner        │ (pro/ultra     │
    │             └────────┬────────┘  modes only)   │
    │                      │                         │
    │             ┌────────▼────────┐               │
    │             │  researcher     │               │
    │             │  (web search,   │               │
    │             │   web fetch,    │               │
    │             │   bash, files)  │               │
    │             └────────┬────────┘               │
    │                      │ spawn sub-agents        │
    │             ┌────────▼────────┐               │
    │             │  sub_agent_1    │               │
    │             │  sub_agent_2    │ (ultra mode)   │
    │             │  sub_agent_N    │               │
    │             └────────┬────────┘               │
    │                      │ structured results      │
    │             ┌────────▼────────┐               │
    │             │  reporter       │               │
    │             │  (synthesis,    │               │
    │             │   output gen)   │               │
    │             └────────┬────────┘               │
    │                      │                         │
    └──────────────────────┴─────────────────────────┘
                     (loop if incomplete)
```

**Key LangGraph features used:**
- **State checkpointing** — durable execution; resume after failures
- **Streaming** — token-by-token output delivered to frontend via SSE
- **Conditional edges** — routing to planner only in pro/ultra modes
- **Subgraph** — each sub-agent is its own graph with isolated state

---

## Sandbox Architecture

DeerFlow has three sandbox modes, selected in `config.yaml`:

### Mode 1: Local Execution
```yaml
sandbox:
  use: deerflow.community.local_sandbox:LocalSandboxProvider
```
Runs code directly on the host machine. Fastest, least isolated. Development/testing only.

### Mode 2: Docker Execution
```yaml
sandbox:
  use: deerflow.community.docker_sandbox:DockerSandboxProvider
```
Each task session spins up a Docker container with a mounted filesystem:

```
/mnt/skills/public/          ← Built-in skill Markdown files
├── research/SKILL.md
├── report-generation/SKILL.md
├── slide-creation/SKILL.md
├── web-page/SKILL.md
└── image-generation/SKILL.md

/mnt/skills/custom/          ← User-defined skill Markdown files

/mnt/user-data/
├── uploads/                 ← Files uploaded by the user
├── workspace/               ← Agent's working directory (read/write)
└── outputs/                 ← Final deliverables
```

**Container lifecycle**: Created at session start, destroyed at session end. State is persisted via the mounted `/mnt/user-data/` volume.

### Mode 3: Docker + Kubernetes (Provisioner)
```yaml
sandbox:
  use: deerflow.community.aio_sandbox:AioSandboxProvider
  provisioner_url: http://provisioner:8080
```
Runs sandbox containers as Kubernetes Pods via a provisioner service. Enables multi-tenant deployments with resource isolation and scheduling.

---

## Model Configuration

DeerFlow uses a `config.yaml`-based model registry rather than hardcoded providers. Any OpenAI-compatible endpoint works:

```yaml
models:
  - name: gpt-4
    display_name: GPT-4
    use: langchain_openai:ChatOpenAI
    model: gpt-4
    api_key: $OPENAI_API_KEY
    max_tokens: 4096
    temperature: 0.7

  - name: openrouter-gemini
    display_name: Gemini 2.5 Flash (OpenRouter)
    use: langchain_openai:ChatOpenAI
    model: google/gemini-2.5-flash-preview
    api_key: $OPENROUTER_API_KEY
    base_url: https://openrouter.ai/api/v1
```

**CLI-backed providers** (v2 addition): DeerFlow can route through CLI tools rather than HTTP APIs:

```yaml
models:
  - name: claude-sonnet-4.6
    display_name: Claude Sonnet 4.6 (Claude Code OAuth)
    use: deerflow.models.claude_provider:ClaudeChatModel
    model: claude-sonnet-4-6
    supports_thinking: true
```

This lets DeerFlow use Claude Code's OAuth credentials or Codex CLI's `~/.codex/auth.json` — enabling LLM access without separate API key management.

**Recommended models** (ByteDance-promoted):
- Doubao-Seed-2.0-Code
- DeepSeek v3.2
- Kimi 2.5

---

## IM Channel Architecture

DeerFlow's Gateway process handles IM channels. Each channel connects without requiring a public IP:

| Channel | Transport | Auth |
|---------|-----------|------|
| Telegram | Bot API (long-polling) | Bot token |
| Slack | Socket Mode | Bot token + App token |
| Feishu / Lark | WebSocket (Long Connection) | App ID + App Secret |

Channels auto-start when configured. They route messages to the LangGraph Server and stream responses back.

Per-channel and per-user session configuration allows different agent personas, execution modes, and recursion limits for different users/channels.

---

## Deployment Options

### Docker Compose (Recommended)
```
make docker-init   # Pull sandbox image
make docker-start  # Start all services (auto-detects sandbox mode)
make up            # Production: build images + start
make down          # Stop and remove
```

### Local Development
```
make check         # Verify Node.js 22+, pnpm, uv, nginx
make install       # Install backend + frontend deps
make dev           # Start all services
```

Access at http://localhost:2026 in both cases.

---

## Configuration File Structure

```
deer-flow/
├── config.yaml          ← Primary config (models, sandbox, channels)
├── .env                 ← API keys (not committed)
├── backend/
│   ├── pyproject.toml
│   └── src/deerflow/
│       ├── agents/      ← LangGraph agent graphs
│       ├── models/      ← Provider implementations
│       ├── community/   ← Sandbox providers
│       └── skills/      ← Skill loading logic
├── frontend/
│   ├── package.json
│   └── src/
└── skills/
    └── public/          ← Built-in SKILL.md files
        ├── research/
        ├── report-generation/
        └── claude-to-deerflow/
```
