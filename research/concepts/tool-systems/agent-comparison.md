---
title: "Agent Comparison"
---

# Agent Comparison — Tool Systems Across Coding Agents

Cross-agent comparison of tool system architectures, sandboxing, permissions, edit strategies, and capabilities.

This document surveys 17 coding agents — from minimal single-tool designs to 30+ tool catalogs — analyzing how each structures its tool system, manages permissions, isolates execution, and handles file edits. The goal is to identify which patterns produce the best outcomes and where the industry is converging.

---

## 1. Tool Count Comparison

The number of tools an agent exposes to the LLM varies dramatically — from zero (Aider) to 30+ (Junie CLI). More tools provide finer control but risk overwhelming the model's context and increasing selection errors.

| Agent | Tool Count | Notable Tools | Tool Source |
|-------|-----------|---------------|-------------|
| Junie CLI | 30+ | Full IDE integration, file ops, terminal, search, refactor, debug | Static + IDE bridge |
| Claude Code | 27 | Read, Write, Edit, MultiEdit, Bash, WebFetch, TodoRead/Write, Notebook, MCP | Static registry |
| Warp | 20+ | File ops, shell, LSP queries, web search, Computer Use, PTY control | Static + dynamic |
| Gemini CLI | 18+ | Shell, file editing, web search, memory read/write, Vertex extensions | Static + discovery |
| OpenCode | 14 | File read/write/edit, bash, LSP diagnostics, glob, grep, fetch, think | Interface registry |
| Droid | 12+ | GitHub Actions repair, PR ops, code search, enterprise integrations | Platform-native |
| Sage-Agent | 10+ | ToolManager dispatch, MCP integration, code analysis, execution | Manager + MCP |
| OpenHands | 9 | bash, str_replace_editor, browser_action, think, finish, delegate | Action classes |
| Ante | 8+ | Rust-native tools, custom MCP SDK, file ops, shell | MCP SDK |
| ForgeCode | 6 | bash, str_replace_editor, view, write, glob, grep | Static |
| Goose | Dynamic | All tools sourced from MCP servers — count depends on config | MCP-native |
| Pi (π-coding-agent) | 4 | bash, read, write, web_search | Extensible core |
| TongAgents | 3–4 | Minimal per-agent tool set, shared via multi-agent coordination | Per-agent static |
| Capy | 3–4 | Shell, file editor, browser (inside Ubuntu VM) | VM-scoped |
| mini-SWE-agent | 1 | bash only — all file ops via shell commands | Single tool |
| Aider | 0 | No function calling — uses 6 text-based edit formats instead | Edit formats |
| Codex CLI | 5–6 | Shell (primary), file read, apply patch — sandbox is the tool system | Shell + sandbox |

### Analysis

The data reveals a bimodal distribution: agents cluster around either 4–6 tools (minimalist) or 18–30 tools (comprehensive). The mid-range (8–14) is sparsely populated.

**More tools ≠ better performance.** Mini-SWE-agent with a single bash tool and Aider with zero function-call tools both achieve competitive SWE-bench scores. The critical factor is not tool count but tool design:

- **Diminishing returns above ~15 tools**: Models start confusing similar tools (e.g., Write vs. Edit vs. MultiEdit). Claude Code mitigates this with tool search — the model queries available tools rather than holding all 27 in context.
- **The "4-tool sweet spot"**: Pi, ForgeCode, and Capy demonstrate that bash + read + write + one extra (search, glob, or browser) covers the vast majority of coding tasks.
- **Zero-tool viability**: Aider proves that structured text output can replace function calling entirely, with benchmarks showing edit formats sometimes outperform tool-based approaches.

---

## 2. Architecture Pattern Per Agent

Each agent's tool system reflects a fundamental architectural choice about how tool definitions, dispatch, and execution are organized.

