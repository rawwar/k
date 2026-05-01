# grok-cli — Tool System

## Built-In Tools

Grouped by category from the README:

### Search
- `search_x` — live xAI-backed search of X (the platform formerly known
  as Twitter)
- `search_web` — live web search

These are the most distinctive built-ins. `search_x` is unique to
grok-cli on the leaderboard — no other agent has X-native search as a
first-class tool.

### Media generation
- `generate_image` — text-to-image, image editing
- `generate_video` — text-to-video, image-to-video, up to ~6 seconds
- Output saved under `.grok/generated-media/` so files outlive the xAI
  signed URLs

### Verify
- `verify` sub-agent (also exposed as `/verify` and `--verify`)

### Computer use (`computer` sub-agent, macOS only)
- `computer_snapshot` — accessibility snapshot, returns refs (`@e1`, `@e2`, …)
- `computer_click @ref`
- `computer_type "text"`
- `computer_scroll`
- `computer_screenshot` — pixel screenshot, fallback

### Bash + filesystem
- Standard bash-first tooling (the README calls out a "bash-first"
  design philosophy)

## Sub-Agents as Tools

Sub-agents are invoked via `task` (foreground) and `delegate` (background
read-only). The model sees them as additional callable units. Six
reserved built-ins (`general`, `explore`, `vision`, `verify`, `computer`,
`delegate`); custom ones via `subAgents` in `~/.grok/user-settings.json`.

## Skills

```
.agents/skills/<name>/SKILL.md        # project-level
~/.agents/skills/                     # user-level
```

`/skills` in the TUI lists what's installed. Skills follow the
Claude-Code-shaped convention.

## MCP

Configured via `/mcps` in the TUI or `mcpServers` block in
`.grok/settings.json`. Standard MCP server registration. The README does
not enumerate transport modes (stdio / SSE) but MCP itself supports both
and grok-cli appears to inherit the standard.

## API Choice

Three places to set the API key:

1. `GROK_API_KEY` environment variable (CI-friendly)
2. `.env` in project (with `.env.example` as template)
3. `grok -k <key>` to register at runtime
4. `apiKey` field in `~/.grok/user-settings.json`

Optional knobs: `GROK_BASE_URL` (default `https://api.x.ai/v1`),
`GROK_MODEL`, `GROK_MAX_TOKENS`.

## Models

Defaults tuned for Grok. `grok models` lists the menu, including:

- `grok-code-fast-1` — fast coding default
- `grok-4-1-fast-reasoning` — reasoning variant
- `grok-4.20-multi-agent-0309` — the model used for the TB2 #43 entry
- Flagship and fast variants

The Telegram-paired control flow does not change the model selection —
remote messages drive the same loop with the same configured model.

---

*Tool surface enumerated from `superagent-ai/grok-cli` README.*
