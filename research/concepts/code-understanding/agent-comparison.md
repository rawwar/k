---
title: Cross-Agent Comparison of Code Understanding
status: complete
---

# Agent Comparison

> A detailed comparison of how all 17 CLI coding agents approach code understanding — indexing methods, search strategies, LSP usage, git integration, and project detection.

## Overview

This document provides a systematic comparison of code understanding capabilities across all 17 agents studied in this research library. The comparison reveals that code understanding is the area with the greatest variance between agents — from sophisticated systems like Aider's repo map and Junie CLI's IDE integration, to minimal approaches where agents rely entirely on the LLM's ability to formulate search queries.

---

## Master Comparison Table

| Agent | Tier | Indexing | Search | Static Analysis | LSP | Git | Project Detection |
|---|---|---|---|---|---|---|---|
| **Claude Code** | 1 | None (on-demand) | ripgrep + glob | tree-sitter (View) | Diagnostics | Medium | CLAUDE.md |
| **Codex** | 1 | None | ripgrep + glob | None | None | Medium | AGENTS.md/CODEX.md |
| **Aider** | 2 | Tree-sitter repo map + PageRank | ripgrep | tree-sitter (deep) | None | Medium-High | Extension-based |
| **ForgeCode** | 1 | Optional embeddings | ripgrep + semantic | AST entry points | None | Medium | Multi-signal |
| **OpenHands** | 1 | None | ripgrep + find | None | None | Medium | Runtime |
| **Gemini CLI** | 2 | None | Shell search | None | None | Low-Medium | Config detection |
| **OpenCode** | 2 | None | ripgrep | tree-sitter (optional) | Partial (Go) | Low-Medium | Go-focused |
| **Goose** | 2 | Via MCP extensions | ripgrep + MCP tools | Via extensions | Via extensions | Low-Medium | Extension-based |
| **Warp** | 2 | None | ripgrep | None | Via IDE | Low-Medium | IDE-based |
| **Ante** | 1 | Embedding index | Semantic + ripgrep | tree-sitter | None | Medium | Multi-language |
| **Droid** | 1 | Incremental tree-sitter | ripgrep + AST | tree-sitter (deep) | Partial | High | Comprehensive |
| **Junie CLI** | 2 | JetBrains index | JetBrains search | Full (JetBrains PSI) | Full | High | Full IDE |
| **mini-SWE-agent** | 2 | None | grep/find | None | None | Low | Minimal |
| **Pi Coding Agent** | 2 | None | ripgrep | None | None | Low | Basic |
| **Sage Agent** | 3 | None | Basic search | None | None | Minimal | Minimal |
| **TongAgents** | 3 | Shared agent memory | Distributed search | Multi-agent analysis | None | Agent-mediated | Agent-mediated |
| **Capy** | 3 | None | Basic search | None | None | Low | Basic |

---

## Detailed Comparison by Category

### Indexing Methods

The approaches to codebase indexing reveal fundamental architectural differences:

| Approach | Agents | Startup Cost | Quality | Description |
|---|---|---|---|---|
| **No index** | Claude Code, Codex, OpenHands, Gemini CLI, Warp, mini-SWE, Pi, Sage, Capy | None | Relies on search | No pre-computation; uses on-demand search for every query |
| **Tree-sitter tag index** | Aider | Seconds | High | Parses all files with tree-sitter, extracts definitions and references, builds graph, ranks with PageRank |
| **Incremental index** | Droid | Seconds (after first build) | High | Similar to Aider but with incremental updates as files change |
| **Embedding index** | Ante, ForgeCode (optional) | Minutes | High for semantic queries | Generates vector embeddings for code chunks, enables semantic search |
| **MCP-based** | Goose | Varies | Varies | Delegates indexing to external MCP servers (e.g., Sourcegraph) |
| **IDE-based** | Junie CLI, Warp (partial) | Seconds (leverages IDE cache) | Highest | Full IDE indexing with type information, symbol tables, project model |
| **Shared memory** | TongAgents | Runtime | Medium | Multi-agent shared memory for accumulated knowledge |

