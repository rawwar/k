# OpenHands — References & Resources

> Comprehensive link collection for the OpenHands (formerly OpenDevin) AI software
> engineering agent. Organized by category for quick lookup.

---

## Primary Sources

| Resource | URL | Description |
|----------|-----|-------------|
| GitHub Repository | <https://github.com/All-Hands-AI/OpenHands> | Main source code (MIT license) |
| Documentation | <https://docs.all-hands.dev> | Official docs site (redirects to docs.openhands.dev) |
| SDK Documentation | <https://docs.openhands.dev/sdk> | Software Agent SDK reference |
| Cloud Platform | <https://app.all-hands.dev> | Hosted SaaS offering |
| Website | <https://openhands.dev> | Marketing / landing page |

---

## Related Repositories

| Repository | URL | Description |
|------------|-----|-------------|
| Software Agent SDK (V1) | <https://github.com/OpenHands/software-agent-sdk> | Standalone SDK for building agents on the OpenHands runtime |
| OpenHands CLI | <https://github.com/OpenHands/OpenHands-CLI> | Command-line interface package (`openhands-cli`) |
| Benchmarks | <https://github.com/OpenHands/benchmarks> | Evaluation harnesses (SWE-bench, HumanEval, etc.) |
| Chrome Extension | <https://github.com/OpenHands/openhands-chrome-extension> | Browser extension for triggering agents from GitHub issues |
| Theory-of-Mind (ToM-SWE) | <https://github.com/OpenHands/ToM-SWE> | Research module for theory-of-mind in SWE tasks |
| Documentation Site | <https://github.com/OpenHands/docs> | Source for <https://docs.openhands.dev> |

---

## Academic Papers

### CodeAct — Executable Code Actions for LLM Agents

- **Authors:** Xingyao Wang, Yangyi Chen, Lifan Yuan, Yizhe Zhang, Yunzhu Li,
  Hao Peng, Heng Ji
- **Link:** <https://arxiv.org/abs/2402.01030>
- **Summary:** Introduces the CodeAct paradigm — consolidating LLM tool use
  into executable Python code actions instead of rigid JSON schemas. Foundation
  of the `CodeActAgent`.

### OpenHands Technical Report

- **Link:** <https://arxiv.org/abs/2511.03690>
- **Summary:** System-level description of the OpenHands platform covering
  architecture, sandboxed runtime, event stream, memory management, and
  benchmark results.

### Citation File

- `CITATION.cff` is available in the repository root for BibTeX / CFF-based
  citation tooling.

---

## Documentation Links

| Page | URL | Notes |
|------|-----|-------|
| Runtime Architecture | <https://docs.all-hands.dev/openhands/usage/architecture/runtime> | Docker sandbox, action-execution server, event flow |
| CLI Mode | <https://docs.openhands.dev/openhands/usage/run-openhands/cli-mode> | Running OpenHands from the terminal |
| Local Setup | <https://docs.openhands.dev/openhands/usage/run-openhands/local-setup> | Development environment setup guide |
| Enterprise | <https://openhands.dev/enterprise> | Enterprise deployment & licensing information |
| Microagents | <https://docs.openhands.dev/openhands/usage/architecture/microagents> | Repo-level and knowledge micro-agents |
| Configuration | <https://docs.openhands.dev/openhands/usage/configuration> | TOML config reference |

---

## Community & Social

| Channel | URL | Description |
|---------|-----|-------------|
| Slack Workspace | <https://openhands.dev/joinslack> (short: <https://dub.sh/openhands>) | Primary community chat |
| Product Roadmap | <https://github.com/orgs/openhands/projects/1> | GitHub Projects board tracking features & milestones |
| Community Guide | `COMMUNITY.md` in repo root | Contribution guidelines, governance, code of conduct |
| Discord / X | Linked from <https://openhands.dev> | Secondary social channels |

---

## Benchmark Results

### SWE-bench

- **Results Spreadsheet:**
  <https://docs.google.com/spreadsheets/d/1wOUdFCMyY6Nt0AIqF705KN4JKOWgeI4wUGUP60krXXs>
- OpenHands regularly publishes verified SWE-bench Lite and Full scores.
  Competitive with top proprietary agents.

### Terminal-Bench 2.0

| Rank | Model | Score |
|------|-------|-------|
| #49 | Claude Opus 4.5 | 51.9 % |
| #58 | GPT-5 | 43.8 % |

> Terminal-Bench evaluates autonomous terminal task completion. Ranks above
> reflect the OpenHands agent paired with the listed backbone model.

