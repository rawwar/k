---
title: Search Strategies for Code Understanding
status: complete
---

# Search Strategies

> How coding agents search codebases — from ripgrep text search to AST-based structural search to semantic search with embeddings — and the strategies agents use to formulate effective queries.

## Overview

Code search is the most frequently used code understanding technique across all 17 agents studied. Every agent provides at least one search tool, and most agents use search as their primary method of discovering relevant code. The critical differentiator between agents is not *what* search tool they use (most use ripgrep) but *how* they use it — the strategies for formulating queries, interpreting results, and deciding when to search more vs. start editing.

### Search Tool Adoption

| Search Tool | Agents Using It | Primary Use |
|---|---|---|
| **ripgrep (rg)** | Claude Code, Codex, OpenHands, Gemini CLI, OpenCode, ForgeCode, Goose, Ante, Droid, Pi Coding Agent | Text search across files |
| **grep** | mini-SWE-agent, Sage Agent | Basic text search |
| **glob / find** | Claude Code, Codex, OpenHands | File discovery by name pattern |
| **ast-grep** | Droid (optional) | AST-based structural search |
| **Semantic search** | Ante, ForgeCode (optional), Cursor, Cody | Embedding-based similarity |
| **JetBrains search** | Junie CLI | IDE-integrated structured search |

---

## Text Search with Ripgrep

### Why Ripgrep Dominates

Ripgrep has become the standard search tool for coding agents because it's fast, respects `.gitignore`, and handles binary files gracefully. Key features agents rely on:

```bash
# Basic pattern search
rg "function authenticate" --type ts

# Case-insensitive search
rg -i "userinput" --type py

# Regex search
rg "def (get|set|update)_user" --type py

# File listing (files containing matches)
rg -l "TODO" --type js

# JSON output (machine-parseable)
rg "class.*Service" --json --max-count 5

# Context lines (show surrounding code)
rg "handleError" -C 3 --type ts

# Search specific directory
rg "import.*from.*react" src/components/

# Exclude patterns
rg "password" --glob "!*.test.*" --glob "!node_modules"

# Count matches per file
rg "console.log" --count --type ts
```

### How Claude Code Wraps Ripgrep

Claude Code's `Grep` tool wraps ripgrep with specific defaults optimized for agent use:

```
Tool: Grep
Parameters:
  - pattern: The search pattern (regex supported)
  - path: Optional directory to search in (defaults to project root)
  - include: Optional glob pattern for file filtering

Behavior:
  - Searches recursively from the specified path
  - Respects .gitignore
  - Returns matching lines with file paths and line numbers
  - Limits results to prevent context overflow
  - Supports regex patterns
```

The tool's design reflects a key insight: agents need **file paths and line numbers** (to decide what to read next), not just matching text.

### How Codex Uses Ripgrep

Codex gives the model direct shell access, so it invokes ripgrep directly:

```bash
# Typical Codex search patterns
rg "function.*export" --type ts -l    # Find files with exported functions
rg "class.*extends.*Error" -l         # Find custom error classes
rg "test\(.*describe" -l              # Find test files
```

### Ripgrep Configuration for Agents

Optimal ripgrep configuration for agent use:

```bash
# .ripgreprc or passed as flags
--max-count=50        # Limit matches per file
--max-filesize=1M     # Skip huge files
--hidden=false        # Skip hidden files
--follow=false        # Don't follow symlinks
--no-heading          # Compact output
--line-number         # Always show line numbers
--color=never         # No ANSI colors (machine readable)
--smart-case          # Case-insensitive unless pattern has uppercase
```

---

## AST-Based Search

### The Problem with Text Search

Text search has fundamental limitations for code:

```python
# Searching for "process" with ripgrep returns ALL of these:
def process_data(input):       # ✓ Function definition (relevant)
    """Process the input."""   # ✗ Docstring (noise)
    # Don't process empty      # ✗ Comment (noise)
    processor = get_processor() # ✗ Different word (noise)
    return processor.process()  # ✓ Method call (maybe relevant)
```

AST-based search eliminates false positives by searching the code's structure, not its text.

### ast-grep

ast-grep uses code patterns to match AST nodes. Think of it as "grep but for syntax trees":

```bash
# Find all console.log calls
ast-grep --pattern 'console.log($$$ARGS)' --lang ts

# Find all functions named 'handle*'
ast-grep --pattern 'function handle$NAME($$$PARAMS) { $$$BODY }' --lang ts

# Find all async arrow functions
ast-grep --pattern 'const $NAME = async ($$$PARAMS) => $BODY' --lang ts

# Find all try-catch without error handling
ast-grep --pattern 'try { $$$BODY } catch($ERR) { }' --lang ts

# Find all React useState hooks
ast-grep --pattern 'const [$STATE, $SETTER] = useState($$$INIT)' --lang tsx
```

