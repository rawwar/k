---
title: Static Analysis for Code Understanding
status: complete
---

# Static Analysis

> How coding agents use static analysis — AST parsing, tree-sitter, semantic analysis, type inference, and complexity metrics — to understand code structure before making edits.

## Overview

Static analysis is the examination of source code without executing it. For coding agents, static analysis provides **structural understanding** — awareness of what functions exist, how classes are organized, what types flow through the system, and how complex the code is. This understanding enables agents to make precise, targeted edits rather than blind modifications.

Among the 17 agents studied, static analysis adoption varies dramatically:

| Agent | Static Analysis Approach | Primary Tool | Depth |
|---|---|---|---|
| **Aider** | Tree-sitter tag extraction for repo map | tree-sitter | Deep — all files indexed |
| **Claude Code** | Tree-sitter for View tool summarization | tree-sitter | Shallow — on-demand per file |
| **Droid** | Tree-sitter for incremental indexing | tree-sitter | Deep — incremental updates |
| **Ante** | Tree-sitter for embedding preparation | tree-sitter | Medium — symbol extraction |
| **OpenCode** | Optional tree-sitter integration | tree-sitter | Shallow — optional feature |
| **ForgeCode** | AST-based entry point detection | tree-sitter | Medium — targeted analysis |
| **Junie CLI** | JetBrains platform PSI trees | JetBrains PSI | Deep — full IDE analysis |
| **Others** | No built-in static analysis | None | None |

The clear pattern: **tree-sitter is the universal foundation** for static analysis in coding agents. Only Junie CLI (backed by JetBrains) uses an alternative, and that's because it inherits JetBrains' decades-old PSI (Program Structure Interface) system.

---

## Abstract Syntax Trees (ASTs)

### What Is an AST?

An Abstract Syntax Tree is a tree representation of source code where each node represents a syntactic construct. Unlike raw text, ASTs capture the *structure* of code — the nesting of blocks, the relationship between function names and their parameters, the hierarchy of class definitions.

Consider this Python function:

```python
def calculate_total(items, tax_rate=0.1):
    subtotal = sum(item.price for item in items)
    return subtotal * (1 + tax_rate)
```

The corresponding AST (simplified) looks like:

```
function_definition
├── name: "calculate_total"
├── parameters
│   ├── parameter: "items"
│   └── default_parameter
│       ├── name: "tax_rate"
│       └── value: 0.1
└── body
    ├── assignment
    │   ├── left: "subtotal"
    │   └── right: call
    │       ├── function: "sum"
    │       └── argument: generator_expression
    │           ├── element: attribute
    │           │   ├── object: "item"
    │           │   └── attribute: "price"
    │           └── for_in_clause
    │               ├── variable: "item"
    │               └── iterable: "items"
    └── return_statement
        └── binary_expression
            ├── left: "subtotal"
            ├── operator: "*"
            └── right: parenthesized_expression
                └── binary_expression
                    ├── left: 1
                    ├── operator: "+"
                    └── right: "tax_rate"
```

### Why ASTs Matter for Agents

ASTs provide several capabilities that raw text cannot:

1. **Precise symbol identification**: An AST can distinguish between a function definition and a function call, between a variable assignment and a variable reference, between a class name in a definition and a class name in an import.

2. **Structural navigation**: Given an AST, you can programmatically find "all function definitions in this file" or "all classes that inherit from BaseModel" without fragile regex patterns.

3. **Safe editing**: AST-aware edits can replace a function body without accidentally modifying comments or strings that happen to contain similar text.

4. **Language-independent patterns**: The same structural queries work across languages — "find all function definitions" maps to `function_definition` in Python, `function_declaration` in JavaScript, `func_declaration` in Go.

---

## Tree-sitter: The Universal Parser

### Why Tree-sitter Won

Tree-sitter has become the de facto standard for code parsing in coding agents. Several properties make it uniquely suited:

1. **Multi-language support**: A single library handles 50+ languages through grammar plugins. An agent using tree-sitter automatically supports Python, JavaScript, TypeScript, Rust, Go, Java, C, C++, Ruby, and many more.

