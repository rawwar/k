# OpenHands Tool System — Deep Technical Analysis

> How tools are defined, mapped to actions, executed by the runtime, and returned as
> observations in the OpenHands agent framework.

---

## 1. Architecture Overview

OpenHands uses a clean three-layer pipeline to translate LLM tool calls into
sandboxed execution:

```
┌─────────┐   tool_call JSON    ┌──────────────────┐   Action dataclass   ┌─────────────┐
│   LLM   │ ──────────────────► │ function_calling  │ ──────────────────► │   Runtime   │
│ (GPT/   │                     │     .py           │                     │ (sandbox)   │
│  Claude) │ ◄────────────────── │                   │ ◄────────────────── │             │
└─────────┘   Observation text   └──────────────────┘   Observation obj    └─────────────┘
```

**Key insight:** "Tools" never exist as executable objects inside the agent.
They are _schemas_ — `ChatCompletionToolParam` dicts following the OpenAI
function-calling spec — that tell the LLM what it can call. The actual
execution happens through Action → Runtime → Observation.

### Core types

| Layer | Type | Location |
|-------|------|----------|
| Schema | `ChatCompletionToolParam` | `tools/*.py` |
| Intent | `Action` dataclass | `openhands/events/action/` |
| Result | `Observation` dataclass | `openhands/events/observation/` |
| Mapping | `response_to_actions()` | `openhands/agenthub/codeact_agent/function_calling.py` |

---

## 2. Tool Definitions

Tools are defined in `openhands/agenthub/codeact_agent/tools/`. Each module
exports a `ChatCompletionToolParam` dict (or a factory function that returns
one). The schemas use standard JSON Schema inside the `"parameters"` key.

### 2.1 `execute_bash` → `CmdRunAction`

Runs a shell command inside the sandboxed runtime container.

```json
{
  "type": "function",
  "function": {
    "name": "execute_bash",
    "description": "Execute a bash command in the terminal ...",
    "parameters": {
      "type": "object",
      "properties": {
        "command":  { "type": "string", "description": "The bash command to run" },
        "is_input": { "type": "boolean", "default": false },
        "timeout":  { "type": "integer" }
      },
      "required": ["command"]
    }
  }
}
```

**Behavioral notes:**

- When `is_input=false` (default), the command is run fresh in a blocking
  shell session. A `timeout` (seconds) controls how long to wait before
  forcefully terminating the process.
- When `is_input=true`, the string is sent as stdin to the _already-running_
  process. This is how the agent interacts with interactive programs (e.g.,
  sending `y` to a confirmation prompt, pressing Ctrl-C via special chars).
- The factory `create_cmd_run_tool(cwd)` injects the current working directory
  into the description so the LLM knows where commands execute.

**Action mapping:**

```python
CmdRunAction(
    command=args["command"],
    is_input=args.get("is_input", False),
    blocking=True,
    timeout=args.get("timeout", DEFAULT_TIMEOUT),
)
```

**Observation:** `CmdOutputObservation` containing `content` (stdout+stderr),
`exit_code`, and `command_id`.

---

### 2.2 `execute_ipython_cell` → `IPythonRunCellAction`

Runs Python code in a persistent Jupyter IPython kernel inside the sandbox.

```json
{
  "type": "function",
  "function": {
    "name": "execute_ipython_cell",
    "parameters": {
      "type": "object",
      "properties": {
        "code": { "type": "string" }
      },
      "required": ["code"]
    }
  }
}
```

**Behavioral notes:**

- The kernel is booted once when the runtime starts (via `JupyterRequirement`
  plugin) and persists across calls — variables, imports, and state are
  retained.
- `AgentSkills` are automatically imported into the kernel namespace. These
  are a curated set of file-manipulation utilities (`open_file`,
  `edit_file_by_replace`, `append_file`, `search_dir`, etc.) that give the
  agent higher-level file operations without raw shell commands.
- Rich output (images, HTML, DataFrames) is captured and serialized back in
  the observation.

**Action mapping:**

```python
IPythonRunCellAction(code=args["code"])
```

**Observation:** `IPythonRunCellObservation` with cell output text.

---

### 2.3 `str_replace_editor` → `FileEditAction` / `FileReadAction`

A multi-command file editor modeled after Anthropic's `text_editor` tool
design. A single tool with a `command` discriminator dispatches to different
file operations.

