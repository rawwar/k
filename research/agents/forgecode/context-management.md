# ForgeCode — Context Management

## Overview

Context management in ForgeCode operates at two levels: **bounded context across sub-agents** (architectural) and the **ForgeCode Services context engine** (runtime). Together, they solve the two biggest context problems in coding agents: context window bloat and wasted exploration time.

## The Context Problem in Coding Agents

Single-loop agents face a compounding context problem:

1. **Exploration bloat**: To find the right file in an unfamiliar codebase, the agent reads directory listings, file contents, and search results. Most of this is irrelevant but consumes context tokens.
2. **History accumulation**: Each tool call and result stays in the conversation. By message 20, the agent is working with a context window dominated by stale exploration results.
3. **Diminishing returns**: More context doesn't help if the agent is oriented incorrectly. A model with 200K context reading the wrong part of the codebase is just more confidently wrong.

ForgeCode addresses these with two mechanisms.

## Mechanism 1: Bounded Context Across Sub-Agents

### How It Works

Each sub-agent (Forge, Muse, Sage) maintains its own bounded context window:

**Sage (Research)**:
- Receives a focused research question (e.g., "How does authentication work in this codebase?")
- Explores files with read-only access
- Returns a summary of findings to the calling agent
- Its internal exploration context (all the files it read, dead ends it hit) is **not** passed to the next agent

**Muse (Planning)**:
- Receives the research summary + the original task description
- Analyzes and produces an implementation plan
- Its planning context (alternatives considered, trade-offs analyzed) stays bounded

**Forge (Implementation)**:
- Receives the plan + the specific files it needs to modify
- Executes with full read+write access
- Its context is focused on the current implementation step, not the full exploration history

### What Gets Passed Between Agents

| From → To | What Transfers | What Doesn't |
|-----------|---------------|--------------|
| Sage → Muse/Forge | Research findings, relevant file paths, key insights | Raw file contents, dead-end explorations, internal search results |
| Muse → Forge | Implementation plan, file-level action items | Alternative approaches considered, full analysis context |
| User → Any agent | Conversation history | Other agents' internal working context |

### Why This Matters

Consider a task like "Add pagination to the API endpoints" in a large codebase:

**Single-loop agent approach**:
1. Read directory tree (500+ tokens)
2. Search for "pagination" (200+ tokens per result)
3. Read 5 potentially relevant files (2000+ tokens each)
4. Realize 3 were wrong, read 3 more
5. Now plan the implementation with 15K+ tokens of stale context

**ForgeCode approach**:
1. Context engine identifies the 2 most relevant files immediately
2. Sage investigates with focused context, returns a 200-token summary
3. Muse creates a plan using the summary (not the raw files)
4. Forge implements using the plan + only the files it needs to edit

The total context consumed is dramatically lower, and more of it is relevant.

## Mechanism 2: ForgeCode Services Context Engine

### Semantic Entry-Point Discovery

Before any agent starts exploring, the context engine performs a semantic pass:

1. **Project indexing**: Run `:sync` to index the project. ForgeCode Services creates vector embeddings of the codebase.
2. **Query analysis**: When a task arrives, the engine analyzes the task description semantically.
3. **Entry-point identification**: The engine returns the most likely starting files and functions — not a random directory listing but a ranked set of relevant entry points.
4. **Directed traversal**: The agent starts in the right place, using context to reason deeply rather than explore broadly.

### Configuration

```bash
# Semantic search parameters
FORGE_SEM_SEARCH_LIMIT=200    # max results from initial vector search
FORGE_SEM_SEARCH_TOP_K=20     # top-k for relevance filtering
```

### Impact

ForgeCode claims "up to 93% fewer tokens" compared to naive exploration. This was validated on TermBench, where entry-point discovery precision was one of the key metrics tracked.

The blog post states it clearly: **"Context size is a multiplier on the right entry point, not a substitute for it."** A model with 200K context starting in the right file outperforms a model with 1M context starting in the wrong directory.

## Token Economy

ForgeCode manages token usage at multiple levels:

### File-Level Truncation
- Files are truncated at `FORGE_MAX_LINE_LENGTH` (default 2000 chars/line)
- Large files show the first 2000 lines with explicit truncation notices
- `total_lines` metadata is included for models that use it
- Explicit text reminders are added for models that don't infer from metadata

### Search Result Limits
- `FORGE_MAX_SEARCH_RESULT_BYTES` (default 10240) caps search output
- Prevents a single grep from flooding the context with thousands of matches

### Directory Traversal Depth
```yaml
# forge.yaml
max_walker_depth: 3  # limit how deep the agent explores the directory tree
```

### Conversation Compaction

ForgeCode performs conversation compaction to keep long sessions manageable. However, this is a known risk area — a July 2025 incident (documented in a public RCA) was caused by aggressive conversation compaction that lost critical context mid-task.

### File Ignoring
- `.gitignore` patterns are respected automatically
- `.ignore` file provides additional filtering without affecting Git
- Ignored files are excluded from ForgeCode Services sync and context engine retrieval
- Hidden files (starting with `.`) in subdirectories are excluded by default

## Dynamic Skill Loading as Context Management

Skills are effectively a context management mechanism:

- Each skill is a specialized instruction set that occupies system prompt space
- Loading all skills at once would bloat the system prompt with irrelevant instructions
- Instead, the skill engine loads only task-relevant skills
- When the task profile changes, skills can be swapped

This is context management at the instruction level, not just the data level.

## Progressive Thinking as Context Management

The progressive thinking policy is also a context management strategy:

- **High thinking (messages 1–10)**: The model generates extensive internal reasoning. This is expensive in tokens but produces better plans.
- **Low thinking (messages 11+)**: The model spends fewer tokens on reasoning and more on execution. The plan is already set.
- **High thinking (verification)**: Verification is a decision point where reasoning quality matters again.

By reducing thinking tokens during execution, ForgeCode effectively reclaims context budget for tool results and file contents that matter more during implementation.

## Context Preservation Across Agent Switches

When the user switches from Muse to Forge (`:forge`), conversation context is preserved. This means:

- The plan Muse created is visible to Forge
- The user's original request and any clarifications carry over
- But Muse's internal working context (alternatives analyzed, risks evaluated) does not transfer as raw context — only through its output

This is a deliberate design: agents share conversation-level context but not working-level context.

## Comparison to Other Agents

| Aspect | ForgeCode | Claude Code | Aider |
|--------|-----------|-------------|-------|
| Context architecture | Multi-agent bounded | Single window | Single window |
| Entry-point discovery | Semantic engine | User-guided | User-guided |
| File exploration | Directed by context engine | Agent-driven | Agent-driven |
| Skill loading | Dynamic, task-specific | N/A | N/A |
| Thinking budget | Progressive (phase-aware) | Fixed | Fixed |
| Conversation compaction | Yes (with known risks) | Yes | Yes |