**Key advantage**: ast-grep patterns look like the code they match. No need to learn regex for structural queries.

**ast-grep YAML rules for linting:**
```yaml
id: no-console-log
message: Remove console.log before committing
severity: warning
language: typescript
rule:
  pattern: console.log($$$ARGS)
  not:
    inside:
      kind: if_statement
      has:
        pattern: process.env.DEBUG
```

### Semgrep

Semgrep (from Returntocorp / Semgrep Inc.) is a more mature AST-based search tool focused on security and code quality:

```yaml
# Semgrep rule for finding SQL injection
rules:
  - id: sql-injection
    patterns:
      - pattern: |
          cursor.execute(f"... {$USERINPUT} ...")
      - pattern-not: |
          cursor.execute(f"... {$CONST} ...")
          ...
          $CONST = "..."
    message: Possible SQL injection via f-string
    languages: [python]
    severity: ERROR
```

```bash
# Run Semgrep with a specific rule
semgrep --config r/python.django.security.injection

# Run with custom pattern
semgrep --pattern 'eval($X)' --lang python

# Search for dangerous functions
semgrep --pattern '$FUNC(request.GET[$KEY])' --lang python
```

### Comparison: Text Search vs. AST Search

| Feature | ripgrep (text) | ast-grep (AST) | Semgrep (AST+) |
|---|---|---|---|
| Speed | Fastest | Fast | Moderate |
| Precision | Low (many false positives) | High | Highest |
| Language awareness | None | Full syntax | Full syntax + semantics |
| Learning curve | Low (regex) | Medium (code patterns) | Medium-High (rules) |
| Cross-file analysis | No | Limited | Yes (with join mode) |
| Security rules | No | Basic | Extensive library |
| Agent adoption | Universal | Rare | Very rare |

---

## Semantic Search with Embeddings

### How Semantic Search Works

Semantic search converts code and queries into vector embeddings, then finds code whose vectors are closest to the query vector:

```python
# Conceptual semantic search pipeline
def semantic_search(query: str, index: VectorIndex, top_k: int = 10):
    # 1. Embed the natural language query
    query_embedding = embed_model.encode(query)

    # 2. Find nearest neighbors in the index
    results = index.search(query_embedding, top_k=top_k)

    # 3. Return matching code chunks with scores
    return [
        {"file": r.file, "chunk": r.content, "score": r.similarity}
        for r in results
    ]

# Example queries that work well with semantic search:
# "authentication middleware" → finds auth code even if named differently
# "database connection pooling" → finds connection management code
# "error handling in API routes" → finds try/catch in route handlers
```

### When Semantic Search Outperforms Text Search

| Query Type | Text Search (ripgrep) | Semantic Search |
|---|---|---|
| Exact symbol name | **Excellent** — `rg "createUser"` | Good |
| Conceptual query | **Poor** — what to search for? | **Excellent** — "user creation logic" |
| Synonym handling | **None** — must try each synonym | **Good** — "auth" ≈ "login" ≈ "authenticate" |
| Code vs. comments | **Cannot distinguish** | **Understands intent** |
| Cross-language | **Literal matching only** | **Conceptual matching** |

### When Text Search Outperforms Semantic Search

| Scenario | Text Search | Semantic Search |
|---|---|---|
| Known symbol name | **Instant, exact** | Slower, may miss |
| Error messages | **Exact string match** | May not embed well |
| Configuration keys | **Literal match** | Poor on config syntax |
| Regex patterns | **Full regex support** | No regex |

### Hybrid Search

The most effective approach combines both:

```python
def hybrid_search(query: str, codebase, top_k: int = 10):
    # Text search for exact matches
    text_results = ripgrep_search(query, codebase)

    # Semantic search for conceptual matches
    semantic_results = embedding_search(query, codebase)

    # Merge and deduplicate
    combined = merge_results(text_results, semantic_results)

    # Re-rank based on combined score
    return rerank(combined, query)[:top_k]
```

---

## File Path Pattern Matching

### Glob Patterns for File Discovery

Before searching file contents, agents often need to find files by name:

```bash
# Find all test files
find . -name "*.test.ts" -o -name "*.spec.ts"
# Or with glob: **/*.test.ts

# Find configuration files
find . -name "*.config.*" -o -name ".*rc"

# Find entry points
find . -name "index.ts" -o -name "main.ts" -o -name "app.ts"
```

