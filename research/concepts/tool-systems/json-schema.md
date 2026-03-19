---
title: "JSON Schema for Tool Definitions"
---

# JSON Schema for Tool Definitions

How tools describe themselves to LLMs — the schema is the entire interface between agent and model.

The tool definition is the most load-bearing artifact in an AI agent system. It is the only
thing the model sees. There is no code, no README, no tutorial — just a JSON Schema blob
injected into the system prompt or API request. If the schema is wrong, ambiguous, or poorly
structured, the model will call tools incorrectly. If it is well-engineered, the model becomes
a reliable tool-caller. This document covers every format, trick, and pitfall discovered across
the major open-source agent codebases.

---

## 1. Standard Formats

### OpenAI Function Calling Format

The dominant format, adopted far beyond OpenAI's own API. Most agent frameworks target this
shape regardless of which LLM provider they actually use.

```json
{
  "type": "function",
  "function": {
    "name": "execute_bash",
    "description": "Execute a bash command in a persistent shell session. Long-running commands will be terminated after a timeout. Use `sleep` or backgrounding for commands that need to persist.",
    "parameters": {
      "type": "object",
      "properties": {
        "command": {
          "type": "string",
          "description": "The bash command to execute. Can be multi-line."
        },
        "timeout": {
          "type": "integer",
          "description": "Maximum seconds to wait for the command to complete. Default: 120."
        }
      },
      "required": ["command"]
    }
  }
}
```

**Used by:** OpenHands, OpenCode, mini-SWE-agent, Gemini CLI (converted internally), ForgeCode,
Capy, TongAgents, and virtually every agent that targets GPT-4+.

The key structural elements:
- **`name`**: Must be alphanumeric with underscores, typically `snake_case`. Models are trained
  on this convention and comply better when names follow it.
- **`description`**: Free-text field that acts as the tool's documentation. This is where most
  prompt engineering happens. Can range from a single sentence to 2000+ tokens.
- **`parameters`**: A JSON Schema object describing the input. Must be `type: "object"` at the
  top level. Nested objects are allowed but empirically problematic (see Section 2).

OpenAI also supports **strict mode** (`"strict": true`), which constrains the model output to
exactly match the schema. In strict mode, all fields must have explicit types, `additionalProperties`
must be `false`, and every property must appear in `required`. This eliminates hallucinated
parameters but is inflexible — you cannot have optional fields.

### Anthropic Tool Use Format

Anthropic's format is structurally similar but embedded differently. Tools are defined in the
top-level API request, and the model returns `tool_use` content blocks:

**Tool definition (in API request):**
```json
{
  "name": "Edit",
  "description": "Replace exact text in a file. old_str must match exactly one location.",
  "input_schema": {
    "type": "object",
    "properties": {
      "file_path": {
        "type": "string",
        "description": "Absolute path to the file to edit."
      },
      "old_str": {
        "type": "string",
        "description": "The exact string to find. Must match exactly once in the file."
      },
      "new_str": {
        "type": "string",
        "description": "The replacement string. Use empty string to delete."
      }
    },
    "required": ["file_path", "old_str", "new_str"]
  }
}
```

**Model response (tool_use block):**
```json
{
  "type": "tool_use",
  "id": "toolu_01A09q90qw90lq917835lq9",
  "name": "Edit",
  "input": {
    "file_path": "/src/main.py",
    "old_str": "def hello():",
    "new_str": "def hello(name: str):"
  }
}
```

The key difference from OpenAI: `input_schema` instead of `parameters`, and tool calls are
returned as typed content blocks rather than a separate `tool_calls` array. Anthropic models
also support a `cache_control` field on tool definitions for prompt caching — critical when
you have 20+ tools, since tool schemas can consume thousands of tokens.

### MCP (Model Context Protocol) Tool Definitions

MCP standardizes tool definitions for interoperability. Any MCP server exposes tools via the
`tools/list` endpoint, and any MCP client can discover and call them:

```json
{
  "name": "codex",
  "description": "Run a Codex coding agent session to work on a task autonomously",
  "inputSchema": {
    "type": "object",
    "properties": {
      "prompt": {
        "type": "string",
        "description": "The task description for Codex to work on"
      },
      "approval-policy": {
        "type": "string",
        "enum": ["untrusted", "on-failure", "on-request", "never"],
        "description": "How to handle tool approvals. 'never' for full autonomy."
      },
      "cwd": {
        "type": "string",
        "description": "Working directory for the session"
      }
    },
    "required": ["prompt"]
  }
}
```

MCP uses `inputSchema` (camelCase) rather than `parameters` or `input_schema`. The schema
itself is standard JSON Schema. This is used by Goose (which exposes all tools as MCP),
Codex (which can act as an MCP server), and increasingly by Claude Code and other tools
that consume MCP servers.

### Schema Derivation from Types

Most mature agents don't hand-write JSON Schema. They derive it from typed code:

**Rust — schemars::JsonSchema (used by Goose):**
```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct BashParameters {
    /// The bash command to execute
    pub command: String,
    /// Working directory. Defaults to the current directory.
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: Option<u64>,
}

fn default_timeout() -> Option<u64> { Some(300) }
```

The `#[derive(JsonSchema)]` macro generates the full JSON Schema at compile time, including
descriptions from doc comments (`///`). This eliminates drift between code and schema.

**Python — Pydantic (used by OpenHands, TongAgents):**
```python
from pydantic import BaseModel, Field

class CmdRunAction(BaseModel):
    command: str = Field(
        description="The bash command to run"
    )
    timeout: int = Field(
        default=120,
        description="Max seconds to wait"
    )
    keep_prompt: bool = Field(
        default=True,
        description="Whether to keep the shell prompt open"
    )

# Generate JSON Schema:
schema = CmdRunAction.model_json_schema()
```

**TypeScript — Zod (used by OpenCode, Capy):**
```typescript
import { z } from "zod";

const BashToolSchema = z.object({
  command: z.string().describe("The bash command to execute"),
  timeout: z.number().optional().describe("Timeout in seconds"),
});

// Convert to JSON Schema for API:
import { zodToJsonSchema } from "zod-to-json-schema";
const jsonSchema = zodToJsonSchema(BashToolSchema);
```

**Go — struct tags (used by Codex, opencode):**
```go
type ExecuteParams struct {
    Command string `json:"command" jsonschema:"required,description=The command to run"`
    Timeout int    `json:"timeout,omitempty" jsonschema:"description=Timeout in seconds"`
}
```

The pattern is universal: define a typed struct/class, derive the schema. This catches
mismatches at compile/import time instead of at runtime when the LLM sends malformed input.

---

## 2. Schema Engineering Tricks

The difference between a tool that works 95% of the time and one that works 99.5% of the time
is entirely in how the schema is engineered. These are empirical findings from real agent codebases.

### ForgeCode's Field Ordering Insight

ForgeCode discovered that putting `required` before `properties` in the JSON Schema improves
model compliance. This is because LLMs generate JSON sequentially, and seeing which fields are
required before the property definitions helps the model plan its output:

```json
{
  "type": "object",
  "required": ["file_path", "old_string", "new_string"],
  "properties": {
    "file_path": { "type": "string", "description": "..." },
    "old_string": { "type": "string", "description": "..." },
    "new_string": { "type": "string", "description": "..." }
  }
}
```

**Why it works:** Training data (OpenAPI specs, JSON Schema examples) frequently puts `required`
first. The model has seen this ordering thousands of times and produces more reliable output
when the schema structure aligns with what it was trained on.

### ForgeCode's Flat Schema Preference

Nested schemas cause significantly more errors. ForgeCode flattens wherever possible:

**Before (nested — error-prone):**
```json
{
  "properties": {
    "edit": {
      "type": "object",
      "properties": {
        "file": { "type": "string" },
        "changes": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "old": { "type": "string" },
              "new": { "type": "string" }
            }
          }
        }
      }
    }
  }
}
```

