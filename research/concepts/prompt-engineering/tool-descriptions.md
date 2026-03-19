# Engineering Tool Descriptions for Coding Agents

Tool descriptions are the contract between an agent framework and the language model
that drives it. Every tool call a model emits — file edits, shell commands, code
searches — is shaped by how that tool was described in the system prompt or tool-use
API payload. A poorly-worded description or a subtly mis-ordered JSON schema can
cause an otherwise capable model to select the wrong tool, hallucinate arguments, or
sequence operations incorrectly. This document synthesizes findings from ForgeCode,
OpenHands, Claude Code, Goose, Aider, Gemini CLI, OpenCode, Ante, Sage-Agent, and
Codex to present a comprehensive guide to engineering tool descriptions that maximize
agent reliability.

---

## 1. Why Tool Descriptions Matter for Agent Performance

Language models do not "understand" tools the way a human programmer reads an API
reference. They predict the next token conditioned on everything that precedes it —
including the tool schema that was injected into context. This means the surface form
of a tool description is not merely documentation; it is a **reliability variable**.

ForgeCode's research quantifies this directly. By restructuring JSON schemas without
changing the underlying tool behavior, they observed measurable reductions in
malformed tool calls. The mechanism is attention-based: when a model emits arguments
deep in a long trajectory, it anchors on the tokens it encountered earliest in the
schema. If `required` appears before `properties`, the model has already internalized
which fields are mandatory before it begins generating argument values. If
`required` appears after a large `properties` block, the constraint information may
fall outside the effective attention window during generation.

Tool descriptions influence agent behavior at three distinct levels:

1. **Tool selection** — The description text determines whether the model picks
   `file_edit` vs `shell_command` for a given task. Ambiguous or overlapping
   descriptions cause selection errors.
2. **Argument construction** — The parameter schema controls whether the model
   generates valid JSON with correct types, required fields, and properly formatted
   values.
3. **Sequencing** — Descriptions that encode preconditions ("file must exist") or
   postconditions ("returns updated content") guide the model's planning of
   multi-step tool chains.

ForgeCode's tool failure taxonomy formalizes these into three categories:

| Failure Mode | Description | Root Cause |
|---|---|---|
| Wrong tool selected | Model calls `create_file` when it should call `edit_file` | Overlapping or ambiguous description text |
| Correct tool, wrong arguments | Model calls `edit_file` but with malformed `old_str` | Schema misalignment, missing examples, poor naming |
| Correct tool, correct arguments, wrong sequencing | Model edits a file before reading it | Missing precondition signals in descriptions |

Each failure mode demands different mitigations in the tool description layer, and
each is independently measurable through targeted evaluations.

---

## 2. Anatomy of a Tool Description

A complete tool description has four components: name, description text, parameter
schema, and (optionally) inline examples. Different frameworks structure these
differently, but the components are universal.

### 2.1 Name

The tool name is the identifier the model emits when selecting a tool. It must be
unambiguous, memorable, and aligned with training-data conventions.

```json
{
  "name": "str_replace_editor",
  "description": "...",
  "parameters": { "..." }
}
```

Naming conventions vary across frameworks:

| Framework | Convention | Example |
|---|---|---|
| OpenHands | snake_case, verb-noun | `str_replace_editor`, `cmd_run` |
| Claude Code | snake_case, descriptive | `edit_file`, `bash` |
| Goose | namespaced, double-underscore | `developer__shell` |
| OpenCode | simple snake_case | `edit`, `bash` |
| Ante | snake_case | `bash`, `read_file` |
| Sage-Agent | camelCase or snake_case | Per `ToolBase` implementation |

### 2.2 Description Text

The description is natural language that tells the model **when** to use the tool,
**what** it does, and **what constraints** apply. This is the single most influential
component for tool selection accuracy.

From Ante's Rust `Tool` trait, the description field is explicitly documented as
being "used by the LLM to decide when to invoke it." This means the description
serves a dual role: it is both documentation for human maintainers and a steering
signal for the model.

Effective descriptions follow a pattern:

```
[What the tool does — one sentence]
[When to use it — disambiguation from similar tools]
[Key constraints — what will fail, what is required]
[Behavioral notes — truncation, output format, side effects]
```

