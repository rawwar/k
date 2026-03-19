# Capy — Agentic Loop

> Three-phase handoff: User describes task → Captain explores and specs → Build executes autonomously.

## Overview

Unlike most coding agents that run a single agentic loop (prompt → tool call → observe → repeat), Capy structures its workflow as a **three-phase handoff** between the user, Captain (planning agent), and Build (execution agent). Each phase has distinct responsibilities and hard capability constraints.

## The Three-Phase Handoff

### Phase 1: User Describes Task

The user creates a new task (or "jam") in the Capy IDE and describes what they want:

- Natural language description of the feature, bug fix, refactor, etc.
- Can reference existing GitHub issues
- Can tag teammates for collaboration
- The task becomes the organizing unit — it groups the chat, branch, environment, and eventual PR

### Phase 2: Captain Plans

Captain activates as the **planning agent**:

1. **Codebase Exploration**: Captain reads the relevant parts of the codebase to understand the current architecture, patterns, and conventions
2. **Clarification Loop**: If the task description is ambiguous, Captain **asks the user clarifying questions**. This is a key capability — Captain can have a back-and-forth dialogue with the user to fully understand requirements
3. **Research**: Captain can research documentation, explore dependencies, and understand the broader context
4. **Spec Writing**: Captain produces an exhaustive specification — described as "a short PRD" (Product Requirements Document) — that fully describes what Build should implement

**Hard constraints on Captain:**
- Cannot write production code
- Cannot run terminal commands
- Cannot push commits
- Must produce a spec as its sole output

The clarification loop is critical: because Build **cannot** ask questions once it starts, Captain must resolve all ambiguities upfront. This creates a natural quality gate — if Captain doesn't ask the right questions, Build may implement the wrong thing.

### Phase 3: Build Executes

Build activates as the **execution agent**, receiving:

- The spec from Captain
- Access to the codebase (via its own git worktree)
- A full Ubuntu VM with sudo access

Build then works **autonomously and asynchronously**:

1. **Reads the spec** to understand what to implement
2. **Edits files** according to the specification
3. **Installs dependencies** as needed
4. **Runs tests** to verify the implementation
5. **Opens a pull request** on GitHub when complete

**Hard constraints on Build:**
- Cannot ask the user clarifying questions mid-task
- Must make judgment calls on any remaining ambiguities
- Works fully asynchronously — user can close the browser and come back later

## Feedback Mechanisms

### During Captain Phase

The feedback loop is **synchronous and interactive**:

```
User ←→ Captain
  │         │
  │  "What auth provider?"  │
  │◄────────────────────────│
  │  "Use OAuth2 with Google" │
  │────────────────────────►│
  │                         │
  │  "Here's the spec..."   │
  │◄────────────────────────│
```

Captain can ask multiple rounds of clarifying questions before producing the final spec.

### During Build Phase

The feedback loop is **asynchronous and one-directional**:

```
Build ──► Code changes ──► Tests ──► PR
  │
  └─ User monitors progress in Capy dashboard
     but cannot intervene mid-execution
```

If Build's output is unsatisfactory, the user creates a **new task** with feedback, potentially triggering another Captain → Build cycle. This is more like a code review loop than an interactive debugging session.

### Parallel Task Management

Because each task runs independently:

- User can launch multiple Captain → Build pipelines simultaneously
- Tasks don't block each other
- The Capy dashboard shows all active tasks and their status
- Up to 25 concurrent jams on the Pro plan

## Comparison to Single-Agent Loops

| Aspect | Single-Agent (e.g., Claude Code) | Capy Captain/Build |
|--------|----------------------------------|-------------------|
| Planning | Same agent plans and codes | Dedicated planning agent |
| Clarification | Can ask anytime during execution | Only during planning phase |
| Execution | Interactive, user monitors | Autonomous, asynchronous |
| Iteration | Edit-test loop within one session | New task for each iteration |
| Parallelism | One task at a time (typically) | Up to 25 concurrent tasks |

## Limitations of This Analysis

The internal details of how Captain explores codebases, how specs are structured, and how Build's execution loop works internally are not publicly documented. The above is reconstructed from Capy's blog posts and marketing materials.