| Agent | Pattern | Language | Key Characteristic |
|-------|---------|----------|-------------------|
| Codex CLI | Router + Registry + Sandbox Pipeline | Rust | Enum-level tool type distinction (Shell vs. File) |
| Goose | MCP-Native Federation | Rust | Everything is an MCP server — no built-in tools |
| OpenHands | Action/Observation Event Stream | Python | Tools are Action subclasses; results are Observations |
| OpenCode | Trait/Interface + Linear Registry | Go | `Tool` interface with `Run(ctx, params)` dispatch |
| Claude Code | Registry + Permission Categories | TypeScript | Tool search for large catalogs, category-based gating |
| Gemini CLI | Registry + Dynamic Discovery | TypeScript | Static tools + runtime extension discovery |
| Aider | Edit Format Protocol System | Python | No tool dispatch — model outputs structured text |
| mini-SWE-agent | Single-Tool Delegation | Python | All complexity delegated to bash; agent is thin |
| ForgeCode | Registry + Correction Layer | Python | Post-hoc tool-call validation and repair |
| Pi | Minimal Extensible Core | Python | 4 base tools + plugin architecture |
| Ante | Custom MCP SDK | Rust | Native Rust MCP implementation, no wrapper layers |
| Warp | Integrated Tool + PTY Pipeline | Proprietary | Terminal-native with PTY capture and LSP integration |
| Capy | Hard Agent-Tool Boundary | Python | Tools exist only inside Ubuntu VM; agent has no host access |
| Junie CLI | IDE Bridge + CLI Dual-Mode | Kotlin | Same tools accessible from IDE plugin or standalone CLI |
| Droid | Enterprise Platform Tools | TypeScript | Tools tightly integrated with GitHub platform APIs |
| Sage-Agent | ToolManager + MCP Bridge | Python | Centralized ToolManager class dispatches to MCP or local |
| TongAgents | Per-Agent Minimal Tools | Python | Each sub-agent gets only the tools relevant to its role |

### Pattern Categories

**Registry-based (Claude Code, Gemini CLI, OpenCode, Junie CLI):** Tools are registered at startup in a central catalog. The LLM receives tool schemas and picks which to call. This is the most common pattern because it maps directly to the OpenAI function-calling API.

**Action/Observation (OpenHands):** Tools are modeled as actions in an event stream. The agent emits an Action; the runtime executes it and returns an Observation. This decouples tool definition from execution and enables serialization, replay, and distributed execution.

**MCP-Native (Goose, Ante, Sage-Agent):** Tools are defined by external MCP servers. The agent is a thin orchestrator that discovers and invokes tools over the Model Context Protocol. This maximizes extensibility but introduces network latency and server management complexity.

**Edit Format (Aider):** The model outputs structured text (unified diff, search/replace blocks, whole-file rewrites) rather than making function calls. A parser extracts edits and applies them. This avoids function-calling overhead and works with models that lack tool-use training.

**Shell-Centric (Codex CLI, mini-SWE-agent):** The primary (or only) tool is a shell. All file operations, builds, tests, and searches happen through bash commands. The agent's value comes from sandbox safety (Codex) or prompt engineering (mini-SWE-agent).

---

## 3. Sandboxing Approach Per Agent

Sandboxing determines how much damage a misbehaving tool call can inflict. The spectrum ranges from full VM isolation to no protection at all.

| Agent | Sandbox Type | Isolation Level | Platform Support | Overhead | Network Access |
|-------|-------------|----------------|-----------------|----------|----------------|
| Codex CLI | 3-layer (bwrap + seccomp + Landlock) | Very High | Linux native, macOS/Windows partial | Low | Blocked by default |
| Capy | Full Ubuntu VM | Very High | Cloud-only | High | Controlled |
| OpenHands | Docker container per session | High | Any Docker host | Medium | Configurable |
| Gemini CLI | Multi-tier (Seatbelt on macOS, Docker optional) | Medium–High | macOS native, Linux via Docker | Low–Medium | Allowed |
| Junie CLI | JVM sandbox + IDE process isolation | Medium | Any JVM platform | Low | Allowed |
| Droid | GitHub Actions runner isolation | Medium | GitHub-hosted | Medium | GitHub scoped |
| Claude Code | None (direct host execution) | None | Any | None | Full |
| OpenCode | None (direct host execution) | None | Any | None | Full |
| Goose | None (direct host execution) | None | Any | None | Full |
| Aider | None (direct host execution) | None | Any | None | Full |
| ForgeCode | None (direct host execution) | None | Any | None | Full |
| Pi | None (direct host execution) | None | Any | None | Full |
| Ante | None (direct host execution) | None | Any | None | Full |
| Warp | Terminal sandbox (PTY isolation) | Low | macOS, Linux | Low | Full |
| mini-SWE-agent | External (Docker when used in benchmarks) | Depends | Any | Depends | Depends |
| Sage-Agent | None (direct host execution) | None | Any | None | Full |
| TongAgents | None (direct host execution) | None | Any | None | Full |

