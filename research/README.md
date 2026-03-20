# Research

Reference materials and personal notes for the CLI Coding Agent learning platform.

## Organization

### `agents/`
Architecture analysis of 17 CLI coding agents (`agents/<agent-name>/`).

Each agent folder contains 8 standard files:
`README.md` · `architecture.md` · `agentic-loop.md` · `tool-system.md` · `context-management.md` · `unique-patterns.md` · `benchmarks.md` · `references.md`

**Tier 1 — Top benchmark performers & major players:**
| # | Agent | Description |
|---|-------|-------------|
| 1 | **forgecode** | #1 Terminal-Bench 2.0 (81.8%), ZSH-native multi-agent |
| 2 | **claude-code** | Anthropic's CLI coding agent |
| 3 | **codex** | OpenAI's Codex CLI agent (Rust, 3-layer sandbox) |
| 4 | **droid** | Factory.ai enterprise multi-interface agent |
| 5 | **ante** | Antigma Labs, Rust-built self-organizing agent |
| 6 | **opencode** | Open-source Go-based CLI agent |
| 7 | **openhands** | Open-source, formerly OpenDevin, event-driven architecture |

**Tier 2 — Notable & differentiated:**
| # | Agent | Description |
|---|-------|-------------|
| 8 | **warp** | AI-native terminal (Rust+Metal GPU rendering) |
| 9 | **gemini-cli** | Google's first-party terminal agent (1M token context) |
| 10 | **goose** | Block (fka Square), MCP-native extensible agent |
| 11 | **junie-cli** | JetBrains' dual IDE/CLI agent |
| 12 | **mini-swe-agent** | Minimal 100-line bash-only agent from Princeton/Stanford |
| 13 | **pi-coding-agent** | Radically extensible 4-tool agent by Mario Zechner |
| 14 | **aider** | Pioneering AI pair programming tool, code-editing benchmark |

**Tier 3 — Emerging / leaderboard notable:**
| # | Agent | Description |
|---|-------|-------------|
| 15 | **sage-agent** | OpenSage multi-agent pipeline |
| 16 | **tongagents** | BIGAI (Beijing), top-3 on Terminal-Bench 2.0 |
| 17 | **capy** | Cloud IDE with two-agent split |

### `concepts/`
Deep-dive concept notes organized into topic folders:

#### `context-management/`
Token counting, compaction, and session management.
`README.md` · `the-problem.md` · `token-counting.md` · `compaction-strategies.md` · `summarization.md` · `sliding-window.md` · `repo-map.md` · `code-search-and-retrieval.md` · `memory-systems.md` · `session-persistence.md` · `multi-agent-context.md` · `tools-and-projects.md` · `agent-comparison.md`

#### `agentic-loop/`
The core loop pattern, orchestration, and evaluation.
`README.md` · `the-react-pattern.md` · `simple-loops.md` · `streaming-loops.md` · `event-driven-loops.md` · `message-passing-loops.md` · `multi-agent-orchestration.md` · `edit-apply-verify.md` · `state-management.md` · `stop-conditions.md` · `error-recovery.md` · `agent-frameworks.md` · `benchmarks-and-evaluation.md` · `observability.md` · `agent-comparison.md`

#### `tool-systems/`
Tool registration, dispatch, execution, and safety.
`README.md` · `design-patterns.md` · `json-schema.md` · `mcp-deep-dive.md` · `sandboxing.md` · `execution-models.md` · `permission-systems.md` · `file-editing-tools.md` · `bash-and-shell.md` · `error-handling.md` · `safety-and-guardrails.md` · `tools-and-projects.md` · `agent-comparison.md`

#### `streaming/`
SSE, chunked transfer, incremental rendering, and multimodal.
`README.md` · `protocols.md` · `openai-streaming.md` · `anthropic-streaming.md` · `google-streaming.md` · `incremental-parsing.md` · `tui-frameworks.md` · `terminal-rendering.md` · `gpu-rendering.md` · `error-recovery.md` · `voice-and-multimodal.md` · `tools-and-projects.md` · `agent-comparison.md`

#### `prompt-engineering/`
System prompts, tool descriptions, chain-of-thought, prompt caching, structured output.
`README.md` · `system-prompts.md` · `tool-descriptions.md` · `chain-of-thought.md` · `few-shot-examples.md` · `prompt-caching.md` · `structured-output.md` · `model-specific-tuning.md` · `tools-and-projects.md` · `agent-comparison.md`

#### `model-providers/`
OpenAI, Anthropic, Google, DeepSeek, open-source, model routing, LiteLLM, pricing.
`README.md` · `openai.md` · `anthropic.md` · `google.md` · `deepseek.md` · `open-source-models.md` · `litellm.md` · `model-routing.md` · `pricing-and-cost.md` · `api-patterns.md` · `agent-comparison.md`

#### `human-in-the-loop/`
Permission prompts, plan-and-confirm, trust levels, feedback loops, UX patterns.
`README.md` · `permission-prompts.md` · `plan-and-confirm.md` · `trust-levels.md` · `feedback-loops.md` · `undo-and-rollback.md` · `interactive-debugging.md` · `ux-patterns.md` · `agent-comparison.md`

#### `code-understanding/`
Static analysis, tree-sitter, LSP, search strategies, dependency graphs, git integration.
`README.md` · `static-analysis.md` · `codebase-indexing.md` · `language-servers.md` · `search-strategies.md` · `dependency-graphs.md` · `git-integration.md` · `project-detection.md` · `tools-and-projects.md` · `agent-comparison.md`

#### `testing-and-verification/`
TDD, lint integration, type checking, build verification, CI/CD, self-review.
`README.md` · `test-driven-development.md` · `lint-integration.md` · `type-checking.md` · `build-verification.md` · `self-review.md` · `rollback-strategies.md` · `ci-cd-integration.md` · `agent-comparison.md`

#### `multi-agent-systems/`
Orchestrator-worker, specialist agents, Swarm patterns, communication protocols.
`README.md` · `orchestrator-worker.md` · `specialist-agents.md` · `swarm-patterns.md` · `peer-to-peer.md` · `communication-protocols.md` · `context-sharing.md` · `evaluation-agent.md` · `real-world-examples.md` · `agent-comparison.md`

#### `llm-apis-and-protocols/`
Chat Completions, Responses API, Messages API, function calling, extended thinking.
`README.md` · `chat-completions.md` · `responses-api.md` · `messages-api.md` · `function-calling.md` · `extended-thinking.md` · `embeddings.md` · `batch-api.md` · `rate-limits-and-retries.md` · `agent-comparison.md`

#### `agent-design-patterns/`
Prompt chaining, routing, parallelization, evaluator-optimizer, simplicity principle.
`README.md` · `augmented-llm.md` · `prompt-chaining.md` · `routing.md` · `parallelization.md` · `orchestrator-workers.md` · `evaluator-optimizer.md` · `simplicity-principle.md` · `when-to-use-agents.md` · `agent-comparison.md`

### `notes/`
Scratchpad for ad-hoc research notes and ideas.
