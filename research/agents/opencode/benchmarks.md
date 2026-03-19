# OpenCode — Benchmarks

## Overview

OpenCode is a relatively young project (early 2025) and has limited benchmark data compared to established agents. The primary benchmark reference is Terminal-Bench 2.0, which evaluates terminal-based coding agents.

## Terminal-Bench 2.0

Terminal-Bench is a benchmark designed specifically for terminal-based AI coding agents, testing their ability to solve real-world software engineering tasks.

### OpenCode Results

| Metric | Value |
|--------|-------|
| **Rank** | #50 |
| **Model** | Claude Opus 4.5 |
| **Score** | 51.7% |
| **Category** | Terminal agents |

### Context

Terminal-Bench 2.0 evaluates agents on a variety of coding tasks including:
- Bug fixing
- Feature implementation
- Code refactoring
- Test writing
- Configuration changes

The 51.7% score places OpenCode in the middle tier of evaluated agents. This is expected for a project in early development that was still rapidly evolving.

### Comparison with Peers

For reference, here are approximate ranges of Terminal-Bench scores for similar agents (scores may vary by model and configuration):

| Agent | Approximate Score Range | Notes |
|-------|------------------------|-------|
| Claude Code | 60-75% | Mature, well-tuned |
| Codex CLI | 55-65% | OpenAI's agent |
| Aider | 40-55% | Depends on model |
| **OpenCode** | **51.7%** | **Early development** |
| Cline | 35-50% | IDE-based |

*Note: Scores are approximate and depend on model choice, configuration, and benchmark version.*

## SWE-Bench

No public SWE-bench results are available for OpenCode as of the time of archival. The project was focused on usability and multi-provider support rather than benchmark optimization.

## Performance Characteristics

While not formal benchmarks, OpenCode has some notable performance characteristics from its Go implementation:

### Startup Time

| Metric | OpenCode (Go) | Typical Python Agent | Typical Node.js Agent |
|--------|---------------|---------------------|-----------------------|
| Cold start | ~50-100ms | 1-5s | 500ms-2s |
| Binary size | ~30-50MB | N/A (needs runtime) | N/A (needs runtime) |

The single Go binary means no dependency resolution, virtual environment activation, or module loading at startup.

### Memory Usage

Go's garbage collector and static compilation typically result in lower memory usage than Python/Node.js agents:

| Metric | Approximate |
|--------|-------------|
| Base memory | ~20-50MB |
| Per-session overhead | ~5-10MB |
| TUI rendering | ~10-20MB |

### Concurrency

Go's goroutine model is particularly well-suited for the agent pattern:
- Streaming responses use channels (near-zero overhead)
- Permission blocking uses channel synchronization
- Multiple sessions can run concurrently
- LSP clients run in background goroutines
- Title generation runs asynchronously

## Cost Efficiency

OpenCode tracks costs per session and per model. Here are the pricing tiers for commonly used models:

### Anthropic Models

| Model | Input ($/1M) | Output ($/1M) | Context Window |
|-------|-------------|---------------|----------------|
| Claude 3.7 Sonnet | $3.00 | $15.00 | 200K |
| Claude 3.5 Haiku | $0.80 | $4.00 | 200K |
| Claude 3 Opus | $15.00 | $75.00 | 200K |

### OpenAI Models

| Model | Input ($/1M) | Output ($/1M) | Context Window |
|-------|-------------|---------------|----------------|
| GPT-4.1 | $2.00 | $8.00 | 1M |
| GPT-4.1-mini | $0.40 | $1.60 | 1M |
| o3-mini | $1.10 | $4.40 | 200K |

### Google Gemini

| Model | Input ($/1M) | Output ($/1M) | Context Window |
|-------|-------------|---------------|----------------|
| Gemini 2.5 Pro | $1.25 | $10.00 | 1M |
| Gemini 2.0 Flash | $0.10 | $0.40 | 1M |

### Free Tier via GitHub Copilot

A unique OpenCode advantage: if users have a GitHub Copilot subscription, they can access multiple models (Claude, GPT, Gemini) at **no additional cost** through the Copilot provider.

## Factors Affecting Benchmark Performance

Several factors explain OpenCode's current benchmark positioning:

### Strengths
1. **Persistent shell**: State preservation across commands
2. **LSP integration**: Real-time diagnostics after edits
3. **Sub-agent delegation**: Efficient search via agent tool
4. **Sourcegraph integration**: Access to reference implementations
5. **Multi-provider support**: Can use the best model for each task

### Limitations
1. **Sequential tool execution**: No parallel tool calls
2. **No turn limit**: Can get stuck in long loops
3. **Basic error recovery**: No sophisticated retry strategies
4. **No test verification**: Doesn't automatically run tests after fixes
5. **Early development**: Many features were still in progress when archived

### Areas for Improvement
1. **Parallel tool execution**: Would significantly reduce latency for multi-tool responses
2. **Smarter context management**: Client-side token counting could prevent context overflow errors
3. **Automated testing loops**: Running tests after code changes to verify fixes
4. **Better command safety**: The banned command list could be more comprehensive
5. **Retrieval augmentation**: File indexing for more targeted context injection

## Benchmark Methodology Notes

When evaluating OpenCode's benchmark scores, consider:

1. **Model dependency**: OpenCode's performance varies significantly by model. Claude Opus 4.5 at 51.7% may differ substantially from GPT-4.1 or Gemini 2.5.
2. **Configuration sensitivity**: The tool set, system prompt, and auto-compact settings all affect performance.
3. **Early development**: The project was in rapid development when benchmarked; later versions (and the Crush successor) may perform differently.
4. **Terminal-Bench specificity**: Terminal-Bench tests terminal-specific interactions that may not reflect general coding ability.
