---
title: OpenHands Architecture Analysis
status: complete
---

# OpenHands

> Open-source AI-powered software development platform by **All Hands AI**; formerly OpenDevin.

|                |                                                                 |
| -------------- | --------------------------------------------------------------- |
| **Repository** | https://github.com/All-Hands-AI/OpenHands (54 k+ ★)            |
| **License**    | MIT (except `enterprise/` directory)                            |
| **Founded by** | Xingyao Wang, Graham Neubig, and collaborators                  |
| **Papers**     | CodeAct (arXiv 2402.01030) · Tech Report (arXiv 2511.03690)    |
| **Docs**       | https://docs.all-hands.dev                                      |

---

## Overview

OpenHands is a fully autonomous coding agent that can write code, fix bugs, and
handle end-to-end software engineering tasks. It grew out of the academic
OpenDevin project and was rebranded once it moved toward production use. The
core insight — articulated in the **CodeAct** paper — is that consolidating
every tool into a unified *code action space* (bash + IPython) dramatically
outperforms JSON-based function-calling schemes for complex, multi-step
engineering work.

The project occupies a unique position: it is simultaneously a **research
baseline** (widely cited, used in SWE-bench evaluations) and a **commercial
product** (hosted cloud, enterprise Kubernetes deployment). This dual life
shapes the codebase — it is more modular and more heavily abstracted than most
agent projects, but also carries the complexity that comes with supporting five
distinct product surfaces from one engine.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        AgentController                           │
│  ┌────────────┐  ┌──────────────┐  ┌─────────────────────────┐  │
│  │ StuckDetect│  │ StateTracker │  │ ReplayManager           │  │
│  └────────────┘  └──────────────┘  └─────────────────────────┘  │
│         │                │                     │                 │
│         ▼                ▼                     ▼                 │
│  ┌─────────────────────────────────────────────────────┐        │
│  │                   EventStream                        │        │
│  │   (central pub/sub bus — all components subscribe)   │        │
│  └──────────┬──────────────────────────────┬────────────┘        │
│             │                              │                     │
│      ┌──────▼───────┐              ┌───────▼────────┐           │
│      │   Actions     │              │  Observations   │           │
│      │ CmdRunAction  │              │ CmdOutputObs    │           │
│      │ IPythonRun    │              │ FileReadObs     │           │
│      │ FileRead/Write│              │ BrowserOutputObs│           │
│      │ FileEdit      │              │ ErrorObs        │           │
│      │ BrowseAction  │              │ ...             │           │
│      │ MCPAction     │              └─────────────────┘           │
│      │ AgentDelegate │                                           │
│      │ AgentFinish   │                                           │
│      └──────┬────────┘                                           │
└─────────────┼────────────────────────────────────────────────────┘
              │  REST API
              ▼
