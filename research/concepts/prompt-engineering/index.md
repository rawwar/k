# Prompt Engineering for Coding Agents

> A comprehensive research guide based on analysis of 17 open-source coding agents,
> examining how they craft prompts to maximize model performance on software engineering tasks.

## Why This Matters

Large language models are powerful, but their usefulness in coding agents depends almost
entirely on how they are prompted. A well-engineered prompt can mean the difference between
an agent that reliably edits files, runs tests, and resolves issues — and one that
hallucinates function signatures, produces malformed diffs, or wanders off-task.

Coding agents face unique prompt engineering challenges that don't arise in chatbot or
content-generation settings:

- **Tool orchestration** — Models must decide when and how to call tools (file editors,
  terminals, search), often dozens of times in a single task.
- **Structured output under pressure** — A single misplaced newline in a diff or JSON
  block can break a file. Prompts must constrain output format without sacrificing the
  model's reasoning ability.
- **Long-context management** — Codebases are large. Agents must selectively include
  context while staying within token limits and keeping costs manageable.
- **Multi-model support** — Most agents target multiple providers. The same task may
  need different prompting strategies for Claude, GPT-4, Gemini, and open-weight models.
- **Agentic loops** — Unlike single-shot prompts, agents run in loops where each
  iteration's prompt depends on prior tool results. Errors compound quickly.

This research folder distills the patterns, trade-offs, and techniques used by the
leading open-source coding agents to solve these problems.

---

## Agents Studied

This research is based on direct source-code analysis of **17 open-source coding agents**:

| # | Agent | Notable For |
|---|-------|-------------|
| 1 | **Claude Code** | Anthropic's reference agent; extensive tool-use prompting |
| 2 | **Aider** | Pioneered diff formats (udiff, whole-file, architect mode) |
| 3 | **Codex** | OpenAI's CLI agent; sandboxed execution model |
| 4 | **OpenHands** | Research-grade agent with microagent architecture |
| 5 | **ForgeCode** | Lightweight Rust agent with focused prompt design |
| 6 | **Goose** | Block-based extensible agent with provider abstraction |
| 7 | **Gemini CLI** | Google's agent; optimized for Gemini-family models |
| 8 | **OpenCode** | Terminal-native agent with multi-provider support |
| 9 | **Warp** | Terminal-integrated AI with shell-aware prompting |
| 10 | **Junie CLI** | JetBrains' agent; IDE-context-aware prompts |
| 11 | **Droid** | Android-focused agent with domain-specific tooling |
| 12 | **Sage-Agent** | Minimal agent emphasizing chain-of-thought reasoning |
| 13 | **Mini-SWE-Agent** | Compact research agent for SWE-bench tasks |
| 14 | **TongAgents** | Multi-agent framework with role-specialized prompts |
| 15 | **Ante** | Convention-driven agent with strong structured output |
| 16 | **Capy** | Copy-edit focused agent with careful diff handling |
| 17 | **Pi-Coding-Agent** | Community agent with iterative refinement loops |

### Methodology

For each agent, we examined:

1. **System prompt templates** — The static instructions sent at conversation start.
2. **Tool descriptions** — How each tool's name, description, and parameter schema are defined.
3. **Output format directives** — How agents constrain model output (JSON schemas, diff
   formats, XML tags, markdown fences).
4. **Few-shot examples** — Whether and how examples are embedded in prompts.
5. **Reasoning directives** — Chain-of-thought, planning, or reflection instructions.
6. **Model-specific adaptations** — Conditional prompt logic based on the target model.
7. **Caching strategies** — How prompts are structured to maximize cache hits.

All analysis was performed on publicly available source code. Commit references are noted
where relevant in individual topic files.

---

## Table of Contents

Each file below covers a specific dimension of prompt engineering as practiced by these agents.

### Core Techniques

| File | Topic | Summary |
|------|-------|---------|
| [system-prompts.md](system-prompts.md) | System Prompt Design | How agents structure the system message: identity, rules, capabilities, constraints, and behavioral guardrails. Covers prompt composition patterns (static vs. dynamic sections), persona framing, and how agents inject repository context. |
| [tool-descriptions.md](tool-descriptions.md) | Tool Description Engineering | Writing tool names, descriptions, and parameter schemas that models actually follow. Covers the gap between "technically correct" and "behaviorally effective" tool definitions, with examples of descriptions that reduced error rates. |
| [few-shot-examples.md](few-shot-examples.md) | Few-Shot Examples | When and how to embed examples in coding prompts. Covers format-demonstration examples (showing desired output shape), error-recovery examples, and the trade-off between example quality and token cost. |
| [chain-of-thought.md](chain-of-thought.md) | Reasoning Strategies | How agents elicit step-by-step reasoning: explicit chain-of-thought directives, think-then-act patterns, planning phases, and reflection/retry loops. Includes analysis of which agents use extended thinking features. |
| [structured-output.md](structured-output.md) | Structured Output | Getting models to produce valid JSON, code blocks, diffs, and tool calls. Covers XML-tag wrapping, JSON mode, diff format design (search/replace, udiff, whole-file), and validation/retry strategies when output is malformed. |