2. **Error tolerance**: Tree-sitter produces valid ASTs even for syntactically incorrect code. This is critical for agents, which frequently encounter partially-edited files. A traditional compiler parser would fail; tree-sitter marks error nodes but continues parsing.

3. **Incremental parsing**: When code changes, tree-sitter can re-parse only the affected portion of the AST rather than re-parsing the entire file. For agents making iterative edits, this is a significant performance win.

4. **Speed**: Tree-sitter parses files in milliseconds. Even parsing every file in a large codebase takes seconds, not minutes.

5. **C runtime, many bindings**: The core library is written in C with bindings for Python, JavaScript, Rust, Go, and more. Agents in any language can use it.

### Tree-sitter Architecture

```
Source Code (text)  →  Grammar (rules)  →  Parser  →  Concrete Syntax Tree (CST)
                                                            │
                                                            ▼
                                                    Node queries via
                                                    S-expression patterns
```

Tree-sitter grammars are defined in JavaScript and compiled to C. Each grammar describes the syntax of a language using rules:

```javascript
// Simplified tree-sitter grammar for a function definition
module.exports = grammar({
  name: 'example',
  rules: {
    function_definition: $ => seq(
      'def',
      $.identifier,
      '(',
      optional($.parameter_list),
      ')',
      ':',
      $.block
    ),
    // ...
  }
});
```

### Tree-sitter Queries

Tree-sitter's query system uses S-expression patterns to match AST nodes. This is how agents extract specific structural information:

```scheme
;; Find all function definitions with their names
(function_definition
  name: (identifier) @function.name) @function.def

;; Find all class definitions
(class_definition
  name: (identifier) @class.name) @class.def

;; Find all import statements
(import_statement
  name: (dotted_name) @import.name) @import.stmt

;; Find all method calls on specific objects
(call
  function: (attribute
    object: (identifier) @object
    attribute: (identifier) @method))
```

### How Aider Uses Tree-sitter

Aider's repo map is the most sophisticated use of tree-sitter among CLI agents. The process:

1. **Parse every file** in the repository with tree-sitter
2. **Extract tags** — definitions (function defs, class defs) and references (function calls, imports)
3. **Build a graph** where files are nodes and tag references create edges
4. **Rank with PageRank** to find the most-referenced symbols
5. **Generate a condensed map** showing the top-ranked symbols with their signatures

```python
# Simplified from Aider's RepoMap implementation
class RepoMap:
    def get_ranked_tags(self, chat_fnames, other_fnames):
        defines = defaultdict(set)  # tag -> set of files that define it
        references = defaultdict(set)  # tag -> set of files that reference it

        for fname in all_fnames:
            tags = self.get_tags(fname)  # tree-sitter extraction
            for tag in tags:
                if tag.kind == "def":
                    defines[tag.name].add(fname)
                elif tag.kind == "ref":
                    references[tag.name].add(fname)

        # Build graph and rank with PageRank
        G = nx.MultiDiGraph()
        for tag_name in defines:
            for definer in defines[tag_name]:
                for referencer in references.get(tag_name, []):
                    G.add_edge(referencer, definer, weight=1.0)

        ranked = nx.pagerank(G)
        return sorted(ranked.items(), key=lambda x: -x[1])
```

The tree-sitter tag extraction uses language-specific query files. For Python:

```scheme
;; tags.scm for Python
(function_definition
  name: (identifier) @name) @definition.function

(class_definition
  name: (identifier) @name) @definition.class

(call
  function: [
    (identifier) @name
    (attribute attribute: (identifier) @name)
  ]) @reference.call
```

### How Claude Code Uses Tree-sitter

Claude Code uses tree-sitter more lightly — for file summarization in its View tool. When the agent reads a file, tree-sitter can provide a structural summary rather than dumping the entire file contents:

```
File: src/auth/middleware.ts (247 lines)

Exports:
  - function authMiddleware(req, res, next)
  - function validateToken(token: string): Promise<User>
  - class AuthError extends Error
  - const AUTH_HEADER = "Authorization"

Imports:
  - jwt from "jsonwebtoken"
  - { User } from "../models/user"
  - { config } from "../config"
```

This summary gives the LLM structural awareness without consuming tokens on implementation details.

---

## Semantic Analysis

Beyond syntax (what the code looks like), semantic analysis examines meaning (what the code does). This is more challenging and less commonly implemented in coding agents.