```json
{
  "type": "function",
  "function": {
    "name": "str_replace_editor",
    "parameters": {
      "type": "object",
      "properties": {
        "command":     { "type": "string", "enum": ["view","create","str_replace","insert","undo_edit"] },
        "path":        { "type": "string" },
        "file_text":   { "type": "string" },
        "old_str":     { "type": "string" },
        "new_str":     { "type": "string" },
        "insert_line": { "type": "integer" },
        "view_range":  { "type": "array", "items": { "type": "integer" } }
      },
      "required": ["command", "path"]
    }
  }
}
```

**Command dispatch:**

| Command | Required params | Action created | Source tag |
|---------|----------------|----------------|-----------|
| `view` | `path`, optional `view_range` | `FileReadAction(path, view_range)` | `OH_ACI` |
| `create` | `path`, `file_text` | `FileEditAction(path, content=file_text)` | `OH_ACI` |
| `str_replace` | `path`, `old_str`, `new_str` | `FileEditAction(path, old_str, new_str)` | `OH_ACI` |
| `insert` | `path`, `new_str`, `insert_line` | `FileEditAction(path, new_str, insert_line)` | `OH_ACI` |
| `undo_edit` | `path` | `FileEditAction(path, undo=True)` | `OH_ACI` |

**Why `OH_ACI`?** The `source` field distinguishes _how_ the edit was
requested. `OH_ACI` = "OpenHands Agentic Code Interface" — the str_replace
editor. This matters because the runtime handles `OH_ACI` edits differently
from `LLM_BASED_EDIT` edits (see §2.7).

The `str_replace` command requires `old_str` to match _exactly one_ location
in the file. If it matches zero or multiple locations, the action fails with a
descriptive error in the observation.

---

### 2.4 `browser` → `BrowseInteractiveAction`

Drives a headless Chromium browser via Playwright for web interaction.

```json
{
  "type": "function",
  "function": {
    "name": "browser",
    "parameters": {
      "type": "object",
      "properties": {
        "code": { "type": "string", "description": "Browser automation code to execute" }
      },
      "required": ["code"]
    }
  }
}
```

**Behavioral notes:**

- The `code` parameter contains browsing action strings (e.g., `goto("https://...")`,
  `click(element_id)`, `type(element_id, "text")`, `scroll(direction)`).
- The `BrowserEnv` maintains page state across calls — navigation, cookies,
  and DOM persist.
- Observations include: current URL, page title, a text-accessibility-tree
  representation of visible content, open tabs, and optionally a base64
  screenshot.

**Action mapping:**

```python
BrowseInteractiveAction(browser_actions=args["code"])
```

**Observation:** `BrowserOutputObservation` with `url`, `content` (accessibility
tree), `screenshot` (base64), `open_pages`, `active_page_index`.

---

### 2.5 `finish` → `AgentFinishAction`

Signals that the agent considers the task complete.

```json
{
  "function": {
    "name": "finish",
    "parameters": {
      "properties": {
        "message": { "type": "string" }
      }
    }
  }
}
```

**Action mapping:**

```python
AgentFinishAction(final_thought=args.get("message", ""))
```

This action terminates the agent loop. The controller checks for
`AgentFinishAction` in the event stream and transitions the agent state to
`FINISHED`.

---

### 2.6 `think` → `AgentThinkAction`

An extended thinking / scratchpad tool. The LLM uses this to reason through
complex problems without taking any external action.

```json
{
  "function": {
    "name": "think",
    "parameters": {
      "properties": {
        "thought": { "type": "string" }
      },
      "required": ["thought"]
    }
  }
}
```

**Action mapping:**

```python
AgentThinkAction(thought=args["thought"])
```

The observation is essentially a no-op acknowledgment. Think actions are
recorded in the event stream (useful for debugging/replay) but trigger no
runtime execution.

---

### 2.7 LLM-Based File Editor (Deprecated)

An older approach where the LLM provides full or partial file content and the
runtime uses another LLM call to apply the edit intelligently.

| Parameter | Description |
|-----------|-------------|
| `path` | File to edit |
| `content` | The desired content or edit instructions |
| `start` | Start line number (optional) |
| `end` | End line number (optional) |

**Action mapping:**

```python
FileEditAction(path=path, content=content, start=start, end=end)
# source = FileEditSource.LLM_BASED_EDIT
```

Deprecated in favor of `str_replace_editor` because deterministic
string-replacement is more reliable and cheaper than an additional LLM call
for every edit.

---

### 2.8 `condensation_request` → `CondensationRequestAction`

Allows the agent to explicitly request that the conversation history be
condensed (summarized) when the context window is becoming saturated.

