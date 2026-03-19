---
title: Warp Context Management
status: complete
---

# Context Management

> Warp's context system combines semantic codebase indexing, multi-modal context sources
> (blocks, images, URLs, selections, @ references), hierarchical rules, reusable skills,
> persistent storage via Warp Drive, and conversation branching/compaction for long sessions.

## Overview

Context management in Warp operates at multiple levels:

```
┌────────────────────────────────────────────────────────────────┐
│                    Context Assembly                              │
│                                                                  │
│  ┌──────────────┐ ┌─────────────┐ ┌──────────────────────────┐ │
│  │  Codebase     │ │ Multi-Modal │ │  Rules & Skills          │ │
│  │  Context      │ │ Context     │ │                          │ │
│  │               │ │             │ │  Global Rules            │ │
│  │  Semantic     │ │ Blocks      │ │  Project AGENTS.md       │ │
│  │  Index        │ │ Images      │ │  Directory AGENTS.md     │ │
│  │  (Embeddings) │ │ URLs        │ │  SKILL.md files          │ │
│  │               │ │ Selections  │ │                          │ │
│  │  Git-tracked  │ │ @ references│ │  Warp Drive objects      │ │
│  │  files        │ │ Clipboard   │ │                          │ │
│  └──────────────┘ └─────────────┘ └──────────────────────────┘ │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  Conversation State                                          ││
│  │  History • Compacted summaries • Forked branches             ││
│  └─────────────────────────────────────────────────────────────┘│
└────────────────────────────────────────────────────────────────┘
```

## Codebase Context: Semantic Indexing

### How It Works

Warp builds a **semantic index** of the codebase using embeddings:

1. **File discovery**: Scans Git-tracked files in the repository
2. **Chunking**: Splits files into meaningful chunks (functions, classes, sections)
3. **Embedding generation**: Creates vector embeddings for each chunk
4. **Index storage**: Stores embeddings locally for fast retrieval
5. **Query**: Agent queries the index with natural language to find relevant code

### Indexing Lifecycle

```
Repository State                   Semantic Index
     │                                  │
     │  ── New conversation ──────────► │  Re-index (if stale)
     │                                  │
     │  ── Periodic check ────────────► │  Re-index (if changed)
     │                                  │
     │  ── Agent query ───────────────► │  Vector similarity search
     │                                  │    │
     │  ◄── Relevant chunks ──────────  │    ▼
     │                                  │  Return top-K matches
```

### Indexing Details

| Aspect | Behavior |
|--------|----------|
| **Scope** | Git-tracked files only (respects .gitignore) |
| **Trigger** | New conversation start, periodic background check |
| **Chunking strategy** | Language-aware (functions, classes, blocks) |
| **Embedding model** | Provider-side embedding (varies by configuration) |
| **Storage** | Local to Warp app (not sent to cloud unless cloud agent) |
| **Query** | Natural language similarity search |
| **Ranking** | Cosine similarity with relevance threshold |

### Benefits Over Repo-Map (Aider-style)

Warp's semantic index differs from aider's tree-sitter repo-map approach:

| Feature | Warp Semantic Index | Aider Repo-Map |
|---------|--------------------|--------------  |
| Approach | Vector embeddings | Tree-sitter AST + PageRank |
| Query type | Natural language | Symbol-based |
| Granularity | Semantic chunks | Function/class signatures |
| Context cost | Only relevant chunks | Full map summary |
| Update | Re-index on change | Rebuild on conversation |
| Search | Fuzzy/semantic | Exact symbol match |

## Multi-Modal Context

Warp's agent can consume context from diverse sources:

### Block Context

Terminal blocks are first-class context objects:

```
User: @block[3] Why did this command fail?

Agent receives:
├── Block command: "npm test"
├── Block output: "FAIL src/auth.test.ts..."
├── Block exit code: 1
├── Block working directory: /Users/dev/myapp
├── Block duration: 4.2s
└── Block timestamp: 2025-01-15 14:32:00
```

Blocks can be referenced by:
- **Number**: @block[3] (Nth most recent block)
- **Selection**: User selects text within a block
- **Auto-inclusion**: Agent automatically includes recent relevant blocks

### Image Context

- **Paste/drag images**: Screenshots, diagrams, mockups
- **Vision model processing**: LLM analyzes image content
- **Use cases**: UI bug reports, design implementation, diagram comprehension

### URL Context