┌─────────────────────────────────┐
│     Docker Sandbox Container    │
│  ┌───────────────────────────┐  │
│  │  action_execution_server  │  │
│  │  (bash, IPython, browser) │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
```

### Event-Driven Core

Everything flows through the **EventStream** — a publish/subscribe bus
implemented in `openhands/events/`. The agent emits **Actions**; the runtime
executes them inside the sandbox and returns **Observations**. The controller
watches this stream, checking for stuck loops and managing state transitions.

Key action types:

| Action                    | Purpose                                      |
| ------------------------- | -------------------------------------------- |
| `CmdRunAction`            | Execute a shell command in the sandbox        |
| `IPythonRunCellAction`    | Run Python code in an IPython kernel          |
| `FileReadAction`          | Read a file from the workspace                |
| `FileWriteAction`         | Write/overwrite a file                        |
| `FileEditAction`          | Apply a surgical edit (search/replace style)  |
| `BrowseInteractiveAction` | Control a headless browser                    |
| `MCPAction`               | Call an MCP-protocol tool server              |
| `AgentDelegateAction`     | Spawn a sub-agent for a subtask               |
| `AgentFinishAction`       | Signal task completion                        |

Each action produces a corresponding observation (`CmdOutputObservation`,
`FileReadObservation`, `BrowserOutputObservation`, `ErrorObservation`, etc.).

### Sandboxed Runtime

Code execution happens inside a **Docker container** that runs an
`action_execution_server` — an HTTP server that accepts action requests and
returns observation responses. The host communicates with it over a REST API on
a mapped port. Alternative runtimes exist (local, remote, Modal) but Docker is
the default and most battle-tested.

The sandbox provides:
- Full Linux environment with apt/pip
- Persistent filesystem across the task
- Headless Chromium for browser actions
- IPython kernel for Python execution
- Network isolation (configurable)

### Memory & Context Management

The `ConversationMemory` module (`openhands/memory/`) manages what the agent
sees in its context window. Because long tasks can produce thousands of events,
a **Condenser** system compresses history using multiple strategies:

- **Recent-events condenser** — keep only the last N events
- **LLM-summary condenser** — ask the LLM to summarize older history
- **Structured condenser** — preserve action/observation pairs, drop filler
- **Hybrid** — combine strategies with configurable thresholds

This is one of the more sophisticated context management systems in the
open-source agent space.

### Microagents

Microagents are a knowledge-injection mechanism. They are markdown files that
get prepended to the agent's prompt based on triggers:

- **Repo microagents** — `.openhands/microagents/` in the target repo; activated
  by keyword matching against the user's task description
- **Global microagents** — shipped with OpenHands; general-purpose knowledge
  (e.g., "how to use git", "debugging tips")
- **Task microagents** — specialized sub-agents that can be delegated to

This system is loosely analogous to Cursor Rules or AGENTS.md files but with
keyword-based activation rather than always-on injection.

---

## Product Ecosystem

OpenHands ships five product surfaces from a single engine:

| Surface              | Description                                             | Analogue        |
| -------------------- | ------------------------------------------------------- | --------------- |
| **Software Agent SDK** | Composable Python library (`pip install openhands-ai`)  | —               |
| **CLI**              | Terminal agent; works with any LiteLLM-supported model   | Claude Code, Codex |
| **Local GUI**        | REST API + React SPA; run locally via Docker Compose     | Devin, Jules    |
| **Cloud**            | Hosted at `app.all-hands.dev`; GitHub/GitLab/Jira/Slack | Devin Cloud     |
| **Enterprise**       | Self-hosted via Kubernetes in customer VPC               | Devin Enterprise|

The SDK is the foundational layer — everything else is a thin wrapper around it.
The CLI was added more recently (mid-2025) and positions OpenHands as a direct
competitor to Claude Code and OpenAI Codex CLI, with the advantage of being
model-agnostic via LiteLLM.

---

## Source Code Structure

The main logic lives under `openhands/`:

```
openhands/
├── agenthub/           # Agent implementations
│   ├── codeact_agent/  #   CodeActAgent — primary agent, unified code actions
│   ├── browsing_agent/ #   BrowsingAgent — web-browsing specialist
│   ├── readonly_agent/ #   ReadonlyAgent — read-only analysis agent
│   └── loc_agent/      #   LocAgent — localization/search agent
├── controller/         # AgentController, Agent base class
│   ├── agent_controller.py
│   ├── stuck.py        #   StuckDetector — breaks infinite loops
│   └── state/          #   StateTracker, ReplayManager
├── events/             # Event system
│   ├── event.py        #   Base Event class
│   ├── stream.py       #   EventStream pub/sub bus
│   ├── action/         #   All Action types
│   ├── observation/    #   All Observation types
│   └── serialization/  #   JSON serialization for events
├── memory/             # Context management
│   ├── conversation_memory.py
│   └── condenser/      #   Multiple condensing strategies
├── microagent/         # Knowledge injection system
├── runtime/            # Execution environments
│   ├── docker/         #   Docker sandbox (default)
│   ├── local/          #   Local execution (no isolation)
│   ├── remote/         #   Remote runtime client
│   ├── modal/          #   Modal.com runtime
│   ├── action_execution_server.py  # Runs inside the container
│   └── browser/        #   Headless Chromium integration
├── llm/                # LLM integration via LiteLLM
│   ├── llm.py          #   Main LLM wrapper
│   └── retry.py        #   Retry logic, rate limiting
├── server/             # Web server (FastAPI) for GUI
│   ├── routes/         #   REST API endpoints
│   └── session/        #   Session management
└── core/               # Shared infrastructure
    ├── config.py       #   Configuration system
    ├── schema.py       #   Enums, constants
    ├── logger.py       #   Logging
    └── exceptions.py   #   Exception hierarchy