### 2.3 Parameter Schema

The parameter schema is a JSON Schema object that defines the arguments the model
must generate. This is where ForgeCode's schema engineering findings apply most
directly.

```json
{
  "type": "object",
  "required": ["command", "path"],
  "properties": {
    "command": {
      "type": "string",
      "enum": ["view", "create", "str_replace"],
      "description": "The operation to perform"
    },
    "path": {
      "type": "string",
      "description": "Absolute path to the target file"
    },
    "old_str": {
      "type": "string",
      "description": "The exact string to find and replace. Must match file content exactly, including whitespace."
    },
    "new_str": {
      "type": "string",
      "description": "The replacement string. If omitted, old_str is deleted."
    }
  }
}
```

Note: `required` appears **before** `properties`. This is deliberate and
significant, as Section 3 explains in detail.

### 2.4 Inline Examples

Some frameworks embed usage examples directly in the description text. OpenHands'
full descriptions can exceed 2000 tokens, with substantial space devoted to examples
showing correct invocation patterns. These examples serve as few-shot demonstrations
within the tool description itself.

```
Example: To replace "foo" with "bar" in /path/to/file.py:
<tool_call>
str_replace_editor(command="str_replace", path="/path/to/file.py", old_str="foo", new_str="bar")
</tool_call>
```

The trade-off is context window consumption. Every token spent on tool description
examples is a token unavailable for code context, conversation history, or reasoning.

---

## 3. Schema Engineering Principles

ForgeCode's research identifies schema structure as a first-class reliability
variable. These findings are the most actionable contributions to tool description
engineering.

### 3.1 Field Ordering: `required` Before `properties`

The single highest-impact schema change ForgeCode reports is placing the `required`
array before the `properties` object in JSON schemas.

**Before (higher error rate):**
```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "File path" },
    "content": { "type": "string", "description": "File content" },
    "encoding": { "type": "string", "description": "File encoding", "default": "utf-8" }
  },
  "required": ["path", "content"]
}
```

**After (lower error rate):**
```json
{
  "type": "object",
  "required": ["path", "content"],
  "properties": {
    "path": { "type": "string", "description": "File path" },
    "content": { "type": "string", "description": "File content" },
    "encoding": { "type": "string", "description": "File encoding", "default": "utf-8" }
  }
}
```

The mechanism is attention anchoring. ForgeCode reports: "When GPT 5.4 emits
arguments deep in a long trajectory, it anchors on what it sees first." By the time
the model is generating the third or fourth tool call in a multi-step plan, the
schema tokens at the beginning of the context are more influential than tokens buried
after a large properties block. Leading with `required` ensures the model has encoded
mandatory-field constraints before it encounters the property definitions, reducing
omission errors.

This effect is model-specific. ForgeCode observed significant improvement with
GPT 5.4 specifically, and their finding underscores a broader principle: schema
optimizations must be validated per-model, not assumed universal.

### 3.2 Flat Schemas Over Nested Schemas

ForgeCode finds that "flat schemas beat nested schemas — fewer structural layers =
fewer mistakes." Nesting introduces two sources of error:

1. **Structural confusion** — "Nested schemas create structural confusion — models
   mix up which `required` array belongs to which object." When a schema has nested
   objects each with their own `required` arrays, the model can misattribute
   constraints, applying a parent's requirements to a child or vice versa.

2. **Brace-matching errors** — Deeply nested JSON requires the model to track
   matching braces across many tokens. Each additional nesting level increases the
   probability of malformed output.

**Nested (error-prone):**
```json
{
  "type": "object",
  "required": ["file_operation"],
  "properties": {
    "file_operation": {
      "type": "object",
      "required": ["action", "target"],
      "properties": {
        "action": {
          "type": "string",
          "enum": ["read", "write", "delete"]
        },
        "target": {
          "type": "object",
          "required": ["path"],
          "properties": {
            "path": { "type": "string" },
            "encoding": { "type": "string", "default": "utf-8" }
          }
        }
      }
    }
  }
}
```