### Type Inference

Understanding types helps agents write correct code. Consider:

```typescript
function processItems(items: Item[]) {
    return items.map(item => item.transform());
}
```

A type-aware agent knows that `items` is an array of `Item`, that `.map()` returns a new array, and that `.transform()` must be a method on the `Item` type. Without type information, the agent must infer these relationships from context — a less reliable process.

**How agents approach types:**

| Approach | Description | Agents Using It |
|---|---|---|
| **Type annotations** | Read explicit types from TypeScript, Java, Rust code | All agents (via file reading) |
| **LSP hover** | Query the language server for inferred types | Junie CLI, OpenCode |
| **Tree-sitter + heuristics** | Parse type annotations from AST nodes | Aider, Droid |
| **LLM inference** | Let the model infer types from context | All agents (implicitly) |

Most CLI agents rely on the LLM's own ability to infer types from code context rather than performing explicit type analysis. This works surprisingly well for common languages but fails for complex generic types or deeply nested inference chains.

### Symbol Resolution

Symbol resolution — determining what a name refers to — is fundamental to code understanding:

```python
# Which `process` is this?
from data_pipeline import process  # Could be this import
from image_utils import process    # Or this one (shadowing)

result = process(input_data)  # Agent needs to know which one
```

Accurate symbol resolution requires:
1. Understanding import semantics (Python's import system, Node's require/import)
2. Scope analysis (local variables vs. module-level vs. global)
3. Shadowing rules (inner scopes override outer scopes)

LSP provides exact symbol resolution. Without it, agents use heuristics — following imports, checking file contents, relying on naming conventions.

### Data Flow Analysis

Understanding how data moves through code:

```python
def handle_request(request):
    user_id = request.params.get("user_id")     # Source: user input
    user = db.query(User).get(user_id)           # Flows to database query
    return jsonify(user.to_dict())               # Flows to response
```

Data flow analysis reveals:
- **Taint tracking**: User input flows to a database query (potential SQL injection)
- **Null propagation**: If `get("user_id")` returns None, the database query will fail
- **API contracts**: The response depends on `User.to_dict()` — changing that method affects this endpoint

No CLI coding agent currently performs explicit data flow analysis. This is typically the domain of dedicated security tools (Semgrep, CodeQL) rather than coding assistants.

---

## Complexity Analysis

Understanding code complexity helps agents prioritize effort and identify refactoring opportunities.

### Cyclomatic Complexity

Measures the number of linearly independent paths through code. Higher complexity means more branches, more test cases needed, and more potential for bugs:

```python
def process_order(order):           # Complexity: 1 (base)
    if order.is_valid():            # +1 = 2
        if order.has_discount():    # +1 = 3
            apply_discount(order)
        if order.is_priority():     # +1 = 4
            expedite(order)
        return submit(order)
    elif order.is_draft():          # +1 = 5
        return save_draft(order)
    else:
        return reject(order)
    # Total cyclomatic complexity: 5
```

### Cognitive Complexity

A more nuanced metric that accounts for nesting depth and control flow readability:

```python
def process(data):                  # Cognitive: 0
    for item in data:               # +1 (loop)
        if item.is_valid():         # +2 (nesting=1, +1 for if, +1 for nesting)
            for sub in item.subs:   # +3 (nesting=2)
                if sub.active:      # +4 (nesting=3)
                    yield sub
    # Total cognitive complexity: 10
```

### How Agents Use Complexity

Currently, no CLI agent explicitly computes complexity metrics. However, the concept influences agent behavior indirectly:

- **Aider's repo map** implicitly favors simpler, more-referenced functions (they appear higher in PageRank rankings because more code calls them)
- **Claude Code** tends to read shorter files first during exploration, implicitly prioritizing less complex code
- **ForgeCode** uses entry-point detection that often identifies well-factored entry points (which tend to be lower complexity)

**Opportunity**: An agent that computed complexity before editing could:
- Warn about making complex functions more complex
- Suggest refactoring before modification
- Allocate more testing effort to high-complexity changes

---

## Language-Specific Parsing Considerations

