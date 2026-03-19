# TongAgents — Benchmarks

> Terminal-Bench 2.0 results and comparison with other coding agents.

## Terminal-Bench 2.0

**Terminal-Bench 2.0** is a benchmark evaluating coding agents in realistic CLI environments. Published at **ICLR 2026** and part of the **Artificial Analysis Intelligence Index v4.0**, it tests agents across 89 tasks spanning:

- Server configuration
- Debugging
- Security hardening
- Data science workflows
- Compilation and build systems

Tasks are executed in Docker containers with realistic system environments. Agents must interact via shell commands to complete each task.

## TongAgents Results

| Configuration | Score | Confidence Interval | Rank | Date |
|--------------|-------|-------------------|------|------|
| TongAgents + Gemini 3.1 Pro | **80.2%** | ±2.6 | **#3** | 2026-03-13 |
| TongAgents + Claude Opus 4.6 | **~71.9%** | — | **#13** | 2026-03 |

### Key Observations

- **80.2% is one of the highest scores** on Terminal-Bench 2.0, placing TongAgents among the top 3 agents
- The **±2.6 confidence interval** is typical for the benchmark (89 tasks → moderate statistical power)
- The **~8pp gap** between Gemini 3.1 Pro and Claude Opus 4.6 is one of the larger model-dependent gaps on the leaderboard
- Both results are from the same agent framework, demonstrating model-agnostic design

## Leaderboard Context (Approximate, March 2026)

To contextualize TongAgents' performance, here is an approximate view of the Terminal-Bench 2.0 leaderboard around the time of submission:

| Rank | Agent | Score | Model |
|------|-------|-------|-------|
| #1 | Top agent | ~83-85% | — |
| #2 | Second agent | ~81-82% | — |
| **#3** | **TongAgents** | **80.2%** | **Gemini 3.1 Pro** |
| #4-12 | Various agents | 72-80% | Various |
| **#13** | **TongAgents** | **~71.9%** | **Claude Opus 4.6** |

*Note: Exact leaderboard positions of other agents are approximate and may have shifted since TongAgents' submission.*

## Performance Analysis

### Strengths (Inferred)
- **Consistent high performance** — achieving 80%+ on a benchmark with diverse task types suggests robust generalization
- **Multi-model viability** — competitive scores with both Gemini and Claude
- **Complex task handling** — Terminal-Bench tasks require multi-step reasoning, suggesting strong planning capabilities

### Model Sensitivity
The performance gap between models deserves analysis:

| Factor | Gemini 3.1 Pro Advantage |
|--------|------------------------|
| Context window | 1M+ tokens vs 200K |
| Tool calling | Potentially more reliable structured output |
| Reasoning depth | May handle complex multi-step plans better |

The gap could also reflect development bias — the agent may have been primarily tuned for Gemini.

## Comparison with Notable Agents

Without exact scores for all competitors, we can note TongAgents' relative position:

- **Top tier (>80%)** — TongAgents with Gemini 3.1 Pro belongs here, alongside only 2-3 other agents
- **Upper tier (70-80%)** — TongAgents with Claude Opus 4.6, competitive but not leading
- **Mid tier (60-70%)** — many well-known agents cluster here

Reaching the top tier with *any* model configuration is a significant achievement, especially for an agent from an institution not previously known for coding agent work.

## Other Benchmarks

No results for TongAgents have been found on other coding agent benchmarks:

- **SWE-bench** — no submission found
- **HumanEval / MBPP** — no submission found
- **Aider polyglot** — no submission found

This may indicate that Terminal-Bench 2.0 was chosen as the initial public demonstration, with other benchmarks to follow.

## What We'd Like to Know

- Per-category breakdowns (which task types does TongAgents excel at?)
- Results with additional models (GPT-5, Qwen, Llama)
- SWE-bench or other benchmark submissions for cross-benchmark comparison
- Ablation studies showing which components contribute most to performance