**Analysis:** The indexing landscape is bimodal — agents either do no indexing at all (relying on on-demand search) or invest in sophisticated indexing systems. There's no middle ground of "lightweight indexing" — an opportunity for agents wanting better performance without the full complexity of Aider's approach.

### Search Strategies

| Agent | Primary Search | Secondary Search | Search Intelligence |
|---|---|---|---|
| **Claude Code** | ripgrep (Grep tool) | glob (ListFiles tool) | LLM formulates queries, interprets results |
| **Codex** | ripgrep (shell) | find (shell) | LLM uses shell directly |
| **Aider** | ripgrep | Repo map (pre-computed) | Repo map guides search; LLM rarely needs to search |
| **ForgeCode** | ripgrep | Semantic search (embeddings) | Hybrid text + semantic search |
| **OpenHands** | ripgrep | find | LLM formulates queries via shell |
| **Gemini CLI** | Shell-based (grep/find/rg) | None | LLM uses shell commands |
| **OpenCode** | Integrated ripgrep | None | Programmatic ripgrep integration |
| **Goose** | ripgrep | MCP search tools | Extensible via MCP servers |
| **Warp** | ripgrep | IDE search | Leverages IDE search capabilities |
| **Ante** | Semantic search | ripgrep (fallback) | Embedding-first, text-fallback |
| **Droid** | ripgrep + AST search | Indexed symbol search | Multi-modal search combining text and structure |
| **Junie CLI** | JetBrains structural search | JetBrains text search | Full structural search with type awareness |
| **mini-SWE-agent** | grep | find | Simple text search |
| **Pi Coding Agent** | ripgrep | None | Basic text search |
| **Sage Agent** | Basic search tools | None | Minimal search capability |
| **TongAgents** | Distributed agent search | Shared memory lookup | Agents coordinate search across codebase |
| **Capy** | Basic search | None | Minimal search |

**Key insight:** Nearly every agent uses ripgrep as its primary search tool. The differentiation comes from what *wraps* ripgrep — Claude Code's Grep tool abstraction, Aider's repo map that reduces the need for search, ForgeCode's semantic search layer.

### Static Analysis Depth

```
None                    Surface              Structural           Semantic              Full
│                         │                      │                    │                   │
mini-SWE,                OpenCode             Aider               Droid                Junie
Codex,                   (optional)           (tree-sitter        (tree-sitter          CLI
OpenHands,                                    tags,               + incremental         (JetBrains
Gemini CLI,                                   PageRank)           + partial LSP)        PSI,
Warp,                                                                                   full type
Pi, Sage,                                     Claude Code         Ante                  analysis)
Capy                                          (tree-sitter        (tree-sitter +
                                              View)               embeddings)
                                              ForgeCode
                                              (AST entry
                                              points)
```

**Analysis by level:**

| Level | What's Understood | Agents | Limitations |
|---|---|---|---|
| **None** | Text only — no structural awareness | 7 agents | Can't distinguish definitions from references, comments from code |
| **Surface** | Basic AST — knows what nodes exist | OpenCode | Can parse but doesn't build relationships |
| **Structural** | Definitions + references + graph | Aider, Claude Code, ForgeCode, Ante | No type information, no semantic understanding |
| **Semantic** | Types + relationships + incremental | Droid | Partial — not full compiler-grade |
| **Full** | Complete compiler model | Junie CLI | Requires IDE platform; not portable |

### LSP Integration