```json
{
  "function": {
    "name": "condensation_request",
    "parameters": {
      "type": "object",
      "properties": {}
    }
  }
}
```

No parameters. The controller intercepts this action and triggers the
configured condensation strategy (e.g., an LLM-based summarizer that
compresses older events while preserving critical context).

---

### 2.9 `task_tracker` → `TaskTrackingAction`

A structured planning tool for the agent to maintain a task list with
status tracking.

```json
{
  "function": {
    "name": "task_tracker",
    "parameters": {
      "properties": {
        "command":   { "type": "string", "enum": ["view", "plan"] },
        "task_list": { "type": "string" }
      },
      "required": ["command"]
    }
  }
}
```

- `view`: Returns the current task list state.
- `plan`: Accepts a `task_list` (markdown-formatted list with status markers)
  and stores/updates it.

**Action mapping:**

```python
TaskTrackingAction(command=args["command"], task_list=args.get("task_list", ""))
```

---

## 3. MCP (Model Context Protocol) Tools

OpenHands supports dynamic tool injection via MCP servers, enabling
microagents to bring their own tools.

### How MCP tools are loaded

1. **Microagent definition** includes an `mcp` block specifying stdio-based
   MCP server configurations (command, args, env).
2. At agent initialization, `agent.set_mcp_tools(mcp_tools)` registers them.
3. MCP tool schemas are converted to `ChatCompletionToolParam` and appended
   to the tool list sent to the LLM.
4. Tool names from MCP servers are tracked in a set so that
   `function_calling.py` can distinguish them from built-in tools.

### MCP action flow

```
LLM calls tool "mcp_tool_name"
  → function_calling.py detects name is in mcp_tool_names set
  → MCPAction(name="mcp_tool_name", arguments={...})
  → Runtime routes to MCP client
  → MCP server executes, returns result
  → MCPObservation(content=result)
```

MCP tools are first-class citizens — they appear alongside built-in tools
in the LLM prompt and follow the same Action → Observation cycle.

---

## 4. Tool Configuration

Tools are conditionally assembled based on `AgentConfig` flags. This happens
in the CodeAct agent's `get_tools()` method:

```python
def get_tools(self) -> list[ChatCompletionToolParam]:
    tools = []

    if self.config.enable_cmd:
        tools.append(create_cmd_run_tool(cwd=self.initial_cwd))

    if self.config.enable_think:
        tools.append(ThinkTool)

    if self.config.enable_finish:
        tools.append(FinishTool)

    if self.config.enable_browsing:
        tools.append(BrowserTool)

    if self.config.enable_jupyter:
        tools.append(IPythonTool)

    if self.config.enable_plan_mode:
        tools.append(create_task_tracker_tool(task_list=...))

    # Mutually exclusive editor modes
    if self.config.enable_llm_editor:
        tools.append(LLMBasedFileEditTool)
    elif self.config.enable_editor:
        tools.append(create_str_replace_editor_tool(cwd=self.initial_cwd))

    if self.config.enable_condensation_request:
        tools.append(CondensationRequestTool)

    # MCP tools appended dynamically
    tools.extend(self.mcp_tools)

    return tools
```

### Default configuration

A typical CodeAct agent session enables: `execute_bash`,
`str_replace_editor`, `think`, `finish`. Browsing and Jupyter are enabled
based on workspace needs. The LLM-based editor is off by default.

---

## 5. Tool Description Adaptation

Different LLM providers have varying token limits for tool descriptions.
OpenHands maintains two description variants:

- **Full descriptions:** Detailed instructions with examples (~2000+ tokens).
- **Short descriptions:** Compact versions (< 1024 tokens) for models with
  strict tool-description token limits.

```python
# In tool factory functions
def create_cmd_run_tool(cwd: str, short: bool = False) -> ChatCompletionToolParam:
    if short:
        description = CMD_RUN_TOOL_SHORT_DESCRIPTION
    else:
        description = CMD_RUN_TOOL_DESCRIPTION.format(cwd=cwd)
    ...
```

Short descriptions are selected for specific model families: `gpt-4*`,
`o1*`, `o3*`, `o4*`. This is determined at agent initialization based on the
configured model name.

---

## 6. Action Execution Flow (Detailed)

### Step-by-step walkthrough