**Flat (reliable):**
```json
{
  "type": "object",
  "required": ["action", "path"],
  "properties": {
    "action": {
      "type": "string",
      "enum": ["read", "write", "delete"]
    },
    "path": { "type": "string" },
    "encoding": { "type": "string", "default": "utf-8" }
  }
}
```

The flat version eliminates two levels of nesting, removes the ambiguity of
multiple `required` arrays, and reduces the total token count. The model's
generation task is simpler: emit a flat JSON object with known fields, rather than
constructing nested structures.

### 3.3 Training-Data-Aligned Naming

ForgeCode identifies that "models have strong priors from training about what tool
calls should look like. When your tool's schema conflicts with those priors, error
rates climb."

Concretely, this means parameter names should align with patterns the model has seen
frequently in training data. The name `old_string` / `new_string` performs better
than `target_text` / `replacement_text` because models have been extensively trained
on diff/patch/sed-like patterns where "old" and "new" are the conventional
descriptors for before-and-after content.

This principle extends to naming conventions more broadly:

| Training-aligned (better) | Generic (worse) | Rationale |
|---|---|---|
| `old_str` / `new_str` | `find` / `replace` | Matches diff/patch conventions |
| `command` | `operation_type` | Matches CLI conventions |
| `path` | `file_location` | Matches filesystem API conventions |
| `content` | `data_payload` | Matches file I/O conventions |
| `line` / `line_number` | `position_index` | Matches editor conventions |
| `cwd` | `working_directory_path` | Matches shell conventions |

The underlying insight is that LLM tool-use is not a clean abstraction. The model
is not reasoning about APIs from first principles — it is pattern-matching against
its training distribution. Aligning with that distribution reduces friction.

### 3.4 Explicit Truncation Signals

ForgeCode mandates that "truncation signals must be explicit." When a tool's output
may be truncated (common for file reads, search results, or command output), the
tool description must state this clearly and describe the indicator.

```json
{
  "name": "read_file",
  "description": "Read the contents of a file. If the file exceeds 2000 lines, output is truncated and ends with '[... truncated, use line_range to read remaining content]'. When you see this marker, issue follow-up calls with appropriate line_range values.",
  "parameters": {
    "..." 
  }
}
```

Without explicit truncation signals, models may treat truncated output as complete,
leading to edits that target content the model never actually saw, or conclusions
based on partial information.

---

## 4. Good vs Bad Tool Description Examples

### 4.1 File Edit Tool

**Bad description:**
```json
{
  "name": "edit",
  "description": "Edit a file.",
  "parameters": {
    "type": "object",
    "properties": {
      "file": { "type": "string" },
      "changes": {
        "type": "object",
        "properties": {
          "find": { "type": "string" },
          "replace": { "type": "string" }
        }
      }
    },
    "required": ["file", "changes"]
  }
}
```

Problems: (1) Description is too vague — does not disambiguate from `create_file`
or `overwrite_file`. (2) Nested `changes` object creates structural ambiguity.
(3) `required` is after `properties`. (4) `find`/`replace` are not
training-data-aligned for this semantic. (5) No constraint documentation — must the
match be exact? What happens on no match?

**Good description:**
```json
{
  "name": "str_replace_editor",
  "description": "Make precise, surgical edits to an existing file by replacing exact string matches. Use this tool when you need to modify specific parts of a file. The old_str must match the file content EXACTLY — including all whitespace, indentation, and line endings. If old_str is not found or matches multiple locations, the edit will fail. To create new files, use the create_file tool instead.",
  "parameters": {
    "type": "object",
    "required": ["command", "path"],
    "properties": {
      "command": {
        "type": "string",
        "enum": ["view", "create", "str_replace", "insert"],
        "description": "The edit command to execute"
      },
      "path": {
        "type": "string",
        "description": "Absolute path to the file. File must exist for str_replace and view commands."
      },
      "old_str": {
        "type": "string",
        "description": "The exact string in the file to replace. Required for str_replace command. Must match exactly one location in the file."
      },
      "new_str": {
        "type": "string",
        "description": "The new string to insert in place of old_str. If empty, old_str is deleted."
      }
    }
  }
}
```

