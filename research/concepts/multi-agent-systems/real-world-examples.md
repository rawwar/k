---
title: Real-World Multi-Agent Implementations
status: complete
---

# Real-World Multi-Agent Implementations

This document provides deep dives into how production coding agents and research
frameworks implement multi-agent architectures. We move from CLI coding agents
(our primary research focus) to broader multi-agent frameworks (MetaGPT, ChatDev,
AutoGen, CrewAI) that have influenced the field. Each section covers architecture,
communication patterns, and key innovations — with specific code examples and
architectural diagrams drawn from our research.

---

## CLI Coding Agents

### Claude Code: Sub-Agent Task Tool

Claude Code implements multi-agent through its **Task tool**, which spawns sub-agents
in isolated context windows. The main agent acts as orchestrator, delegating exploration,
planning, and implementation to workers.

```mermaid
flowchart TD
    MA["Main Agent (Sonnet/Opus, full tools)\nUser: Refactor auth to use JWT\n→ Spawns explore sub-agent\n→ Gets summary of auth module\n→ Spawns another explore for test patterns\n→ Plans the refactor\n→ Implements changes directly\n→ Runs tests"]
    E1["Explore (Haiku)\nRead-only\nFast, cheap"]
    E2["Explore (Haiku)\nRead-only\nFast, cheap"]
    PL["Plan (Parent model)\nRead-only\nAnalysis only"]
    GP["General-purpose (Parent model)\nAll tools\nFull capability"]
    MA --> E1
    MA --> E2
    MA --> PL
    MA --> GP
```

**Key architectural decisions:**

- **No nesting:** Sub-agents cannot spawn other sub-agents. This keeps the hierarchy flat.
- **Context isolation:** Each sub-agent gets its own context window. Only the summary returns.
- **Model tiering:** Explore agents use Haiku (fast, cheap) for research tasks.
- **Custom agents:** Teams define custom sub-agents as markdown files in `.claude/agents/`.
- **Worktree isolation:** `isolation: worktree` gives sub-agents their own git worktree.

**Communication protocol:** Standard Anthropic Messages API tool-use. The Agent tool is
invoked identically to any other tool — no special plumbing required.

### Codex CLI: Resource-Controlled Parallel Agents

Codex CLI (by OpenAI) implements multi-agent with a Rust-based resource management
layer that enforces concurrency limits and agent lifecycle management.

```mermaid
flowchart TD
    AC["AgentControl"]
    GD["Guards (atomic resource management)\nactive_agents: Mutex\ntotal_count: AtomicUsize\nSpawnReservation (RAII cleanup)"]
    EX["Explorer\n(fast model)"]
    WA["Worker\n(file owner A)"]
    WB["Worker\n(file owner B)"]
    SQ["SQ/EQ Message Passing\nSubmission Queue → Event Queue\n(TUI, exec, MCP frontends)"]
    AC --> GD
    AC --> EX
    AC --> WA
    AC --> WB
    AC --> SQ
```

**Key innovations:**

- **CAS-based max enforcement:** Atomic compare-and-swap prevents exceeding agent limits.
- **SpawnReservation with Drop:** Rust's ownership system ensures cleanup if spawn fails.
- **Role system:** `default`, `explorer`, `worker` with different capabilities.
- **File ownership:** Workers can be assigned ownership of specific files.
- **JSONL rollout persistence:** Sub-agent sessions can be resumed from JSONL files.
- **Parallel tool execution:** `futures::future::join_all` for concurrent tool calls.

### ForgeCode: Three-Agent Bounded Context

ForgeCode achieves the highest Terminal-Bench score (81.8%) with a three-agent
model built on bounded context and enforced verification.

