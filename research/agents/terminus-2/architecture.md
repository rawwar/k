# Terminus 2 — Architecture

## Source Layout

Lives at `terminal_bench/agents/terminus_2/` inside the
[`harbor-framework/terminal-bench`](https://github.com/harbor-framework/terminal-bench)
repository. Three files:

```
terminus_2/
  __init__.py
  terminus_2.py                  # Main agent class (~23 KB)
  terminus_json_plain_parser.py  # JSON command parser
  terminus_xml_plain_parser.py   # XML command parser
```

The prompt templates live one level up at
`terminal_bench/agents/prompt-templates/terminus-{json,xml}-plain.txt` and
`timeout.txt`.

## Class Layout

```python
class Terminus2(BaseAgent):
    def __init__(
        self,
        model_name: str,
        max_episodes: int | None = None,
        parser_name: str = "json",     # "json" | "xml"
        api_base: str | None = None,
        temperature: float = 0.7,
        **kwargs,
    ):
        ...
        self._llm = LiteLLM(model_name, api_base, temperature)
        self._parser = self._get_parser()        # JSON or XML
        self._prompt_template = ...              # loaded from disk
        self._timeout_template = ...
        self._max_episodes = max_episodes or 1_000_000
        self._chat: Chat | None = None
        self._timestamped_markers: list[tuple[float, str]] = []
        self._pending_completion = False
```

## Components

### `LiteLLM`
Thin wrapper around `litellm` so any model that LiteLLM understands can drive
the agent — this is what lets the same Terminus-2 scaffold appear under
GPT-5.3-Codex, Claude Opus 4.6, Gemini 3 Pro, GLM 5, Kimi K2.5, MiniMax M2,
DeepSeek-V3.2, Grok 4, Qwen 3 Coder 480B, and more.

### `Chat`
A mutable conversation buffer used as the model's working memory. Messages
are appended turn by turn; surfaces `ContextLengthExceededError` and
`OutputLengthExceededError` so the agent can fail gracefully when a small
model runs out of room.

### `TmuxSession`
The execution surface. Terminus 2 does not call shell commands directly — it
sends keystrokes into a tmux pane and waits a model-specified duration, then
reads back the screen. This matches Terminal-Bench's evaluation model and is
the single most important architectural choice in the scaffold.

### Parsers
`TerminusJSONPlainParser` and `TerminusXMLPlainParser` extract a sequence of
`Command(keystrokes, duration_sec)` records from a model response. Choice is
controlled by the `parser_name` constructor arg; the leaderboard does not
publish per-row parser settings, but JSON is the default.

## Failure Modes

The agent catches and reports `FailureMode` values when:

- The model exceeds context length mid-task
- The model exceeds its output length budget
- The parser cannot extract any valid command
- The episode budget is exhausted

These are recorded so leaderboard runs can attribute losses to the model vs.
the harness.

## What's Deliberately Absent

- No sub-agents, no delegation
- No MCP integration
- No skills or memory blocks
- No file-edit tools — edits happen via shell (`sed`, editors in tmux)
- No planning / observation / summary specialists
- No retrieval, no embedding, no repo map

This minimalism is the point: anything Terminus 2 *can't* do is something the
underlying model has to figure out from raw tmux interaction.

---

*All architectural claims here are read directly from the public source at
`harbor-framework/terminal-bench@1a6ffa9` (`terminus_2.py`).*