Improvements: (1) Description explains when to use it, disambiguates from
`create_file`, and documents failure conditions. (2) Flat schema. (3) `required`
before `properties`. (4) `old_str`/`new_str` are training-data-aligned names.
(5) Each parameter description includes constraints and behavioral notes.

### 4.2 Shell Command Tool

**Bad description:**
```json
{
  "name": "run",
  "description": "Run a command.",
  "parameters": {
    "type": "object",
    "properties": {
      "cmd": { "type": "string" }
    }
  }
}
```

**Good description:**
```json
{
  "name": "bash",
  "description": "Execute a bash command in the user's environment. Commands run in the current working directory. Use for: running tests, installing packages, git operations, file system operations, and build commands. For long-running commands, output is truncated after 120 seconds — the marker '[command timed out]' indicates truncation. Prefer concise commands; chain with && for sequential operations. Do NOT use interactive commands (vim, less) or commands requiring user input.",
  "parameters": {
    "type": "object",
    "required": ["command"],
    "properties": {
      "command": {
        "type": "string",
        "description": "The bash command to execute. Must be a valid bash expression."
      },
      "cwd": {
        "type": "string",
        "description": "Working directory for the command. Defaults to the project root."
      }
    }
  }
}
```

The good version names the tool `bash` (universally recognized), uses `command`
not `cmd` (training-aligned), documents timeout behavior with its truncation
marker, explains positive and negative use cases, and places `required` first.

---

## 5. Description Length Adaptation

OpenHands implements a dual-description system that explicitly addresses the tension
between description completeness and context efficiency.

### 5.1 The Full/Short Pattern

OpenHands maintains two description variants for each tool:

- **Full descriptions** (~2000+ tokens): Include comprehensive behavioral
  documentation, multiple usage examples, edge case notes, and detailed parameter
  explanations. These are the default.
- **Short descriptions** (<1024 tokens): Stripped-down versions that retain core
  semantics but remove examples and verbose explanations. Selected for models with
  strict context limits or known sensitivity to prompt length.

The selection logic keys on model name. Short descriptions are activated for
`gpt-4*`, `o1*`, `o3*`, and `o4*` model families:

```python
def create_cmd_run_tool(cwd: str, short: bool = False) -> dict:
    """Create the command run tool description.
    
    Args:
        cwd: Current working directory, injected into description.
        short: If True, use compact description for context-limited models.
    """
    if short:
        return {
            "name": "cmd_run",
            "description": f"Run a shell command in {cwd}. Timeout: 120s.",
            "parameters": { "..." }
        }
    else:
        return {
            "name": "cmd_run",
            "description": (
                f"Run a shell command in the terminal at {cwd}.\n\n"
                "* Commands timeout after 120 seconds.\n"
                "* Prefer commands that complete quickly.\n"
                "* For file modifications, prefer the str_replace_editor.\n"
                "* For searching, prefer the search tool.\n\n"
                "Example:\n"
                "  cmd_run(command='ls -la')\n"
                "  cmd_run(command='python -m pytest tests/ -x')\n"
                "..."
            ),
            "parameters": { "..." }
        }
```

Note the `cwd` injection: the current working directory is dynamically inserted
into every tool description instance. This is not cosmetic — it provides the model
with grounding context that prevents incorrect path assumptions.

### 5.2 When to Use Short Descriptions

The decision between full and short is not purely about context window size. Some
models perform better with less verbose instructions because:

1. **Attention dilution** — Long descriptions compete with task-relevant context for
   model attention. A 2000-token tool description across 30 tools consumes 60K
   tokens before a single line of code enters the context.
2. **Instruction-following variance** — Some models are prone to over-indexing on
   examples in tool descriptions, reproducing example patterns instead of adapting
   to the actual task.
3. **Diminishing returns** — Beyond a certain detail level, additional description
   text does not improve tool-use accuracy and may degrade it.

The optimal description length is model-specific and must be determined empirically.
OpenHands' model-keyed selection is a practical heuristic; ForgeCode's per-model
evaluation approach (Section 10) provides the rigorous methodology.

---

## 6. Tool Naming Conventions and Namespacing

### 6.1 The Name as a Semantic Signal