**After (flat — reliable):**
```json
{
  "properties": {
    "file_path": { "type": "string" },
    "old_string": { "type": "string" },
    "new_string": { "type": "string" }
  }
}
```

Models make fewer structural errors (wrong nesting, missing braces, incorrect array wrapping)
when schemas are flat. The edit tool in particular benefits from this — a single old/new
replacement per call, iterated as needed, is more reliable than a batch edit schema.

### ForgeCode's Naming Alignment with Training Data

ForgeCode found that `old_string` / `new_string` works better than `old_text` / `new_text`
or `search` / `replace`. The hypothesis: `old_string` and `new_string` appear more frequently
in the fine-tuning data for tool-calling models. Naming parameters to match the conventions
in training data improves compliance.

Similarly, `file_path` outperforms `filename`, `filepath`, or `path` for the same reason.
These are small changes with measurable impact on tool-call accuracy.

### OpenHands' Description Length Adaptation

OpenHands adjusts tool description length based on the model family:

```python
def create_cmd_run_tool(cwd: str, short: bool = False):
    if short:
        description = CMD_RUN_TOOL_SHORT_DESCRIPTION   # < 1024 tokens
    else:
        description = CMD_RUN_TOOL_DESCRIPTION          # ~2000+ tokens

# Short descriptions used for:
SHORT_DESCRIPTION_MODELS = [
    "gpt-4",
    "o1",
    "o3",
    "o4-mini",
]
```

**Why:** Smaller or reasoning-focused models have tighter context constraints and can be
overwhelmed by very long tool descriptions. GPT-4-class models perform better with concise
descriptions. Claude models handle longer descriptions well and benefit from the extra detail.

The full `CMD_RUN_TOOL_DESCRIPTION` includes examples, edge cases, and detailed behavioral
rules. The short version strips examples and keeps only the essential behavioral constraints.

### OpenHands' Discriminated Union Pattern

Instead of 5 separate tools for file operations, OpenHands exposes a single `file_edit` tool
with a `command` enum that discriminates between operations:

```json
{
  "name": "file_edit",
  "parameters": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "enum": ["view", "create", "str_replace", "insert", "undo_edit"]
      },
      "path": { "type": "string" },
      "old_str": { "type": "string" },
      "new_str": { "type": "string" },
      "insert_line": { "type": "integer" },
      "view_range": {
        "type": "array",
        "items": { "type": "integer" }
      }
    },
    "required": ["command", "path"]
  }
}
```