| Agent | LSP Status | What's Used | Limitations |
|---|---|---|---|
| **Junie CLI** | Full | Go-to-def, references, diagnostics, refactoring, completions | Tied to JetBrains platform |
| **OpenCode** | Partial | Go-to-def, diagnostics for Go | Go-only, limited language support |
| **Claude Code** | Diagnostics only | Reads compiler/linter output after edits | Reactive, not proactive |
| **Droid** | Partial | Some diagnostic integration | Limited scope |
| **Warp** | Via IDE | Inherits IDE's LSP when embedded | Only when used within IDE |
| **Goose** | Via extensions | Possible through MCP servers | Depends on extension availability |
| **All others** | None | — | Major opportunity gap |

**The LSP gap is significant.** 11 of 17 agents have zero LSP integration. The agents with LSP (Junie CLI, OpenCode) demonstrate dramatically better code understanding for their supported languages.

### Git Integration Depth

| Agent | Status | Diff | Blame | Log | Auto-commit | Branch Context |
|---|---|---|---|---|---|---|
| **Aider** | Medium-High | ✅ | ❌ | ✅ | ✅ (auto) | ❌ |
| **Claude Code** | Medium | ✅ | ✅ (tool) | ✅ (tool) | ❌ | ❌ |
| **Codex** | Medium | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Droid** | High | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Junie CLI** | High | ✅ | ✅ | ✅ | ✅ | ✅ |
| **ForgeCode** | Medium | ✅ | ❌ | ❌ | ❌ | ❌ |
| **OpenHands** | Medium | ✅ | ❌ | ❌ | ✅ | ❌ |
| **Gemini CLI** | Low-Medium | ✅ | ❌ | ❌ | ❌ | ❌ |
| **OpenCode** | Low-Medium | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Goose** | Low-Medium | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Warp** | Low-Medium | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Ante** | Medium | ✅ | ✅ | ✅ | ❌ | ❌ |
| **mini-SWE-agent** | Low | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Pi Coding Agent** | Low | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Sage Agent** | Minimal | ❌ | ❌ | ❌ | ❌ | ❌ |
| **TongAgents** | Agent-mediated | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Capy** | Low | ✅ | ❌ | ❌ | ❌ | ❌ |

**Analysis:** Every agent uses `git diff` at minimum, but only Aider uses auto-commit (its most distinctive git feature). Blame and log are available to agents that expose shell access but are rarely used systematically.

### Project Detection

| Agent | Instruction File | Language Detection | Framework Detection | Build Detection | Test Detection |
|---|---|---|---|---|---|
| **Claude Code** | CLAUDE.md | ✅ | ✅ | ✅ | ✅ |
| **Codex** | AGENTS.md, CODEX.md | ✅ | Partial | ✅ | ✅ |
| **Aider** | .aider.conf.yml | ✅ (extensions) | ❌ | ❌ | ❌ |
| **ForgeCode** | Project config | ✅ | ✅ | ✅ | ✅ |
| **OpenHands** | None standard | ✅ (runtime) | Partial | Partial | Partial |
| **Gemini CLI** | GEMINI.md | ✅ | Partial | Partial | Partial |
| **OpenCode** | None | ✅ (Go-focused) | Go only | Go only | Go only |
| **Goose** | Via extensions | ✅ | Via extensions | Via extensions | Via extensions |
| **Warp** | None | ✅ | Via IDE | Via IDE | Via IDE |
| **Ante** | Project config | ✅ | ✅ | ✅ | ✅ |
| **Droid** | Project config | ✅ | ✅ | ✅ | ✅ |
| **Junie CLI** | JetBrains project | ✅ | ✅ | ✅ | ✅ |
| **mini-SWE-agent** | None | ✅ (basic) | ❌ | ❌ | ❌ |
| **Pi Coding Agent** | None | ✅ (basic) | ❌ | Partial | Partial |
| **Sage Agent** | None | Minimal | ❌ | ❌ | ❌ |
| **TongAgents** | Agent-defined | Agent-mediated | Agent-mediated | Agent-mediated | Agent-mediated |
| **Capy** | None | ✅ (basic) | ❌ | ❌ | ❌ |

**Convergence pattern:** The CLAUDE.md / AGENTS.md / CODEX.md pattern is becoming the standard. Agents are converging on explicit instruction files rather than trying to auto-detect everything.