### The Sandboxing Spectrum

**Full OS Isolation (Codex CLI):** Codex CLI implements the most sophisticated sandbox in the ecosystem. Three nested layers work together:
1. **bubblewrap (bwrap):** Linux namespace isolation — the process sees a restricted filesystem, PID space, and network.
2. **seccomp:** System call filtering — blocks dangerous syscalls like `mount`, `reboot`, `ptrace`.
3. **Landlock:** Filesystem access control — even within the namespace, only whitelisted paths are writable.

On macOS, Codex falls back to Apple's Seatbelt sandbox (the same technology used for App Store apps). The overhead is minimal because all three mechanisms are kernel-level, not virtualization.

**Container Isolation (OpenHands):** Docker provides strong isolation with moderate overhead. Each coding session runs in a fresh container with mounted workspace. The main trade-off is startup time (~2–5 seconds) and the requirement for Docker to be installed.

**VM Isolation (Capy):** The maximum isolation — a full Ubuntu VM means the agent literally cannot affect the host. This is ideal for untrusted code execution but imposes significant overhead (VM boot time, resource allocation, snapshot management).

**No Isolation (most agents):** The majority of agents — including Claude Code, Goose, OpenCode, and Aider — run tools directly on the host with the user's permissions. They rely on permission prompts (see Section 4) rather than OS-level sandboxing. This is a conscious trade-off: zero overhead and full capability at the cost of trusting the model.

---

## 4. Permission Model Per Agent

Permission systems determine when the agent must ask the user before acting. They range from "ask for everything" to "auto-approve all."

| Agent | Model Type | Modes/Levels | Granularity | Config Format |
|-------|-----------|-------------|-------------|---------------|
| Claude Code | 5 permission modes | Suggest, Ask, Auto-edit, Auto-bash, Full auto | Per-tool-category | CLI flags + settings |
| Junie CLI | 4-level permissions | Ask all, Ask destructive, Ask external, Trust all | Per-action-type | Config file |
| Codex CLI | 3 execution policies | Suggest, Auto-edit, Full auto | Per-policy | CLI flag |
| Gemini CLI | 3 safety levels | Conservative, balanced, permissive | Per-tool-type | CLI flag + config |
| OpenCode | Allowlist system | Explicit approval for dangerous commands | Per-command-pattern | Config file (regex) |
| Goose | 4-tier inspection | Pre/post inspect, approve, deny | Per-MCP-server | MCP config |
| OpenHands | Confirmation prompts | Confirm destructive actions | Per-action-class | Runtime config |
| Capy | VM boundary | Everything allowed inside VM, nothing outside | Binary (in/out) | Architecture |
| Warp | Smart prompting | Context-aware approval requests | Per-risk-level | Adaptive |
| Droid | Enterprise policy | Organization-level permissions | Per-repo / per-org | GitHub settings |
| Aider | User confirmation | Ask before applying edits, auto-commit optional | Per-edit-batch | CLI flags |
| ForgeCode | Minimal | Trust model by default | None | None |
| Pi | Minimal | Trust model by default | None | None |
| mini-SWE-agent | None | Full trust (sandboxed externally) | None | None |
| Ante | MCP-level | Per-MCP-server permissions | Per-server | MCP config |
| Sage-Agent | ToolManager | Per-tool approval | Per-tool | Manager config |
| TongAgents | None | Agent coordination handles safety | Per-agent-role | Multi-agent config |

### Permission Design Principles

**The Claude Code approach** is the most granular: 5 distinct modes ranging from "suggest only" (never executes anything) to "full auto" (executes everything without asking). Intermediate modes allow auto-editing files but prompting for bash commands, or auto-running whitelisted commands while prompting for others. This maps well to trust escalation — start cautious, increase autonomy as trust builds.

