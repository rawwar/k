# Claude Code — Benchmarks & Performance

> Benchmark results for Claude Code across Terminal-Bench, SWE-bench, and model-level evaluations.

## Terminal-Bench 2.0

Terminal-Bench is a benchmark specifically designed to evaluate terminal-based coding agents. It tests agents on real-world coding tasks performed through the terminal.

### Claude Code Rankings

| Rank | Agent + Model | Score | Notes |
|------|--------------|-------|-------|
| #39 | Claude Code (Claude Opus 4.6) | 58.0% | Best Claude Code configuration |
| #48 | Claude Code (Claude Opus 4.5) | 52.1% | Previous generation Opus |
| #69 | Claude Code (Claude Sonnet 4.5) | 40.1% | Cost-effective configuration |

### Context

- Terminal-Bench 2.0 evaluates agents in realistic terminal environments
- Tasks include: code generation, debugging, refactoring, file manipulation, git operations
- Claude Code competes against specialized agents and other general-purpose coding tools
- Rankings are publicly maintained and updated as agents improve

### Observations

- **Opus 4.6 vs Opus 4.5**: ~6% improvement between model generations, showing direct model quality impact
- **Opus vs Sonnet**: ~18% gap between Opus 4.6 and Sonnet 4.5, confirming Opus's stronger reasoning
- **Mid-pack positioning**: Claude Code ranks in the middle tier, not at the top. This suggests the agentic harness quality matters as much as the underlying model
- **Model dependence**: Performance is strongly tied to the underlying model, as expected for an agentic system

## SWE-bench

SWE-bench evaluates AI systems on real software engineering tasks derived from GitHub issues and pull requests.

### Known Results

Anthropic has published SWE-bench results for Claude models (used by various agents including Claude Code):

| Benchmark | Model | Score | Date |
|-----------|-------|-------|------|
| SWE-bench Verified | Claude Sonnet 4 | 72.7% | Jun 2025 |
| SWE-bench Verified | Claude Opus 4 | 72.5% | Jun 2025 |
| SWE-bench Verified | Claude Sonnet 3.5 (v2) | 49.0% | Oct 2024 |
| SWE-bench Verified | Claude Sonnet 3.5 | 33.4% | Jun 2024 |

**Important caveat**: These are model-level scores using Anthropic's internal scaffolding, not necessarily Claude Code specifically. Different agentic harnesses (Claude Code vs. Devin vs. SWE-Agent) can produce different scores with the same underlying model.

### SWE-bench Context

- SWE-bench Verified is a curated subset of 500 tasks from the full SWE-bench dataset
- Tasks involve fixing real GitHub issues: understanding the issue, locating relevant code, implementing a fix, and passing tests
- The benchmark tests both reasoning (understanding the bug) and execution (making the right edit)
- Claude Code's performance on SWE-bench is likely close to the model scores above, given its full toolset

## Model-Level Performance Characteristics

From Anthropic's documentation and observable behavior, different models show different strengths:

### Claude Opus 4.6 / 4.5
- **Strongest reasoning**: Best for complex architectural decisions, difficult bugs
- **Adaptive reasoning**: Dynamically allocates thinking tokens based on effort level
- **Extended thinking**: Deeper analysis before responding
- **Cost**: Most expensive per token
- **Speed**: Slowest of the three tiers
- **Best for**: Complex refactors, architecture design, difficult debugging

### Claude Sonnet 4.6 / 4.5 / 4
- **Balanced performance**: Good reasoning at moderate cost
- **Daily driver**: Recommended for most coding tasks
- **Adaptive reasoning**: Same as Opus on 4.6
- **Cost**: Mid-tier
- **Speed**: Moderate
- **Best for**: Feature implementation, routine debugging, code review

### Claude Haiku
- **Fastest**: Lowest latency, cheapest
- **Used by**: Explore sub-agent (built-in), Claude Code Guide sub-agent
- **Trade-off**: Less reasoning depth, more suitable for search/lookup tasks
- **Best for**: Codebase exploration, quick lookups, sub-agent tasks

## Effort Level Impact

Claude Code supports effort levels (`/effort low|medium|high`) on Opus 4.6 and Sonnet 4.6:

| Effort Level | Behavior | Use Case |
|-------------|----------|----------|
| `low` | Minimal thinking, fast responses | Simple questions, quick edits |
| `medium` | Balanced thinking (default) | Standard coding tasks |
| `high` | Deep reasoning, more thinking tokens | Complex architecture, hard bugs |

The `ultrathink` keyword in a prompt sets effort to high for that turn only.

## Context Window as Performance Factor

From Anthropic's best practices, context window usage directly impacts quality:

- **Fresh context**: Highest quality responses, instructions followed precisely
- **50-70% full**: Good quality, may start losing early instructions
- **Near capacity**: Quality degrades, earlier instructions may be forgotten
- **Auto-compaction triggered**: Quality reset after compaction, but nuance may be lost

This is why the docs recommend:
- `/clear` between unrelated tasks
- Sub-agents for exploration (separate context)
- CLAUDE.md for persistent instructions (survives compaction)
- Skills for on-demand knowledge (not loaded until needed)

## Comparative Positioning

### vs. Other CLI Agents

| Agent | Approach | Relative Strength |
|-------|----------|------------------|
| **Claude Code** | First-party Anthropic agent, deep Claude integration | Permission model, context management, multi-surface |
| **GitHub Copilot CLI** | GitHub/Microsoft agent for terminal | GitHub integration, broad model support |
| **Aider** | Open-source Python CLI agent | Model-agnostic, git-centric, transparent |
| **Cline** | VS Code extension (formerly Claude Dev) | IDE integration, model-agnostic |

### vs. IDE Agents

| Agent | Approach | Relative Strength |
|-------|----------|------------------|
| **Cursor** | Full IDE (Electron) with agent mode | Tight IDE integration, inline editing |
| **Windsurf** | IDE with Cascade agent | Flow-based editing, inline experience |
| **Claude Code (VS Code)** | Extension in existing IDE | Uses the full Claude Code engine inside VS Code |

### Key Competitive Observations

1. **Model lock-in vs. model advantage**: Claude Code only works with Claude models, but has the deepest integration with them (adaptive reasoning, effort levels, extended thinking). Competitors offer model flexibility.

2. **Terminal-first vs. IDE-first**: Claude Code is architecturally terminal-first (IDE support is via extensions). Cursor and Windsurf are IDE-first. This shapes the user experience and workflow differently.

3. **Open vs. closed**: Aider is fully open-source. Claude Code is closed-source with a public npm package and plugin system. This affects community contribution and transparency.

4. **Permission sophistication**: Claude Code's 5-mode permission system with glob patterns, scoped settings, hooks, and sandboxing is the most sophisticated in the space. Most competitors have simpler models.

## Limitations

Based on documentation and observable behavior:

1. **Claude-only**: Cannot use non-Anthropic models (GPT-4, Gemini, Llama, etc.)
2. **Context window ceiling**: Like all LLM agents, limited by context window size; long sessions degrade quality
3. **Cost**: Usage-based pricing; Opus sessions can be expensive for long tasks
4. **Closed-source core**: The agentic harness is not open-source; community cannot inspect or contribute to core
5. **Terminal-centric**: While IDE extensions exist, the core experience is terminal-based, which may not suit all developers
6. **LSP via plugins only**: Type checking and code intelligence require separate plugin installation