---

## Key Source Code Files

Useful starting points for architecture analysis of the
`All-Hands-AI/OpenHands` repository.

### Event System

| File | Purpose |
|------|---------|
| `openhands/events/stream.py` | Central event bus — append, subscribe, filter |
| `openhands/events/event.py` | Base `Event` dataclass (id, timestamp, source, cause) |

### Actions (`openhands/events/action/`)

| File | Purpose |
|------|---------|
| `commands.py` | `CmdRunAction`, `IPythonRunCellAction` |
| `files.py` | `FileReadAction`, `FileWriteAction`, `FileEditAction` |
| `agent.py` | `AgentFinishAction`, `AgentDelegateAction`, `AgentThinkAction` |
| `browse.py` | `BrowseURLAction`, `BrowseInteractiveAction` |
| `mcp.py` | MCP (Model Context Protocol) tool-call actions |

### Observations (`openhands/events/observation/`)

| File | Purpose |
|------|---------|
| `commands.py` | `CmdOutputObservation` — shell command results |
| `files.py` | `FileReadObservation`, `FileWriteObservation`, `FileEditObservation` |
| `browse.py` | `BrowserOutputObservation` — browser state snapshots |
| `error.py` | `ErrorObservation` — runtime / agent errors |
| `mcp.py` | MCP tool-call responses |

### Agent & Controller

| File | Purpose |
|------|---------|
| `openhands/controller/agent_controller.py` | Main control loop — step, budget, delegate, stuck detection |
| `openhands/controller/agent.py` | Abstract `Agent` base class |
| `openhands/controller/stuck.py` | `StuckDetector` — identifies and breaks infinite loops |
| `openhands/agenthub/codeact_agent/codeact_agent.py` | Default `CodeActAgent` implementation |
| `openhands/agenthub/codeact_agent/function_calling.py` | Maps actions ↔ LLM function-call schemas |

### Runtime

| File | Purpose |
|------|---------|
| `openhands/runtime/base.py` | Abstract `Runtime` — execute actions, manage sandbox |
| `openhands/runtime/action_execution_server.py` | FastAPI server running *inside* the Docker sandbox |

### Memory & Context Management

| File | Purpose |
|------|---------|
| `openhands/memory/memory.py` | High-level memory orchestrator |
| `openhands/memory/conversation_memory.py` | Builds LLM message lists from event history |
| `openhands/memory/condenser/condenser.py` | Base condenser interface for context compression |
| `openhands/memory/condenser/impl/` | Concrete strategies: `LLMSummarizingCondenser`, `RecentEventsCondenser`, `AmortizedForgettingCondenser`, etc. |

### Microagents

| File | Purpose |
|------|---------|
| `openhands/microagent/microagent.py` | Loads repo-level `.openhands/microagents/*.md` prompt snippets |

---

## Configuration Files

| File | Purpose |
|------|---------|
| `config.template.toml` | Annotated configuration template (LLM, agent, sandbox settings) |
| `docker-compose.yml` | Multi-container setup for local development |
| `pyproject.toml` | Python project metadata, dependencies, entry points |
| `.openhands/microagents/` | Directory for repo-specific microagent prompts |

---

## Version & Licensing

| Item | Detail |
|------|--------|
| Current Version | **v1.0.0+** (V1 SDK migration in progress) |
| Legacy V0 Deprecation | V0 code paths marked for removal **April 1, 2026** |
| Core License | MIT (`LICENSE` in repo root) |
| Enterprise License | Separate license under `enterprise/` directory |
| Python Requirement | ≥ 3.12 |
| Container Runtime | Docker (required for sandboxed execution) |

---

## Quick-Start Commands

```bash
# Install via pip
pip install openhands-ai

# Run in headless / CLI mode
openhands --mode cli --model anthropic/claude-sonnet-4-20250514

# Run via Docker (recommended)
docker run -it \
  -e LLM_API_KEY=$OPENAI_API_KEY \
  -p 3000:3000 \
  ghcr.io/all-hands-ai/openhands:latest

# Development setup
git clone https://github.com/All-Hands-AI/OpenHands.git
cd OpenHands
make build        # builds Docker sandbox image
make run          # starts the full stack
```

---

## See Also

- [overview.md](./overview.md) — Architecture deep-dive and design analysis
- [key-innovations.md](./key-innovations.md) — What makes OpenHands unique
- [comparison.md](../comparison.md) — Side-by-side with other AI coding agents

---

*Last updated: 2025-07-17*