---

## Tier Analysis

### Tier 1: Sophisticated Code Understanding

**Agents:** Claude Code, Codex, ForgeCode, Ante, Droid, OpenHands

These agents have invested significantly in code understanding, though their approaches differ:

- **Claude Code** relies on iterative search-read loops with LLM intelligence driving discovery. Its code understanding is "emergent" — the model figures out what to look at.
- **ForgeCode** takes a hybrid approach with embedding-based search and entry-point detection.
- **Ante** combines tree-sitter analysis with embedding indexes for semantic search.
- **Droid** has the deepest built-in analysis with incremental indexing and partial LSP.
- **Codex** keeps it simple — user-directed file selection with powerful search when needed.
- **OpenHands** relies on the model's ability to use shell tools effectively.

### Tier 2: Moderate Code Understanding

**Agents:** Aider, Gemini CLI, OpenCode, Goose, Warp, Junie CLI, mini-SWE-agent, Pi Coding Agent

Despite being "Tier 2" agents in the overall classification, some have excellent code understanding:

- **Aider** has the best indexing system of any agent (repo map with PageRank), arguably the most sophisticated code understanding feature in the entire ecosystem.
- **Junie CLI** has the deepest code understanding of any agent through JetBrains IDE integration, but it's tied to the JetBrains platform.
- **OpenCode** shows what focused LSP integration looks like (for Go).
- **Goose** demonstrates the MCP extensibility model — code understanding via external servers.
- **Gemini CLI** compensates for minimal indexing with massive context windows (1M tokens).

### Tier 3: Minimal Code Understanding

**Agents:** Sage Agent, TongAgents, Capy

These agents have minimal built-in code understanding:
- **TongAgents** compensates with multi-agent coordination
- **Sage Agent** and **Capy** rely almost entirely on the LLM's intrinsic ability

---

## Capability Heat Map

```
                  Indexing  Search  Static   LSP    Git    Project
                                   Analysis        Integ  Detection
Claude Code       ░░░░░░  ████░░  ██░░░░  █░░░░░  ███░░░  █████░
Codex             ░░░░░░  ████░░  ░░░░░░  ░░░░░░  ███░░░  █████░
Aider             ██████  ███░░░  █████░  ░░░░░░  ████░░  ██░░░░
ForgeCode         ████░░  █████░  ███░░░  ░░░░░░  ███░░░  █████░
OpenHands         ░░░░░░  ███░░░  ░░░░░░  ░░░░░░  ███░░░  ██░░░░
Gemini CLI        ░░░░░░  ██░░░░  ░░░░░░  ░░░░░░  ██░░░░  ███░░░
OpenCode          ░░░░░░  ███░░░  █░░░░░  ██░░░░  ██░░░░  ██░░░░
Goose             ██░░░░  ███░░░  █░░░░░  █░░░░░  ██░░░░  ██░░░░
Warp              ░░░░░░  ███░░░  ░░░░░░  ██░░░░  ██░░░░  ██░░░░
Ante              ████░░  █████░  ███░░░  ░░░░░░  ███░░░  ████░░
Droid             █████░  █████░  █████░  ██░░░░  █████░  █████░
Junie CLI         ██████  ██████  ██████  ██████  █████░  ██████
mini-SWE-agent    ░░░░░░  █░░░░░  ░░░░░░  ░░░░░░  █░░░░░  █░░░░░
Pi Coding Agent   ░░░░░░  ██░░░░  ░░░░░░  ░░░░░░  █░░░░░  █░░░░░
Sage Agent        ░░░░░░  █░░░░░  ░░░░░░  ░░░░░░  ░░░░░░  ░░░░░░
TongAgents        ██░░░░  ██░░░░  █░░░░░  ░░░░░░  █░░░░░  █░░░░░
Capy              ░░░░░░  █░░░░░  ░░░░░░  ░░░░░░  █░░░░░  █░░░░░

Legend: ██████ = Full  ████░░ = High  ██░░░░ = Medium  ░░░░░░ = None
```

