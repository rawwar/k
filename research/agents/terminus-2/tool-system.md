# Terminus 2 — Tool System

## One Tool

Terminus 2 has effectively **one tool**: send keystrokes to a tmux pane and
wait a specified number of seconds. Everything else — file editing, package
install, build, test, git operations — is achieved by typing the appropriate
shell commands into that pane.

```python
@dataclass
class Command:
    keystrokes: str
    duration_sec: float
```

This is a deliberate design choice. It makes the scaffold model-agnostic
(no function-calling required) and makes leaderboard comparisons clean
(differences are attributable to the model, not to a richer tool surface).

## Command Format

### JSON parser (default)

The model is prompted to emit something like:

```json
{
  "commands": [
    {"keystrokes": "ls -la\n", "duration_sec": 1.0},
    {"keystrokes": "pytest -x\n", "duration_sec": 60.0}
  ],
  "is_task_complete": false
}
```

`TerminusJSONPlainParser` extracts the `commands` array and the
`is_task_complete` flag. Trailing newlines in `keystrokes` matter — they are
the equivalent of pressing Enter.

### XML parser (alternative)

For models that produce more reliable XML than JSON, the same information is
expressed in `<command>` blocks:

```xml
<commands>
  <command><keystrokes>ls -la
</keystrokes><duration_sec>1.0</duration_sec></command>
</commands>
<is_task_complete>false</is_task_complete>
```

The XML parser exists so weaker or older models that mangle escaped JSON
strings can still drive the agent.

## Special Keystrokes

`keystrokes` is a raw string passed through to tmux `send-keys`, so it can
include control characters: `C-c` to interrupt, `C-d` to EOF, escape
sequences, arrow keys for editor navigation. The model has to know tmux
keystroke syntax — there is no abstraction layer.

## No File-Edit Tool

Unlike most modern agents (Claude Code, Codex, Aider, Goose), Terminus 2
does not provide `read_file`, `write_file`, or any structured edit tool. To
edit a file the model must:

- `cat` it,
- open `vi`/`nano`/`sed`,
- use `heredoc` redirection,
- or pipe shell commands.

This is one of the most-cited reasons Terminus 2 underperforms purpose-built
agents on Terminal-Bench, and one of the strongest arguments for why
"reference scaffold" results matter — they are showing you what the *model*
can do unaided.

## No Sub-Agents, No MCP

The loop is single-process and single-LLM. There is no notion of delegating
to a worker agent, no MCP server registry, and no skills directory. If a
task needs more than one model call, the same model just runs more episodes.

---

*Tool semantics from `terminus_2.py` and the JSON/XML parser sources at
`harbor-framework/terminal-bench`.*
