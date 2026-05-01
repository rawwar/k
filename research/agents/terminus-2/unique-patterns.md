# Terminus 2 — Unique Patterns

## 1. The Reference-Scaffold Philosophy

Terminus 2's defining characteristic is what it *isn't*. It deliberately
omits every modern agent affordance — sub-agents, MCP, skills, file-edit
tools, repo maps, summarisation, planning specialists — so that leaderboard
results expose model capability rather than harness engineering.

This makes it the **most model-comparable** agent on TB2: when you see
"Terminus 2 / Model X = 54.0%" and "Terminus 2 / Model Y = 47.6%", the
delta is almost entirely the models talking.

## 2. JSON or XML Parser Choice

Two parsers ship in the box, controlled by `parser_name`. Most modern
agents commit to one structured format. Terminus 2 keeps both because some
models still produce more reliable XML than escaped JSON, and the project
needs to be fair to all of them.

## 3. `(keystrokes, duration_sec)` as the Primitive

Modelling each action as a tmux keystroke buffer plus a wait time, rather
than a shell command and a return code, is unusual. It:

- captures interactive TUI workflows (editors, REPLs, `top`),
- forces the model to reason about *time* (how long will this build take?),
- avoids the fiction that every command has a clean exit boundary.

The tradeoff is that the model carries more cognitive load — it has to
predict how long things take and recover when it guesses wrong.

## 4. Two-Step Completion Confirmation

`_pending_completion` requires the model to declare "done" twice in a row.
A common LLM failure mode on coding tasks is premature victory; the
confirm-on-second-turn pattern catches it cheaply.

## 5. Effectively Unlimited Episodes

Default `max_episodes = 1_000_000` and a warning if you set anything lower.
This is the opposite of most production agents (which cap turns aggressively
for cost). The reasoning: the bench cares about *capability* not cost, so
let the model thrash if it needs to.

## 6. Timeout-as-Prompt

Long-running commands don't get killed at `duration_sec`. Instead the
`timeout.txt` template is appended to chat, telling the model the command
is still going and asking what to do. This lets multi-minute builds survive
under model supervision rather than being interrupted.

## 7. LiteLLM-First

By targeting `LiteLLM` rather than a specific provider SDK, Terminus 2
gets every commercial and open model "for free". This is why the same
agent appears under 20+ different model rows on the leaderboard, including
`GPT-OSS-20B`, `GLM 4.6`, `Kimi K2`, `Grok 4`, `Qwen 3 Coder 480B`, and
`AfterQuery-GPT-OSS-20B`.

## Comparison

| Pattern | Terminus 2 | Claude Code | Mux | Letta Code |
|---|---|---|---|---|
| Tool surface | 1 (tmux keys) | ~15 | ~10 | ~10+skills |
| Sub-agents | ❌ | ✅ | ✅ | ✅ |
| MCP | ❌ | ✅ | (limited) | ✅ |
| Memory | ❌ | session | session+compaction | persistent |
| Episode cap | none | low | low | low |
| Purpose | benchmark scaffold | product | product | product |

---

*Patterns identified directly from public source.*