### Optimization & Adaptation

| File | Topic | Summary |
|------|-------|---------|
| [prompt-caching.md](prompt-caching.md) | Prompt Caching | How provider caching mechanisms (Anthropic cache control, OpenAI automatic caching, Gemini context caching) work, and how agents structure prompts to maximize cache hit rates. Includes cost analysis and breakeven calculations. |
| [model-specific-tuning.md](model-specific-tuning.md) | Model-Specific Tuning | Adapting prompts per model family. Covers differences in how Claude, GPT-4, Gemini, and open-weight models respond to the same instructions, and how agents conditionally adjust prompts based on the active model. |

### Comparative Analysis

| File | Topic | Summary |
|------|-------|---------|
| [agent-comparison.md](agent-comparison.md) | Agent Comparison | Side-by-side analysis of how all 17 agents approach prompt engineering. Feature matrix, architectural patterns, and a taxonomy of design decisions (monolithic vs. composed prompts, static vs. dynamic, etc.). |
| [tools-and-projects.md](tools-and-projects.md) | Tools & Resources | Prompt engineering libraries, testing frameworks, and related projects. Covers tools like PromptFoo, LangSmith, and provider playgrounds, plus academic papers relevant to coding-agent prompting. |

---

## Key Takeaways (Preview)

These themes emerged repeatedly across the 17 agents studied:

### 1. System Prompts Are Living Documents
No agent ships a single static system prompt. Every agent dynamically composes the system
message from multiple sections — identity, tool lists, repository context, user preferences,
and behavioral rules — assembled at runtime based on the current state.

### 2. Tool Descriptions Are Underrated
The quality of tool descriptions has an outsized impact on agent behavior. Several agents
(notably Claude Code and Aider) include behavioral guidance *inside* tool descriptions —
not just what the tool does, but when to use it, common mistakes to avoid, and examples of
correct invocation.

### 3. Diff Format Is a Pivotal Design Decision
How an agent asks the model to express code changes (whole-file replacement, unified diff,
search/replace blocks, line-numbered edits) is one of the most consequential prompt
engineering choices. Aider's research on this is particularly instructive — they tested
multiple formats and found significant accuracy differences across models.

### 4. Structured Output Requires Defense in Depth
No single technique reliably produces valid structured output. The most robust agents
combine multiple strategies: format instructions in the system prompt, examples showing
the exact shape, XML/JSON wrapper tags, regex-based extraction as a fallback, and
retry-with-error-feedback loops when parsing fails.

### 5. Caching Changes How You Write Prompts
Provider caching mechanisms reward prompts that keep a stable prefix. This creates a
tension with dynamic prompt composition — agents must balance personalization against
cache efficiency. The best agents solve this by ordering sections from most-stable to
least-stable.

### 6. Models Are Not Interchangeable
The same prompt can produce excellent results with one model and poor results with another.
Agents that support multiple providers almost always include model-specific prompt
adjustments — different diff formats, different levels of explicitness, different
temperature settings, and sometimes entirely different system prompts.

### 7. Less Is Often More
Several agents found that shorter, more focused prompts outperform longer, more detailed
ones — especially for capable models. The trick is knowing which instructions the model
already "knows" and which it genuinely needs to be told.

---

## How to Use This Research

- **Building a coding agent?** Start with [system-prompts.md](system-prompts.md) and
  [tool-descriptions.md](tool-descriptions.md) for foundational patterns, then read
  [structured-output.md](structured-output.md) for output format design.
- **Optimizing an existing agent?** Check [prompt-caching.md](prompt-caching.md) for
  cost reduction and [model-specific-tuning.md](model-specific-tuning.md) for per-model
  improvements.
- **Comparing approaches?** The [agent-comparison.md](agent-comparison.md) file provides
  a feature matrix across all 17 agents.
- **Looking for tools?** See [tools-and-projects.md](tools-and-projects.md) for testing
  frameworks and prompt libraries.

---

## Conventions

Throughout these files:

- **Direct quotes** from agent source code are shown in fenced code blocks with a file
  path comment indicating the source.
- **Agent names** are bolded on first mention in each section.
- **Cross-references** between files use relative markdown links.
- Findings are labeled with confidence levels where appropriate:
  - 🟢 **Observed in 10+ agents** — Strong consensus pattern
  - 🟡 **Observed in 4–9 agents** — Common but not universal
  - 🔴 **Observed in 1–3 agents** — Niche or experimental approach

---

*Last updated: July 2025*
*Source: Direct analysis of public repositories for all 17 agents listed above.*