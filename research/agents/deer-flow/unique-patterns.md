---
title: DeerFlow Unique Patterns
status: complete
---

# DeerFlow Unique Patterns

DeerFlow introduces several patterns not seen in the CLI coding agents covered elsewhere in this research. These ideas are worth understanding as independent concepts, not just as DeerFlow implementation details.

---

## 1. Skills as Markdown Capability Modules

**The pattern**: A "skill" is a `.md` file, not code.

Every other agent in this research expresses capabilities as code: TypeScript functions (Claude Code), Rust trait implementations (Codex, Goose), Python classes (OpenHands). DeerFlow's skills are **declarative Markdown documents** that describe a workflow, best practices, and expected outputs.

```markdown
# Research Skill

## Workflow
1. Decompose the question into sub-questions
2. Search for each sub-question
3. Cross-reference sources
4. Synthesize findings

## Best Practices
- Verify claims with ≥ 2 sources
- Save intermediate findings to workspace/notes.md

## Output Format
Structured report with: Summary, Findings, Sources, Confidence Assessment
```

**Why this is interesting:**

The skill is consumed by the LLM directly as context — it is not parsed by code. This means:
- No skill registration system required; drop a `.md` file and it works
- Non-engineers can author new skills without touching code
- Skills can include nuanced guidance (e.g., "for financial data, always cite the source date") that would be awkward to encode in function signatures
- Skills are version-controlled as plain text; diffable, reviewable

**The trade-off**: Skills provide guidance but not enforcement. A code-based tool has a strict interface contract; a Markdown skill relies on the LLM to follow the workflow. This makes skills powerful for high-level orchestration guidance but less suitable for strict constraints.

**Closest equivalent in other agents:**
- Claude Code's `CLAUDE.md` / `.claude/agents/<agent>.md` — project-level Markdown instructions. But these are unstructured notes, not named installable capability modules.
- Goose's `GOOSE_INSTRUCTIONS` env var — similar in being Markdown text, but not modular or on-demand loaded.

---

## 2. Progressive Skill Loading

**The pattern**: Load only the skills the current task needs; don't pre-fill the context with everything.

Most frameworks load all system context at session start. DeerFlow's task classifier infers which skills are relevant and loads only those:

```
User: "Create a presentation on X"
→ Loads: slide-creation/SKILL.md
→ Does not load: research, report-generation, web-page, image-generation

User: "Research X and then create a presentation"
→ Loads: research/SKILL.md, slide-creation/SKILL.md
→ Does not load: report-generation, web-page, image-generation
```

**Why this matters**: At scale, a harness might have dozens or hundreds of skills. Loading all of them wastes context budget and confuses the model with irrelevant instructions. Progressive loading is the skills equivalent of **code splitting** in frontend development — you only load the module when you need it.

---

## 3. Super Agent Harness vs. Framework

**The concept**: The distinction between a *framework* (components you wire together) and a *harness* (a complete runtime that works out of the box).

DeerFlow's documentation explicitly articulates this:

> "DeerFlow 2.0 is no longer a framework you wire together. It's a super agent harness — batteries included, fully extensible."

Traditional agent frameworks (LangChain, LangGraph, CrewAI) provide the building blocks: a graph runner, memory abstractions, tool registries. You assemble them into an agent.

A harness ships with everything pre-assembled: a filesystem, memory, skills, sandboxed execution, sub-agent spawning, IM channel dispatch, and a web UI. You run it, configure it, and extend it — but the core infrastructure is already there.

**The progression in this research:**

```
Raw LLM API
    └── Tool calling (function invocation)
        └── Agent framework (LangGraph, CrewAI — wire your own)
            └── Agent harness (DeerFlow — batteries included)
                └── Specialized agent (Claude Code — coding-first harness)
```

This framing helps position DeerFlow relative to the other agents in this corpus. It's not competing with mini-SWE-agent (minimal single-purpose) or ForgeCode (specialized coding). It's competing with "build your own agent infrastructure from scratch."

