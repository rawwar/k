---
title: "File Editing Tools"
---

# File Editing Tools

How coding agents modify code — from whole-file replacement to syntax-aware incremental edits.

## 1. The File Editing Challenge

Modifying source code is the single most critical capability of a coding agent. Every
other feature — planning, retrieval, reasoning — is in service of producing correct
edits. And yet, reliable code editing is one of the hardest problems in agent design.

LLMs don't produce perfect diffs. They hallucinate line numbers, forget context,
and sometimes invent code that was never in the file. A model asked to produce a
unified diff will confidently write `@@ -42,7 +42,8 @@` when the actual content
starts at line 56.

This creates a fundamental design tension with three axes:

- **Precision**: How surgically can the agent target the exact code to change?
- **Token cost**: How many tokens does the edit format consume?
- **Error recovery**: What happens when the model's output doesn't perfectly match?

The evolution roughly follows this trajectory:

1. **Whole-file replacement** (2023): Simple, expensive, works with weak models
2. **Search/replace blocks** (2023-2024): Aider's breakthrough — exact text matching
3. **Line-range edits** (2024): Specify line numbers for replacement regions
4. **Syntax-aware edits** (2024-2025): Tree-sitter guided structural modifications
5. **Multi-edit batching** (2025): Multiple surgical edits in a single tool call

Each generation reduced token cost while increasing the complexity of the matching
and error-recovery machinery needed to make edits reliable.

## 2. Aider's 6 Edit Formats — A Deep Dive

Aider is the most extensively benchmarked coding agent for edit format design. Paul
Gauthier's systematic exploration has shaped the entire field. Aider supports six
distinct edit formats, each optimized for different models.

### 2.1 diff (SEARCH/REPLACE Blocks)

The flagship format. The model identifies existing code by content match, then
provides the replacement:

```
path/to/file.py
<<<<<<< SEARCH
def calculate_total(items):
    total = 0
    for item in items:
        total += item.price
    return total
=======
def calculate_total(items):
    total = 0
    for item in items:
        total += item.price * item.quantity
    return total
>>>>>>> REPLACE
```

**Why this works**: The model doesn't need line numbers. It identifies target code
by *content*, which LLMs are much better at reproducing than positional metadata.

**Best for**: Claude 3.5 Sonnet, GPT-4o, and other strong models.

**Aider's fuzzy matching** when exact match fails:

1. **Exact match**: Character-for-character
2. **Strip trailing whitespace**: Remove trailing spaces/tabs, retry
3. **Ignore blank lines**: Remove all blank lines, retry
4. **Normalized whitespace**: Collapse whitespace sequences to single spaces

This cascade is critical — models frequently introduce subtle whitespace differences.

### 2.2 whole

The model returns the entire file content. Just overwrite.

**Best for**: Weak models and small files. Highest token cost but most reliable —
no matching needed. Impractical above ~500 lines due to token cost and code drift.

### 2.3 diff-fenced

SEARCH/REPLACE with the filename inside a fenced code block:

```python path/to/file.py
<<<<<<< SEARCH
old code
=======
new code
>>>>>>> REPLACE
```

**Why this exists**: Gemini models. Google's Gemini was trained on massive quantities
of documentation containing fenced code blocks. The models produce more reliable
output when edits are wrapped in fenced blocks with language identifiers.

### 2.4 udiff (Unified Diff)

Standard unified diff format with `@@` line number headers:

```diff
--- a/path/to/file.py
+++ b/path/to/file.py
@@ -1,5 +1,5 @@
 def calculate_total(items):
     total = 0
     for item in items:
-        total += item.price
+        total += item.price * item.quantity
     return total
```

**Designed for**: GPT-4 Turbo. It had a tendency toward "laziness" — the unified diff
format forces context lines, acting as an anti-laziness mechanism. Models frequently
get `@@` line numbers wrong, so Aider matches on content instead.

### 2.5 architect

A two-model pipeline separating planning from execution:

```
┌─────────────────┐    natural language     ┌──────────────────┐
│  Reasoning Model │ ── change description ─→│  Editing Model   │
│  (o1, o3, R1)   │                         │  (Claude, GPT-4) │
└─────────────────┘                         └──────────────────┘
```

The reasoning model describes changes in natural language; the editing model produces
SEARCH/REPLACE edits. Reasoning models excel at *planning* but are poor at producing
precisely formatted edit blocks — they paraphrase code rather than reproduce it exactly.

### 2.6 Function Calling Variants (whole-func / diff-func)