### Claude Code's ListFiles/Glob Tool

Claude Code provides a dedicated file listing tool:

```
Tool: ListFiles
Parameters:
  - path: Directory to list (recursive)
  - pattern: Optional glob pattern

Returns: File paths matching the pattern, with sizes and types
```

This tool is often the agent's first step: list files to understand project structure before doing content search.

### Intelligent File Filtering

Agents should skip files that are unlikely to be useful:

```python
# Files to always skip
SKIP_PATTERNS = [
    "node_modules/",
    ".git/",
    "dist/", "build/", "out/",
    "*.min.js", "*.min.css",
    "*.map",
    "package-lock.json", "yarn.lock", "pnpm-lock.yaml",
    "*.png", "*.jpg", "*.gif", "*.ico",
    "*.woff", "*.woff2", "*.ttf",
    ".env", ".env.*",
]

# Files to prioritize
PRIORITY_PATTERNS = [
    "README.md", "CONTRIBUTING.md",
    "package.json", "pyproject.toml", "Cargo.toml", "go.mod",
    "src/index.*", "src/main.*", "src/app.*",
    "*.config.*", ".*rc.js",
]
```

---

## Search Strategies: How Agents Decide What to Search

The most important aspect of code search isn't the tool — it's the strategy. How does an agent decide what to search for?

### Strategy 1: Keyword Extraction from Task

Extract key terms from the user's task and search for them:

```
User: "Fix the bug where users can't log in with email"

Agent reasoning:
  → Search 1: rg "login" --type ts -l           (find login-related files)
  → Search 2: rg "email.*auth" --type ts         (find email authentication)
  → Search 3: rg "class.*Auth" --type ts          (find auth classes)
```

**Strength**: Simple, fast, usually finds something relevant.
**Weakness**: Fails when the code uses different terminology than the task.

### Strategy 2: Structural Exploration

Start from known entry points and follow the code structure:

```
User: "Add rate limiting to the API"

Agent reasoning:
  → Search 1: find . -name "routes.*" -o -name "router.*"   (find route files)
  → Read route file, find middleware chain
  → Search 2: rg "middleware" -l                              (find existing middleware)
  → Read middleware files, understand pattern
  → Now knows where and how to add rate limiting
```

**Strength**: Builds understanding of the codebase, not just the search results.
**Weakness**: Slower, requires multiple search-read cycles.

### Strategy 3: Reference Chain Following

Find one relevant symbol, then follow its references:

```
User: "Refactor the UserService to use the new database client"

Agent reasoning:
  → Search 1: rg "class UserService" -l           (find the class)
  → Read UserService, find current database usage
  → Search 2: rg "UserService" -l                  (find all files using it)
  → Search 3: rg "new.*DatabaseClient" -l          (find the new client)
  → Now knows: what to change, where it's used, what the new API looks like
```

**Strength**: Thorough, minimizes risk of missing callers.
**Weakness**: Can explode for widely-used symbols.

### Strategy 4: Pattern-Based Discovery

Search for patterns rather than specific strings:

```
User: "Add logging to all API endpoints"

Agent reasoning:
  → Search 1: rg "app\.(get|post|put|delete)\(" -l   (find all route handlers)
  → Search 2: rg "router\.(get|post|put|delete)\(" -l (alternative pattern)
  → Search 3: rg "import.*logger" -l                   (find existing logging)
  → Now knows: all endpoints, existing logging pattern
```

**Strength**: Catches all instances of a pattern, not just one specific string.
**Weakness**: Requires knowing the pattern, which depends on framework knowledge.

### Strategy 5: Iterative Refinement

Start broad, narrow based on results:

```
User: "Fix the performance issue in the search feature"

Agent reasoning:
  → Search 1: rg "search" -l --count          (how many files mention search?)
  → 47 files — too many. Narrow.
  → Search 2: rg "search" src/api/ -l          (search in API layer)
  → 8 files — manageable
  → Read the 8 files, identify the slow path
  → Search 3: rg "database.query" src/api/search/  (find database calls)
  → Found the N+1 query causing the performance issue
```

**Strength**: Adapts to codebase size and structure.
**Weakness**: Takes more turns, consuming context tokens.

---

## Search Result Ranking

### The Ranking Problem

When a search returns 50 results, which ones should the agent look at first? Text search tools like ripgrep don't rank — they return results in file-path order. Agents must impose their own ranking.

### Ranking Signals

