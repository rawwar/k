# ForgeCode — Unique Patterns & Key Differentiators

## Overview

ForgeCode introduces several patterns that are either unique among terminal coding agents or represent notably different approaches. This document catalogs the key differentiators.

---

## 1. ZSH-Native Integration (`:` Sentinel)

**What**: Instead of entering a separate REPL (like Claude Code's `claude` command or Aider's `aider` command), ForgeCode integrates directly into the user's ZSH session. The `:` character followed by a space triggers a prompt to ForgeCode.

**How it works**:
- A ZSH plugin is installed via `forge setup`
- The plugin intercepts lines starting with `: ` and routes them to the ForgeCode runtime
- The user's shell environment (aliases, functions, Oh My Zsh plugins, PATH, env vars) remains fully intact
- Pressing Tab after `:` opens a command completion list

**Key features**:
- **File tagging**: `@` + partial filename + Tab opens a fuzzy file picker (uses `fd` and `fzf`)
- **Agent switching**: `:muse`, `:forge`, `:sage` switch agents inline
- **New conversation**: `:new` clears context; `:new <prompt>` starts fresh with a prompt
- **Conversation switching**: `:conversation` lists saved conversations; `:-` jumps to the last one
- **Multiline input**: Shift+Enter (Windows/Linux) or Option+Enter (macOS)
- **Editor mode**: `:edit` opens `$FORGE_EDITOR` or `$EDITOR` for long prompts
- **Retry**: `:retry` resends the last prompt after a cancel

**Why it matters**: Most coding agents force a context switch — you leave your shell, enter their REPL, lose access to your shell customizations, and must mentally switch between "shell mode" and "agent mode." ForgeCode eliminates this boundary. Shell commands and AI prompts live in the same workflow, which reduces friction for power users.

**Comparison**:
| Feature | ForgeCode | Claude Code | Aider |
|---------|-----------|-------------|-------|
| Invocation | `:` in native ZSH | `claude` REPL | `aider` REPL |
| Shell env preserved | Yes | No | No |
| Aliases work | Yes | No | No |
| File fuzzy-picker | `@` + Tab | N/A | N/A |

---

## 2. Multi-Agent Architecture (Not a Single Loop)

**What**: Three specialized agents (Forge, Muse, Sage) with distinct access levels and purposes, vs. the single-loop approach used by most other agents.

**Why it's different**: Single-loop agents like Claude Code and Aider use one agent that does everything — research, planning, implementation — in one growing context window. ForgeCode separates concerns:

- **Muse** (read-only): Plans without side effects. Cannot accidentally modify files while analyzing.
- **Forge** (read+write): Implements without re-deliberating. Has a plan to follow.
- **Sage** (read-only): Researches without bloating other agents' context. Returns summaries, not raw exploration data.

**The bounded context benefit**: Each agent operates on minimal, relevant context for its role. Research findings are summarized before being passed to the planner. The plan is passed to the implementer without the full analysis context. This prevents the context degradation that hits single-loop agents on long tasks.

---

## 3. Model Routing Per Task Type

**What**: Users can assign different models to different task phases and switch mid-session with `:model`, preserving conversation context.

**Recommended workflow**:
1. Use a **thinking model** (Opus 4, O3, DeepSeek-R1) during Muse planning phase
2. Switch to a **fast model** (Sonnet, GPT-4.1, Grok-4) for Forge execution
3. Use a **large-context model** (Gemini 3.1 Pro) for big-file analysis

**Why it's different**: Most coding agents are tied to one model per session. Switching models means starting over. ForgeCode's model-agnostic architecture means you can use the most cost-effective model for each phase without losing context.

**Provider support**: ForgeCode connects to 100+ models through providers including Anthropic, OpenAI, Google, DeepSeek, Mistral, Meta, OpenRouter, and any OpenAI-compatible endpoint.

---

## 4. ForgeCode Services Context Engine

**What**: A proprietary runtime layer that provides semantic entry-point discovery, achieving "up to 93% fewer tokens" compared to naive codebase exploration.

**How it works**:
1. `:sync` indexes the project (creates vector embeddings)
2. When a task arrives, the engine semantically identifies the most likely starting files/functions
3. The agent begins in the right location instead of exploring randomly
4. `sem_search` provides ongoing semantic code search throughout the session

**Why it matters**: The blog states: "Context size is a multiplier on the right entry point, not a substitute for it." On TermBench, Google reports Gemini 3.1 Pro at 68.5%. ForgeCode ran the same model and scored 78.4%. The 10-point delta is attributed primarily to the context engine getting the agent oriented faster.

