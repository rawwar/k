# Mux — Tool System

## Tool List (from instruction docs)

The Mux instruction-files documentation enumerates the common tools that
custom `Tool:` blocks can target:

- `bash`
- `file_read`
- `file_edit_replace_string`
- `file_edit_insert`
- `propose_plan`
- `ask_user_question`
- `todo_write`
- `todo_read`
- `web_fetch`
- `web_search`

This is a typical modern coding-agent surface — close to Claude Code minus
the MCP-everything firehose.

## Tool Description Augmentation

The `## Tool: <tool_name>` heading in `AGENTS.md` appends content to a
tool's description rather than the system prompt:

```markdown
## Tool: bash
- Use `rg` instead of `grep` for file searching

## Tool: file_edit_replace_string
- Run `prettier --write` after editing files

## Tool: todo_write
- Keep the TODO list current during multi-step work; sidebar progress is derived from it.
```

This is a clean separation: the LLM sees the augmentation only when
considering that tool, not in the global preamble. Note also the third
example — Mux's UI sidebar reads `todo_write` state to render workspace
progress, which is unusual (most agents treat todos as model-only).

## Tool Availability per Model

Tools are matched case-insensitive but only augmented for tools the active
model actually has. Different providers expose slightly different sets, and
Mux respects that.

## MCP

The README and docs do not foreground MCP support. There is an
`mcpServers` configuration mentioned in adjacent agents (e.g. `grok-cli`)
but Mux's docs site does not have a dedicated MCP page as of 2026-05.

## Plan / Edit / Bash Triad

The user-visible tool surface in normal use is:

- `propose_plan` — read-only plan, gate to exec mode
- `bash` — workspace shell (executes inside the active runtime: local,
  worktree, or SSH)
- `file_read` / `file_edit_*` — structured file mutations

Sub-agent delegation is not explicitly documented; the parallelism story is
"open multiple workspaces" rather than "one agent spawns many".

---

*Tool surface from <https://mux.coder.com/agents/instruction-files>.*