```

### CodeActAgent — The Primary Agent

`CodeActAgent` is the default and most capable agent. Its design follows the
CodeAct paper's thesis: rather than exposing tools as JSON-schema functions, give
the agent a bash shell and an IPython kernel and let it *write code* to
accomplish tasks. This means:

- File edits are done by writing sed/awk commands or using the built-in
  `FileEditAction` (which itself is a search/replace primitive)
- Web browsing is done through a Python API to a headless browser
- Tool use is just… writing code that calls the tool

The agent also supports `AgentDelegateAction` to spawn sub-agents (e.g.,
delegating a browsing task to `BrowsingAgent`).

---

## Benchmarks

| Benchmark              | Score / Rank                                    |
| ---------------------- | ----------------------------------------------- |
| **SWE-bench Verified** | **77.6%** (shown in their repo badge)           |
| **Terminal-Bench 2.0** | Rank #49 (Claude Opus 4.5, 51.9%)               |
| **Terminal-Bench 2.0** | Rank #58 (GPT-5, 43.8%)                         |

The SWE-bench score is competitive with top proprietary agents. However, it is
worth noting that SWE-bench performance depends heavily on the underlying LLM —
OpenHands acts as scaffolding, so the score reflects the combined system (agent
+ model). The 77.6% figure likely uses a frontier model (Claude Sonnet 4 or
similar).

Terminal-Bench results are more modest, which is consistent with terminal-heavy
benchmarks testing a different skill distribution (system administration,
debugging, configuration) versus SWE-bench's focus on code patches.

---

## Comparison with Other Agents

| Dimension               | OpenHands              | Claude Code          | Codex CLI            | Devin              |
| ----------------------- | ---------------------- | -------------------- | -------------------- | ------------------- |
| **Open source**         | ✅ MIT                 | ❌ Proprietary        | ✅ Apache 2.0        | ❌ Proprietary      |
| **Model-agnostic**      | ✅ Via LiteLLM         | ❌ Claude only        | ❌ OpenAI only       | ❌ Proprietary      |
| **Sandbox**             | Docker container       | macOS seatbelt       | Docker/Firecracker   | Cloud VM            |
| **Browser actions**     | ✅ Headless Chromium   | ❌                    | ❌                   | ✅                  |
| **Sub-agent delegation**| ✅ AgentDelegate       | ❌                    | ❌                   | ✅                  |
| **Context management**  | Condenser system       | Conversation summary | Basic truncation     | Proprietary         |
| **MCP support**         | ✅                     | ✅                    | ✅                   | ✅                  |
| **GUI**                 | ✅ Web UI              | ❌ Terminal only      | ❌ Terminal only     | ✅ Web UI           |
| **Self-hostable**       | ✅                     | N/A                  | ✅                   | Enterprise only     |

**Strengths**: Model-agnostic, full-featured (browser + terminal + file edit),
strong research pedigree, multiple deployment modes, active community.

**Weaknesses**: Docker dependency adds setup friction compared to Claude Code's
zero-install experience. The abstraction layers (five product surfaces, multiple
runtimes, event serialization) add complexity. Terminal-Bench scores suggest the
agent scaffolding may not be as effective for non-SWE-bench-style tasks.

---

## Interesting Design Patterns

### Action/Observation Abstraction
The strict separation of Actions (agent intent) from Observations (environment
feedback) creates a clean, serializable record of every agent step. This enables
replay, debugging, and evaluation — you can re-run any task from its event
stream without re-executing against the LLM.

### StuckDetector
The controller includes a `StuckDetector` that monitors the event stream for
repeated action patterns (e.g., the agent running the same command in a loop).
When detected, it injects a nudge observation to break the cycle. This is a
practical solution to a common failure mode in long-running agent tasks.

### Unified Code Action Space
Rather than defining a fixed tool schema, CodeActAgent lets the LLM write
arbitrary bash/Python to accomplish goals. This is more flexible than
function-calling but requires stronger models — weaker models may produce
unsafe or incorrect commands. The tradeoff is central to the CodeAct thesis.

### Runtime Abstraction
The runtime layer is abstracted behind a common interface, allowing the same
agent to run in Docker locally, on a remote server, or on Modal.com. This
separation of "what the agent does" from "where it runs" is well-executed and
enables the multi-product strategy.

---

## Maturity & Status

- **Production-ready for cloud/enterprise use** — the hosted platform at
  `app.all-hands.dev` is actively used, with GitHub/GitLab integrations
- **Active development** — high commit velocity, frequent releases
- **Large community** — 54k+ GitHub stars, active Discord, regular contributors
- **Research lineage** — multiple published papers, used as baseline in academic
  evaluations
- **CLI is newer** — the terminal experience is less polished than Claude Code
  but rapidly improving

The project's main risk is complexity: supporting five product surfaces from one
codebase creates a large surface area for bugs and makes contribution harder for
newcomers. The enterprise/cloud layers also introduce licensing ambiguity (the
`enterprise/` directory is excluded from MIT).

---

## References

- Repository: https://github.com/All-Hands-AI/OpenHands
- Documentation: https://docs.all-hands.dev
- CodeAct Paper: https://arxiv.org/abs/2402.01030
- Tech Report: https://arxiv.org/abs/2511.03690
- Cloud Platform: https://app.all-hands.dev