```python
def rank_search_results(results, query, context):
    for result in results:
        score = 0.0

        # Signal 1: File relevance (is this a source file or test/config?)
        if is_source_file(result.file):
            score += 2.0
        if is_test_file(result.file):
            score += 0.5  # Tests are less relevant for understanding

        # Signal 2: Match quality (exact match vs. partial)
        if exact_match(result.text, query):
            score += 3.0
        elif word_boundary_match(result.text, query):
            score += 2.0

        # Signal 3: Definition vs. reference
        if is_definition(result.text):  # "def foo", "class Foo", "function foo"
            score += 2.0
        elif is_import(result.text):     # "import foo", "from x import foo"
            score += 1.0

        # Signal 4: File freshness (recently modified files may be more relevant)
        recency = get_git_recency(result.file)
        score += recency * 0.5

        # Signal 5: File centrality (frequently imported files are more important)
        if result.file in high_centrality_files:
            score += 1.5

        result.score = score

    return sorted(results, key=lambda r: -r.score)
```

### Aider's Approach: Pre-Ranked via Repo Map

Aider sidesteps the ranking problem by using its repo map. Instead of searching and ranking results, it already knows which symbols are most important (via PageRank) and can guide the LLM to the right files:

```
LLM sees repo map → knows which files contain relevant symbols →
requests those specific files → no need to search at all
```

This is a fundamental architectural difference: **search-first** (Claude Code, Codex) vs. **index-first** (Aider).

---

## Context-Aware Search

### Using Conversation Context to Improve Search

Agents that track conversation state can improve search quality over time:

```python
class ContextAwareSearcher:
    def __init__(self):
        self.seen_files = set()       # Files already read in this conversation
        self.seen_symbols = set()     # Symbols already encountered
        self.failed_searches = []     # Searches that returned no useful results

    def search(self, query):
        # Avoid re-searching seen files
        results = ripgrep(query)
        results = [r for r in results if r.file not in self.seen_files]

        # Boost files that mention already-seen symbols
        for result in results:
            if any(sym in result.text for sym in self.seen_symbols):
                result.boost += 1.0

        # Learn from failed searches
        if not results and self.failed_searches:
            # Try alternative query formulations
            alternatives = self.generate_alternatives(query)
            for alt in alternatives:
                results = ripgrep(alt)
                if results:
                    break

        return results
```

### Multi-Step Search Patterns

Effective agents chain searches together:

```
Step 1: Find the file          rg "className" -l
Step 2: Find the function      rg "functionName" specific_file.ts
Step 3: Find callers           rg "functionName\(" -l
Step 4: Find related tests     rg "describe.*className" **/*.test.ts
Step 5: Find configuration     rg "className" **/*.config.* **/.*rc
```

---

## Search Performance Optimization

### Limiting Search Scope

```bash
# Scope to specific directories
rg "pattern" src/ lib/  # Only search source directories

# Scope to specific file types
rg "pattern" --type ts --type js  # Only TS/JS files

# Use .gitignore (ripgrep default)
rg "pattern"  # Automatically respects .gitignore

# Limit result count
rg "pattern" --max-count 10  # Max 10 matches per file
rg "pattern" -l | head -20   # Max 20 files
```

### Parallel Search

For agents needing multiple independent searches, run them in parallel:

```python
import asyncio

async def parallel_search(queries):
    tasks = [
        asyncio.create_subprocess_exec(
            "rg", query, "--json", "--max-count", "10",
            stdout=asyncio.subprocess.PIPE
        )
        for query in queries
    ]
    processes = await asyncio.gather(*[t for t in tasks])
    results = await asyncio.gather(*[p.communicate() for p in processes])
    return results
```

---

## Key Takeaways

1. **Ripgrep is the universal foundation.** Every agent uses text search, and ripgrep is the tool of choice. It's fast enough to be used on-demand without pre-indexing.

2. **Search strategy matters more than search tools.** The LLM's ability to formulate good queries, interpret results, and decide next steps is the real differentiator.

3. **AST-based search is underutilized.** ast-grep and semgrep provide much higher precision than text search but are rarely integrated into coding agents.

4. **Semantic search fills a real gap.** When the user describes what they want conceptually (not by exact symbol name), embedding-based search outperforms text search.

5. **The best agents combine multiple search strategies.** Keyword extraction for quick hits, structural exploration for understanding, reference following for completeness, iterative refinement for large codebases.

6. **Search ranking is an unsolved problem.** Most agents return search results in file-path order. Better ranking (by relevance, recency, centrality) would significantly improve agent efficiency.