- **Paste URLs**: Agent fetches and parses web page content
- **Documentation references**: Link to API docs, Stack Overflow answers
- **Auto-extraction**: Agent can pull key information from linked pages

### Selection Context

- **Text selection**: User selects text in terminal and references it
- **Partial block selection**: Select specific output lines within a block
- **Multi-selection**: Reference multiple selections simultaneously

### @ References

Rich structured references using @ syntax:

| Reference | Syntax | Description |
|-----------|--------|-------------|
| **File** | @file.ts | Include file contents in context |
| **Folder** | @src/components/ | Include folder structure and contents |
| **Symbol** | @MyComponent | Reference a code symbol (function, class) |
| **Block** | @block[N] | Reference terminal block N |
| **Warp Drive** | @drive/item | Reference Warp Drive stored object |
| **URL** | @https://... | Fetch and include URL content |

### Context Window Budget

The agent manages context to fit within model token limits:

```
Total Context Budget
├── System prompt & rules .......... ~5-10%
├── Codebase context (semantic) .... ~20-40%
├── Conversation history ........... ~20-30%
├── User message + references ...... ~10-20%
├── Tool definitions ............... ~5-10%
└── Reserved for response .......... ~15-20%
```

When context exceeds the budget:
1. Older conversation turns are compacted (summarized)
2. Less relevant codebase chunks are dropped
3. Block context is trimmed to most recent/relevant
4. Agent may suggest explicit /compact or /fork-and-compact

## Rules System

### Hierarchy

Warp's rules system applies configuration hierarchically:

```
┌─────────────────────────────────────────┐
│  Global Rules (Warp app settings)        │  ← Applies to all projects
├─────────────────────────────────────────┤
│  Project Rules (root AGENTS.md)          │  ← Applies to entire project
├─────────────────────────────────────────┤
│  Directory Rules (nested AGENTS.md)      │  ← Applies to directory subtree
├─────────────────────────────────────────┤
│  Conversation Rules (per-session)        │  ← Applies to current conversation
└─────────────────────────────────────────┘
     ↑ Higher priority overrides lower
```

### AGENTS.md Format

AGENTS.md files are markdown documents that instruct the agent:

```markdown
# Project Rules

## Code Style
- Use TypeScript strict mode
- Prefer functional components with hooks
- All functions must have JSDoc comments

## Testing
- Every new function needs a unit test
- Use Jest with React Testing Library
- Minimum 80% code coverage for new files

## Architecture
- Follow the repository pattern for data access
- Use dependency injection for services
- Keep components under 200 lines

## Forbidden
- Do not modify files in src/legacy/
- Do not use `any` type in TypeScript
- Do not commit directly to main branch
```

### Rule Resolution

When the agent operates on a file, rules are resolved by walking up the directory tree:

```
/project/
├── AGENTS.md                    ← "Use TypeScript strict mode"
├── src/
│   ├── AGENTS.md               ← "Use React hooks, not class components"
│   ├── components/
│   │   ├── AGENTS.md           ← "Components must have Storybook stories"
│   │   └── Button.tsx          ← All three AGENTS.md files apply
│   └── api/
│       ├── AGENTS.md           ← "Use Zod for request validation"
│       └── users.ts            ← Root + src + api AGENTS.md apply
└── scripts/
    └── deploy.sh               ← Only root AGENTS.md applies
```

## Skills System

### What Are Skills?

Skills are reusable markdown instruction sets stored as SKILL.md files:

```markdown
# Code Review Skill

$ARGUMENTS: The pull request URL or branch name to review

## Instructions

1. Fetch the diff for $ARGUMENTS
2. For each changed file:
   - Check for security issues (SQL injection, XSS, etc.)
   - Verify error handling
   - Check for performance issues
   - Verify test coverage
3. Generate a summary report with:
   - Critical issues (must fix)
   - Suggestions (nice to have)
   - Positive observations

## Output Format

Use a markdown table for findings:
| Severity | File | Line | Issue |
|----------|------|------|-------|
```

### Skill Discovery

Skills are discovered from multiple locations:

```
Discovery paths (in priority order):
├── .agents/skills/          ← Project-specific skills
├── .warp/skills/            ← Warp-specific project skills
├── ~/.warp/skills/          ← User global skills
└── Warp Drive skills/       ← Team-shared skills
```

### Skill Parameterization

Skills support the `$ARGUMENTS` placeholder:

```
User: "Use the code-review skill on PR #42"

Agent:
1. Discovers .agents/skills/code-review.md
2. Substitutes $ARGUMENTS with "PR #42"
3. Follows the skill instructions step by step
```

### Skill Composition

Skills can reference other skills and build on each other:
- A "deploy" skill might internally invoke a "test" skill
- Skills can be chained in task lists
- Agent can suggest relevant skills based on the current task

## Warp Drive

Warp Drive is persistent, shared storage for agent-related artifacts:

### Stored Objects

| Object Type | Description |
|-------------|-------------|
| **Plans** | Structured plans with version history |
| **Conversations** | Saved conversation sessions |
| **Skills** | Team-shared skill definitions |
| **Rules** | Shared rule sets |
| **Snippets** | Reusable code/command snippets |
| **Workflows** | Multi-step automation workflows |

### Sharing and Collaboration

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Developer A │     │  Warp Drive  │     │  Developer B │
│              │────►│              │◄────│              │
│  Creates     │     │  Shared      │     │  Uses shared │
│  skill       │     │  storage     │     │  skill       │
│              │     │              │     │              │
│  Saves plan  │────►│  Versioned   │◄────│  Reviews     │
│              │     │  persistent  │     │  plan        │
└─────────────┘     └─────────────┘     └─────────────┘
```

### Warp Drive Features

- **Cross-device sync**: Access artifacts from any device with Warp
- **Team sharing**: Share with team members (access control)
- **Version history**: Track changes to stored objects
- **Search**: Full-text search across stored artifacts
- **@ reference**: Reference Drive objects in conversations with @drive/

## Conversation Forking and Compaction

### Forking

Conversation forking creates branching exploration paths:

```
Main conversation
├── Turn 1: "Fix the auth bug"
├── Turn 2: Agent investigates
├── Turn 3: Agent proposes JWT fix
│
├── /fork → Branch A: "Actually, try session-based approach"
│   ├── Turn 4a: Agent implements sessions
│   └── Turn 5a: Tests pass ✓
│
└── /fork from Turn 2 → Branch B: "Check if it's a CORS issue"
    ├── Turn 3b: Agent checks CORS
    └── Turn 4b: Not a CORS issue
```

**Fork commands**:
- `/fork` — Branch from current point, preserving full history
- `/fork-and-compact` — Branch with summarized parent history (saves context)
- `/fork from [point]` — Branch from a specific earlier turn

### Compaction

When conversations grow long, compaction summarizes history to free context:

```
Before compaction:                    After /compact:
├── Turn 1: User request             ├── [Summary]: "User reported auth
├── Turn 2: Agent analysis (long)    │   bug. Investigated JWT expiry,
├── Turn 3: User clarification       │   found race condition in refresh
├── Turn 4: Agent code analysis      │   flow. Fixed by adding mutex
├── Turn 5: Agent proposes fix       │   lock. Tests pass."
├── Turn 6: User feedback            │
├── Turn 7: Agent revises            ├── Turn 8: [continues from here
├── Turn 8: Tests pass               │   with full context of summary]
                                     │
~4000 tokens                         ~200 tokens
```

**Compaction strategy**:
- Preserves key decisions and their rationale
- Retains code changes and test outcomes
- Drops verbose intermediate analysis
- Maintains enough context for agent to continue effectively
- Agent may suggest compaction proactively when context is large

## Context in Cloud Agents

Cloud agents have similar context capabilities with some differences:

| Feature | Local Agent | Cloud Agent |
|---------|-------------|-------------|
| Semantic index | Local index, fast queries | Built on environment setup |
| Block context | Live terminal blocks | Simulated command outputs |
| Image context | User-provided images | Screenshots from Computer Use |
| Rules | Local AGENTS.md + global | Repository AGENTS.md only |
| Skills | All discovery paths | Repository + Warp Drive only |
| Warp Drive | Full access | Full access |
| Conversation | Real-time | Async via web app/API |

## Summary

Warp's context management is among the most sophisticated of any coding agent, combining
semantic code understanding (embeddings index), rich multi-modal inputs (blocks, images,
URLs, @ references), hierarchical project configuration (Rules + Skills), persistent team
storage (Warp Drive), and conversation management (forking, compaction). The terminal-native
architecture adds unique context sources — particularly block-level structured command
history — that wrapper-based agents cannot access.
