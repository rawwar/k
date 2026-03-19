# Droid — Context Management

> Factory's compaction engine is Droid's most differentiated capability — enabling multi-week sessions, cross-interface context persistence, and "teach once, build many" workflows.

## The Problem

Context window management is the central challenge for every coding agent. Models have finite token limits, and real engineering work often spans:

- Large codebases with hundreds or thousands of files
- Multi-day tasks that accumulate extensive conversation history
- Cross-repository dependencies and upstream packages
- Design decisions, reviewer feedback, and architectural rationale

Most agents handle this with simple truncation, sliding windows, or forced session resets. Factory took a fundamentally different approach.

## The Compaction Engine

Factory's proprietary **compaction engine** is what enables Droid's multi-week sessions. Based on public information (primarily the Chainguard case study and docs), the engine works through:

### Incremental Compression with Anchor Points

Rather than discarding old context or performing naive summarization, the compaction engine:

1. **Identifies anchor points** — key decisions, important code changes, critical context that must persist.
2. **Incrementally compresses** surrounding context — reducing verbosity while preserving semantic meaning.
3. **Maintains referenceability** — compressed context can be recalled when relevant, not lost entirely.

The result is described by Factory's docs as "near-perfect context" — developers experience it as working with a colleague who "just remembers" what's been discussed.

### How It Differs from Competitors

| Approach | Agent | Limitation |
|----------|-------|------------|
| Session reset | Most agents | Lose all context between sessions |
| Sliding window | Basic agents | Lose early context as conversation grows |
| Manual curation | Claude Code (CLAUDE.md) | Requires developer to maintain context files |
| Memory MCPs/tools | Various | Josh Wolf (Chainguard): "None of them offered what they promised" |
| **Compaction** | **Droid** | **Automatic, incremental, preserves weeks of context** |

### Real-World Validation

From the Chainguard case study:

> "I keep telling Matt (Moore, CTO Chainguard), I've had multiple Droid sessions going on for weeks. Because compaction is just that good."

> "When you don't have to think about context windows, you can treat Droid like a colleague that just remembers what you've been talking about."

Key metrics from the case study:
- **2-week continuous session** without context degradation
- **6 repositories** managed within sessions
- **80 packages built** using the "teach once" pattern

## Context-Confidence

Factory's documentation describes their context approach as **"context-confidence"** — compressing incrementally with anchor points for near-perfect context. This suggests a confidence-scored system where:

- Each piece of context has an associated relevance/confidence score.
- Higher-confidence context (anchor points) is preserved at full fidelity.
- Lower-confidence context is progressively compressed.
- The system dynamically adjusts based on current task relevance.

## Multi-Source Context Assembly

Droid doesn't just manage conversation context — it assembles context from multiple sources:

### Code Context
- Repository structure and file contents
- Git history and diffs
- Cross-repository references (multiple repos in a single session)

### Project Context (via Integrations)
- **Jira/Linear**: Task descriptions, acceptance criteria, story points
- **Notion/Confluence/Google Drive**: Design documents, architectural decisions, team knowledge
- **Sentry/PagerDuty**: Error details, stack traces, incident history

### Session Context
- Accumulated conversation history (compressed via compaction)
- Teaching interactions ("I showed Droid how to build a package once")
- Review feedback and decision rationale

### Configuration Context
- `.droid.yaml` review guidelines and path-specific rules
- Repository-specific patterns and conventions
- Team-level model preferences

## "Teach Once, Build Many" Pattern

One of the most powerful context management patterns enabled by compaction:

1. Developer works with Droid to build something the first time, explaining patterns and decisions.
2. Droid learns the pattern within the session context.
3. For subsequent similar tasks, Droid applies the pattern independently.

From Chainguard:
> "I can start a session with Droid, as I would with a junior engineer, and naturally teach it to perform a task once or twice, and we'll work on it together. Then I have a senior engineer that can operate independently with minimal intervention."

This was specifically used to build **80 open-source packages** — the developer taught the pattern for a few, then Droid replicated it across the remaining packages with minimal guidance.

## Cross-Interface Context Persistence

Because the agent core is interface-agnostic, context persists across interface transitions:

- Start a brainstorming session in the **web workspace**.
- Continue implementation in the **CLI**.
- Review the PR through **GitHub integration**.
- Discuss feedback in **Slack**.

The compaction engine maintains continuity throughout — the context of why a decision was made, what alternatives were considered, and what reviewer feedback was given all stays accessible regardless of which interface the developer uses.

## Token Economics and Model Routing

Context management intersects with model routing for cost optimization:

- Factory Analytics tracks **token consumption** by model, input/output split, and **cache efficiency**.
- Teams can route context-heavy reasoning work to frontier models (Opus 4.6, o3) while routing execution to cost-efficient models (GLM-5, Kimi-K2.5).
- Cache efficiency metrics suggest the system optimizes for prompt caching — keeping common context in cached prefixes to reduce costs.

## No Manual Context Curation Required

A key differentiator versus competitors: developers don't need to manually curate context. From the Chainguard case study:

> "The thing I dislike about other CLI agents is that you have to meticulously curate things like skills and commands to effectively seed the context window to perform repeatable tasks. I don't have to do any of that with Droid."

This is a direct contrast to agents that rely on:
- `CLAUDE.md` files (Claude Code)
- Custom instructions (Cursor)
- Memory MCPs (various)
- Manual `/add` file management (Aider)

Droid's compaction engine handles context curation automatically, reducing the "meta-work" of using an AI coding agent.