```mermaid
flowchart TD
    RT["ForgeCode Services Runtime"]
    CE["Context Engine\nsem_search — 93% fewer tokens"]
    DSL["Dynamic Skill Loading"]
    TCC["Tool-Call Correction Layer"]
    TE["Todo Enforcement\n(mandatory for multi-step)"]
    PRB["Progressive Reasoning Budget\nMsgs 1-10: very high | 11+: low | verify: high"]
    FORGE["FORGE (R+W)\n:forge — Implement"]
    MUSE["MUSE (R only)\n:muse — Plan"]
    SAGE["SAGE (R only)\ninternal only — Research"]
    VER["Verification Skill (ENFORCED)\nRuntime blocks completion until verification pass"]
    RT --> CE
    RT --> DSL
    RT --> TCC
    RT --> TE
    RT --> PRB
    RT --> FORGE
    RT --> MUSE
    RT --> SAGE
    RT --> VER
```

**Key innovations:**

- **Bounded context:** Each agent boundary is a compression point. Raw data never
  crosses boundaries — only summaries.
- **Enforced verification:** The runtime architecturally prevents task completion
  without verification. "Biggest single improvement."
- **Tool-call correction:** Heuristic + static analysis layer auto-corrects
  malformed tool calls. Makes any model more reliable.
- **Progressive reasoning:** Budget varies by conversation stage, not agent type.
- **ZSH-native:** `:forge` and `:muse` commands integrate directly into the shell.

### OpenHands: Event-Sourced Micro-Agents

OpenHands uses an event-sourced architecture where all agent actions are recorded
as events on a shared EventStream.

```mermaid
flowchart TD
    ES["EventStream\nEvent 1: CmdRunAction\nEvent 2: CmdOutputObservation\nEvent 3: FileWriteAction\nEvent 4: FileWriteObservation\nEvent 5: AgentDelegateAction\nEvent 6: AgentDelegateObservation"]
    AC["AGENT_CONTROLLER"]
    RES["RESOLVER"]
    SRV["SERVER"]
    RUN["RUNTIME"]
    MEM["MEMORY"]
    TST["TEST"]
    ES -->|"subscriber"| AC
    ES -->|"subscriber"| RES
    ES -->|"subscriber"| SRV
    ES -->|"subscriber"| RUN
    ES -->|"subscriber"| MEM
    ES -->|"subscriber"| TST
```

```mermaid
flowchart TD
    MS["Microagent System"]
    RM["RepoMicroagent\nAlways active\nfrom .openhands/microagents/\nAlso reads: .cursorrules, AGENTS.md"]
    KM["KnowledgeMicroagent\nKEYWORD-TRIGGERED\ne.g. 'django' → Django expertise injected\nLightweight RAG without embeddings"]
    TM["TaskMicroagent\n/command-triggered workflows"]
    MS --> RM
    MS --> KM
    MS --> TM
```

**Key innovations:**

- **Event sourcing:** Full replay capability, crash recovery, audit trails.
- **AgentDelegateAction:** Sub-agent delegation through the event system.
- **Keyword-triggered microagents:** "django" in conversation → Django expertise
  automatically injected. Lightweight RAG without vector databases.
- **Docker sandbox:** HTTP-based split — host runs agent, Docker container runs code.
- **StuckDetector:** Monitors for agents caught in loops and intervenes.
- **Action/Observation symmetry:** Every event is typed with causal linking.

### Capy: Captain/Build Hard Split

Capy separates planning and execution with enforced capability boundaries:

```mermaid
flowchart LR
    CAP["CAPTAIN\nCan: Read codebase, Research docs,\nAsk user questions, Write specs\nCannot: Write code, Run commands,\nPush commits"]
    BUILD["BUILD\nCan: Edit files, Run commands,\nInstall deps, Open PRs\nCannot: Ask questions,\nInteract with user, Modify spec"]
    CAP -->|"spec"| BUILD
    NOTE["Parallel: Up to 25 concurrent jams\nEach jam: own VM + git worktree + lifecycle"]
    BUILD --- NOTE
```

**Key innovations:**

- **Forcing functions:** Constraints that make each agent better. Build can't ask
  questions → Captain must be thorough. Captain can't code → pure planning focus.