```
 Step 1: LLM Response
 ─────────────────────
 The LLM returns a ChatCompletion with one or more tool_calls:
   tool_calls: [
     { id: "call_abc123",
       function: { name: "execute_bash", arguments: '{"command":"ls -la"}' } },
     { id: "call_def456",
       function: { name: "str_replace_editor", arguments: '{"command":"view","path":"/app/main.py"}' } }
   ]

 Step 2: Parsing & Validation (function_calling.py → response_to_actions)
 ────────────────────────────────────────────────────────────────────────
 For each tool_call:
   a. Parse arguments JSON (with repair for malformed JSON)
   b. Route by function name to the appropriate handler
   c. Validate required parameters
   d. Construct the Action dataclass
   e. Attach ToolCallMetadata:
        ToolCallMetadata(
            tool_call_id="call_abc123",
            function_name="execute_bash",
            model_response=response,
            total_calls_in_response=2,
        )

 Step 3: Event Stream
 ────────────────────
 Each Action is added to the EventStream with a unique event ID.
 The controller processes actions sequentially (by default) or can
 handle parallel tool calls depending on configuration.

 Step 4: Runtime Execution
 ─────────────────────────
 The Runtime (typically a Docker sandbox via EventStreamRuntime)
 receives each Action:
   - CmdRunAction     → executed via ActionExecutionServer's bash handler
   - FileEditAction   → executed via file manipulation in the sandbox
   - FileReadAction   → reads file content from sandbox filesystem
   - IPythonRunCell   → sent to the Jupyter kernel via ZMQ
   - BrowseInteractive → routed to BrowserEnv (Playwright)
   - MCPAction        → routed to the appropriate MCP server client

 Step 5: Observation
 ───────────────────
 The Runtime returns an Observation with:
   - content: The result text/data
   - cause: The event ID of the originating Action (linkage)
   The observation is added to the EventStream.

 Step 6: Next LLM Turn
 ─────────────────────
 Observations are formatted as tool_call results in the next
 LLM prompt, keyed by tool_call_id for proper multi-tool-call
 response association.
```

### Parallel tool calls

When the LLM returns multiple tool calls in a single response, OpenHands
processes them and returns all observations together in the next LLM turn.
The `total_calls_in_response` field in `ToolCallMetadata` tracks how many
sibling calls exist.

---

## 7. Security Model

Every Action carries security metadata:

```python
class Action:
    security_risk: ActionSecurityRisk = ActionSecurityRisk.UNKNOWN
    confirmation_state: ActionConfirmationStatus = ActionConfirmationStatus.CONFIRMED
```

### Security risk levels

| Level | Meaning |
|-------|---------|
| `UNKNOWN` | Not yet evaluated |
| `LOW` | Safe operation (file reads, think, finish) |
| `MEDIUM` | Potentially impactful (file writes, installs) |
| `HIGH` | Dangerous (rm -rf, network access, credential use) |

### Confirmation flow

1. `SecurityAnalyzer` evaluates each Action before execution.
2. If risk exceeds the configured threshold, `confirmation_state` is set to
   `PENDING`.
3. A pending action pauses the agent loop until a human confirms or rejects.
4. `REJECTED` actions are not executed; the agent receives an error
   observation and must try an alternative approach.

The security analyzer can be rule-based or LLM-based (using a separate
model call to assess risk).

---

## 8. Plugin System

Plugins extend the runtime sandbox environment before the agent starts
working.

### Plugin types

| Plugin | Purpose |
|--------|---------|
| `AgentSkillsRequirement` | Pre-loads Python utility functions into the IPython kernel |
| `JupyterRequirement` | Boots the IPython/Jupyter kernel inside the sandbox |

### Initialization sequence

```
Runtime container starts
  → ActionExecutionServer initializes
  → Plugins are loaded in order:
      1. JupyterRequirement: starts Jupyter kernel, opens ZMQ channels
      2. AgentSkillsRequirement: imports agent_skills module into kernel
  → Runtime signals "ready" to the controller
  → Agent loop begins
```

### AgentSkills

The `agent_skills` module provides these utilities pre-loaded in IPython:

- `open_file(path, line_number)` — display file with line numbers
- `goto_line(line_number)` — scroll to line in currently open file
- `search_dir(query, dir)` — grep across a directory
- `search_file(query, file)` — grep within a file
- `find_file(filename, dir)` — locate files by name
- `edit_file_by_replace(file, old, new)` — str_replace via Python

These exist as a legacy interface; the `str_replace_editor` tool is now
preferred for file operations.

---

## 9. Complete Tool Reference Table