Aider tested wrapping edits in JSON function calls:

```json
{
  "name": "apply_edit",
  "arguments": {
    "path": "file.py",
    "search": "def calculate_total(items):\n    total = 0\n",
    "replace": "def calculate_total(items, tax_rate=0.0):\n    total = 0\n"
  }
}
```

**Key finding**: Function calling performed **worse** than plain text. Producing valid
JSON with escaped strings diverts model attention from code reasoning. This shaped
industry thinking — plain text with clear delimiters often outperforms structured JSON
for code editing. Several agents (Claude Code, OpenHands) adopted text-based formats
over function calling for this reason.

## 3. Claude Code's File Editing

Claude Code provides four specialized file manipulation tools:

**Edit Tool** — Primary mechanism. Uses `old_str` / `new_str` parameters:

```
Tool: Edit
file_path: src/auth.py
old_str: |
  def verify_token(token):
      return jwt.decode(token, SECRET_KEY)
new_str: |
  def verify_token(token):
      try:
          return jwt.decode(token, SECRET_KEY, algorithms=["HS256"])
      except jwt.ExpiredSignatureError:
          raise AuthenticationError("Token expired")
```

The `old_str` must match exactly one location. Zero matches → fail. Multiple
matches → fail with request for more context to disambiguate.

**Write Tool** — Creates new files or completely replaces existing content.

**MultiEdit Tool** — Batches multiple search-and-replace operations in one call.
Edits apply sequentially — each subsequent edit sees previous results:

```
Tool: MultiEdit
file_path: src/models.py
edits:
  - old_str: "class User:"     → new_str: "class User(BaseModel):"
  - old_str: "class Product:"  → new_str: "class Product(BaseModel):"
  - old_str: "class Order:"    → new_str: "class Order(BaseModel):"
```

**NotebookEdit Tool** — Jupyter notebook cell editing. Operates on cells by index,
handling `.ipynb` JSON structure transparently.

## 4. OpenCode's File Tools

OpenCode takes a line-range approach to editing:

**FileEdit** — Specifies a contiguous range of lines to replace:

```json
{
  "tool": "file_edit",
  "path": "src/handler.go",
  "start_line": 42,
  "end_line": 48,
  "content": "func handleRequest(w http.ResponseWriter, r *http.Request) {\n    ctx := r.Context()\n}"
}
```

Line numbers are fragile (they shift with earlier edits) but unambiguous —
eliminating the "multiple matches" problem of search-and-replace.

**FileWrite / FileRead** — Create/overwrite files and read current contents.

**LSP Integration** — OpenCode's distinguishing feature. After every edit, it
queries the language server for diagnostics — type errors, undefined references.
This creates a tight edit-verify-fix feedback loop.

## 5. OpenHands' str_replace_editor

OpenHands uses a discriminated-union design: one tool, five operations.

```python
# View file contents
{"command": "view", "path": "/repo/src/main.py", "view_range": [1, 50]}

# Create new file
{"command": "create", "path": "/repo/src/utils.py", "file_text": "def helper():\n    pass"}

# Search and replace
{"command": "str_replace", "path": "/repo/src/main.py",
 "old_str": "import os", "new_str": "import os\nimport sys"}

# Insert at line
{"command": "insert", "path": "/repo/src/main.py",
 "insert_line": 10, "new_str": "# TODO: add error handling"}

# Undo last edit
{"command": "undo_edit", "path": "/repo/src/main.py"}
```

**Undo capability** is unique among major agents. When an edit produces broken code,
the agent reverts and retries. The stack is per-file and single-level.

**Error messages** include helpful context on failure:
```
No match found for old_str. Did you mean one of:
  Line 12: "import os  "  (trailing whitespace differs)
```

Originally inspired by SWE-agent's edit tool from the Princeton NLP group.

## 6. diff-match-patch (Google)

Google's diff-match-patch library (2006) is battle-tested infrastructure that
several agents build upon. Three core algorithms:

