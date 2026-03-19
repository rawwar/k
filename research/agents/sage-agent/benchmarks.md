# SageAgent — Benchmarks

## Terminal-Bench 2.0

SageAgent's primary public benchmark result is from Terminal-Bench 2.0, a benchmark
for evaluating coding agent performance on terminal-based tasks.

### Result

| Field | Value |
|---|---|
| Benchmark | Terminal-Bench 2.0 |
| Agent | SageAgent |
| Model | GPT-5.3-Codex |
| Score | **78.4% ±2.2** |
| Rank | **#5** |
| Date | 2026-03-13 |

### Leaderboard Context

SageAgent's score of 78.4% places it at rank #5 on the Terminal-Bench 2.0 leaderboard.
This score ties with ForgeCode using Gemini 3.1 Pro, which also achieved 78.4%.

The ±2.2 confidence interval suggests the benchmark was run multiple times with
variance across runs, which is standard practice for stochastic agent evaluations.

### Interpretation

- **Competitive showing**: Rank #5 on a major coding benchmark is a solid result for
  a multi-agent framework, demonstrating that the pipeline architecture can translate
  to effective task completion.
- **Model dependency**: The result is achieved with GPT-5.3-Codex specifically. Performance
  with other models is not publicly documented, though the roadmap mentions expanding
  tested model support.
- **Multi-agent overhead**: The pipeline architecture (5 agents, feedback loop) adds
  latency compared to single-agent approaches. The benchmark score suggests this
  overhead is offset by improved task decomposition and iterative refinement.

## Other Benchmarks

No other public benchmark results were found for SageAgent. The roadmap items suggest
the project is still in relatively early stages, with benchmark coverage likely to
expand as the framework matures.

## Limitations of This Analysis

- Only one benchmark data point is available
- Historical performance trends are not documented
- Per-category breakdown within Terminal-Bench 2.0 is not available
- Comparison across different model backends is not available

---

*Tier 3 analysis — benchmark data from Terminal-Bench 2.0 leaderboard.*