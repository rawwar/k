# SageAgent — Architecture

## Multi-Agent Pipeline

> **2026 Q2 note**: A `TaskRouterAgent` was briefly inserted between `TaskAnalysisAgent`
> and `PlanningAgent` in earlier 2026 releases but was removed on 2026-04-24
> (`c25a7fa` "remove TaskRouterAgent and simplify default flow"), restoring the canonical
> 5-agent pipeline described below.

SageAgent's core architecture is a sequential pipeline of five specialized agents, coordinated
by an `AgentController` that serves as the main entry point for user requests.

```
AgentController
  ├── TaskAnalysisAgent   — requirement understanding
  ├── PlanningAgent       — subtask decomposition
  ├── ExecutorAgent       — tool-assisted execution
  ├── ObservationAgent    — progress monitoring + feedback
  └── TaskSummaryAgent    — output generation
```

Each agent inherits from `AgentBase`, which provides common infrastructure for LLM interaction,
message handling, and state management.

## Directory Layout

```
agents/
    agent/
        agent_controller.py      # Entry point, orchestrates pipeline
        agent_base.py            # Base class for all agents
        task_analysis_agent/     # Requirement analysis
        planning_agent/          # Task decomposition and planning
        executor_agent/          # Execution with tool access
        observation_agent/       # Progress evaluation + feedback loop
        task_summary_agent/      # Final summary generation
    tool/
        tool_base.py             # Tool interface
        tool_manager.py          # Registration, discovery, dispatch
        calculation_tool.py      # Built-in calculation tool
    professional_agents/         # Placeholder for domain-specific agents
    task/                        # Task data structures
    utils/                       # Shared utilities
examples/
    sage_demo.py                 # Streamlit web demo
mcp_servers/
    mcp_setting.json             # MCP server configuration
```

## AgentBase

All agents extend `AgentBase`, which likely provides:
- LLM client initialization and prompt management
- Structured message input/output (normal + thinking types)
- Common logging and error handling

> Note: Specific `AgentBase` internals not deeply reviewed (Tier 3 analysis).

## AgentController

The `AgentController` is the user-facing entry point. It:
1. Accepts user input
2. Routes through the pipeline in sequence
3. Manages the feedback loop between ObservationAgent and PlanningAgent
4. Returns the final summary to the user

## Two Execution Modes

SageAgent supports dual execution modes selectable at invocation time:

### Deep Research Mode
- Full multi-agent pipeline engagement
- All five agents participate
- Feedback loop enabled — ObservationAgent can send incomplete tasks back to PlanningAgent
- Suitable for complex, multi-step research and analysis tasks

### Rapid Execution Mode
- Lightweight, fast task completion
- Likely bypasses or simplifies some pipeline stages
- Designed for straightforward tasks that don't need iterative refinement
- Reduced latency at the cost of less thorough analysis

The mode selection mechanism is configured at the controller level, though the exact
toggle API is not deeply documented in the public README.

## Professional Agents (Planned)

The `professional_agents/` directory suggests a plugin system for domain-specialized agents.
This is listed on the roadmap but appears to be in early stages. The concept would allow
specialized agents (e.g., for code review, data analysis) to be plugged into the pipeline.

A **self-check agent** was added on 2026-04-29 (`260bd1b`) — an early example of this
extensibility pattern in practice, alongside passthrough sandbox config and a chat
markdown workspace.

## Clients (2026 Q2)

Two first-party UIs now ship with the framework:

- **Streamlit web demo** (`sage_demo.py`) — original UI from initial release.
- **Sage Terminal (TUI)** — new terminal client added across 2026-04-25 → 2026-04-30
  (`add29b9`, `4232168`, `2713790`, `58f74b1`). Driven by the same `AgentController`
  runtime; adds composer/overlay polish and a packaged runtime distribution.
- **Desktop bundles** — Windows NSIS-only installer and macOS desktop builds landed
  2026-04-26 → 2026-04-27 (`867c977`, `d50bdf3`, `5f10f04`).

---

*Tier 3 analysis — architecture inferred primarily from README and directory structure.*