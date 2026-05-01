# grok-cli — Unique Patterns

## 1. X-Native Search as a First-Class Tool

`search_x` exposes live xAI-backed X (Twitter) search to the agent.
No other agent on TB2 has this. For tasks that benefit from real-time
chatter — security advisories, library breakage rumours, library author
opinions — this is genuinely differentiated.

## 2. Telegram Remote Control

`/remote-control → Telegram → /pair` lets the user drive the agent from
a phone while the CLI runs on a workstation. This is closer to "ChatOps
for solo developers" than anything else on the leaderboard. Schedules,
status checks, and follow-up prompts all flow through the bot.

## 3. macOS Desktop Sub-Agent

The `computer` sub-agent uses [`agent-desktop`](https://github.com/lahfir/agent-desktop)
to drive the macOS host: take accessibility snapshots, click on stable
refs (`@e1`), type, scroll. This is "computer use" but tuned for
**actual desktop apps**, not just browsers — useful for QA flows that
involve native UIs.

## 4. Recurring Schedules via Daemon

`grok daemon --background` plus a natural-language schedule definition
gives you a cron-like recurring agent without leaving the CLI. The
combination of schedules + `--batch-api` is a thoughtful answer to
"unattended overnight work without burning cost".

## 5. Built-in Image / Video Generation

`generate_image` and `generate_video` are exposed as agent tools. The
agent can "produce a hero image for the README" or "animate the cover
art" without leaving the chat. Outputs land under `.grok/generated-media/`
so they outlive the temporary xAI signed URLs.

## 6. NDJSON Event Stream

`--format json` produces a clean machine-readable stream — easy to pipe
into other tools, observability, or downstream agents. Few CLIs publish
their loop events as a stable schema like this.

## 7. Sub-Agents On By Default

The README repeats this several times: sub-agents are not a power-user
opt-in, they're the default. `task` (foreground) and `delegate`
(background) are the two dispatch verbs.

## 8. Community Project, Not xAI

The disclaimer in the README ("not affiliated with xAI Corp") is unusual
on a leaderboard agent — most are first-party. The TB2 leaderboard
attributes the agent to **"Vibe Kit"** (the team's brand;
`superagent-ai` on GitHub).

## Comparison

| Pattern | grok-cli | Codex | Claude Code | Letta Code |
|---|---|---|---|---|
| Live X search | ✅ | ❌ | ❌ | ❌ |
| Telegram remote | ✅ | ❌ | ❌ | ✅ (via channels) |
| Built-in computer use | ✅ macOS | ❌ | ❌ | ❌ |
| Schedules / daemon | ✅ | ❌ | ❌ | ❌ |
| Image/video generation | ✅ | ❌ | ❌ | ❌ |
| First-party / community | community | first-party | first-party | first-party |

---

*Patterns from `superagent-ai/grok-cli` README.*