---

## Strengths and Gaps by Agent

### Claude Code
- **Strength**: Iterative discovery loop — the model is excellent at formulating search queries and narrowing focus
- **Strength**: CLAUDE.md instruction files for project context
- **Gap**: No indexing — every task starts from scratch
- **Gap**: No proactive LSP usage

### Codex
- **Strength**: Simplicity — user-directed context keeps things predictable
- **Strength**: AGENTS.md/CODEX.md hierarchical instruction files
- **Gap**: No indexing, no static analysis
- **Gap**: Heavy reliance on user to provide context

### Aider
- **Strength**: Repo map is the most sophisticated indexing in any CLI agent
- **Strength**: Auto-commit provides safety and clear change tracking
- **Gap**: No LSP integration
- **Gap**: No semantic search (text-only via ripgrep)
- **Gap**: Limited framework/build system detection

### ForgeCode
- **Strength**: Hybrid approach — combines multiple search strategies
- **Strength**: Entry-point detection for understanding codebase architecture
- **Gap**: Embedding search adds startup latency
- **Gap**: No LSP integration

### Junie CLI
- **Strength**: Deepest code understanding of any agent (full JetBrains platform)
- **Strength**: Complete LSP integration, structural search, refactoring
- **Gap**: Tied to JetBrains ecosystem — not portable
- **Gap**: IDE overhead — heavier than pure CLI agents

---

## Patterns and Trends

### 1. The Indexing Spectrum
Agents are distributed across an indexing spectrum from "no index, pure search" to "full index, guided navigation." The trend is moving toward the middle — lightweight indexes that provide structural awareness without heavy startup costs.

### 2. Search Convergence
Despite different architectures, every agent converges on ripgrep as the search backbone. The differentiation is in the search strategy layer above ripgrep, not the search tool itself.

### 3. LSP as the Frontier
LSP integration is the clearest next frontier for CLI agents. Junie CLI and OpenCode prove the value; the challenge is making it work with CLI agents' instant-startup expectation.

### 4. Project Instruction Files
The CLAUDE.md / AGENTS.md pattern is the most impactful recent innovation. It bypasses the detection problem entirely — users tell agents what they need to know.

### 5. Compensating Strategies
Agents without sophisticated code understanding compensate in different ways:
- **Claude Code**: Relies on powerful LLM intelligence for search
- **Gemini CLI**: Relies on massive context window (1M tokens)
- **Codex**: Relies on user to provide context
- **TongAgents**: Relies on multi-agent coordination

---

## Recommendations

### For Agent Developers Building New Agents

1. **Start with ripgrep + tree-sitter.** These two tools provide the best cost/benefit ratio for code understanding.
2. **Add a repo map (Aider-style).** Tree-sitter tags + graph ranking dramatically improves the LLM's ability to navigate codebases.
3. **Support project instruction files.** Reading CLAUDE.md/AGENTS.md is trivial to implement and provides enormous value.
4. **Invest in LSP integration.** Start with diagnostics (lowest effort, high value), then add go-to-definition.
5. **Use git signals for prioritization.** Recently modified files, co-changed files, and branch context should influence search ranking.

### For Existing Agents

| Agent | Highest-Impact Addition |
|---|---|
| Claude Code | Add lightweight indexing (tree-sitter tags + graph) |
| Codex | Add tree-sitter-based repo map |
| Aider | Add LSP diagnostics for edit verification |
| ForgeCode | Add LSP for supported languages |
| OpenHands | Add any form of static analysis |
| Gemini CLI | Add tree-sitter-based file summarization |
| mini-SWE-agent | Add ripgrep (replace grep) |

---

## Key Takeaways

1. **Code understanding capability is bimodal.** Agents either invest heavily in it or barely invest at all. There's a large opportunity in the middle ground.