Different `command` values require different subsets of parameters. The description explains
which parameters apply to which command. This pattern reduces the total number of tools
(keeping the model's decision space smaller) while preserving full functionality.

**Trade-off:** Discriminated unions require good descriptions to explain which fields apply
to which variant. If the description is unclear, the model may pass `old_str` when using
the `view` command, or omit `insert_line` when using `insert`.

### Goose's Argument Coercion

Goose does not reject tool calls with type mismatches. Instead, it coerces:

```rust
// In Goose's tool execution layer:
// If schema says "integer" but model sends "42" (string), coerce to 42
// If schema says "boolean" but model sends "true" (string), coerce to true
// If schema says "array" but model sends a single value, wrap in array
```

This is pragmatic: models frequently send `"42"` instead of `42`, especially for optional
numeric fields. Rejecting the call and asking the model to retry wastes tokens and time.
Coercion handles the 90% case silently.

### Claude Code's MCP Tool Search

When the number of MCP tools exceeds roughly 10% of the context window, Claude Code switches
from injecting all tool definitions to a lazy-loading strategy:

1. Only a `mcp_tool_search` meta-tool is injected into the prompt
2. The model calls `mcp_tool_search(query="file editing")` to discover relevant tools
3. Matching tool definitions are returned and made available for subsequent calls

This prevents tool definitions from consuming the entire context window when dozens of MCP
servers are connected. The search uses keyword matching over tool names and descriptions.

### mini-SWE-agent's Baked-In Tool Definitions

mini-SWE-agent takes the opposite approach to schema derivation: tools are defined as raw
strings directly in the system prompt, not as structured API objects:

```
Available commands:
find_file <file_name> [<dir>] - Find files matching name
search_dir <search_term> [<dir>] - Search for text in files  
open_file <path> [<line_number>] - Open a file for viewing
edit_file <start_line>:<end_line> <<EOF
...new content...
EOF
```

There is no JSON Schema at all. The model parses free-text commands. This works for simple
agents targeting strong models, but has no validation — any malformed output passes through.

---

## 3. JSON Schema Subset LLMs Understand Best

Not all JSON Schema keywords are created equal. LLMs were trained primarily on simple schemas
from OpenAPI specs, and their reliability drops sharply with advanced keywords.

### Keywords That Work Well

| Keyword | Reliability | Notes |
|---------|------------|-------|
| `type` | ★★★★★ | `string`, `integer`, `number`, `boolean`, `array`, `object` all work |
| `properties` | ★★★★★ | The core of every tool schema |
| `required` | ★★★★★ | Models respect this consistently |
| `enum` | ★★★★★ | Excellent for constraining string values |
| `description` | ★★★★★ | The primary channel for communicating intent |
| `items` | ★★★★☆ | For arrays — works well with simple item types |
| `default` | ★★★★☆ | Models sometimes include the default, sometimes omit the field |
| `minimum`/`maximum` | ★★★☆☆ | Models often ignore numeric constraints |
| `pattern` | ★★☆☆☆ | Regex patterns are rarely followed precisely |

### Keywords That Confuse Models

| Keyword | Problem |
|---------|---------|
| `allOf` | Models struggle with merging multiple subschemas |
| `oneOf` | Models may mix properties from different branches |
| `anyOf` | Similar issues to `oneOf` — ambiguous which branch to follow |
| `$ref` | Reference resolution is not something models can do |
| `if`/`then`/`else` | Conditional schemas are almost never followed correctly |
| `additionalProperties` | Models ignore this and add extra fields anyway |
| `patternProperties` | Virtually never followed |
| `dependencies` | Complex inter-field dependencies are not understood |

### Best Practices

1. **Use flat `type: "object"` with `properties` and `required`** — this covers 95% of needs.
2. **Use `enum` liberally** — it is the most reliable way to constrain values.
3. **Put constraints in `description`** — if a field must match a pattern, say so in English
   rather than relying on `pattern`.
4. **Avoid polymorphic schemas** — if a field can be either a string or an array, make two
   separate parameters instead.
5. **Keep arrays simple** — `items: { type: "string" }` works; `items: { type: "object", ... }`
   with nested properties is fragile.
6. **Use `description` on every field** — models use descriptions as the primary guide for
   what to put in each field. A property without a description is a guessing game.

---

## 4. Generating Schemas from Code

### Python: Pydantic

The most common approach in the Python ecosystem. Pydantic v2 generates clean JSON Schema:

```python
from pydantic import BaseModel, Field
from typing import Optional, Literal

class FileEditParams(BaseModel):
    """Edit a file by replacing text."""
    
    command: Literal["str_replace", "insert", "view"] = Field(
        description="The edit operation to perform"
    )
    path: str = Field(
        description="Absolute path to the file"
    )
    old_str: Optional[str] = Field(
        default=None,
        description="Exact string to find (for str_replace)"
    )
    new_str: Optional[str] = Field(
        default=None,
        description="Replacement string"
    )

# This produces a clean JSON Schema:
import json
print(json.dumps(FileEditParams.model_json_schema(), indent=2))
```

Output:
```json
{
  "type": "object",
  "title": "FileEditParams",
  "description": "Edit a file by replacing text.",
  "properties": {
    "command": {
      "type": "string",
      "enum": ["str_replace", "insert", "view"],
      "description": "The edit operation to perform"
    },
    "path": {
      "type": "string",
      "description": "Absolute path to the file"
    },
    "old_str": {
      "anyOf": [{"type": "string"}, {"type": "null"}],
      "default": null,
      "description": "Exact string to find (for str_replace)"
    },
    "new_str": {
      "anyOf": [{"type": "string"}, {"type": "null"}],
      "default": null,
      "description": "Replacement string"
    }
  },
  "required": ["command", "path"]
}
```

**Gotcha:** Pydantic v2 generates `anyOf: [{type: string}, {type: null}]` for Optional fields.
Some providers don't handle `anyOf` well. OpenHands works around this by post-processing the
schema to replace `anyOf` with just `type: "string"` and marking the field as not required.

### TypeScript: Zod

The standard in the TypeScript ecosystem, used by OpenCode and Capy:

```typescript
import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";

const GlobToolSchema = z.object({
  pattern: z
    .string()
    .describe("Glob pattern to match files (e.g., '**/*.ts')"),
  path: z
    .string()
    .optional()
    .describe("Directory to search in. Defaults to cwd."),
});

const schema = zodToJsonSchema(GlobToolSchema, {
  // Strip Zod-specific metadata for cleaner output
  $refStrategy: "none",
});
```

Zod schemas are also used at runtime for **validating** the model's tool-call output before
execution, providing a two-way contract: schema for the model, validation for the runtime.

### Rust: schemars

Used by Goose and other Rust-based agents:

```rust
use schemars::JsonSchema;

#[derive(JsonSchema)]
pub struct SearchParams {
    /// Regex pattern to search for
    pub pattern: String,
    /// File glob to filter (e.g., "*.rs")
    #[serde(default)]
    pub include: Option<String>,
    /// Maximum number of results
    #[serde(default = "default_max")]
    pub max_results: Option<u32>,
}

// Generate schema:
let schema = schemars::schema_for!(SearchParams);
```

The `schemars` crate reads doc comments as descriptions and `serde` attributes for
optionality and defaults. The generated schema is clean and flat.

### Go: Struct Tags and Reflection

Go agents typically use struct tags with a schema generation library:

```go
type ReadFileParams struct {
    FilePath   string `json:"file_path" jsonschema:"required" description:"Path to read"`
    StartLine  int    `json:"start_line,omitempty" description:"First line (1-indexed)"`
    EndLine    int    `json:"end_line,omitempty" description:"Last line (1-indexed)"`
}

// Using github.com/invopop/jsonschema:
reflector := jsonschema.Reflector{}
schema := reflector.Reflect(&ReadFileParams{})
```

Go's approach is less ergonomic than Pydantic or Zod but works. The main limitation is that
Go's type system lacks union types, making discriminated-union tool schemas harder to express.

---

## 5. Schema Design Impact Table

| Technique | Effect | Source | Magnitude |
|-----------|--------|--------|-----------|
| Put `required` before `properties` | Better field compliance | ForgeCode | ~2-5% fewer missing fields |
| Flat schemas (no nesting) | Fewer structural errors | ForgeCode | Significant reduction in malformed JSON |
| Name alignment with training data (`old_string` vs `old_text`) | Higher parameter accuracy | ForgeCode | Measurable on edit tool benchmarks |
| Short descriptions for reasoning models | Reduced confusion | OpenHands | Required for o1/o3/o4 families |
| Long descriptions with examples for Claude | Better tool selection | OpenHands | Improves complex multi-tool workflows |
| Discriminated union (one tool, enum command) | Smaller decision space | OpenHands | Fewer wrong-tool selections |
| Argument coercion (string→int) | Fewer retries | Goose | Eliminates ~5-10% of failures |
| Lazy tool loading via search | Context window savings | Claude Code | Enables 100+ MCP tools |
| `enum` for constrained values | Near-perfect compliance | Universal | ★★★★★ reliability |
| `description` on every field | Correct parameter filling | Universal | Missing descriptions → wrong values |
| Strict mode (OpenAI) | Zero hallucinated params | OpenAI API | 100% schema compliance, but inflexible |
| Baked-in text commands (no schema) | Simplicity, no validation | mini-SWE-agent | Works only with strong models |

---

## 6. Provider-Specific Considerations

### OpenAI

- **Strict mode:** Set `"strict": true` on a function definition to force the model's output
  to conform exactly to the schema. All properties must be in `required`, and
  `additionalProperties: false` is mandatory. Useful for production but removes optional fields.
- **Parallel tool calls:** OpenAI models can return multiple tool calls in a single response.
  Set `parallel_tool_calls: false` if your agent processes tools sequentially and ordering matters.
- **Token counting:** Tool definitions count against context. OpenAI recommends keeping
  function descriptions under 100 words for optimal performance, though agents routinely
  exceed this with good results on GPT-4-class models.

### Anthropic

- **Tool use blocks:** Tool calls are content blocks (`type: "tool_use"`) interleaved with
  text blocks. The model can think aloud before calling a tool.
- **Thinking + tools interaction:** With extended thinking enabled, the model produces a
  `thinking` block, then a `tool_use` block. The thinking block is not cached, which affects
  prompt caching strategies.
- **Prompt caching:** Tool definitions support `cache_control: { type: "ephemeral" }` to
  cache them across turns. This is critical for agents with many tools — caching 20 tool
  definitions saves thousands of input tokens per turn.
- **No strict mode:** Anthropic does not have an equivalent of OpenAI's strict mode. Schema
  compliance is handled through clear descriptions and, if needed, retry logic.

### Google Gemini

- **Function declarations:** Gemini uses a slightly different format:
  ```json
  {
    "function_declarations": [{
      "name": "search_files",
      "description": "Search for files matching a pattern",
      "parameters": {
        "type": "object",
        "properties": {
          "query": { "type": "string" }
        },
        "required": ["query"]
      }
    }]
  }
  ```
- **Automatic function calling:** Gemini supports a mode where it automatically executes
  function calls without returning them to the client, which agents typically disable to
  maintain control over execution.
- **Gemini CLI:** Internally converts between Gemini's function declaration format and the
  OpenAI-compatible format that most tool implementations expect.

### Local Models (Ollama, llama.cpp)

- **Variable support:** Not all local models support function calling. Those fine-tuned for
  tool use (e.g., Llama 3.1+, Mistral) work; base models do not.
- **Aider's approach:** Aider supports local models by not using function calling at all.
  Instead, it uses structured prompting — asking the model to output edits in a specific
  text format (unified diff or search/replace blocks) and parsing the text response.
- **Template differences:** Different local models use different chat templates for tool
  calling. Ollama abstracts this, but raw llama.cpp requires matching the template to the
  model (Llama uses `<|python_tag|>`, Mistral uses `[TOOL_CALLS]`).

---

## 7. Key Takeaways

1. **The schema IS the interface.** There is no other way to tell the model what a tool does.
   Invest in schema quality the way you'd invest in API documentation.

2. **Derive schemas from types.** Hand-written JSON Schema drifts from implementation. Use
   Pydantic, Zod, schemars, or struct tags to generate schemas from the same types your
   code actually uses.

3. **Respect the training distribution.** LLMs are pattern matchers. Naming, ordering, and
   structure that aligns with common training data (OpenAPI specs, existing function-calling
   datasets) produces better compliance than novel conventions.

4. **Flat beats nested.** Every level of nesting is a chance for the model to make a
   structural error. Flatten aggressively.

5. **Descriptions are your primary tool.** Put constraints, examples, and behavioral rules
   in the `description` field. Models read descriptions more reliably than they interpret
   advanced JSON Schema keywords.

6. **Adapt to the model.** Different models have different strengths. Short descriptions for
   reasoning models, long descriptions for Claude. Strict mode for OpenAI when you need
   guarantees. No schema at all for weak models that can't do function calling.

7. **Coerce, don't reject.** When a model sends `"42"` instead of `42`, just coerce it.
   Retrying costs more than the type cast.

8. **Watch your context budget.** Tool definitions are tokens. When you have dozens of tools,
   the definitions alone can consume 20%+ of context. Use caching (Anthropic), strict mode
   compression (OpenAI), or lazy loading (Claude Code MCP search) to manage this.
