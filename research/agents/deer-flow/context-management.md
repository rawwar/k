---
title: DeerFlow Context Management
status: complete
---

# DeerFlow Context Management

## The Challenge

DeerFlow handles long-horizon, multi-step tasks that can span dozens of tool calls, multiple web pages fetched, files written and read, and parallel sub-agent results merged. Without aggressive context management, the LLM's context window fills quickly.

DeerFlow addresses this with four complementary strategies:
1. **Sub-agent context isolation** — parallel work happens in separate context windows
2. **Progressive skill loading** — only load skill specs when the task needs them
3. **In-session summarization** — compress completed sub-tasks before they consume too much space
4. **Filesystem offload** — write intermediate results to disk rather than keeping them in context

---

## Strategy 1: Sub-Agent Context Isolation

The most powerful context management technique in ultra mode is simply not putting everything in one context window.

Each sub-agent runs in its own **isolated subgraph** with a fresh context:

```
Lead Agent context window:
┌────────────────────────────────────────┐
│  System prompt                          │
│  User task                              │
│  Plan (step names only, ~500 tokens)   │
│  Sub-Agent 1 result (summary)          │ ← ~200 tokens
│  Sub-Agent 2 result (summary)          │ ← ~200 tokens
│  Sub-Agent N result (summary)          │ ← ~200 tokens
│  Reporter synthesis                     │
└────────────────────────────────────────┘

Sub-Agent 1 context (separate):
┌────────────────────────────────────────┐
│  System prompt                          │
│  Sub-task: "Research market size of X" │
│  Tool calls + results (web searches)   │ ← potentially large
│  Intermediate findings                  │
│  Final SubAgentResult                  │
└────────────────────────────────────────┘  (discarded after completion)
```

**Key invariant**: Only the `SubAgentResult` (a compact structured summary) crosses the sub-agent boundary. The raw tool calls, intermediate web page content, and reasoning steps are discarded when the sub-agent completes.

This mirrors the pattern documented in [ForgeCode's bounded context model](../../concepts/multi-agent-systems/real-world-examples.md) and [Claude Code's sub-agent isolation](../../concepts/context-management/multi-agent-context.md), but DeerFlow applies it to dynamically spawned sub-agents rather than statically defined ones.

---

## Strategy 2: Progressive Skill Loading

Skills (Markdown workflow specifications) are loaded **only when the current task needs them**. At the start of a session, no skill files are in context.

```
Session start → context: [system_prompt, user_message]
                                          ↓
Task: "Write a research report on X"
                                          ↓
Classifier detects intent → loads skills:
  context: [system_prompt, user_message,
            research/SKILL.md (~2K tokens),
            report-generation/SKILL.md (~2K tokens)]
                                          ↓
Task: "Create slides from the report"
                                          ↓
New intent detected → loads slide skill:
  context: [..., slide-creation/SKILL.md (~2K tokens)]
```

**Contrast with always-loading all skills:**
- 5 built-in skills × ~2K tokens each = ~10K tokens always consumed
- Progressive loading: 0 tokens at start, load only what's needed

For models with smaller context windows (32K–64K), this savings is significant.

---

## Strategy 3: In-Session Summarization

Within a long researcher loop, DeerFlow monitors context usage and summarizes completed sub-tasks before they crowd out future work:

```python
# Simplified representation
def researcher(state):
    while not task_complete(state):
        response = llm.invoke_with_tools(state.messages, tools)
        state = append_tool_results(state, response)

        # Context pressure check
        if context_usage(state) > SUMMARIZATION_THRESHOLD:
            # Identify completed sub-tasks in message history
            completed = find_completed_subtasks(state.messages)
            # Compress them: replace raw tool calls + results with a summary
            summary = llm.summarize(completed)
            state = replace_with_summary(state, completed, summary)
```

The summarization threshold is set conservatively enough to ensure there's always room for the current tool call and its response.

---

## Strategy 4: Filesystem Offload

Rather than accumulating all intermediate results in the context window, DeerFlow encourages (via skill instructions and tool behavior) writing results to the sandbox filesystem:

```
Instead of:
  context: [search result 1 (2000 tokens), search result 2 (1800 tokens),
            fetched page 1 (5000 tokens), fetched page 2 (4200 tokens), ...]

DeerFlow does:
  write: /mnt/user-data/workspace/research-notes.md ← (stored on disk)
  write: /mnt/user-data/workspace/source-1.md
  write: /mnt/user-data/workspace/source-2.md

  context: ["Saved 4 sources to workspace. Reading now as needed."]
```

The agent can then `file_read` specific sections on demand rather than keeping all content in the active context window simultaneously.

This is especially effective for research tasks where raw web content can easily exceed 50K tokens across multiple pages.

---

## Memory: Cross-Session Persistence

Beyond within-session context management, DeerFlow maintains **long-term memory** across sessions:

```
Session N ends → memory update:
{
  "user_profile": {
    "name": "...",
    "technical_stack": ["Python", "FastAPI", "PostgreSQL"],
    "writing_style": "concise, bullet-point heavy",
    "preferred_output": "markdown reports",
    "recurring_workflows": ["competitor analysis", "market research"]
  },
  "accumulated_knowledge": {
    "domain_context": [...],
    "past_research_summaries": [...]
  }
}

Session N+1 starts → memory injected into system prompt:
  "Based on past interactions, this user prefers Python stack,
   concise markdown reports, and works frequently on market research."
```

Memory is stored **locally** and remains under user control. It is not sent to any cloud service by default.

This is architecturally similar to [Mem0's approach](../../../concepts/context-management/memory-systems.md#mem0) — automatic extraction of useful facts from conversations — but implemented as part of the harness itself rather than a separate service.

---

## Comparison with Other Agents

| Technique | DeerFlow | Claude Code | ForgeCode | Goose |
|-----------|----------|-------------|-----------|-------|
| Sub-agent context isolation | ✓ (all sub-agents) | ✓ (task tool) | ✓ (bounded context) | ✓ (summon) |
| Progressive skill/tool loading | ✓ (on-demand) | — | — | Partial |
| In-session summarization | ✓ | ✓ (auto-compact) | ✓ | ✓ (tool-pair) |
| Filesystem offload | ✓ (sandbox FS) | ✓ (workspace) | — | — |
| Cross-session memory | ✓ (built-in) | ✓ (CLAUDE.md) | — | Partial |
| Token counting pre-send | Implicit (LangGraph) | ✓ (explicit) | ✓ (token budget) | ✓ |
