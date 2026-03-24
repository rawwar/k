---
title: DeerFlow Agentic Loop
status: complete
---

# DeerFlow Agentic Loop

## Overview

DeerFlow's agentic loop is implemented as a **LangGraph state graph** — a directed graph where nodes are Python functions and edges are conditional routing decisions. The loop is explicitly typed (via Pydantic models), checkpointed (durable execution), and streaming (token-by-token output via SSE).

Unlike single-agent loops (Claude Code, Aider), DeerFlow's loop is two-level:
1. **Outer loop**: Lead agent orchestrates the overall task
2. **Inner loop**: Each sub-agent runs its own isolated mini-loop

---

## Execution Modes

The loop's behavior is controlled by the execution mode, selected per-message:

| Mode | Planning | Sub-Agents | Behavior |
|------|----------|------------|----------|
| **flash** | No | No | Direct researcher → reporter path; minimal reasoning |
| **standard** | No | No | Full coordinator → researcher → reporter; no parallelism |
| **pro** | Yes | No | Adds planner node; coordinator decomposes task first |
| **ultra** | Yes | Yes | Full fan-out; sub-agents run in parallel |

---

## Lead Agent Loop (Pseudocode)

```python
# Simplified representation of the LangGraph state graph

State = {
    "messages": list[Message],      # conversation history
    "plan": Plan | None,            # task decomposition (pro/ultra modes)
    "sub_results": list[Result],    # collected from sub-agents
    "context": AgentContext,        # mode, thinking_enabled, etc.
    "memory": UserMemory,           # persistent cross-session data
}

def coordinator(state: State) -> Command:
    """Decides whether to plan, research directly, or respond."""
    if state.context.is_plan_mode:
        return Command(goto="planner")
    if state.context.subagent_enabled and task_is_complex(state):
        return Command(goto="planner")
    return Command(goto="researcher")

def planner(state: State) -> State:
    """Decomposes task into structured sub-tasks (pro/ultra modes)."""
    plan = llm.invoke(PLANNER_PROMPT, state.messages)
    return {**state, "plan": plan}

def researcher(state: State) -> State:
    """Core tool-use loop: search, fetch, bash, files."""
    while not task_complete(state):
        response = llm.invoke_with_tools(state.messages, TOOLS)
        if response.tool_calls:
            results = execute_tools(response.tool_calls, sandbox)
            state = append_tool_results(state, results)
        if response.is_final:
            break
        if context_near_limit(state):
            state = summarize_completed_subtasks(state)
    return state

def sub_agent_spawner(state: State) -> State:
    """Ultra mode: spawns sub-agents in parallel for each plan step."""
    sub_tasks = state.plan.steps
    # LangGraph's Send() primitive for parallel dispatch
    return [Send("sub_agent", {"task": t, "context": state.context})
            for t in sub_tasks]

def sub_agent(state: SubAgentState) -> SubAgentState:
    """Isolated mini-loop: own context, own tools, own termination."""
    # Runs identically to researcher() but in isolated subgraph
    # Cannot see parent context or sibling sub-agents
    # Returns structured SubAgentResult on completion
    ...

def reporter(state: State) -> State:
    """Synthesizes all results into final output."""
    if state.sub_results:
        # Merge parallel sub-agent results
        synthesis = llm.invoke(REPORTER_PROMPT, state.sub_results)
    else:
        synthesis = llm.invoke(REPORTER_PROMPT, state.messages)
    # Output may be: text report, slide deck, web page, code, etc.
    return {**state, "messages": [*state.messages, synthesis]}
```

---

## Sub-Agent Architecture

Sub-agents are the key differentiator in ultra mode:

```
Lead Agent (coordinator/planner/reporter)
    │
    ├── Sub-Agent 1 (isolated context)
    │   ├── Task: "Research market size of X"
    │   ├── Tools: web_search, web_fetch, file_write
    │   └── Returns: SubAgentResult{summary, sources, files}
    │
    ├── Sub-Agent 2 (isolated context)
    │   ├── Task: "Find competitors for X"
    │   ├── Tools: web_search, web_fetch, file_write
    │   └── Returns: SubAgentResult{summary, sources, files}
    │
    └── Sub-Agent N (isolated context)
        ├── Task: "Analyze financial data for X"
        ├── Tools: web_search, bash, file_read, file_write
        └── Returns: SubAgentResult{summary, files, charts}
```

**Key design decisions:**

1. **Isolated context per sub-agent** — sub-agents cannot see each other or the lead agent's context. Prevents distraction and context pollution.
2. **Scoped tools** — each sub-agent gets only the tools it needs for its task.
3. **Termination conditions** — explicit stop criteria prevent runaway sub-agents.
4. **Parallel execution** — LangGraph's `Send()` primitive dispatches sub-agents concurrently when possible.
5. **Structured results** — sub-agents return typed `SubAgentResult` objects, not raw text. The reporter receives clean, structured data.

**What sub-agents share with the lead agent:**
- Sandbox filesystem (read/write to `/mnt/user-data/workspace/`)
- Skill definitions (can load and follow skills)
- Memory (read-only; cannot mutate)
- Model configuration

**What sub-agents do NOT share:**
- Message history (completely isolated)
- Tool call history
- Intermediate reasoning

---

## Tool Execution in the Loop

Tools run inside the sandbox container. The loop dispatches tool calls and collects results:

```
Researcher node
    │
    ├── LLM produces ToolCall(name="web_search", args={query: "..."})
    │
    ├── Tool dispatcher routes to sandbox execution
    │   ├── web_search → Tavily/InfoQuest API
    │   ├── web_fetch  → HTTP fetch in sandbox
    │   ├── bash       → Execute in Docker container
    │   └── file_*     → Read/write /mnt/user-data/workspace/
    │
    └── ToolResult returned to message history
```

---

## Context Management in the Loop

DeerFlow manages context pressure within the researcher loop:

1. **Skill progressive loading** — skills load only when the task needs them; not at loop start
2. **Sub-task summarization** — completed sub-tasks are summarized and their raw content removed
3. **Filesystem offload** — intermediate results written to `/mnt/workspace/` rather than kept in context
4. **Sub-agent isolation** — parallel work happens in separate context windows; only structured summaries return to lead

See [context-management.md](context-management.md) for details.

---

## Streaming

DeerFlow streams responses token-by-token via **Server-Sent Events (SSE)**:

```
LangGraph Server (port 2024)
    │
    └── /stream endpoint
        ├── Emits: {"type": "message_chunk", "content": "..."}
        ├── Emits: {"type": "tool_call", "name": "web_search", ...}
        ├── Emits: {"type": "tool_result", "content": "..."}
        ├── Emits: {"type": "sub_agent_start", "task": "..."}
        └── Emits: {"type": "done"}

Frontend (port 3000)
    └── EventSource → incremental rendering in Next.js UI
```

The IM channel gateway also streams responses back to Telegram/Slack/Feishu in real-time.

---

## Comparison with Other Agents in This Research

| Aspect | DeerFlow | Claude Code | ForgeCode | SageAgent |
|--------|----------|-------------|-----------|-----------|
| Loop implementation | LangGraph graph | Custom TypeScript | Custom (ZSH-native) | Custom Python |
| Sub-agent spawning | Dynamic, on-the-fly | Static task types | Static 3-agent | Static 5-agent pipeline |
| Sub-agent isolation | Full context isolation | Full context isolation | Summary-only handoff | Summary handoff |
| Parallel execution | Yes (Send primitive) | No (sequential) | No | No |
| Execution modes | 4 (flash→ultra) | 3 (default/plan/agent) | 1 | 1 |
| Streaming | SSE token-by-token | SSE token-by-token | Terminal streaming | No |
| Sandbox | Docker container | Host process | Host shell | Host process |
| Checkpointing | Yes (LangGraph) | No | No | No |
