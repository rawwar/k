# Codex CLI — Benchmarks & Performance Data

## Overview

This document compiles available benchmark data for Codex CLI and its associated
models. Codex CLI's performance is primarily measured through the models it uses
(GPT-5.x family) rather than the CLI tool itself.

## Terminal-Bench 2.0 Scores

Terminal-Bench 2.0 evaluates terminal-based coding agents on real-world software
engineering tasks. Codex CLI entries using different model configurations:

| Rank | Agent + Model | Score | Notes |
|---|---|---|---|
| **#27** | Codex CLI (GPT-5.2) | **62.9%** | Previous-gen model |
| **#34** | Codex CLI (GPT-5.1-Codex-Max) | **60.4%** | Specialized coding model |
| **#53** | Codex CLI (GPT-5) | **49.6%** | Base model |

### Observations

- GPT-5.2 outperforms the specialized "Codex-Max" variant by ~2.5 points,
  suggesting that general frontier capability matters more than coding
  specialization at the model level
- The 13.3 percentage point spread between GPT-5.2 (62.9%) and GPT-5 (49.6%)
  shows significant model generation improvement
- These scores reflect the combined effect of model capability + Codex CLI's
  tool system + sandbox interaction patterns

## Model Capabilities

### Recommended Models

| Model | Context Window | Best For | Availability |
|---|---|---|---|
| **GPT-5.4** | 272K tokens | General coding (recommended) | All plans |
| **GPT-5.3-Codex** | 272K tokens | Specialized coding | All plans |
| **GPT-5.3-Codex-Spark** | ~272K tokens | Fast tasks | Pro only |
| **GPT-5.2-Codex** | 272K tokens | Previous generation | All plans |

### Model Features

| Feature | GPT-5.4 | GPT-5.3-Codex | GPT-5.2-Codex |
|---|---|---|---|
| Reasoning summaries | ✅ | ✅ | ✅ |
| Parallel tool calls | ✅ | ✅ | ✅ |
| Image inputs | ✅ | ✅ | ✅ |
| Web search | ✅ | ✅ | ✅ |
| Realtime (WebSocket) | ✅ | ❓ | ❓ |
| Computer use | ✅ | ❌ | ❌ |

### Reasoning Effort Levels

Configurable reasoning effort affects quality/speed trade-off:

| Level | Description | Use Case |
|---|---|---|
| `none` | No reasoning | Simple lookups |
| `minimal` | Bare minimum | Trivial tasks |
| `low` | Light reasoning | File navigation |
| `medium` (default) | Standard | Most coding tasks |
| `high` | Deep reasoning | Complex architecture |
| `xhigh` | Maximum | Critical/novel problems |

## Context Window Performance

### Default Parameters

| Parameter | Value | Impact |
|---|---|---|
| Context window | 272,000 tokens | Maximum conversation length |
| Effective window | 95% (258,400 tokens) | Usable space (5% reserved) |
| Auto-compact threshold | 90% (244,800 tokens) | Triggers compaction |
| Per-output truncation | 10,000 bytes | Individual tool output cap |
| Bytes per token estimate | ~4 | Heuristic for token counting |

### Token Estimation Accuracy

Codex uses a byte-based heuristic (`text.len() / 4`) instead of a real tokenizer.
This trades accuracy for speed:

- **Advantage**: No tokenizer loading cost, works with any model
- **Disadvantage**: Can over/under-estimate for non-English text or code
- **Mitigation**: Conservative thresholds (90% for compaction) provide buffer

### Compaction Efficiency

When context exceeds the auto-compact threshold:
1. History is sent to the model's compaction endpoint
2. Model returns a compressed summary
3. Previous history is replaced with summary + preserved markers
4. `GhostSnapshot` items survive compaction for undo support

This typically reduces context by 60-80% while preserving essential information.

## Sandbox Performance Impact

### Overhead Measurements

The sandbox adds minimal overhead to command execution:

| Sandbox Layer | Typical Overhead | One-time Cost |
|---|---|---|
| Bubblewrap (Linux) | ~5-10ms per command | Mount namespace setup |
| seccomp filter | ~1ms | Filter installation |
| Landlock (legacy) | ~1ms | Rule installation |
| Seatbelt (macOS) | ~5-15ms per command | Profile parsing |
| Windows sandbox | ~50-100ms first command | User account creation |

The bubblewrap + seccomp combination is particularly efficient because:
- Mount namespace reuses existing filesystem (bind mounts, no copies)
- seccomp is a one-time kernel-level filter (BPF program)
- No container image pulling, no VM boot, no Docker daemon

