# Goose — Benchmarks

## Overview

Goose has been evaluated on Terminal-Bench, a benchmark focused on terminal-based coding agent performance. It has appeared across both Terminal-Bench 1.0 and 2.0 leaderboards.

Note: Goose's benchmark presence is more limited compared to agents like Claude Code or Codex CLI, partly because it's a newer entrant and partly because its multi-provider architecture means performance varies significantly by model choice.

## Terminal-Bench 2.0

Terminal-Bench 2.0 is an updated benchmark with harder tasks and more rigorous evaluation.

### Results

| Configuration | Rank | Score | Notes |
|--------------|------|-------|-------|
| Goose + Claude Opus 4.5 | #44 | 54.3% | Best Goose configuration on TB2.0 |
| Goose + Claude Sonnet 4.5 | #61 | 43.1% | More cost-effective option |

### Context

For comparison on Terminal-Bench 2.0:
- Top agents score in the 70-80%+ range
- Claude Code (native) typically ranks in the top 5-10
- Goose's mid-table positioning reflects its generalist architecture — it's optimized for extensibility rather than raw benchmark performance

### Analysis

The ~11% gap between Opus 4.5 and Sonnet 4.5 configurations suggests Goose benefits significantly from stronger reasoning models. This is consistent with its architecture: the agent loop handles tool dispatch and context management, but the quality of planning and code generation depends entirely on the underlying model.

## Terminal-Bench 1.0

Terminal-Bench 1.0 was the original benchmark with a broader set of terminal-based tasks.

### Results

| Configuration | Rank | Score | Notes |
|--------------|------|-------|-------|
| Goose + claude-opus-4 | #17 | 45.3% | Strong showing on TB1.0 |

### Context

Rank #17 on Terminal-Bench 1.0 places Goose in the competitive middle tier. The benchmark measures end-to-end task completion in terminal environments, which aligns well with Goose's developer tool extension (shell, file editing, tree).

## SWE-bench

Goose has not been widely reported on SWE-bench as of this research. This may be because:
- SWE-bench focuses on repository-level bug fixing, which requires deep code understanding
- Goose's strength is in extensibility and automation workflows rather than concentrated code reasoning
- The multi-provider architecture makes it harder to submit a single canonical result

## Performance Factors

### Model Dependency

Goose's performance is heavily model-dependent since it's provider-agnostic:

| Provider | Expected Relative Performance |
|----------|------------------------------|
| Claude Opus 4 / 4.5 | Highest (best tool-calling) |
| Claude Sonnet 4 / 4.5 | Strong (recommended default) |
| GPT-4o | Good |
| Gemini 2.5 Pro | Good |
| Local models (Ollama) | Lower (toolshim adds overhead) |

The docs note: "goose relies heavily on tool calling capabilities and currently works best with Claude 4 models."

### Architectural Overhead

Goose's extensibility comes with overhead that affects benchmark performance:

1. **MCP serialization**: Even built-in tools go through MCP protocol serialization
2. **Extension loading**: Multiple MCP server processes start at session beginning
3. **Security inspection**: Every tool call passes through 4 inspectors
4. **Context management**: Background summarization and MOIM injection add tokens
5. **Tool namespacing**: `extension__tool` prefixes consume tokens in the tool list

### Strengths for Benchmarks

1. **Robust error recovery**: Auto-compaction and retry logic handle long tasks gracefully
2. **Broad tool access**: Extensions can provide specialized tools for specific task types
3. **Recipe mode**: Automated retry with success criteria helps complete challenging tasks
4. **Max turns = 1000**: Very high iteration limit allows persistent problem-solving

### Weaknesses for Benchmarks

1. **Generalist design**: Not optimized for any single benchmark's task distribution
2. **Extension overhead**: Multiple MCP servers add latency per turn
3. **Token overhead**: Tool namespacing, MOIM injection, and system prompt instrumentation use tokens
4. **No specialized reasoning**: Unlike agents with built-in code analysis or search, Goose delegates everything to extensions

## Benchmark Methodology Notes

### Terminal-Bench Specifics

Terminal-Bench evaluates agents on their ability to:
- Execute multi-step terminal commands
- Navigate file systems
- Edit code and configuration files
- Debug and fix issues
- Install dependencies and run tests

Goose's Developer extension (shell, edit, write, tree) is the primary tool set for these tasks. The benchmark score reflects the combined quality of:
1. The underlying LLM's reasoning and planning
2. Goose's tool dispatch efficiency
3. Error recovery and retry behavior
4. Context management over long task sequences

### Reproducibility

Running Goose benchmarks requires specifying:
- Exact model version (e.g., `claude-opus-4-20250514`)
- Extension configuration (which extensions are enabled)
- Permission mode (autonomous for benchmarks)
- Context limit and compaction settings

## Summary Table

| Benchmark | Version | Model | Rank | Score |
|-----------|---------|-------|------|-------|
| Terminal-Bench | 2.0 | Claude Opus 4.5 | #44 | 54.3% |
| Terminal-Bench | 2.0 | Claude Sonnet 4.5 | #61 | 43.1% |
| Terminal-Bench | 1.0 | claude-opus-4 | #17 | 45.3% |

## Verdict

Goose performs respectably on benchmarks but is not a top-tier performer. Its architecture prioritizes extensibility, provider flexibility, and enterprise safety over raw benchmark scores. For organizations that need a configurable, secure, MCP-native agent that works with their chosen LLM provider, Goose's moderate benchmark performance is an acceptable trade-off. For pure coding performance, dedicated agents like Claude Code with their tighter model integration tend to perform better.