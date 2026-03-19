# ForgeCode — Tool System & Tool Corrections

## Overview

ForgeCode's tool system is standard for a terminal coding agent — file operations, shell execution, search — but what distinguishes it is the **tool-call correction layer** that sits between the model's tool-call output and actual execution. This layer was the single highest-impact engineering decision in reaching #1 on TermBench 2.0.

## Available Tools

Based on the open-source codebase and documentation, ForgeCode provides these core tools:

### File Operations
- **`read_file`** / **`read_image`**: Read file contents (text files truncated at `FORGE_MAX_LINE_LENGTH`, default 2000 chars/line)
- **`edit`** (`old_string` / `new_string`): Structured file editing with find-and-replace semantics
- **`write_file`**: Create or overwrite files
- **`search`**: Code search with configurable result size (`FORGE_MAX_SEARCH_RESULT_BYTES`)

### Shell Execution
- **`shell`**: Execute arbitrary shell commands with the user's permissions
- Runs in the user's native ZSH environment (aliases, functions, PATH all work)
- Timeout controlled by `FORGE_TOOL_TIMEOUT` (default 300 seconds)
- Output truncated at `FORGE_STDOUT_MAX_LINE_LENGTH` (default 2000 chars/line)

### Semantic Search
- **`sem_search`**: Semantic code search over an indexed codebase (requires ForgeCode Services + `:sync`)
- Uses vector search with configurable limits (`FORGE_SEM_SEARCH_LIMIT`, `FORGE_SEM_SEARCH_TOP_K`)

### Task Management
- **`todo_write`**: Create, update, and mark task items — enforced as mandatory for multi-step tasks by ForgeCode Services

## Tool-Call Failure Taxonomy

ForgeCode's blog posts document three distinct failure classes, discovered through TermBench micro-evaluations:

### 1. Wrong Tool Selected
The model uses `shell` to apply a code edit instead of the structured `edit` tool. This produces working results but bypasses the correction layer's validation.

### 2. Correct Tool, Wrong Arguments
The model calls the right tool but with incorrect argument names or shapes. Field names are close but don't match the schema exactly.

### 3. Correct Tool, Correct Arguments, Wrong Sequencing
The tool is called before its preconditions are met (e.g., editing a file before reading it to understand the current state).

**Key insight**: These three classes are invisible in aggregate pass-rate metrics. ForgeCode built **per-tool, per-model micro-evaluations** that isolate each failure class individually.

## The Tool-Call Correction Layer

ForgeCode Services provides a runtime correction layer that intercepts every tool call before dispatch. This is not prompt engineering — it's programmatic interception and repair.

### What It Does

1. **Argument validation**: Checks that all required fields are present and correctly typed
2. **Pattern matching**: Catches common error patterns (misnamed fields, wrong nesting levels)
3. **Auto-correction**: Repairs fixable errors rather than failing silently
4. **Dispatch gating**: Only valid calls are forwarded to actual execution

### Why It Exists

Models have strong priors from training about what tool calls should look like. When your tool's schema conflicts with those priors (unusual field names, deep nesting, unfamiliar argument patterns), error rates climb — not because the model can't understand the description, but because it **pattern-matches against training data first**.

## Schema Engineering for Reliability

The TermBench blog posts reveal specific schema engineering insights that dramatically improved tool-call reliability:

### Field Ordering Matters

Moving `required` before `properties` in JSON schemas reduced malformed calls measurably with GPT 5.4:

```json
// BEFORE (less reliable) — required after properties
{
  "type": "object",
  "properties": { "title": { "type": "string" }, "status": { "type": "string" } },
  "required": ["title", "status"]
}

// AFTER (more reliable) — required before properties
{
  "type": "object",
  "required": ["title", "status"],
  "properties": { "title": { "type": "string" }, "status": { "type": "string" } }
}
```

**Why**: When GPT 5.4 emits arguments deep in a long trajectory, it anchors on what it sees first. Putting `required` early tells the model which fields matter before it starts generating the `properties` block.

### Flat Schemas Over Nested

Nested schemas create structural confusion — models mix up which `required` array belongs to which object:

```json
// NESTED (more error-prone) — two required arrays, two object layers
{
  "type": "object",
  "required": ["task"],
  "properties": {
    "task": {
      "type": "object",
      "required": ["title"],
      "properties": { "title": { "type": "string" } }
    }
  }
}

// FLAT (more reliable) — one required array, one object layer
{
  "type": "object",
  "required": ["task_title"],
  "properties": { "task_title": { "type": "string" } }
}
```

**Trade-off**: You lose semantic grouping but gain reliability. ForgeCode chose reliability.

### Training-Data-Aligned Naming

Renaming tool arguments to match common training data patterns reduced error rates:

- Generic internal names → `old_string` / `new_string` (for edit tools)
- These names appear frequently in training data for file-editing operations
- The model pattern-matches against its training priors, so alignment helps

### Explicit Truncation Signals

When ForgeCode truncates large files (typically at 2000 lines), different models handle the truncation differently:

- **Opus 4.6**: Reads `total_lines` metadata and infers more content exists
- **GPT 5.4**: Often proceeds as if it saw the whole file

**Fix**: Add a plain-text reminder directly in the tool result body:

```
[NOTE: Output truncated. Showing lines 1-2000 of 5847 total. Use offset to read more.]
```

This made truncation handling reliable across models.

## Model-Specific Tool Behavior

A key finding from the TermBench work: **different models need different tool handling**. ForgeCode's correction layer applies model-specific adaptations:

| Behavior | Opus 4.6 | GPT 5.4 |
|----------|----------|---------|
| Schema tolerance | Handles messy schemas | Needs clean field ordering |
| Nesting | Handles nested schemas | Needs flat schemas |
| Truncation inference | Reads metadata | Needs explicit text reminders |
| Verification | Naturally does extra passes | Needs enforced verification |
| Overall | More forgiving | Same score with more runtime support |

This is not a capability gap — it's a behavioral difference. Both models reach 81.8% on TermBench, but the runtime compensates differently for each.

## Tool Failure Limits

ForgeCode prevents infinite retry loops:

```yaml
# forge.yaml
max_tool_failure_per_turn: 3  # max failures per tool before forcing completion
```

When a tool fails `max_tool_failure_per_turn` times, the agent is forced to move on rather than retrying the same broken operation.

## Tool Timeout

```bash
FORGE_TOOL_TIMEOUT=300  # seconds before a hanging tool is killed
```

This prevents a single hung shell command from blocking the entire session.

## Micro-Evaluations

ForgeCode runs per-tool reliability evaluations in CI/CD:

- **Tool-call correctness rates** per tool, per model
- **`todo_write` compliance** for decomposed tasks
- **Entry-point discovery precision**
- **Skill routing accuracy**

These are not full TermBench runs — they are small, fast, targeted evals that gate releases. Each one exists because TermBench surfaced a specific failure class that needed continuous monitoring.

## Key Takeaway

The tool correction layer is ForgeCode's most practically impactful innovation. It recognizes that tool-call reliability is a **runtime engineering problem**, not a model capabilities problem. The same model, with the same weights, produces dramatically different pass rates depending on how its tool calls are validated, corrected, and dispatched.