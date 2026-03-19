# Tool System

> mini-SWE-agent has exactly one tool: bash. This is not a limitation -- it's the point.

## Philosophy: Bash Is All You Need

The original SWE-agent (2024) invested heavily in custom tool interfaces -- specialized file editors, search tools, context managers, and navigation commands. These tools had carefully designed interfaces optimized for LM interaction.

mini-SWE-agent throws all of that away. It gives the LM a single tool -- execute a bash command -- and trusts the model to figure out the rest. This reflects a fundamental shift in thinking:

> **Instead of building intelligence into the scaffold, put the LM in the center and let it use the shell to its full potential.**

Want the agent to search files? It can use `grep`, `find`, `ripgrep`. Want it to edit files? It can use `sed`, `cat <<'EOF'`, or any editor. Want it to open a PR? Tell it to use `gh pr create`. The LM already knows how to use these tools from its training data.

## Implementation

### The Single Tool Definition

The entire tool system is defined in one dictionary:

```python
BASH_TOOL = {
    "type": "function",
    "function": {
        "name": "bash",
        "description": "Execute a bash command",
        "parameters": {
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute",
                }
            },
            "required": ["command"],
        },
    },
}
```

This is passed to `litellm.completion()` as the `tools` parameter:

```python
def _query(self, messages, **kwargs):
    return litellm.completion(
        model=self.config.model_name,
        messages=messages,
        tools=[BASH_TOOL],
        **(self.config.model_kwargs | kwargs),
    )
```

### Tool Call Parsing

When the LM responds with a tool call, parsing is straightforward -- validate it's a `bash` call and extract the command:

```python
def parse_toolcall_actions(tool_calls: list, *, format_error_template: str) -> list[dict]:
    if not tool_calls:
        raise FormatError(...)  # Every response MUST include a tool call

    actions = []
    for tool_call in tool_calls:
        args = json.loads(tool_call.function.arguments)
        if tool_call.function.name != "bash":
            raise FormatError(...)  # Unknown tool
        if "command" not in args:
            raise FormatError(...)  # Missing command
        actions.append({"command": args["command"], "tool_call_id": tool_call.id})
    return actions
```

### Alternative: Text-Based Parsing

For models that don't support tool calling (or for simpler setups), mini-SWE-agent also supports **triple-backtick parsing**. The LM formats its action as:

```
THOUGHT: I need to explore the repository structure.

` ``mswea_bash_command
ls -la
` ``
```

This is parsed with a simple regex, making mini-SWE-agent compatible with literally any model that can generate text -- including open-source models served via local inference.

### Observation Formatting

After execution, the output is rendered via a Jinja2 template:

```python
observation_template: str = (
    '{% if output.exception_info %}<exception>{{output.exception_info}}</exception>\n{% endif %}'
    '<returncode>{{output.returncode}}</returncode>\n<output>\n{{output.output}}</output>'
)
```

A typical observation message looks like:

```xml
<returncode>0</returncode>
<output>
total 48
drwxr-xr-x  12 user  staff   384 Jun 15 10:23 .
drwxr-xr-x   5 user  staff   160 Jun 15 10:20 ..
-rw-r--r--   1 user  staff  1234 Jun 15 10:23 main.py
</output>
```

### Output Truncation

The default config includes intelligent output truncation for long outputs (>10,000 chars):

```yaml
observation_template: |
  {% if output.output | length < 10000 %}
  <output>{{ output.output }}</output>
  {% else %}
  <warning>The output of your last command was too long.</warning>
  <output_head>{{ output.output[:5000] }}</output_head>
  <elided_chars>{{ elided_chars }} characters elided</elided_chars>
  <output_tail>{{ output.output[-5000:] }}</output_tail>
  {% endif %}
```

This gives the LM the head and tail of long outputs with a clear indication that content was elided -- prompting it to use more targeted commands.

## The System Prompt: Teaching Bash Conventions

The system prompt in `default.yaml` teaches the LM the key conventions:

```yaml
system_template: |
  Your response must contain exactly ONE bash code block with ONE command
  (or commands connected with && or ||).
  Include a THOUGHT section before your command.

  ## Important Rules
  1. Every response must contain exactly one action
  2. Directory or environment variable changes are not persistent.
     Every action is executed in a new subshell.
     However, you can prefix any action with
     `MY_ENV_VAR=MY_VALUE cd /path/to/working/dir && ...`
```

It also provides concrete examples of common operations:

```yaml
  ## Useful command examples

  ### Create a new file:
  cat <<'EOF' > newfile.py
  import numpy as np
  print("hello")
  EOF

  ### Edit files with sed:
  sed -i 's/old_string/new_string/g' filename.py

  ### View file content:
  nl -ba filename.py | sed -n '10,20p'
```

## Why Bash-Only Works

### 1. Bash Is the Universal Interface

Every development environment has bash (or a POSIX shell). Docker containers, VMs, cloud instances, CI/CD pipelines -- bash is always there. This means:
- **Zero setup in sandboxes** -- no pip install, no tool compilation
- **Universal model compatibility** -- any model that can write code can write bash
- **No abstraction leakage** -- the LM interacts with the real system, not an abstraction

### 2. LMs Already Know Bash

Modern LMs have been trained on millions of bash commands, scripts, and terminal sessions. They know `grep`, `sed`, `find`, `git`, `python`, `curl`, and hundreds of other tools. Custom agent tools are abstractions *on top of* things the LM already knows how to use.

### 3. Custom Tools Create Coupling

Every custom tool is:
- Code to maintain
- An interface to document
- A potential source of bugs
- A thing to install in every environment
- A thing to test across model versions

Bash avoids all of this coupling.

### 4. Diminishing Returns on Tool Sophistication

The SWE-agent team's own data shows this: as base models improved from GPT-3.5 to GPT-4 to Claude 3.5 to Claude 4+, the gap between "custom tools" and "bash only" shrank dramatically. The sophisticated tools that were essential in 2024 became nice-to-haves in 2025.

## Comparison: SWE-agent Tools vs mini-SWE-agent

| Operation | SWE-agent | mini-SWE-agent |
|-----------|-----------|----------------|
| View file | Custom `open` command with scroll | `cat`, `head`, `tail`, `sed -n` |
| Edit file | Custom line editor with undo | `sed -i`, `cat <<'EOF'` |
| Search | Custom `search_file`, `find_file` | `grep`, `find`, `ripgrep` |
| Navigate | Custom `goto`, `scroll_up/down` | `cd && ls`, `find` |
| Submit | Custom `submit` command | `echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT` |

The mini approach requires the LM to know more about bash -- but modern LMs do. The payoff is radical simplicity in the agent scaffold.

## Format Error Handling

When the LM doesn't follow the expected format, the agent sends a corrective message:

```yaml
format_error_template: |
  Format error:
  <error>{{error}}</error>

  Please always provide EXACTLY ONE action in triple backticks.
  If you want to end the task, please issue:
  `echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT`
```

This is implemented as a `FormatError` exception (inheriting from `InterruptAgentFlow`), which adds the error message to the trajectory and lets the LM try again on the next step.

## Environment Variable Configuration

The default config suppresses interactive pagers and progress bars to prevent hangs:

```yaml
environment:
  env:
    PAGER: cat
    MANPAGER: cat
    LESS: -R
    PIP_PROGRESS_BAR: 'off'
    TQDM_DISABLE: '1'
```

This is critical for `subprocess.run` -- since there's no TTY, interactive pagers would block forever.