- **Spec document as interface:** The only communication channel between agents.
- **25 concurrent VMs:** Each task runs in its own sandboxed Ubuntu VM with its
  own git worktree.
- **Fire-and-forget execution:** Build runs asynchronously after Captain finishes.

### Goose: Agent-of-Agents via ACP

Goose's unique contribution is the **Agent Communication Protocol (ACP)**, which
allows using other coding agents as backend LLM providers:

```mermaid
flowchart TD
    GC["Goose Core\n(MCP-First Architecture)"]
    PE["Platform Extensions (7 types)\ndeveloper, analyze, summon,\ncomputercontroller, memory, ..."]
    ACP["ACP: Other agents as providers\nClaude Code ACP | Codex ACP | Gemini ACP"]
    MOIM["MOIM: Per-turn context injection\nfrom all extensions"]
    BTS["Background tool-pair summarization\n(proactive context management)"]
    GC --> PE
    GC --> ACP
    GC --> MOIM
    GC --> BTS
```

### SageAgent: Five-Stage Pipeline

SageAgent implements a sequential pipeline with a single feedback loop:

```mermaid
flowchart TD
    TA["Task Analysis Agent"]
    PL["Planning Agent"]
    EX["Executor Agent"]
    OB["Observation Agent"]
    SUM["Task Summary Agent"]
    TA --> PL --> EX --> OB
    OB -->|"feedback"| PL
    OB -->|"done"| SUM
```

Two modes: **Deep Research** (full pipeline, feedback enabled) and
**Rapid Execution** (simplified, no feedback loop).

### Junie CLI: Multi-Model Pipeline

Junie CLI's innovation is **dynamic model routing** across its three-stage pipeline:

```mermaid
flowchart TD
    MMR["Multi-Model Router\nComplex reasoning → Claude Sonnet/Opus\nFast edits → Gemini Flash\nCode generation → Best for language\nPlanning → Reasoning model"]
    PL["Planner"]
    EX["Executor"]
    VR["Verifier"]
    RES["Result: 71.0% multi-model vs 64.3% single-model\n= +6.7pp improvement from model routing"]
    MMR --> PL
    MMR --> EX
    MMR --> VR
    VR --> RES
```

### Ante: Lock-Free Meta-Agent

Ante uses a Meta-Agent that dynamically manages a pool of concurrent sub-agents
with **zero-mutex concurrency**:

```mermaid
flowchart TD
    MA["META-AGENT\nFully Rust-native | Single binary\nCloud + local inference"]
    LS["Lock-Free Scheduler\n(atomic ops, wait-free queues)"]
    S1["Sub-Agent 1"]
    S2["Sub-Agent 2"]
    SN["..."]
    MA --> LS
    LS --> S1
    LS --> S2
    LS --> SN
```

---

## Multi-Agent Frameworks

### MetaGPT: Software Company as Multi-Agent System

MetaGPT's core philosophy: `Code = SOP(Team)` — Standard Operating Procedures
applied to a team of LLM agents that mirrors a software company.

```mermaid
flowchart TD
    REQ["One line requirement"]
    PM["Product Manager\nRequirements & user stories"]
    AR["Architect\nSystem design, APIs, data structures"]
    PRJ["Project Manager\nTask breakdown, schedules"]
    ENG["Engineer(s)\nCode generation per module"]
    QA["QA Engineer\nTest generation"]
    OUT["Full software project\n(PRDs, system designs, task lists, code, tests)"]
    REQ --> PM --> AR --> PRJ --> ENG --> QA --> OUT
```

**Output:** User stories, competitive analysis, requirements, data structures,
APIs, documentation — the full software development pipeline.

### ChatDev: Virtual Software Company

ChatDev simulates a software company with agents in conversational roles:

```mermaid
flowchart TD
    D["Phase 1: DESIGN\nCEO ↔ CTO\n'What should we build?'"]
    C["Phase 2: CODING\nCTO ↔ Programmer\n'Here's the design, implement it'"]
    T["Phase 3: TESTING\nProgrammer ↔ Tester\n'Here's the code, find bugs'"]
    DC["Phase 4: DOCUMENTATION\nCTO ↔ Programmer\n'Write the docs'"]
    NOTE["Communication: Chat-based dialogue\nInnovation: Thought Instruction prompts"]
    D --> C --> T --> DC --> NOTE
```

**Key insight:** ChatDev's innovation is that agents communicate through
**natural language dialogue** rather than structured messages. This allows
for nuanced negotiation but introduces unpredictability.

### Microsoft AutoGen: Multi-Agent Conversation Framework

AutoGen provides a layered architecture for building multi-agent applications:

```mermaid
flowchart TD
    AG["AutoGen"]
    CORE["Core API\nMessage passing between agents\nLocal and distributed runtime\nCross-language (.NET + Python)"]
    CHAT["AgentChat API\nTwo-agent chat\nGroup chat (round-robin, selector)\nAgentTool (agents-as-tools)"]
    EXT["Extensions API\nLLM clients (OpenAI, Azure, etc.)\nCode execution\nMCP integration"]
    PAT["Multi-Agent Patterns\nAgentTool | Handoffs | GroupChat | Termination"]
    M1["Magentic-One\n(web browsing, code execution, file handling)"]
    AG --> CORE
    AG --> CHAT
    AG --> EXT
    AG --> PAT
    AG --> M1
```

**AutoGen's AgentTool pattern:**

```python
from autogen_agentchat.agents import AssistantAgent
from autogen_agentchat.tools import AgentTool

# Create specialist agents
code_expert = AssistantAgent(
    "code_expert", model_client=client,
    system_message="You are a coding expert.",
)

# Wrap as tool for orchestrator
code_tool = AgentTool(code_expert, return_value_as_last_message=True)

# Orchestrator uses specialists as tools
orchestrator = AssistantAgent(
    "orchestrator",
    tools=[code_tool],
    system_message="Use expert tools when needed.",
)
```

### CrewAI: Role-Based Multi-Agent Framework

CrewAI provides a role-based framework with two complementary concepts:
**Crews** (autonomous agent teams) and **Flows** (event-driven workflows).

```mermaid
flowchart TD
    CA["CrewAI"]
    CW["CREWS (autonomous agent teams)\nAgent: role + goal + backstory\nTask: description + expected output\nCrew: agents + tasks + process\nProcess: sequential | hierarchical | consensual"]
    FL["FLOWS (event-driven control)\nFine-grained execution paths\nState management between tasks\nConditional branching\nIntegration with Crews"]
    PROD["Combined: Flows orchestrate Crews\n(Production architecture)"]
    CA --> CW
    CA --> FL
    CW --> PROD
    FL --> PROD
```

**CrewAI coding example:**

```python
from crewai import Agent, Task, Crew, Process

researcher = Agent(
    role="Code Researcher",
    goal="Understand the existing codebase and identify patterns",
    backstory="You are a senior engineer who excels at code analysis.",
    tools=[grep_tool, read_file_tool],
)

developer = Agent(
    role="Software Developer",
    goal="Implement clean, tested code changes",
    backstory="You are a staff engineer who writes production-quality code.",
    tools=[edit_file_tool, run_tests_tool],
)

reviewer = Agent(
    role="Code Reviewer",
    goal="Ensure code quality, security, and correctness",
    backstory="You are a principal engineer focused on code quality.",
    tools=[read_file_tool, run_linter_tool],
)

# Define tasks
research_task = Task(
    description="Analyze the auth module and document current patterns",
    agent=researcher,
    expected_output="Summary of auth patterns and dependencies",
)

implement_task = Task(
    description="Implement JWT authentication based on research findings",
    agent=developer,
    expected_output="Code changes implementing JWT auth",
)

review_task = Task(
    description="Review the implementation for bugs and security issues",
    agent=reviewer,
    expected_output="Review report with issues and approval status",
)

# Create and run crew
crew = Crew(
    agents=[researcher, developer, reviewer],
    tasks=[research_task, implement_task, review_task],
    process=Process.sequential,
)

result = crew.kickoff()
```