### JavaScript / TypeScript
- **JSX/TSX**: Tree-sitter handles JSX natively through the TypeScript grammar
- **Dynamic typing**: Type inference is limited without TypeScript annotations
- **Module systems**: CommonJS (`require`) and ESM (`import`) coexist, complicating symbol resolution
- **Decorators**: Both Stage 3 and legacy decorator syntax must be handled

```typescript
// Tree-sitter can parse all of these patterns
import { Component } from 'react';
const utils = require('./utils');
export default class App extends Component {
  @autobind
  handleClick() { /* ... */ }
}
```

### Python
- **Dynamic dispatch**: Methods can be added at runtime, making static analysis incomplete
- **Decorators**: `@property`, `@staticmethod`, `@classmethod` change method semantics
- **Type hints**: Optional, so many codebases lack them
- **Import complexity**: Relative imports, `__init__.py`, namespace packages

```python
# Tree-sitter parses the structure; semantics require deeper analysis
class Model(BaseModel):
    @validator('email')
    def validate_email(cls, v):
        return v.lower()

    class Config:
        orm_mode = True
```

### Rust
- **Ownership/borrowing**: Static analysis can track borrow relationships
- **Trait implementations**: `impl Trait for Type` creates relationships not visible from definitions alone
- **Macros**: `macro_rules!` and procedural macros generate code that tree-sitter cannot parse in expanded form
- **Generics**: Complex generic bounds require deep type analysis

```rust
// Tree-sitter sees the structure but not the expanded macro
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    timeout: Duration,
}

impl From<RawConfig> for Config {
    fn from(raw: RawConfig) -> Self { /* ... */ }
}
```

### Go
- **Interfaces**: Implicit implementation (no `implements` keyword) makes relationship tracking harder
- **Goroutines/channels**: Concurrency patterns are difficult to analyze statically
- **Code generation**: `go generate` produces code that must be parsed separately

```go
// The interface satisfaction is implicit — only detectable via type checking
type Handler interface {
    ServeHTTP(w http.ResponseWriter, r *http.Request)
}

type MyHandler struct{}

func (h *MyHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
    // Satisfies Handler interface without explicit declaration
}
```

---

## Dead Code Detection

Identifying unused code helps agents understand which code is active and which is legacy. This is a specialized application of reference analysis:

```python
# Reference counting approach
def find_dead_code(repo_tags):
    definitions = {}  # symbol -> definition location
    references = set()  # set of referenced symbols

    for tag in repo_tags:
        if tag.kind == "def":
            definitions[tag.name] = tag.location
        elif tag.kind == "ref":
            references.add(tag.name)

    dead = {name: loc for name, loc in definitions.items()
            if name not in references}
    return dead
```

**Limitations of dead code detection in agents:**
- **Dynamic dispatch**: Code called via reflection, string-based lookups, or dynamic imports appears dead but isn't
- **Entry points**: Main functions, HTTP handlers, CLI commands may have no callers within the codebase
- **Test code**: Test functions are called by the test runner, not by other code
- **Public APIs**: Library code may be called by external consumers

Aider's repo map implicitly performs a form of dead code detection: symbols with zero references rank lowest in PageRank and are excluded from the map, effectively making them invisible to the LLM.

---

## Building a Static Analysis Pipeline for Agents

A comprehensive static analysis pipeline for a coding agent would include:

```
Source Files
    │
    ▼
┌──────────────────┐
│ Language Detection│  Identify language per file (extension + heuristics)
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Tree-sitter Parse│  Parse each file into CST
│                  │  Handle errors gracefully
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Tag Extraction   │  Extract definitions and references
│                  │  Function names, class names, imports
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Symbol Resolution│  Match references to definitions
│                  │  Build cross-file symbol table
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Graph Building   │  Construct dependency graph
│                  │  File → File, Symbol → Symbol
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Ranking          │  PageRank or similar for importance
│                  │  Identify entry points, hot paths
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ Summary Gen      │  Produce condensed representations
│                  │  Repo map, file summaries, symbol index
└──────────────────┘
```

### Implementation Example: Minimal Tag Extractor

