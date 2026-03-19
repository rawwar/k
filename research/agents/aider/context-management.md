# Aider — Context Management

## Overview

Context management is one of Aider's strongest technical contributions. The challenge: LLMs have finite context windows, but real codebases have thousands of files. Aider solves this with a multi-layered context strategy centered on the **repo-map** — a tree-sitter-powered, graph-ranked summary of the entire codebase.

## The Three Layers of Context

```
┌─────────────────────────────────────────────────┐
│  Layer 1: ADDED FILES (full content)             │
│  Files the user explicitly /add'd to the chat    │
│  These are editable by the LLM                   │
│  Highest token cost, highest fidelity             │
├─────────────────────────────────────────────────┤
│  Layer 2: REPO MAP (ranked summary)              │
│  Tree-sitter extracted symbols from ALL files    │
│  Graph-ranked by importance/relevance            │
│  Token-budgeted to fit available context          │
├─────────────────────────────────────────────────┤
│  Layer 3: CHAT HISTORY                           │
│  Previous conversation messages                   │
│  Summarized when too long                         │
└─────────────────────────────────────────────────┘
```

## Repo-Map: Deep Dive

### What It Is

The repo-map is a **concise structural summary** of the entire git repository. It shows:
- File paths
- Class definitions (names, inheritance)
- Function/method signatures (names, parameters, types)
- Key variable declarations
- Import relationships

But critically, it does **not** show:
- Function bodies / implementation details
- Comments
- Most variable assignments
- Test code internals

### How It's Built

#### Step 1: Tree-Sitter Parsing

Aider uses [tree-sitter](https://tree-sitter.github.io/tree-sitter/) to parse every source file into an Abstract Syntax Tree (AST). Tree-sitter is the same parsing library used by many IDEs for syntax highlighting, code folding, and navigation.

For each file, aider extracts:
- **Definitions**: Where symbols (functions, classes, variables) are defined
- **References**: Where symbols from other files are used

This is done using custom `.scm` (Scheme) query files in `aider/queries/`, one per language:
- `tree-sitter-python-tags.scm`
- `tree-sitter-javascript-tags.scm`
- `tree-sitter-rust-tags.scm`
- ... (supports 100+ languages)

#### Step 2: Dependency Graph Construction

From the definitions and references, aider builds a **file-level dependency graph**:

```
Node = source file
Edge = file A references a symbol defined in file B
```

For example:
```
app.py ──references──→ models.py (uses User class)
app.py ──references──→ utils.py (uses validate_email)
tests/test_app.py ──references──→ app.py (tests endpoints)
```

#### Step 3: Graph Ranking (PageRank-style)

Aider uses a **personalized PageRank** algorithm on this dependency graph to determine which files and symbols are most important. The ranking is "personalized" — biased toward:

1. **Files currently in the chat** — symbols referenced by added files rank higher
2. **Mentioned identifiers** — if the user mentions a class or function name, files containing that symbol rank higher
3. **Recently discussed files** — context from the conversation influences ranking

This is implemented in `repomap.py` using a graph algorithm similar to Google's PageRank, adapted for code structure.

#### Step 4: Token-Budgeted Selection

The ranked symbols are selected to fit within the **token budget** (`--map-tokens`, default 1024 tokens):

1. Start with the highest-ranked symbols
2. For each symbol, include the "key lines" — the definition signature, class declaration, etc.
3. Use `TreeContext` (from `grep_ast`) to format the output with contextual "elision" markers (`⋮...`)
4. Keep adding symbols until the token budget is consumed

#### Step 5: Dynamic Sizing

The repo map size is **not static** — it adjusts based on the conversation state:

- **No files added**: Map expands significantly (up to `map_mul_no_files × map_tokens`, default 8×) because the LLM needs maximum codebase awareness
- **Files added**: Map shrinks to leave room for file content
- **Long conversation**: Map shrinks to leave room for chat history
- **The map never exceeds what fits** — aider calculates available tokens dynamically

### What the Map Looks Like

```
aider/coders/base_coder.py:
⋮...
│class Coder:
│    abs_fnames = None
⋮...
│    @classmethod
│    def create(
│        self,
│        main_model,
│        edit_format,
│        io,
│        skip_model_availabily_check=False,
│        **kwargs,
⋮...
│    def abs_root_path(self, path):
⋮...
│    def run(self, with_message=None):
⋮...

aider/commands.py:
⋮...
│class Commands:
│    voice = None
⋮...
│    def get_commands(self):
⋮...
│    def run(self, inp):
⋮...
```

The `⋮...` markers indicate elided code. The `│` markers show the indentation level. This compact format conveys the structure without wasting tokens on implementation.

### Caching

The repo map uses multiple caching layers for performance:

1. **Tags cache** — Tree-sitter parse results are cached to disk (`.aider.tags.cache.v4/`)
2. **Tree context cache** — Formatted output is cached in memory
3. **Map cache** — The fully assembled map is cached, keyed on the set of chat files + mentioned identifiers
4. **Refresh modes**:
   - `auto` (default) — Refresh when files change
   - `always` — Rebuild every time
   - `manual` — Only refresh on `/map-refresh`

## File Selection: Manual vs. Assisted

### Manual Selection
Users explicitly add files with `/add`:
```
/add src/auth/login.py src/models/user.py
```

The repo map helps here — the LLM can see the structure of the entire codebase and ask for specific files:
> "I need to see `src/utils/validation.py` to understand how validation works. Can you add it?"

Aider will then prompt the user to add the requested file.

### Watch Mode (Semi-Automatic)
With `--watch-files`, adding an `# AI` comment to any file automatically adds it to the chat context.

### Mentioned File Detection
Aider automatically detects when the LLM mentions file paths in its response and can suggest adding them.

## Token Budget Management

Aider carefully manages the available context window:

```
Total Context Window (e.g., 128k tokens)
├── System prompt + edit format instructions (~1-2k)
├── Repo map (dynamic, default budget: 1k, expandable to 8k)
├── Added files (full content)
├── Read-only files (full content)
├── Chat history (dynamic, summarized when too long)
├── Current user message
└── Reserved for LLM response (~4k minimum)
```

### Token Counting

Aider counts tokens using the model's actual tokenizer when messages are short. For longer texts, it uses a **sampling-based estimation** for efficiency:

```python
def token_count(self, text):
    if len(text) < 200:
        return self.main_model.token_count(text)
    # Sample every 100th line, extrapolate
    lines = text.splitlines(keepends=True)
    step = len(lines) // 100 or 1
    sample = lines[::step]
    sample_tokens = self.main_model.token_count("".join(sample))
    return sample_tokens / len("".join(sample)) * len(text)
```

### History Summarization

When conversation history grows too large, aider can summarize older messages to reclaim context space. This keeps the most recent and most relevant context while compressing earlier discussion.

## Comparison to Other Tools

| Feature | Aider | Claude Code | Cursor | Copilot |
|---------|-------|-------------|--------|---------|
| **Codebase awareness** | Repo-map (tree-sitter + PageRank) | Tool calls to read files | Embeddings index | Line-level context |
| **Automatic context** | Repo-map auto-ranks | Agent decides what to read | RAG retrieval | Immediate context |
| **Manual control** | /add, /drop | Agent handles it | @file mentions | None |
| **Token efficiency** | Ranked symbol extraction | Full file reads | Chunk-based | Line windows |
| **Language support** | 100+ via tree-sitter | All (reads raw text) | Many | Many |

The repo-map approach is uniquely **proactive** — it gives the LLM awareness of the full codebase structure before it even asks. This is in contrast to agent-based approaches where the model must explicitly request files, potentially missing relevant context it didn't know existed.