---

## Devin: Cloud-Based Multi-Agent Architecture

Devin (by Cognition) operates as a cloud-based coding agent with a multi-agent
architecture that separates planning, execution, and verification:

```mermaid
flowchart TD
    PA["Planning Agent\nAnalyzes tasks, creates plans"]
    EA["Execution Agent(s)\nFull VM access: terminal, browser, editor, git"]
    VA["Verification Agent\nTests, reviews, validates"]
    KF["Key features:\n• Full cloud VM per task\n• Browser automation\n• Long-running autonomous execution\n• Slack/IDE integration\n• Session replay for debugging"]
    PA --> EA --> VA
    VA --> KF
```

Devin's multi-agent approach is notable for its **full cloud VM** per task, allowing
agents to use any tool — terminal, browser, editor — just as a human developer would.

---

## Cross-Implementation Comparison

| System | Agents | Communication | Parallelism | Verification | Context Strategy |
|--------|--------|---------------|-------------|-------------|-----------------|
| **Claude Code** | 3 built-in + custom | Tool-use protocol | Sub-agent isolation | Prompt-based | Summary handoff |
| **Codex CLI** | 3 roles | SQ/EQ messages | join_all parallel | Optional | Role-based filtering |
| **ForgeCode** | 3 (Forge/Muse/Sage) | Bounded context | Low-complexity parallel | **Enforced** | 93% compression |
| **OpenHands** | Main + delegates | EventStream pub/sub | ThreadPool subscribers | StuckDetector | Event replay |
| **Capy** | 2 (Captain/Build) | Spec document | 25 concurrent VMs | Tests in Build | Spec-as-interface |
| **Goose** | Summon + ACP | MCP everywhere | Via Summon | Recipe retry | MOIM per-turn |
| **SageAgent** | 5-stage pipeline | Linear + feedback | Sequential | ObservationAgent | Pipeline handoff |
| **Junie CLI** | 3-stage pipeline | Backend proxy | Multi-model | Test-driven loop | Model routing |
| **Ante** | Meta + pool | Lock-free scheduler | Concurrent sub-agents | Not documented | Atomic state |
| **MetaGPT** | 5 company roles | Structured artifacts | Per-module coding | QA Engineer | SOPs |
| **ChatDev** | 4 phases | Chat dialogue | Sequential phases | Tester phase | Conversation |
| **AutoGen** | Configurable | AgentChat messages | Group chat | Configurable | Framework-level |
| **CrewAI** | Role-based | Task outputs | Sequential/hierarchical | Reviewer agent | Task chaining |
| **Devin** | Planning/Exec/Verify | Internal | Per-task VM | Verification agent | Cloud VM state |
| **DeerFlow** | Lead + dynamic sub-agents | Structured SubAgentResult | Parallel (Send) | Skills-guided | Isolated subgraphs |

---

## DeerFlow: Dynamic Sub-Agent Harness

DeerFlow (by ByteDance) is a **super agent harness** built on LangGraph that represents a different approach from the purpose-built coding agents above — it is a general-purpose orchestration runtime with dynamic sub-agent spawning.

```mermaid
flowchart TD
    LA["Lead Agent (LangGraph graph)\ncoordinator → planner → researcher → reporter"]
    SA1["Sub-Agent 1\nIsolated context\nScoped tools"]
    SA2["Sub-Agent 2\nIsolated context\nScoped tools"]
    SAN["Sub-Agent N\nIsolated context\nScoped tools"]
    SB["Sandbox (Docker)\n/mnt/user-data/workspace\n/mnt/user-data/outputs"]
    LA -->|"LangGraph Send() — parallel"| SA1
    LA -->|"LangGraph Send() — parallel"| SA2
    LA -->|"LangGraph Send() — parallel"| SAN
    SA1 -->|"SubAgentResult (structured)"| LA
    SA2 -->|"SubAgentResult (structured)"| LA
    SAN -->|"SubAgentResult (structured)"| LA
    LA --- SB
    SA1 --- SB
    SA2 --- SB
```

