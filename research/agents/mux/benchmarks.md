# Mux — Benchmarks

## Terminal-Bench 2.0

| Model | Date | Rank | Score |
|---|---|---|---|
| GPT-5.3-Codex | 2026-03-06 | **#12** | 74.6% ±2.5 |
| Claude Opus 4.6 | 2026-02-13 | #22 | 66.5% ±2.5 |
| GPT-5.2 | 2026-01-17 | #34 | 60.7% |
| Claude Opus 4.5 | 2026-01-17 | #38 | 58.4% |

## Reading the Spread

A 16-point gap between Mux's best (GPT-5.3-Codex, 74.6%) and worst
published (Claude Opus 4.5, 58.4%) is on the larger end for a single agent
on TB2 — comparable to Droid (63.1% → 77.3%) but wider than Letta Code
(53.5% → 59.1%).

The clear ordering is:

`GPT-5.3-Codex > Claude Opus 4.6 > GPT-5.2 > Claude Opus 4.5`

…which matches the ordering most other multi-model agents see, suggesting
the loop itself is reasonably model-neutral.

## Vs. Reference Scaffold

| Model | Mux | Terminus 2 | Δ |
|---|---:|---:|---:|
| GPT-5.3-Codex | 74.6% | 64.7% | +9.9 |
| Claude Opus 4.6 | 66.5% | 62.9% | +3.6 |
| GPT-5.2 | 60.7% | 54.0% | +6.7 |
| Claude Opus 4.5 | 58.4% | 57.8% | +0.6 |

The lift over Terminus 2 (the scaffold) is real but modest, especially on
Anthropic models — interesting given Mux's Claude-Code lineage. The
biggest delta is on GPT-5.3-Codex, suggesting the loop is best-tuned for
OpenAI's tool-calling style.

## Other Benchmarks

No other public scores published as of 2026-05.

---

*Numbers from <https://www.tbench.ai/leaderboard/terminal-bench/2.0>
(snapshot 2026-05-01).*