---

## 4. Ground-Up Rewrite Driven by Community Scope Creep

**The pattern**: v1 was a deep-research framework. The community used it as a general automation harness. ByteDance rebuilt it from scratch as a proper harness.

This is a case study in how open-source users define a tool's true purpose better than its creators:

- v1 was released as a deep-research automation tool
- Community built data pipelines, slide decks, dashboards, content workflows on top of it
- ByteDance recognized this signal: "DeerFlow wasn't just a research tool. It was a harness."
- v2 was a ground-up rewrite with no shared code from v1
- v1 is maintained on the `1.x` branch; v2 is the active codebase

**The implication for agent design**: Minimal, composable primitives tend to be extended by users in unpredictable directions. The right response (per DeerFlow's v2 decision) is to formalize and support those extensions with proper infrastructure, rather than patching the original design.

---

## 5. The Claude Code ↔ DeerFlow Bridge

**The pattern**: A skill that makes one agent system (Claude Code) a client of another (DeerFlow).

The `claude-to-deerflow` skill creates a bidirectional relationship between two complementary systems:

- **Claude Code** excels at: coding tasks, file editing, running tests, git operations, tight feedback loops
- **DeerFlow** excels at: deep research, multi-source synthesis, parallel sub-agent fan-out, long-horizon tasks

The skill lets a developer working in Claude Code delegate research-heavy or long-horizon tasks to DeerFlow without context-switching:

```
Claude Code session:
  User: /claude-to-deerflow research the landscape for [X] and write a report
  → Claude Code sends HTTP request to DeerFlow Gateway
  → DeerFlow runs ultra-mode multi-agent research
  → Results stream back to Claude Code session
  → Developer continues coding with research context available
```

This is an early, practical implementation of the **agent-to-agent composition** pattern — one specialist agent delegating to another via protocol rather than being monolithically merged.

---

## 6. IM Channel Dispatch Without Public IP

**The pattern**: Receive agent tasks from messaging apps without a public-facing server.

Most webhook-based integrations require a publicly accessible URL. DeerFlow's IM channel integrations use **outbound-only transports** that don't require ngrok, localtunnel, or cloud deployment:

| Channel | Transport | Direction |
|---------|-----------|-----------|
| Telegram | Bot API long-polling | Outbound: polls for messages |
| Slack | Socket Mode | Outbound: WebSocket to Slack servers |
| Feishu / Lark | Long Connection WebSocket | Outbound: WebSocket to Feishu servers |

This makes it possible to run DeerFlow entirely on a local machine (or behind a corporate firewall) while still receiving tasks from mobile apps and messaging platforms.

**Developer implication**: This pattern is reusable for any agent that needs to receive tasks from messaging apps without infrastructure overhead.

---

## 7. InfoQuest: Vertically Integrated Search

**The pattern**: First-party search tooling integrated at the harness level.

BytePlus (ByteDance's enterprise cloud) provides **InfoQuest**, an intelligent search and crawling toolset. DeerFlow ships with native InfoQuest integration, giving ByteDance-ecosystem users a more structured, accurate search experience than general-purpose search APIs.

From an architectural standpoint, this represents a different model than pure MCP extensibility: the harness has an opinionated, first-party integration for a core capability (web search) rather than treating all search tools equally.

---

## Key Takeaways

| Pattern | Why It Matters |
|---------|---------------|
| Skills as Markdown | Non-engineer authoring; LLM reads guidance directly; no code parsing |
| Progressive skill loading | Context budget conservation at scale; composability |
| Super agent harness concept | New category above "framework"; clarifies DeerFlow's positioning |
| v1→v2 rewrite | Community scope creep as product signal; importance of flexibility |
| Claude Code bridge | Agent-to-agent composition via skills; practical A2A before formal standards |
| IM channel dispatch | Outbound-only transports = no public IP required; reusable pattern |