The tool name is the first token the model generates when making a tool call. It
must be semantically loaded — a model reading `bash` instantly activates shell-
command priors, while `execute_arbitrary_system_process` does not trigger the same
associations.

Aider takes this to its logical extreme: it has no traditional tool system at all.
Edit formats ARE the interface. The model outputs structured text (unified diffs,
search-replace blocks, whole files) that is parsed by format-specific handlers.
Aider's finding: "The simpler the output format, the better the LLM's actual coding
performance." This suggests that tool naming should optimize for simplicity and
familiarity.

### 6.2 Namespacing for Multi-Source Tools

When tools come from multiple sources (built-in, MCP servers, plugins), naming
collisions become a real risk. Two approaches dominate:

**Goose: Double-underscore namespacing**

Goose uses `extensionname__toolname` format:
```
developer__shell
developer__read_file
github__create_issue
jira__search_tickets
```

This provides unambiguous identification and prevents collisions when multiple
extensions provide similar capabilities. The double underscore is visually distinct
and unlikely to appear in natural tool names.

**OpenCode: Server-name prefixing**

OpenCode prefixes MCP tools with their server name:
```
filesystem_read_file
github_search_code
```

This is simpler than Goose's approach but provides the same disambiguation.

**Claude Code**: Does not namespace built-in tools but uses MCP Tool Search when MCP
tools exceed 10% of the context window. This is a filtering approach rather than a
naming approach — instead of embedding all tool names, it dynamically searches for
relevant tools at invocation time.

### 6.3 Naming Recommendations

1. Use short, verb-noun names for built-in tools: `read_file`, `edit_file`, `bash`.
2. Apply consistent namespacing for extension/plugin tools: `source__tool_name`.
3. Avoid generic names that overlap with common programming terms: `run`, `execute`,
   `process` are ambiguous out of context.
4. Match training-data conventions: `bash` over `shell`, `grep` over `search_text`,
   `diff` over `compare_files`.

---

## 7. Argument Coercion and Error Handling

### 7.1 Goose's Coercion Layer

Goose implements automatic argument coercion, addressing a pragmatic reality: models
frequently generate arguments with minor type mismatches. Rather than failing on
these mismatches and consuming a retry cycle, Goose coerces LLM arguments to match
schemas.

The coercion handles common cases:

```rust
// Pseudocode for Goose's coercion logic
fn coerce_argument(value: JsonValue, expected: &SchemaType) -> Result<JsonValue> {
    match (value, expected) {
        // String "42" -> integer 42
        (JsonValue::String(s), SchemaType::Integer) => {
            s.parse::<i64>().map(JsonValue::Integer)
        }
        // Integer 42 -> string "42"
        (JsonValue::Integer(n), SchemaType::String) => {
            Ok(JsonValue::String(n.to_string()))
        }
        // String "true" -> boolean true
        (JsonValue::String(s), SchemaType::Boolean) => {
            match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(JsonValue::Boolean(true)),
                "false" | "0" | "no" => Ok(JsonValue::Boolean(false)),
                _ => Err(CoercionError::InvalidBoolean(s))
            }
        }
        // Single value -> array of one
        (val, SchemaType::Array(_)) if !val.is_array() => {
            Ok(JsonValue::Array(vec![val]))
        }
        _ => Ok(value)
    }
}
```

This coercion layer operates transparently between the model's output and the tool
dispatch system. The model never knows coercion occurred; the tool receives
correctly-typed arguments.

### 7.2 The Cost of Silent Coercion

Coercion trades correctness for resilience. The risk is that systematic type
mismatches go undetected, masking a tool description problem that should be fixed at
the schema level. A model consistently generating `"42"` instead of `42` for a
line-number parameter suggests the schema description is ambiguous about the expected
type.

Best practice: log all coercions, monitor their frequency, and treat persistent
coercion patterns as signals to improve the tool description.

### 7.3 Goose's Schema Auto-Generation