### Network Isolation Cost

Network blocking via namespace + seccomp has **zero ongoing overhead** — blocked
syscalls return `EPERM` immediately rather than timing out.

## Streaming Performance

### SSE vs WebSocket

| Transport | Latency | Throughput | Use Case |
|---|---|---|---|
| SSE | Slightly higher | Good | Standard (default) |
| WebSocket | Lower | Better | Realtime conversations |

### Retry Configuration

| Setting | Default | Max |
|---|---|---|
| Request max retries | 4 | 100 |
| Stream max retries | 5 | 100 |
| Stream idle timeout | 300,000ms (5 min) | - |

## Multi-Agent Performance

### Resource Limits

Sub-agent spawning is controlled by atomic CAS guards:

```rust
pub(crate) struct Guards {
    active_agents: Mutex<ActiveAgents>,
    total_count: AtomicUsize,  // max_threads enforcement
}
```

Each sub-agent:
- Has its own context window (separate from parent)
- Makes independent model API calls
- Shares the sandbox manager (no extra sandbox overhead)
- Consumes additional tokens proportional to its work

### When to Use Sub-Agents

| Scenario | Benefit | Cost |
|---|---|---|
| Multi-file refactoring | Parallelism | 2-3x token usage |
| Code review + fix | Separation of concerns | 1.5-2x tokens |
| Research + implementation | Specialized roles | 2x tokens |
| Single file edit | None | Overhead only |

## Comparison with Other Agents

### SWE-bench Lite (Approximate)

These are model-level comparisons — actual agent performance depends on
tool system, prompting, and execution strategy:

| Agent + Model | SWE-bench Lite | Notes |
|---|---|---|
| Claude Code (Opus 4) | ~72% | Top performer |
| Codex CLI (GPT-5.4) | ~65% (est.) | Based on model capability |
| Aider (GPT-4) | ~26% | Older model baseline |
| Devin | ~14% | Early autonomous agent |

### Feature Comparison

| Feature | Codex CLI | Claude Code | Aider |
|---|---|---|---|
| **Language** | Rust | TypeScript | Python |
| **Binary size** | ~50MB static | ~100MB (Node.js) | Python install |
| **Startup time** | ~100ms | ~500ms | ~1-2s |
| **Sandbox** | OS-native | None (default) | None |
| **Multi-agent** | Yes (built-in) | Yes (sub-agents) | No |
| **Resume** | Yes (rollout) | Yes (conversation) | No |
| **Web search** | Yes (cached/live) | No built-in | No |
| **Image input** | Yes | Yes | Yes (limited) |
| **MCP support** | Client + Server | Client | No |
| **IDE integration** | VS Code extension | VS Code extension | Editor plugins |
| **Windows** | WSL + native | WSL only | Python native |
| **Enterprise** | OTel + MDM | Team plan | None |
| **License** | Apache-2.0 | Proprietary | Apache-2.0 |
| **Open source** | Yes | Partial (CLI only) | Yes |

### Performance Characteristics

| Metric | Codex CLI | Claude Code | Notes |
|---|---|---|---|
| Token efficiency | Moderate | Higher | Claude's longer context helps |
| Tool call latency | Low (OS sandbox) | Low (no sandbox) | Sandbox adds ~10ms |
| Context management | Byte heuristic | Tokenizer | Trade-off: speed vs accuracy |
| Compaction | Model-based | Server-managed | Both effective |
| Retry resilience | Configurable (4/5) | Built-in | Codex more configurable |

## Local Model Performance

### Ollama Integration

| Setting | Value |
|---|---|
| Default model | `gpt-oss:20b` |
| Min Ollama version | 0.13.4 |
| Default port | 11434 |
| API | OpenAI-compatible `/v1/` |

### LM Studio Integration

| Setting | Value |
|---|---|
| Default model | `openai/gpt-oss-20b` |
| Default port | 1234 |
| API | OpenAI-compatible `/models` |

Local models trade capability for privacy and cost — all inference stays on
device with no network calls.

## Cost Considerations

Codex CLI supports multiple pricing tiers via ChatGPT plans:

| Plan | Included | Rate Limits |
|---|---|---|
| Plus | Standard Codex access | Moderate |
| Pro | + Codex-Spark (fast) | Higher |
| Business | + Team features | Business tier |
| Enterprise | + MDM, OTel, compliance | Enterprise tier |
| API key | Pay-per-token | Per-key limits |

The `service_tier` config option controls request priority:
- `fast` — Higher priority, potentially more expensive
- `flex` — Lower priority, cost-optimized