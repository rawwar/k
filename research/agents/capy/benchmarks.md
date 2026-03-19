# Capy — Benchmarks

> Terminal-Bench 2.0 results: rank #7 with Claude Opus 4.6 at 75.3% ±2.4.

## Terminal-Bench 2.0

| Agent + Model | Rank | Score | Date |
|---------------|------|-------|------|
| Capy + Claude Opus 4.6 | #7 | 75.3% ±2.4 | 2026-03-12 |

### Context

**Terminal-Bench 2.0** is a benchmark for evaluating AI coding agents on real-world terminal-based software engineering tasks. A rank of #7 places Capy in the upper tier of commercial coding agents.

For reference, nearby results on the same benchmark (as of the same period):

| Rank | Agent | Score |
|------|-------|-------|
| #1 | ForgeCode (Claude Opus 4.6 / GPT-5.4) | 81.8% |
| #6 | Droid + GPT-5.3-Codex | 77.3% |
| **#7** | **Capy + Claude Opus 4.6** | **75.3% ±2.4** |

### Interpretation

Capy's 75.3% score is notable for several reasons:

1. **Cloud IDE vs terminal agents**: Terminal-Bench is designed for terminal-based agents, so Capy (a cloud IDE) may be at a structural disadvantage compared to terminal-native tools like ForgeCode.

2. **Single model configuration**: Only one model configuration (Claude Opus 4.6) has a published score. Other agents (Droid, ForgeCode) show results across multiple model configurations.

3. **Captain/Build overhead**: The two-agent architecture adds a planning step before execution begins. On benchmark tasks (which tend to be well-specified), this planning phase may add overhead without proportional benefit — the task description is already the spec.

4. **Parallel advantage not captured**: Capy's primary differentiator (parallel task execution) is not measured by Terminal-Bench, which evaluates individual task completion.

## Other Benchmarks

No other publicly available benchmark results for Capy were found as of this research. The platform's strengths (parallel execution, team collaboration, async workflows) are difficult to capture in standard coding benchmarks, which typically measure single-task, single-agent performance.

## What Would Better Measure Capy

Benchmarks that would better capture Capy's strengths:

- **Sprint-level benchmarks**: Multiple related tasks executed in parallel, measuring total completion time and consistency
- **Spec quality benchmarks**: Evaluating the Captain agent's ability to produce specs that lead to correct implementations
- **Ambiguity resolution benchmarks**: Measuring how well Captain's clarification questions resolve underspecified tasks
- **Team collaboration benchmarks**: Multiple users and agents working on the same repository concurrently