Goose auto-generates tool schemas from Rust structs using `schemars::JsonSchema`:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShellArgs {
    /// The shell command to execute
    pub command: String,
    /// Working directory (optional, defaults to current)
    pub cwd: Option<String>,
    /// Timeout in seconds (optional, defaults to 120)
    pub timeout: Option<u64>,
}
```

The `schemars` derive macro converts Rust doc comments into JSON Schema
`description` fields and Rust types into JSON Schema types. This ensures the schema
always matches the implementation — a significant advantage over hand-maintained
schemas that can drift from the actual tool signature.

---

## 8. Tool Visibility and Filtering

### 8.1 Codex's ToolRouter

Codex explicitly separates internal tool implementations from what the model sees.
The `ToolRouter` maintains a `model_visible_specs: Vec<ToolSpec>` field that controls
exactly which tools appear in the model's context.

```rust
pub struct ToolRouter {
    /// All registered tools (internal)
    tools: HashMap<String, Box<dyn Tool>>,
    /// Only these specs are sent to the model
    model_visible_specs: Vec<ToolSpec>,
}
```

This separation enables several patterns:

1. **Hidden tools** — Internal tools used by the framework but not directly
   invocable by the model (logging, metrics, state management).
2. **Contextual visibility** — Different tool sets for different task phases (e.g.,
   planning phase shows `search` tools; editing phase adds `edit` tools).
3. **Capability gating** — Restricting tools based on permission level or sandbox
   configuration.

### 8.2 Claude Code's MCP Tool Search

Claude Code faces a unique challenge: with 30+ built-in tools plus an arbitrary
number of MCP-connected external tools, the total tool description payload can
dominate the context window.

Their solution: when MCP tools exceed 10% of the context window, they activate
a search-based tool discovery mechanism. Instead of listing all MCP tools in the
system prompt, the model is given a single meta-tool that searches available tools
by description. The model calls this search tool, discovers relevant tools, and then
uses them.

This two-phase approach (discover, then invoke) trades latency for context
efficiency. It works because MCP tool usage is typically sparse — most turns use
only built-in tools.

### 8.3 Permission Tiers

Claude Code organizes its 30+ tools into permission tiers that control which tools
require user confirmation before execution. This is not directly a description
engineering concern, but it affects how tools are presented:

```json
{
  "name": "bash",
  "requires_confirmation": true,
  "sandbox": "optional",
  "description": "..."
}
```

Gemini CLI takes a similar approach. Each tool includes `confirmation_requirements`
and `sandbox_requirements` metadata alongside the description and parameter schema.
This metadata is not shown to the model but controls framework behavior.

---

## 9. Tool-Call Correction Layers

### 9.1 ForgeCode's Correction Architecture

ForgeCode implements a correction layer that intercepts tool calls between the
model's output and the tool dispatch system. This layer uses heuristic and static
analysis to validate arguments, catch common errors, and auto-correct when possible.

The correction pipeline:

```
Model Output -> JSON Parse -> Schema Validate -> Heuristic Correct -> Dispatch
                                  |                    |
                                  v                    v
                             [Reject + retry]    [Log correction]