**Codex CLI's policy engine** is simpler but architecturally elegant. The three policies (suggest, auto-edit, full-auto) are enforced at the sandbox level. In "suggest" mode, the sandbox blocks all writes. In "auto-edit" mode, file writes are allowed but network access remains blocked. In "full-auto" mode, all sandbox restrictions are relaxed. The policy is the sandbox configuration.

**Goose's 4-tier inspection pipeline** is unique: every tool call passes through pre-inspection (before execution), approval (human or policy), execution, and post-inspection (after execution). Post-inspection is particularly novel — it can detect that a tool call succeeded but produced problematic output (e.g., a file write that introduced a syntax error) and flag it for review.

**Capy's architectural permission model** is the most radical: there is no permission system because there doesn't need to be one. The agent runs inside a full VM. It can `rm -rf /` and the only consequence is the VM gets rebuilt. The sandbox IS the permission system.

---

## 5. Edit Strategy Per Agent

How agents modify files is perhaps the most consequential tool design decision. A single wrong edit can break an entire codebase.

| Agent | Edit Approach | Error Recovery | Syntax Awareness | Multi-file |
|-------|-------------- |---------------|-----------------|-----------|
| Aider | 6 text-based edit formats | 4-level fuzzy matching | tree-sitter linting | Yes (repo map) |
| Claude Code | search-and-replace (old_str/new_str) | Retry with error msg | None built-in | Yes (MultiEdit) |
| OpenHands | str_replace_editor (5 operations) | Error as Observation | None built-in | Sequential |
| OpenCode | Line-range editing | LSP diagnostics post-edit | LSP integration | Sequential |
| Gemini CLI | search-and-replace + full rewrite | Retry with context | None built-in | Yes |
| ForgeCode | str_replace (SWE-bench naming) | Correction layer | None built-in | Sequential |
| Codex CLI | Shell-based (sed, patch, heredoc) | Sandbox retry | None built-in | Shell scripts |
| mini-SWE-agent | Shell-based (sed, ed, tee) | Model retry | None built-in | Shell scripts |
| Warp | Direct file write + LSP validation | LSP error detection | LSP | Yes |
| Junie CLI | IDE-native refactoring + direct edit | IDE error detection | Full IDE analysis | Yes (IDE) |
| Pi | File write (whole file) | Model retry | None | Sequential |
| Goose | MCP server dependent | Server dependent | Server dependent | Server dependent |
| Capy | Shell-based (inside VM) | VM snapshot rollback | None | Shell scripts |
| Ante | MCP-based editing | MCP server dependent | None | Server dependent |
| Droid | GitHub API (PR-based edits) | PR revision | GitHub checks | Yes (PR scope) |
| Sage-Agent | Managed file write | ToolManager validation | None | Sequential |
| TongAgents | Per-agent file ops | Agent coordination | None | Multi-agent |

### Edit Format Deep Dive: Aider's 6 Formats

Aider stands alone in offering multiple edit formats, each optimized for different models and tasks:

1. **Whole file**: Model outputs entire file content. Simple but token-expensive for large files.
2. **Unified diff**: Standard `diff -u` format. Token-efficient but models often produce malformed diffs.
3. **Search/replace**: Blocks of `<<<<<<< SEARCH` / `>>>>>>> REPLACE`. The most reliable format.
4. **Editor diff**: Optimized diff format for editor-trained models.
5. **Editor whole**: Hybrid of editor diff and whole file.
6. **Diff-fenced**: Fenced code blocks containing diffs.

Aider's key insight: **function calling is worse than text for edits.** Their benchmarks show that text-based edit formats outperform equivalent function-calling approaches because models can use their natural language abilities to describe changes rather than filling structured schemas.

### The Convergence on Search-and-Replace

Despite different origins, most agents have converged on some variant of search-and-replace:
- **Claude Code**: `old_str` / `new_str` parameters
- **OpenHands**: `str_replace` command within editor tool
- **ForgeCode**: `str_replace_editor` (named to match SWE-bench training data)
- **Aider**: Search/replace blocks in text output
- **Gemini CLI**: Search-and-replace with fallback to full rewrite