1. **Diff** (Myers' algorithm): Minimal edit distance between two texts
2. **Match** (Bitap): Fuzzy string matching tolerating errors
3. **Patch**: Best-effort patch application — if the target has shifted, fuzzy
   matching finds the closest valid application point

```python
import diff_match_patch as dmp_module
dmp = dmp_module.diff_match_patch()
patches = dmp.patch_make("original text here", "modified text here")
result, success = dmp.patch_apply(patches, "original text here  ")  # shifted text
# Patch applied successfully despite differences
```

Available in C++, C#, Java, JavaScript, Python, and more. Aider uses it as the
final fallback when all other matching strategies have failed.

## 7. tree-sitter for Syntax-Aware Edits

Tree-sitter is an incremental parser generator producing concrete syntax trees.
Originally built for the Atom editor, it's now foundational for coding agents.

**Key properties**: Incremental (re-parses only changed portions), error-tolerant
(valid tree even with syntax errors), 200+ language grammars, uniform API.

**How agents use it**:

- **Aider — Repository Map**: Extracts function signatures, class definitions across
  the entire repo. Included in prompt context for structural overview.

```python
# tree-sitter query for Python function definitions
(function_definition
  name: (identifier) @function.name
  parameters: (parameters) @function.params)
```

- **Claude Code**: Structural navigation — finding class boundaries, function scopes
- **Syntax-aware edits**: Target tree-sitter nodes ("replace the body of function X")
  instead of searching for text strings — eliminates ambiguity
- **Symbol extraction**: Building call graphs and dependency maps for context management

## 8. LSP Integration for Code Intelligence

The Language Server Protocol provides rich feedback for validating edits:

**Diagnostics** — After an edit, the LSP reports errors immediately:
```json
{
  "diagnostics": [{
    "range": {"start": {"line": 12}},
    "severity": 1,
    "message": "Property 'naem' does not exist on type 'User'. Did you mean 'name'?"
  }]
}
```

**Code navigation** — Go-to-definition, find-references, and hover for type info.
Essential for safe renaming and understanding code before editing.

**Agent adoption**: OpenCode has deep LSP integration (waits for diagnostics after
each edit). OpenHands can invoke type checking. Warp uses LSP for real-time
intelligence. Claude Code favors tree-sitter over full LSP for speed.

**Limitation**: LSP startup cost. Language servers for TypeScript or Rust can take
10-30 seconds to initialize. For short agent sessions, this overhead may not pay off.

## 9. Edit Format Trade-offs

| Dimension          | Search/Replace      | Unified Diff         | Whole File          |
|--------------------|---------------------|----------------------|---------------------|
| **Token cost**     | Low                 | Medium               | High                |
| **Precision**      | High (exact text)   | High (line-level)    | Perfect (no match)  |
| **Error recovery** | Fuzzy matching      | Line-number tolerance| None needed         |
| **Model req.**     | Strong              | Medium               | Any                 |
| **File size limit**| Scales well         | Scales well          | ~500 lines max      |
| **Multi-edit**     | Multiple blocks     | Multiple hunks       | N/A                 |
| **Failure mode**   | Ambiguous/no match  | Wrong line numbers   | Code drift          |

**Practical guidelines**:
- Small files (<100 lines): Whole-file is simplest
- Medium files (100-500 lines): Search/replace offers the best balance
- Large files (500+ lines): Search/replace essential; whole-file impractical
- Weak models: Whole-file regardless of size
- Reasoning models (o1, R1): Architect mode — they plan well but edit poorly

## 10. Error Recovery When Edits Fail

Every edit format fails sometimes. Robust agents implement multiple recovery layers.

**Aider's 4-level fuzzy matching**:
```
Level 0: Exact character-by-character match
Level 1: Strip trailing whitespace
Level 2: Remove all blank lines
Level 3: Normalize all whitespace (collapse to single spaces)
Level 4: diff-match-patch best-effort fuzzy application
```

**Line number recalculation**: For diff formats, agents track running offsets. If
Edit A inserts 3 lines at line 10, Edit B targeting line 50 adjusts to line 53.

**Retry with expanded context**: Re-read the file, include the failed edit and error
message, ask the model to produce a corrected edit with more surrounding context.

**Undo and retry** (OpenHands): Apply edit → run tests → tests fail → undo →
re-read file → apply corrected edit → run tests → pass.

**Tool-call correction layers**: Middleware between LLM output and file system that
fixes common JSON escaping errors, normalizes paths, detects edits that would produce
syntax errors (via tree-sitter), and auto-corrects obvious issues.

**The cost of failure**: Each failed edit requires at minimum one additional LLM
round-trip. At $3-15 per million tokens, a single failed edit costs $0.01-0.10 —
compounding across a session. This economic pressure drives investment in fuzzy
matching: it's cheaper to build robust matching than to pay for retry loops.

---

*The choice of edit format is one of the most consequential architectural decisions
in coding agent design. The field is converging on search-and-replace as the default
for strong models, with whole-file as fallback, but innovations in syntax-aware
editing and LSP-guided validation continue to push the frontier.*