```

Common corrections include:

1. **Path normalization** — Resolving relative paths, fixing path separators,
   expanding `~` to home directory.
2. **Whitespace repair** — Fixing common `old_str` mismatches caused by
   indentation differences.
3. **Argument transplant** — Moving an argument supplied in the wrong field to the
   correct field (e.g., content provided as `old_str` instead of `new_str` when
   creating a file).
4. **Type coercion** — Similar to Goose, converting string-encoded numbers to
   actual numbers.

### 9.2 Failure Budgets

ForgeCode enforces `max_tool_failure_per_turn: 3`. After three failed tool calls
in a single turn, the framework halts and reports the failures to the model as
context for its next generation. This prevents infinite retry loops where the model
repeatedly generates the same malformed call.

The failure budget is a pragmatic control: it acknowledges that models will
sometimes fail, bounds the cost of failure, and converts failures into learning
signal for subsequent attempts.

### 9.3 Correction vs Schema Improvement

Correction layers are tactical — they fix symptoms. Schema improvement is strategic
— it fixes causes. The ideal progression:

1. Deploy correction layer to handle immediate reliability issues.
2. Log all corrections with tool name, error type, and frequency.
3. Analyze correction logs to identify systematic patterns.
4. Improve tool descriptions to eliminate the root causes.
5. Watch correction frequency drop as descriptions improve.

A mature system still maintains the correction layer as a safety net, but the
correction rate should be low and declining.

---

## 10. Testing Tool Descriptions

### 10.1 ForgeCode's Per-Tool Micro-Evaluations

ForgeCode advocates per-tool, per-model micro-evaluations integrated into CI/CD.
These are not end-to-end agent benchmarks — they are narrow tests that isolate tool
description quality from agent planning quality.

A micro-evaluation tests a single tool against a set of scenarios:

```python
# Pseudo-code for a tool description micro-evaluation
class TestStrReplaceEditor:
    tool_schema = load_schema("str_replace_editor")
    
    def test_basic_replacement(self, model):
        """Model should generate correct str_replace call."""
        prompt = (
            "Replace 'hello' with 'world' in /tmp/test.py. "
            "The file contains: def greet():\n    print('hello')"
        )
        result = model.generate_tool_call(
            tools=[self.tool_schema],
            prompt=prompt
        )
        assert result.tool == "str_replace_editor"
        assert result.args["command"] == "str_replace"
        assert result.args["old_str"] == "hello"
        assert result.args["new_str"] == "world"
    
    def test_multiline_replacement(self, model):
        """Model should preserve indentation in multiline old_str."""
        # ...
    
    def test_no_match_awareness(self, model):
        """Model should not attempt replacement when told content doesn't exist."""
        # ...
    
    def test_disambiguation_from_create(self, model):
        """Model should pick create, not str_replace, for new files."""
        # ...
```

### 10.2 Evaluation Dimensions

Each micro-evaluation should measure:

1. **Selection accuracy** — Did the model pick the right tool?
2. **Argument validity** — Are all required fields present with correct types?
3. **Argument correctness** — Are the argument values semantically correct?
4. **Robustness** — Does accuracy hold across prompt variations?

### 10.3 CI/CD Integration

Tool description changes should trigger micro-evaluations automatically:

```yaml
# .github/workflows/tool-eval.yml
on:
  push:
    paths:
      - 'tools/schemas/**'
      - 'tools/descriptions/**'

jobs:
  tool-eval:
    strategy:
      matrix:
        model: [gpt-4.1, gpt-5.4, claude-sonnet-4-20250514, claude-sonnet-4-20250514]
    steps:
      - name: Run tool micro-evaluations
        run: |
          python -m tool_eval \
            --schema-dir tools/schemas/ \
            --model ${{ matrix.model }} \
            --output results/${{ matrix.model }}.json
      - name: Check regression
        run: |
          python -m tool_eval.compare \
            --baseline results/baseline/${{ matrix.model }}.json \
            --current results/${{ matrix.model }}.json \
            --threshold 0.95