This convergence is driven by a fundamental property: search-and-replace requires the model to reproduce the exact existing code before modifying it, which forces the model to verify its understanding of the current state. Whole-file rewrites don't have this property and are prone to accidentally deleting code.

---

## 6. Analysis: What Works Best?

### Trend 1: The "Sweet Spot" for Tool Count Is 4–8

High-performing agents on SWE-bench span the full tool-count range, but the most efficient designs cluster around 4–8 tools. This range provides enough granularity for common operations (read, write, edit, search, execute) without overwhelming the model's tool selection.

Agents with 20+ tools (Claude Code, Junie CLI, Warp) compensate with tool search, categorization, or dynamic loading to keep the active tool set small. The effective tool count — what the model actually considers per turn — is usually under 10 even in large-catalog agents.

### Trend 2: MCP Adoption Is Accelerating

The Model Context Protocol is becoming the standard for tool extensibility:
- **Goose**: MCP-native — all tools are MCP servers
- **Claude Code**: MCP support for external tools alongside built-in catalog
- **Ante**: Custom Rust MCP SDK for native performance
- **Sage-Agent**: ToolManager bridges local tools and MCP servers
- **Gemini CLI**: Extension system compatible with MCP concepts

MCP's appeal is composability: an MCP server written for Goose works with Claude Code works with any MCP-compatible agent. This creates a tool ecosystem independent of any single agent.

### Trend 3: The Sandboxing Spectrum Has Clear Trade-offs

| Approach | Safety | Performance | Capability | Complexity |
|----------|--------|-------------|------------|------------|
| Full VM (Capy) | ★★★★★ | ★★ | ★★★ | ★★★★ |
| OS sandbox (Codex) | ★★★★ | ★★★★ | ★★★ | ★★★★ |
| Container (OpenHands) | ★★★★ | ★★★ | ★★★★ | ★★★ |
| App sandbox (Gemini) | ★★★ | ★★★★ | ★★★★ | ★★ |
| None + permissions | ★★ | ★★★★★ | ★★★★★ | ★ |

Most agents choose "none + permissions" because the development experience is frictionless. Codex CLI's achievement is showing that OS-level sandboxing can be nearly zero-overhead while providing real isolation.

### Trend 4: Permission Systems Are Shifting Toward Configurable Autonomy

Early agents were binary: ask everything or trust everything. Modern agents offer graduated autonomy:
- Start in "ask" mode for new users
- Allow per-command or per-tool-type exceptions
- Support "full auto" for CI/CD and batch operations
- Provide session-level trust escalation

This mirrors the Unix permission model's evolution from simple user/group/other to ACLs and capabilities.

### Trend 5: Edit Tools Are Converging on Search-and-Replace

The industry has largely settled on search-and-replace as the primary edit mechanism because it:
1. Forces the model to demonstrate understanding of existing code
2. Supports surgical edits without rewriting entire files
3. Is naturally idempotent (same search-and-replace applied twice fails gracefully)
4. Maps well to `git diff` for change review

Aider's text-based approach remains a credible alternative, particularly for models without strong function-calling training. The Aider team's data showing function calling underperforms text editing is the most counterintuitive finding in the space.

---

## 7. Key Differentiators Table

For each agent, the single most distinctive aspect of its tool system:

| Agent | Key Differentiator | Why It Matters |
|-------|-------------------|----------------|
| Codex CLI | 3-layer OS sandbox with shell command parser | Only agent with kernel-level tool isolation at near-zero overhead |
| Goose | MCP-native: everything is an MCP server | Proves MCP can be the sole tool abstraction, not just an add-on |
| Aider | Function calling is WORSE than text editing | Challenges the industry assumption that structured tool calls are superior |
| ForgeCode | Tool-call correction layer | Catches and repairs malformed tool calls before execution |
| OpenHands | Action/observation event stream | Enables replay, distributed execution, and trajectory analysis |
| Claude Code | Tool search for 27-tool catalog | Solves tool overflow by letting the model query available tools |
| Gemini CLI | Multi-tier sandboxing with platform adaptation | Seatbelt on macOS, Docker on Linux — native feel on each OS |
| mini-SWE-agent | Competitive with ONE tool (bash only) | Proves agent complexity is often unnecessary overhead |
| OpenCode | LSP integration for edit validation | Post-edit diagnostics catch errors other agents miss |
| Warp | Active AI error monitoring with PTY capture | Terminal-native intelligence — errors detected as they appear |
| Capy | Hard agent-tool boundary via full VM | Most extreme isolation — the VM IS the permission system |
| Junie CLI | IDE + CLI dual-mode with shared tools | Same tool system works in IntelliJ plugin and standalone CLI |
| Pi | 4 extensible tools covering full workflow | Minimal viable tool set that's still production-capable |
| Ante | Custom Rust MCP SDK | Native MCP without wrapper overhead — MCP at systems-language speed |
| Droid | GitHub Actions repair and enterprise integration | Only agent purpose-built for CI/CD pipeline debugging |
| Sage-Agent | ToolManager with MCP bridge | Clean abstraction layer between local tools and MCP ecosystem |
| TongAgents | Per-agent minimal tools with multi-agent coordination | Tool minimalism per agent compensated by agent-level composition |

---

## 8. Cross-Cutting Comparison Matrix

A consolidated view across all dimensions for quick reference:

| Agent | Tools | Sandbox | Permissions | Edit Strategy | Language | MCP |
|-------|-------|---------|-------------|--------------|----------|-----|
| Codex CLI | 5–6 | 3-layer OS | 3 policies | Shell-based | Rust | No |
| Goose | Dynamic | None | 4-tier inspect | MCP-dependent | Rust | Native |
| OpenHands | 9 | Docker | Confirmation | str_replace | Python | No |
| OpenCode | 14 | None | Allowlist | Line-range | Go | No |
| Claude Code | 27 | None | 5 modes | search-replace | TypeScript | Yes |
| Gemini CLI | 18+ | Multi-tier | 3 levels | search-replace | TypeScript | Partial |
| mini-SWE-agent | 1 | External | None | Shell-based | Python | No |
| Aider | 0 | None | Confirm edits | 6 text formats | Python | No |
| ForgeCode | 6 | None | Minimal | str_replace | Python | No |
| Pi | 4 | None | Minimal | File write | Python | No |
| Ante | 8+ | None | MCP-level | MCP-based | Rust | Native |
| Warp | 20+ | PTY | Smart prompts | Direct + LSP | Proprietary | No |
| Capy | 3–4 | Full VM | VM boundary | Shell-based | Python | No |
| Junie CLI | 30+ | JVM | 4 levels | IDE-native | Kotlin | No |
| Droid | 12+ | Actions runner | Enterprise | PR-based | TypeScript | No |
| Sage-Agent | 10+ | None | Per-tool | Managed write | Python | Bridge |
| TongAgents | 3–4 | None | None | Per-agent | Python | No |

---

## 9. Conclusions

**There is no single best tool system design.** The optimal architecture depends on the deployment context:

- **For maximum safety** (enterprise, untrusted code): Codex CLI's OS sandbox or Capy's VM isolation
- **For maximum extensibility** (diverse workflows): Goose's MCP-native approach
- **For maximum simplicity** (quick setup, solo dev): mini-SWE-agent or Pi's minimal tools
- **For maximum edit quality** (code modification focus): Aider's text formats or OpenCode's LSP validation
- **For maximum integration** (IDE users): Junie CLI's dual-mode architecture
- **For maximum autonomy** (CI/CD, batch): Claude Code's full-auto mode with 27 tools

The industry is converging on three key patterns:
1. **Search-and-replace as the primary edit mechanism** — adopted by 10+ of the 17 agents surveyed
2. **MCP as the extensibility standard** — 6 of 17 agents have some MCP support, and growing
3. **Graduated permission systems** — binary ask/trust is being replaced by multi-level autonomy

The most surprising finding remains Aider's: for file editing, structured text output can outperform function calling. This challenges the assumption that tool-use APIs are always superior and suggests the optimal tool system might use function calling for execution (bash, search) but text protocols for code modification.