```python
import tree_sitter_python as tspython
from tree_sitter import Language, Parser

PY_LANGUAGE = Language(tspython.language())

parser = Parser(PY_LANGUAGE)

TAG_QUERY = PY_LANGUAGE.query("""
(function_definition
  name: (identifier) @function.def)

(class_definition
  name: (identifier) @class.def)

(call
  function: [
    (identifier) @function.ref
    (attribute attribute: (identifier) @method.ref)
  ])

(import_from_statement
  module_name: (dotted_name) @import.module
  name: (dotted_name) @import.name)
""")

def extract_tags(source_code: bytes, filename: str):
    tree = parser.parse(source_code)
    captures = TAG_QUERY.captures(tree.root_node)

    tags = []
    for node, capture_name in captures:
        kind = "def" if ".def" in capture_name else "ref"
        tags.append({
            "name": node.text.decode(),
            "kind": kind,
            "type": capture_name.split(".")[0],
            "file": filename,
            "line": node.start_point[0] + 1,
            "col": node.start_point[1],
        })
    return tags
```

---

## Performance Considerations

### Parsing Speed

Tree-sitter parsing is remarkably fast:

| Codebase Size | Files | Parse Time | Memory |
|---|---|---|---|
| Small (< 100 files) | ~50 | < 100ms | ~10MB |
| Medium (100-1000 files) | ~500 | < 1s | ~50MB |
| Large (1000-10000 files) | ~5000 | 2-5s | ~200MB |
| Very large (10000+ files) | ~20000 | 10-30s | ~1GB |

For CLI agents, parsing the entire codebase at startup is feasible for all but the very largest repositories.

### Incremental Updates

Tree-sitter's incremental parsing API allows re-parsing only changed regions:

```python
# Initial parse
tree = parser.parse(source_bytes)

# After editing (change "old_text" to "new_text" starting at byte 100)
tree.edit(
    start_byte=100,
    old_end_byte=108,
    new_end_byte=108,
    start_point=(5, 10),
    old_end_point=(5, 18),
    new_end_point=(5, 18),
)
new_tree = parser.parse(new_source_bytes, tree)
# Only re-parses the affected region
```

This is particularly valuable for agents making iterative edits — after each edit, the AST can be updated in microseconds rather than milliseconds.

---

## Current Limitations and Future Directions

### Limitations

1. **Macro expansion**: Tree-sitter parses the surface syntax. Macros (Rust's `macro_rules!`, C's `#define`, Lisp's `defmacro`) generate code that tree-sitter cannot see.

2. **Dynamic languages**: In Python and JavaScript, runtime behavior (monkey patching, dynamic imports, eval) cannot be captured by static analysis.

3. **Cross-language boundaries**: FFI calls, WASM imports, and polyglot projects create analysis gaps at language boundaries.

4. **Generated code**: Protocol buffers, GraphQL codegen, and ORM-generated models are invisible to static analysis until generated.

### Future Directions

1. **Hybrid analysis**: Combining tree-sitter's speed with LSP's accuracy — use tree-sitter for broad indexing and LSP for precise queries on hot paths.

2. **LLM-assisted analysis**: Using the LLM itself to interpret complex patterns that static analysis misses (e.g., "this decorator makes this class a FastAPI router").

3. **Incremental semantic analysis**: Building type inference and data flow analysis that updates incrementally as the agent edits code.

4. **Cross-repository analysis**: Understanding how the current codebase relates to its dependencies — what APIs are available, what conventions they follow.

---

## Key Takeaways

1. **Tree-sitter is the foundation.** Every agent doing static analysis uses tree-sitter. Its multi-language support, error tolerance, and speed make it the only practical choice for coding agents.

2. **AST-level understanding is sufficient for most tasks.** Agents don't need full semantic analysis for the majority of coding tasks. Knowing what functions exist, where they're defined, and what calls them covers 80% of code understanding needs.

3. **The gap between structural and semantic analysis is the frontier.** Agents currently live at the AST level. Moving to semantic understanding (types, data flow, effects) would dramatically improve edit quality.

4. **Aider's repo map is the gold standard.** Its combination of tree-sitter tag extraction, graph construction, and PageRank ranking is the most sophisticated static analysis pipeline in any CLI coding agent.

5. **Static analysis enables other capabilities.** Indexing, search, dependency graphs, and project detection all build on the foundation of static analysis. Improving the analysis layer improves everything above it.