| Tool Name | Action Class | Observation Class | Config Flag | Default |
|-----------|-------------|-------------------|-------------|---------|
| `execute_bash` | `CmdRunAction` | `CmdOutputObservation` | `enable_cmd` | ✅ On |
| `execute_ipython_cell` | `IPythonRunCellAction` | `IPythonRunCellObservation` | `enable_jupyter` | Conditional |
| `str_replace_editor` | `FileEditAction` / `FileReadAction` | `FileEditObservation` / `FileReadObservation` | `enable_editor` | ✅ On |
| `browser` | `BrowseInteractiveAction` | `BrowserOutputObservation` | `enable_browsing` | Conditional |
| `finish` | `AgentFinishAction` | — (terminates loop) | `enable_finish` | ✅ On |
| `think` | `AgentThinkAction` | `AgentThinkObservation` | `enable_think` | ✅ On |
| `llm_editor` | `FileEditAction` (LLM source) | `FileEditObservation` | `enable_llm_editor` | ❌ Off |
| `condensation_request` | `CondensationRequestAction` | — (controller handles) | `enable_condensation_request` | Conditional |
| `task_tracker` | `TaskTrackingAction` | `TaskTrackingObservation` | `enable_plan_mode` | Conditional |
| *(MCP tools)* | `MCPAction` | `MCPObservation` | Dynamic | Dynamic |

---

## 10. Design Principles & Observations

### Why schemas, not executable tools?

OpenHands deliberately separates _what the LLM can call_ (schemas) from
_how it gets executed_ (Actions + Runtime). This provides:

1. **Sandbox isolation:** The LLM never has direct access to execution
   primitives. All execution happens inside a Docker container.
2. **Serialization:** Actions are dataclasses that serialize cleanly to the
   EventStream, enabling replay, debugging, and audit trails.
3. **Security interception:** The Action layer is the natural point to
   insert security checks before execution.
4. **Runtime flexibility:** The same Action can be executed by different
   Runtime backends (local, Docker, remote Kubernetes) without changing
   tool definitions.

### The str_replace pattern

The `str_replace_editor` design (inspired by Anthropic's approach) is
notable for its reliability:

- Deterministic: exact string matching, no fuzzy edits.
- Verifiable: the agent can `view` to confirm changes.
- Reversible: `undo_edit` reverts the last change.
- Error-resistant: fails cleanly on ambiguous matches.

This is significantly more reliable than asking the LLM to produce entire
file contents or using line-number-based editing.

### Event-sourced architecture

The EventStream is append-only. Every Action and Observation is permanently
recorded. This enables:

- Full session replay for debugging
- Conversation condensation (summarize old events, keep recent ones)
- Branching / forking of agent sessions
- Metrics and analytics on agent behavior

---

## 11. Example: End-to-End Tool Call

```
User: "Create a Python file that prints hello world"

Agent LLM response:
  tool_calls: [{
    id: "call_001",
    function: {
      name: "str_replace_editor",
      arguments: {
        "command": "create",
        "path": "/workspace/hello.py",
        "file_text": "print('hello world')\n"
      }
    }
  }]

function_calling.py:
  → Parses arguments
  → Creates FileEditAction(
        path="/workspace/hello.py",
        content="print('hello world')\n",
        source=FileEditSource.OH_ACI
    )
  → Attaches ToolCallMetadata(tool_call_id="call_001", ...)

EventStream:
  → Event #42: FileEditAction (cause=None)

Runtime (sandbox):
  → Receives FileEditAction
  → Writes file to /workspace/hello.py
  → Returns FileEditObservation(content="File created at /workspace/hello.py")

EventStream:
  → Event #43: FileEditObservation (cause=#42)

Next LLM turn includes:
  role: "tool", tool_call_id: "call_001",
  content: "File created at /workspace/hello.py"

Agent LLM response:
  tool_calls: [{
    id: "call_002",
    function: { name: "execute_bash", arguments: { "command": "python /workspace/hello.py" } }
  }]

  ... cycle continues ...
```

---

## 12. Key Source Files

| File | Purpose |
|------|---------|
| `openhands/agenthub/codeact_agent/tools/*.py` | Tool schema definitions |
| `openhands/agenthub/codeact_agent/function_calling.py` | Tool call → Action mapping |
| `openhands/events/action/*.py` | Action dataclass definitions |
| `openhands/events/observation/*.py` | Observation dataclass definitions |
| `openhands/runtime/action_execution_server.py` | Sandbox-side action executor |
| `openhands/runtime/plugins/` | Plugin system (AgentSkills, Jupyter) |
| `openhands/security/analyzer.py` | Security risk analysis |
| `openhands/controller/agent_controller.py` | Main agent loop orchestration |
| `openhands/mcp/` | MCP client/server integration |