```

This catches regressions: if a schema change degrades tool-call accuracy for any
model in the matrix, the CI pipeline fails.

---

## 11. Tool Description Anti-Patterns

### 11.1 The "Kitchen Sink" Description

Overloading a single tool with too many capabilities, using long `enum` lists for
a `command` parameter:

```json
{
  "name": "file_tool",
  "parameters": {
    "type": "object",
    "properties": {
      "command": {
        "enum": ["read", "write", "append", "delete", "rename", "copy",
                 "move", "chmod", "chown", "stat", "search", "replace",
                 "diff", "merge", "compress", "decompress"]
      }
    }
  }
}
```

This creates tool selection ambiguity (when to use `file_tool` with `command:
search` vs a dedicated `search` tool) and argument confusion (each command needs
different arguments, but they all share one parameter schema).

### 11.2 The "Invisible Constraint"

Failing to document constraints that the tool enforces:

```json
{
  "name": "edit_file",
  "description": "Edit a file.",
  "parameters": {
    "properties": {
      "path": { "type": "string" },
      "old_str": { "type": "string" },
      "new_str": { "type": "string" }
    }
  }
}
```

The description does not mention that `old_str` must be unique in the file, that
the file must exist, that paths must be absolute, or what happens on failure. Every
undocumented constraint is a potential error the model cannot avoid because it was
never told about it.

### 11.3 The "Copy-Paste Schema"

Using identical or near-identical descriptions for tools that serve different
purposes. If `read_file` and `view_file` have the same description, the model has
no basis for choosing between them.

### 11.4 The "Nested Everything" Schema

Deeply nesting parameters when flat alternatives exist (see Section 3.2).

### 11.5 The "Missing Truncation" Signal

Returning large outputs without documenting truncation behavior. The model receives
partial output, treats it as complete, and makes decisions on incomplete information.

### 11.6 The "Dynamic Description" Without Grounding

Generating tool descriptions dynamically but failing to inject context that the
model needs. OpenHands avoids this by injecting `cwd` into every tool description.
A common mistake is describing a file-edit tool without mentioning the project root
or current directory, forcing the model to guess path prefixes.

### 11.7 The "Description-Schema Mismatch"

When the description text promises behavior that the schema does not support:

```json
{
  "name": "search",
  "description": "Search files by content or name. Supports regex patterns.",
  "parameters": {
    "properties": {
      "query": { "type": "string", "description": "Search query" }
    }
  }
}
```

The description mentions "by content or name" and "regex patterns," but the schema
has a single `query` field with no way to specify search type or enable regex mode.
The model may attempt to encode these distinctions into the query string in
unpredictable ways.

---

## 12. Best Practices Summary

### Schema Engineering

1. **Place `required` before `properties`** in every JSON schema. This is the
   single highest-ROI change for reducing malformed tool calls.
2. **Keep schemas flat.** Eliminate nesting wherever possible. If a tool needs
   complex input, prefer multiple flat parameters over nested objects.
3. **Use training-data-aligned names.** `old_str`/`new_str`, `command`, `path`,
   `content` — not creative alternatives.
4. **Document truncation explicitly.** State the truncation threshold, the marker
   string, and the recovery action.

### Description Text

5. **Follow the four-part pattern:** what it does, when to use it, constraints, and
   behavioral notes.
6. **Disambiguate from similar tools.** If `edit_file` and `create_file` both
   exist, each description should mention the other and explain when to prefer it.
7. **Document failure modes.** What happens when `old_str` is not found? When the
   file does not exist? When the command times out?
8. **Adapt description length to the model.** Use OpenHands' full/short pattern or
   a similar mechanism to serve optimal descriptions per model family.

### System Architecture

9. **Separate model-visible specs from internal tools.** Follow Codex's
   `model_visible_specs` pattern to control context usage.
10. **Implement argument coercion.** Follow Goose's approach but log all coercions
    for schema improvement feedback.
11. **Deploy a correction layer.** Intercept and fix common errors before dispatch,
    with a failure budget (e.g., `max_tool_failure_per_turn: 3`).
12. **Namespace external tools.** Use `source__tool_name` or `source_tool_name` to
    prevent collisions.

### Testing and Maintenance

13. **Run per-tool, per-model micro-evaluations.** Test selection accuracy, argument
    validity, argument correctness, and robustness.
14. **Integrate evaluations into CI/CD.** Schema changes trigger automated
    regression checks across all supported models.
15. **Monitor correction logs.** Persistent coercion or correction patterns indicate
    description deficiencies that should be fixed at the source.
16. **Treat tool descriptions as code.** Version them, review them, test them, and
    measure their impact on agent performance.

### The Aider Principle

17. **Simplicity wins.** Aider's finding — "The simpler the output format, the
    better the LLM's actual coding performance" — applies to tool descriptions too.
    Every layer of complexity in a tool schema is a potential error surface. Start
    simple, add complexity only when measurement shows it improves outcomes.

---

## References

- ForgeCode schema engineering findings and tool failure taxonomy
- OpenHands dual-description system (`create_cmd_run_tool`, full/short selection)
- Claude Code tool-use architecture (30+ tools, permission tiers, MCP Tool Search)
- Goose argument coercion and `schemars::JsonSchema` auto-generation
- Aider edit-format-as-interface design philosophy
- Gemini CLI tool metadata (confirmation, sandbox requirements, `get_internal_docs`)
- OpenCode `ToolInfo` struct and MCP prefixing
- Ante Rust `Tool` trait (`name()`, `description()`, `input_schema()`, `call()`)
- Sage-Agent `ToolBase` interface and tool catalog assembly
- Codex `ToolRouter` and `model_visible_specs`