**What makes DeerFlow different:**

1. **Dynamic sub-agent spawning** — unlike ForgeCode, Claude Code, or Capy where agent roles are statically defined, DeerFlow's lead agent spawns sub-agents on the fly for the specific sub-tasks of the current job. The number and scope of sub-agents varies per task.

2. **LangGraph-native** — orchestration is a typed state graph with checkpointing, time-travel debugging, and durable execution. Sub-agents are subgraphs with isolated state.

3. **Parallel execution via `Send()`** — LangGraph's `Send()` primitive dispatches multiple sub-agents concurrently when tasks are independent.

4. **Skills-as-Markdown** — sub-agents load and follow Markdown skill files (workflow specifications) rather than hard-coded prompts. Skills are loaded progressively, only when needed.

5. **Execution modes** — flash (fast), standard, pro (with planning), ultra (with sub-agents). Users select the trade-off between speed and thoroughness.

**Multi-agent level**: Level 1–2 on the research spectrum. Standard/pro modes are Level 1 (sub-agent delegation); ultra mode is Level 2 (parallel orchestration).

**Where it fits vs. other systems:**
- More dynamic than ForgeCode (static 3-agent), Claude Code (static types), SageAgent (fixed 5-stage pipeline)
- Less specialized than ForgeCode (general-purpose, not coding-first)
- More complete than AutoGen/CrewAI (batteries-included harness, not just framework primitives)

See [`/research/agents/deer-flow/`](../../agents/deer-flow/index.md) for full analysis.

---

## Key Takeaways

### 1. Context Management Drives Architecture

Every production system we studied adopted multi-agent primarily for **context window
management**, not task specialization. Claude Code's documentation is explicit:
sub-agents exist to keep the main context clean.

### 2. Hard Boundaries Beat Soft Prompts

ForgeCode (enforced verification), Capy (capability constraints), and Claude Code
(tool restrictions) consistently outperform systems that rely on prompts alone to
enforce role boundaries.

### 3. Verification Is Non-Negotiable

The top-performing systems all have explicit verification mechanisms — whether
programmatic (ForgeCode), observer-based (SageAgent), or test-driven (Junie CLI).

### 4. Model Tiering Is Cost-Effective

Claude Code's Haiku explore agents and Junie CLI's multi-model router demonstrate
that using cheaper models for routine tasks and expensive models for complex
reasoning produces both better results and lower costs.

### 5. Frameworks Provide Primitives, Not Solutions

MetaGPT, AutoGen, and CrewAI provide building blocks. Production coding agents
build custom multi-agent systems tailored to their specific needs rather than
adopting a framework wholesale.

---

## Cross-References

- [orchestrator-worker.md](./orchestrator-worker.md) — The dominant pattern across implementations
- [specialist-agents.md](./specialist-agents.md) — How agents specialize in practice
- [evaluation-agent.md](./evaluation-agent.md) — Verification approaches compared
- [communication-protocols.md](./communication-protocols.md) — Protocol implementations
- [context-sharing.md](./context-sharing.md) — How context flows in each system
- [agent-comparison.md](./agent-comparison.md) — Detailed comparison table

---

## References

- MetaGPT. https://github.com/geekan/MetaGPT
- Microsoft AutoGen. https://github.com/microsoft/autogen
- CrewAI. https://github.com/crewAIInc/crewAI
- Cognition. "Devin." https://devin.ai
- Research files: `/research/agents/*/` — all agent directories
- ByteDance. "DeerFlow." https://github.com/bytedance/deer-flow
