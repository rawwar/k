# ForgeCode — Multi-Agent Architecture

## Overview

ForgeCode's architecture is fundamentally different from single-loop coding agents like Claude Code or Aider. Instead of one monolithic agent with full context and all tools, ForgeCode uses **three specialized sub-agents** — each with distinct access levels, purposes, and bounded context windows. A proprietary runtime layer (ForgeCode Services) sits beneath all three, providing shared infrastructure.

## The Three Sub-Agents

### FORGE (Implementation Agent)

- **Access**: Read + Write
- **Purpose**: Direct implementation — modifying files, creating code, executing shell commands
- **Active by default** when the user starts a session
- **Ideal for**: Quick fixes, feature implementation, refactoring, routine tasks
- **Invocation**: `:forge` or `/forge` from ZSH

Forge is the hands-on builder. It has full system access (same permissions as the user) and can create, edit, and delete files, run tests, execute arbitrary shell commands. It operates like a junior developer who can write code fast but benefits from a plan.

### MUSE (Planning & Analysis Agent)

- **Access**: Read-only
- **Purpose**: Strategic planning, impact analysis, architecture review
- **Ideal for**: Complex refactoring plans, understanding scope before changes, critical systems review
- **Invocation**: `:muse` or `/muse`

Muse deliberately operates in read-only mode. This constraint is a feature: by preventing Muse from making changes, the system ensures that planning and analysis happen without side effects. Muse creates detailed implementation plans, identifies risks, and proposes solutions. It cannot touch your code — it can only think about it.

### SAGE (Research & Investigation Agent)

- **Access**: Read-only
- **Purpose**: Deep codebase research, dependency tracing, architecture mapping
- **Not user-facing** — used internally by both Forge and Muse
- **Invocation**: Automatic (called transparently by other agents when they need codebase understanding)

Sage is the internal research engine. When Forge or Muse needs to understand how a codebase works — trace a bug across files, map module dependencies, or find the right entry point — they delegate to Sage. The user never interacts with Sage directly; it operates behind the scenes.

## Bounded Context Model

A core architectural principle is **bounded context**: each sub-agent operates on the minimal, relevant context for its current task rather than stuffing everything into one massive context window.

### How Bounded Context Works

1. **Task decomposition**: When a complex task arrives, it is decomposed into sub-tasks. Research tasks go to Sage, planning tasks go to Muse, implementation tasks go to Forge.

2. **Context isolation**: Each sub-agent receives only the context relevant to its specific sub-task. Sage gets the files and symbols it needs to investigate. Muse gets the analysis results and task description. Forge gets the plan and the specific files to modify.

3. **Context doesn't bloat across agents**: If Sage researches 50 files to find the right entry point, only the relevant findings (not all 50 files) are passed to the next agent. This prevents the context window explosion that plagues single-loop agents on large codebases.

4. **Conversation context is preserved when switching agents**: The user's conversation history carries across agent switches, but each agent's internal working context remains bounded.

### Why This Matters

Single-loop agents like Claude Code operate with one growing context window. As a session progresses, the context fills with tool results, file contents, and conversation history. By the time the agent is executing step 15 of a complex task, it may be working with degraded context quality.

ForgeCode's approach keeps each agent's context lean:
- Sage investigates with focused read-only context
- Muse plans with research results + task description
- Forge executes with plan + targeted file contents

## ForgeCode Services (Runtime Layer)

ForgeCode Services is the proprietary infrastructure that sits beneath all three agents. It provides five key capabilities:

### 1. Context Engine (Semantic Entry-Point Discovery)

Before any agent starts exploring a codebase, a lightweight semantic pass identifies the most likely starting files and functions based on the task description. This converts random codebase exploration into directed traversal.

- Uses semantic search (`sem_search` tool) over an indexed project
- Achieves "up to 93% fewer tokens" compared to naive exploration
- Requires `:sync` to index the project initially

The context engine solves a critical problem identified in TermBench evaluation: **context size is a multiplier on the right entry point, not a substitute for it**. Finding the right file early matters more than having a larger context window.

### 2. Dynamic Skill Loading

Skills are specialized instruction sets for particular task types. They are loaded only when the task profile requires them:

- A test-writing task loads the testing skill
- A debugging task loads the debugging skill
- A refactoring task loads the refactoring skill

This keeps the system prompt lean and relevant. Skills that aren't needed don't consume context.

### 3. Tool-Call Correction Layer

A heuristic + static analysis layer intercepts tool calls before dispatch:

- Validates argument shapes against schemas
- Catches common error patterns (wrong field names, nested schema confusion)
- Auto-corrects where possible rather than failing silently

This is especially important for local/open-weight models that have higher tool-call error rates.

### 4. Todo Enforcement

The `todo_write` tool is made non-optional for decomposed tasks:

- Multi-step tasks must have explicit task items created
- Each item must be updated as progress is made
- Completion must be explicitly marked

The runtime treats failure to maintain task state as a reliability failure, not a stylistic choice.

### 5. Progressive Reasoning Budget Control

The reasoning budget (thinking tokens) is managed automatically based on turn count:

| Phase | Messages | Thinking Budget |
|-------|----------|-----------------|
| Planning | First 10 assistant messages | Very high |
| Execution | Messages 11+ | Low (by default) |
| Verification | When verification skill is called | Switches back to high |

This prevents the agent from over-deliberating during execution while ensuring deep reasoning at critical decision points.

## Model Routing

ForgeCode supports mixing models within a single session:

- **Thinking models** (Opus 4, O3, DeepSeek-R1): Complex planning, architecture decisions
- **Fast models** (Sonnet, GPT-4.1, Grok-4): Routine edits, quick fixes, execution
- **Large-context models** (Gemini 3.1 Pro): Big file analysis, cross-file reasoning

Users switch with `:model` — conversation context is preserved across model changes. The recommended workflow is to use a reasoning model during the Muse planning phase, then switch to a fast model for Forge execution.

## Sub-Agent Parallelization

Low-complexity work is delegated to sub-agents running with minimal thinking budget. This keeps the main agent's latency low:

- File reads, pattern searches, and routine edits run as parallel sub-agent calls
- The main agent reserves its thinking budget for high-value decisions
- This speed architecture was a key factor in reaching 78.4% → 81.8% on TermBench

## Configuration

Agents are configured via `forge.yaml` and `AGENTS.md`:

```yaml
# forge.yaml
model: "claude-sonnet-4"
custom_rules: |
  Always add error handling.
  Include unit tests for new functions.
max_walker_depth: 3
max_requests_per_turn: 50
```

`AGENTS.md` in the project root injects team-specific guidelines into the system prompt for all agents. Custom agents can also be defined for domain-specific workflows.

## Architecture Comparison

| Property | ForgeCode | Claude Code | Aider |
|----------|-----------|-------------|-------|
| Agent count | 3 specialized | 1 monolithic | 1 monolithic |
| Context model | Bounded per agent | Single growing window | Single growing window |
| Shell integration | ZSH-native (`:`) | Separate REPL | Separate REPL |
| Tool corrections | Auto-correction layer | None | None |
| Model switching | Mid-session, preserved context | Single model | Single model |
| Verification enforcement | Programmatic | Prompt-based | None |
| Open source | Core yes, services no | No | Yes |