---

## 5. Tool-Call Correction Layer

**What**: A heuristic + static analysis layer that intercepts every tool call before dispatch, validates arguments, catches common patterns, and auto-corrects where possible.

**Why it's different**: Other agents let tool calls fail and rely on the model to retry. ForgeCode catches and repairs errors at the dispatch boundary, preventing the cascading failures that degrade complex task trajectories.

**Key insights from their work**:
- **Field ordering in JSON schemas is a reliability variable** — `required` before `properties` reduces malformed calls
- **Flat schemas beat nested schemas** — fewer structural layers = fewer mistakes
- **Training-data-aligned naming matters** — `old_string`/`new_string` instead of generic names
- **Truncation signals must be explicit** — some models don't infer from metadata

**Model-specific corrections**: The layer applies different corrections for different models. Opus 4.6 tolerates messier schemas. GPT 5.4 needs cleaner structure. Both reach 81.8% with model-appropriate corrections.

---

## 6. Enforced Verification Skill

**What**: Before a task is marked complete, the runtime programmatically requires a verification pass. The model switches from builder mode to reviewer mode and generates a checklist:
- What was requested
- What was actually done
- What evidence exists that it worked
- What is still missing

**Why it's different**: Normal prompting ("please verify your work") doesn't produce reliable verification. ForgeCode **enforces** it — if the agent hasn't called the verification skill before finishing, the runtime injects a reminder and blocks completion.

**Impact**: This was "the biggest single improvement" according to their blog. GPT 5.4 particularly benefited — it would implement solutions, sound confident, and stop before the task was actually complete. Enforcement caught these premature completions.

---

## 7. Progressive Thinking Policy

**What**: Automatic reasoning budget control based on turn count:
- Messages 1–10: Very high thinking (planning phase)
- Messages 11+: Low thinking (execution phase)
- Verification: Switches back to high thinking

**Why it's different**: Other agents use a fixed reasoning budget throughout. ForgeCode recognizes that different phases need different levels of deliberation. Over-thinking during execution wastes time; under-thinking during planning produces bad plans.

---

## 8. #1 on Terminal-Bench 2.0

**What**: 81.8% completion rate with both Claude Opus 4.6 and GPT 5.4 — the top two positions on the TermBench 2.0 leaderboard, held by the same agent with different models.

**Benchmark comparisons from their landing page**:
| Agent | Score |
|-------|-------|
| ForgeCode | 81.8% |
| Warp | 61.2% |
| Claude Code | 58% |
| Open Code | 51.7% |

**Why it matters**: ForgeCode is open-source at its core and achieved #1 without proprietary model fine-tuning. The result came from runtime engineering — schema optimization, tool corrections, verification enforcement, context management. This validates the thesis that **agent runtime design matters as much as model capability**.

---

## 9. Non-Interactive Mode for Autonomous Execution

**What**: A separate runtime profile that disables interactive behavior (no clarification questions, no user confirmations, no hedging). Essential for benchmarks and CI/CD pipelines.

**Why it's different**: Most agents are designed for interactive use first. ForgeCode explicitly maintains two operational modes:
- **Interactive mode**: Asks questions, confirms decisions, checks with the user
- **Non-interactive mode**: Assumes reasonable defaults, commits to answers, never waits for input

This dual-mode approach means the same agent works both as a human-facing assistant and as a CI/CD automation tool.

---

## 10. AGENTS.md for Team Configuration

**What**: An `AGENTS.md` file in the project root that injects team-specific guidelines into the system prompt for all agents. This is ForgeCode's equivalent of Claude Code's `.claude` or Cursor's `.cursorrules`, but uses standard markdown.

**Priority system**: AGENTS.md is searched in base path → git root → cwd order. The first file found wins.

**Why it matters**: Custom rules become part of every agent's "personality" for the session, ensuring consistent code style, testing patterns, and architectural decisions across the team.

---

## Summary: What Makes ForgeCode Architecturally Unique

The core thesis is that **agent runtime engineering matters as much as — or more than — raw model capability**. ForgeCode's innovations are all at the runtime layer:

1. Shell integration that eliminates the agent/shell context switch
2. Multi-agent separation of concerns with bounded context
3. Model routing that uses the right tool for each phase
4. A context engine that gets the agent oriented before it starts
5. Tool corrections that make any model more reliable
6. Enforced verification that catches premature completion
7. Progressive thinking that allocates reasoning budget where it matters

The result: the same model weights, in ForgeCode's runtime, score 10+ percentage points higher than in the vendor's default harness. That delta is the product of all these patterns working together.