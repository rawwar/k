# Deep Agents — Benchmarks

## Terminal-Bench 2.0

| Model | Date | Rank | Score |
|---|---|---|---|
| GPT-5.2-Codex | 2026-02-12 | **#21** | 66.5% ±3.1 |

Single published row as of 2026-05.

## Context

Rank #21 with 66.5% places Deep Agents in the upper-middle of TB2.

Closely-clustered neighbours:

| Rank | Agent | Model | Score |
|---:|---|---|---:|
| 20 | Crux | Claude Opus 4.6 | 66.9% |
| **21** | **Deep Agents** | **GPT-5.2-Codex** | **66.5%** |
| 22 | Mux | Claude Opus 4.6 | 66.5% |
| 23 | SageAgent | Gemini 3 Pro | 65.2% |
| 24 | Droid | GPT-5.2 | 64.9% |

## Vs. Reference Scaffold

The closest comparable Terminus 2 row (GPT-5.2 — exact GPT-5.2-Codex
not available as a Terminus 2 row):

| Agent | Model | Rank | Score |
|---|---|---:|---:|
| **Deep Agents** | **GPT-5.2-Codex** | **#21** | **66.5%** |
| Terminus 2 | GPT-5.2 | #47 | 54.0% |
| Terminus 2 | GPT-5-Codex | #61 | 43.4% |

A ~12-point lift over Terminus 2 / GPT-5.2. The harness — todos,
sub-agents, files-as-overflow, auto-summarisation — is doing real work
here.

## Other Benchmarks

No other public benchmark scores listed for Deep Agents specifically as
of 2026-05. The package has been cited extensively in the LangChain
ecosystem (recipe blogs, tutorials, Studio examples) but the
Terminal-Bench result is the headline external evaluation.

---

*Numbers from <https://www.tbench.ai/leaderboard/terminal-bench/2.0>
(snapshot 2026-05-01).*
