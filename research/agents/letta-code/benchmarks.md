# Letta Code — Benchmarks

## Terminal-Bench 2.0

| Model | Date | Rank | Score |
|---|---|---|---|
| Claude Opus 4.5 | 2025-12-17 | **#36** | 59.1% ±2.4 |
| Gemini 3 Pro | 2025-12-17 | #45 | 56.0% ±3.0 |
| GPT-5.1-Codex | 2025-12-17 | #48 | 53.5% ±2.8 |

All three runs were submitted on the same day (2025-12-17), suggesting a
single coordinated benchmark sweep at that release point.

## Reading the Numbers

A 5.6-point spread across three top-tier models is **narrower** than
most multi-model agents (Mux: 16, Droid: 14, Terminus 2: huge). This is
consistent with the design hypothesis: a memory-rich, skill-driven loop
should depend less on raw model power because more of the heavy lifting
happens outside the model call.

The results are mid-pack on the leaderboard — the Letta Code rows sit
between Codex CLI variants (#35 GPT-5.1-Codex-Max at 60.4%, #50 Claude
Code at 52.1%). Letta Code is competitive with first-party CLI tools
without being best-in-class on raw score.

## Vs. Reference Scaffold

Comparable Terminus 2 numbers for the same models:

| Model | Letta Code | Terminus 2 | Δ |
|---|---:|---:|---:|
| Claude Opus 4.5 | 59.1% | 57.8% | +1.3 |
| Gemini 3 Pro | 56.0% | 56.9% | -0.9 |
| GPT-5.1-Codex | 53.5% | 36.9% | **+16.6** |

The huge GPT-5.1-Codex lift is striking — Terminus 2 specifically
struggles with that model (parsing? prompt format?) and Letta's harness
recovers most of the gap.

## Letta's Own Leaderboard

Letta publishes a separate model leaderboard at
<https://leaderboard.letta.com/> that ranks LLMs on Letta-specific tasks
(memory, tool use, agentic reasoning). This is distinct from
Terminal-Bench and uses Letta's internal eval set. Their published
recommendation as of early 2026 is Opus 4.5 or GPT-5.2 for best
performance.

## Other Public Benchmarks

The Letta team has historically published evaluations of memory and
long-context behaviour (continuing the MemGPT line of work) but no
broadly-comparable coding-agent score outside Terminal-Bench.

---

*Numbers from <https://www.tbench.ai/leaderboard/terminal-bench/2.0>
(snapshot 2026-05-01).*