2. **Aider's repo map is the most influential innovation** in CLI agent code understanding. Its combination of tree-sitter, graph construction, and PageRank ranking should be replicated by other agents.

3. **LSP is the biggest untapped opportunity.** The gap between tree-sitter-level understanding and compiler-grade understanding is enormous, and LSP bridges it.

4. **The best code understanding is multi-layered.** No single technique is sufficient. The best agents combine indexing, search, static analysis, git context, and project detection.

5. **Simpler approaches can compensate** — Claude Code's search-first approach with powerful LLMs, and Gemini CLI's massive context windows, show that sophisticated indexing isn't always necessary. But as tasks get more complex, code understanding quality becomes the bottleneck.

---

## Appendix A: Feature Availability Matrix

A quick-reference binary matrix showing feature presence across all agents:

```
Feature                  CC  CX  AI  FC  OH  GC  OC  GO  WA  AN  DR  JU  MS  PI  SA  TA  CA
──────────────────────── ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──
Tree-sitter parsing      ✓   .   ✓   ✓   .   .   ✓   .   .   ✓   ✓   .   .   .   .   .   .
Tag extraction           .   .   ✓   ✓   .   .   .   .   .   ✓   ✓   ✓   .   .   .   .   .
Graph-based indexing     .   .   ✓   .   .   .   .   .   .   .   ✓   ✓   .   .   .   .   .
Embedding index          .   .   .   ✓   .   .   .   .   .   ✓   .   .   .   .   .   .   .
Ripgrep search           ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   .   .   ✓   .   .   .
AST-based search         .   .   .   .   .   .   .   .   .   .   ✓   ✓   .   .   .   .   .
Semantic search          .   .   .   ✓   .   .   .   .   .   ✓   .   .   .   .   .   .   .
LSP go-to-definition     .   .   .   .   .   .   ✓   .   ✓   .   ✓   ✓   .   .   .   .   .
LSP find-references      .   .   .   .   .   .   .   .   ✓   .   .   ✓   .   .   .   .   .
LSP diagnostics          ✓   .   .   .   .   .   ✓   .   ✓   .   ✓   ✓   .   .   .   .   .
Git diff                 ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   .   ✓   ✓
Git blame                ✓   .   .   .   .   .   .   .   .   ✓   ✓   ✓   .   .   .   .   .
Git log                  ✓   .   ✓   .   .   .   .   .   .   ✓   ✓   ✓   .   .   .   .   .
Git auto-commit          .   .   ✓   .   ✓   .   .   .   .   .   ✓   ✓   .   .   .   .   .
Instruction files        ✓   ✓   .   ✓   .   ✓   .   .   .   ✓   ✓   ✓   .   .   .   .   .
Language detection       ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   .   ✓   ✓
Framework detection      ✓   .   .   ✓   .   .   .   .   .   ✓   ✓   ✓   .   .   .   .   .
Build system detection   ✓   ✓   .   ✓   .   .   .   .   .   ✓   ✓   ✓   .   .   .   .   .
Test framework detection ✓   ✓   .   ✓   .   .   .   .   .   ✓   ✓   ✓   .   .   .   .   .
Monorepo support         ✓   ✓   .   ✓   .   .   .   .   .   .   ✓   ✓   .   .   .   .   .
──────────────────────── ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──  ──
Feature count            10   6  7   11  3   3   5   2   5  11  15  15  1   2   0   2   1

Legend: CC=Claude Code, CX=Codex, AI=Aider, FC=ForgeCode, OH=OpenHands, GC=Gemini CLI,
        OC=OpenCode, GO=Goose, WA=Warp, AN=Ante, DR=Droid, JU=Junie CLI, MS=mini-SWE,
        PI=Pi Coding, SA=Sage, TA=TongAgents, CA=Capy
```

---

## Appendix B: Evolution Timeline

A timeline of code understanding capabilities in CLI coding agents:

| Period | Milestone | Impact |
|---|---|---|
| **2023 Q1** | Aider introduces tree-sitter repo map | First agent with structural code understanding |
| **2023 Q3** | Aider adds PageRank ranking | Repo map becomes context-efficient |
| **2024 Q1** | Claude Code launches with Grep/ListFiles tools | Sets the pattern for search-based understanding |
| **2024 Q2** | Codex introduces AGENTS.md / CODEX.md | Establishes project instruction file pattern |
| **2024 Q3** | Claude Code adds CLAUDE.md support | Project instruction files become standard |
| **2024 Q3** | Cody adds code graph context | Structural awareness in IDE agents |
| **2024 Q4** | ForgeCode adds embedding search | First CLI agent with semantic search |
| **2024 Q4** | Ante adds embedding index | Second CLI agent with semantic search |
| **2025 Q1** | Droid adds incremental indexing + partial LSP | Most comprehensive CLI agent code understanding |
| **2025 Q1** | Gemini CLI launches with massive context | Proves context window can compensate for indexing |
| **2025 Q2** | Junie CLI launches with full JetBrains integration | Deepest code understanding of any agent |

---

## Appendix C: Architectural Patterns

Three dominant patterns have emerged for how agents organize their code understanding:

### Pattern 1: Search-First (Claude Code, Codex, OpenHands)

```
User Task → LLM formulates search → ripgrep/glob → LLM reads results →
  → Decides: search more or start editing → Iterates until confident
```

**Characteristics:**
- No startup cost (no index to build)
- Quality depends on LLM's search formulation ability
- Works well for small-medium codebases
- Degrades on large codebases (too many search results)

### Pattern 2: Index-First (Aider, Droid, Junie CLI)

```
Project opened → Build index (tree-sitter/IDE) → 
  User Task → Consult index → Navigate directly to relevant code → Edit
```

**Characteristics:**
- Startup cost (seconds to minutes)
- Quality depends on index comprehensiveness
- Works well for all codebase sizes
- Requires index maintenance (invalidation, updates)

### Pattern 3: Context-Window-First (Gemini CLI)

```
User Task → Load maximum code into context window → 
  LLM processes everything at once → Edit
```

**Characteristics:**
- No startup cost
- Quality depends on context window size and model attention
- Works well when context window > codebase size
- Degrades when codebase exceeds context window
- Highest token cost per task

### Pattern 4: Hybrid (ForgeCode, Ante)

```
Project opened → Build lightweight index → 
  User Task → Semantic search + ripgrep → LLM combines results → Edit
```

**Characteristics:**
- Moderate startup cost
- Combines structural and semantic understanding
- Most flexible but most complex to implement
- Best theoretical quality ceiling

---

## Appendix D: Open Research Questions

1. **What is the minimum viable index?** Aider's repo map uses tree-sitter tags + PageRank. Could a simpler index (e.g., just function names and file paths) provide 80% of the value?

2. **Can LLMs learn to use LSP?** If an agent provides go-to-definition and find-references as tools, will the LLM learn to use them effectively? Early evidence from Claude Code's diagnostic reading suggests yes.

3. **Is embedding search worth the cost for CLI agents?** ForgeCode and Ante use it, but the startup latency conflicts with CLI expectations. Could streaming/progressive indexing solve this?

4. **How should agents handle polyglot projects?** Real projects often mix 3-5 languages. Running multiple language servers is expensive. What's the optimal strategy?

5. **Can code understanding transfer across sessions?** Aider rebuilds its repo map each session. Could persistent, incrementally-updated indexes provide better understanding over time?

6. **What's the relationship between context window size and code understanding need?** As context windows grow (1M, 10M tokens), does the need for indexing diminish? Or does the need shift from "what to include" to "what to prioritize"?

7. **Can agents build their own code understanding?** Rather than pre-programmed analysis, could agents learn to analyze codebases through observation — reading existing patterns, inferring conventions, building their own mental models?