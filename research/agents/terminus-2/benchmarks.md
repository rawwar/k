# Terminus 2 — Benchmarks

## Terminal-Bench 2.0

Terminus 2 has the most leaderboard entries of any agent on TB2 — 25 rows
spanning every major commercial and open model — because it doubles as the
reference scaffold the project ships.

Snapshot from <https://www.tbench.ai/leaderboard/terminal-bench/2.0>
(2026-05-01):

| Rank | Model | Date | Score |
|-----:|---|---|---|
|  25 | GPT-5.3-Codex | 2026-02-05 | 64.7% ±2.7 |
|  28 | Claude Opus 4.6 | 2026-02-06 | 62.9% ±2.7 |
|  42 | Claude Opus 4.5 | 2025-11-22 | 57.8% ±2.5 |
|  44 | Gemini 3 Pro | 2025-11-21 | 56.9% ±2.5 |
|  47 | GPT-5.2 | 2025-12-12 | 54.0% ±2.9 |
|  49 | GLM 5 | 2026-02-23 | 52.4% ±2.6 |
|  52 | Gemini 3 Flash | 2026-01-07 | 51.7% ±3.1 |
|  56 | GPT-5.1 | 2025-11-16 | 47.6% ±2.8 |
|  61 | GPT-5-Codex | 2025-10-31 | 43.4% ±2.9 |
|  62 | Kimi K2.5 | 2026-02-04 | 43.2% ±2.9 |
|  65 | Claude Sonnet 4.5 | 2025-10-31 | 42.8% ±2.8 |
|  70 | Minimax m2.5 | 2026-02-23 | 42.2% ±2.6 |
|  73 | DeepSeek-V3.2 | 2026-02-10 | 39.6% ±2.8 |
|  74 | Claude Opus 4.1 | 2025-10-31 | 38.0% ±2.6 |
|  76 | GPT-5.1-Codex | 2025-11-17 | 36.9% ±3.2 |
|  78 | Kimi K2 Thinking | 2025-11-11 | 35.7% ±2.8 |
|  80 | GPT-5 | 2025-10-31 | 35.2% ±3.1 |
|  85 | GLM 4.7 | 2026-01-28 | 33.4% ±2.8 |
|  87 | Gemini 2.5 Pro | 2025-10-31 | 32.6% ±3.0 |
|  89 | MiniMax M2 | 2025-11-01 | 30.0% ±2.7 |
|  91 | MiniMax M2.1 | 2025-12-23 | 29.2% ±2.9 |
|  93 | Claude Haiku 4.5 | 2025-10-31 | 28.3% ±2.9 |
|  94 | Kimi K2 Instruct | 2025-11-01 | 27.8% ±2.5 |
| 103 | GLM 4.6 | 2025-11-01 | 24.5% ±2.4 |
| 104 | GPT-5-Mini | 2025-10-31 | 24.0% ±2.5 |
| 105 | Qwen 3 Coder 480B | 2025-11-01 | 23.9% ±2.8 |
| 106 | Grok 4 | 2025-10-31 | 23.1% ±2.9 |
| 109 | GPT-OSS-120B | 2025-11-01 | 18.7% ±2.7 |
| 111 | AfterQuery-GPT-OSS-20B | 2026-03-31 | 17.0% ±2.5 |
| 112 | Gemini 2.5 Flash | 2025-10-31 | 16.9% ±2.4 |
| 116 | Grok Code Fast 1 | 2025-10-31 | 14.2% ±2.5 |
| 121 | GPT-5-Nano | 2025-10-31 | 7.9% ±1.9 |
| 124 | GPT-OSS-20B | 2025-11-01 | 3.1% ±1.5 |

## Reading the Ladder

- Best Terminus 2 result is **#25 with GPT-5.3-Codex (64.7%)** — note that
  the same model in a richer harness (e.g. Codex, Mux, ForgeCode) reaches
  74-82%, illustrating the scaffold tax.
- The lowest-scoring rows (`GPT-OSS-20B = 3.1%`, `GPT-5-Nano = 7.9%`) are
  not "bad agent" — they are small-context / small-capability models
  failing at the unaided-tmux task.
- AfterQuery published an in-house variant of `GPT-OSS-20B` (entry #111
  at 17.0%) — a >5x lift over baseline `GPT-OSS-20B` — suggesting the
  scaffold has been used as a fine-tuning evaluator for AfterQuery's own
  model work.

## Other Benchmarks

No public scores from Terminus 2 outside Terminal-Bench. As a reference
scaffold rather than a product, it isn't really intended for SWE-Bench,
Aider polyglot, or HumanEval comparisons.

---

*Numbers verbatim from the TB2 leaderboard (2026-05-01).*
