# Context Management: Tools & Projects Ecosystem

## 1. Introduction

The context management tools ecosystem spans five distinct layers, each addressing
a different aspect of the fundamental challenge: making the most of finite context windows.

**The five layers:**

1. **Tokenization & Counting** — Know exactly how much space you have
2. **Code Intelligence & Parsing** — Understand code structure, not just text
3. **Memory & Long-Context Systems** — Manage what stays in and what gets evicted
4. **Code RAG & Retrieval** — Find and inject the right context on demand
5. **Persistence & Memory Stores** — Remember across sessions and conversations

These are the building blocks that coding agents compose into their context management
strategies. No single tool solves the problem — agents combine tools across layers.
Aider uses tiktoken + tree-sitter + SQLite. Cursor uses embeddings + tree-sitter +
vector stores. OpenHands chains condensers with retrieval. Understanding these tools
individually is essential to understanding how agents build their context pipelines.

---

## 2. Tokenization & Counting Tools

Accurate token counting is the foundation. Every context management decision —
what to include, what to evict, when to summarize — depends on knowing how many
tokens something costs. Off-by-20% errors compound into either wasted capacity
or context overflow crashes.

### tiktoken

- **URL:** https://github.com/openai/tiktoken
- **License:** MIT
- **Language:** Python with Rust core (via PyO3)
- **Stars:** 12k+ (as of 2025)

OpenAI's fast BPE tokenizer. The reference implementation for counting tokens
against OpenAI models. It is 3–6x faster than alternatives like `transformers`
tokenizers for the same encodings.

**Encodings:**

| Encoding | Models | Vocab Size |
|---|---|---|
| `cl100k_base` | GPT-4, GPT-3.5-turbo, text-embedding-ada-002 | 100,256 |
| `o200k_base` | GPT-4o, o1, o3, o4-mini | 200,019 |
| `p50k_base` | text-davinci-002/003, Codex | 50,281 |

**How agents use it:**
- Aider uses tiktoken for exact token counting of repo-maps and chat messages
- Many agents use it for budget management: "do I have room for this file?"
- Cost estimation before API calls

**Key pattern:** exact counting for short text, sampling for long text.

```python
import tiktoken

# Get encoding for a specific model
enc = tiktoken.encoding_for_model("gpt-4o")

# Or by encoding name directly
enc = tiktoken.get_encoding("cl100k_base")

tokens = enc.encode("Hello, world!")
print(len(tokens))  # 4
print(tokens)        # [9906, 11, 1917, 0]

# Decode back
text = enc.decode(tokens)
print(text)          # "Hello, world!"

# Count tokens for a code file
def count_file_tokens(path: str, encoding: str = "cl100k_base") -> int:
    enc = tiktoken.get_encoding(encoding)
    with open(path) as f:
        return len(enc.encode(f.read()))
```

**Performance note:** tiktoken uses Rust under the hood, making it fast enough
to count tokens on every keystroke in editor integrations. For very large files
(100K+ lines), agents sample a subset and extrapolate.

### Hugging Face tokenizers

- **URL:** https://github.com/huggingface/tokenizers
- **License:** Apache 2.0
- **Language:** Rust core, bindings for Python, Node.js, Ruby

The universal tokenizer library. Supports every major tokenization algorithm:
BPE, WordPiece, Unigram, and SentencePiece. Works with any model on Hugging Face
Hub, not just OpenAI models.

**Key differentiator:** alignment tracking — map tokens back to exact source spans.
This is critical for code agents that need to know which bytes correspond to which
tokens (e.g., for highlighting, error localization, or partial context extraction).

```python
from tokenizers import Tokenizer

# Load any model's tokenizer from HuggingFace Hub
tokenizer = Tokenizer.from_pretrained("gpt2")
output = tokenizer.encode("def hello():")

print(output.tokens)    # ['def', 'Ġhello', '():']
print(output.ids)       # [4299, 23748, 33529]
print(output.offsets)   # [(0, 3), (3, 9), (9, 12)]

# Alignment: token 1 ('Ġhello') maps to source bytes 3-9
# This enables precise code-span-to-token mapping

# Batch encoding for efficiency
outputs = tokenizer.encode_batch([
    "def foo(): pass",
    "class Bar: ...",
    "import os"
])
total_tokens = sum(len(o.ids) for o in outputs)
```

**How agents use it:**
- Universal tokenizer when supporting multiple model providers (OpenAI, Anthropic, local)
- Alignment tracking for precise code mapping (which function costs how many tokens)
- Batch encoding when processing entire repositories

### gpt-tokenizer

- **URL:** https://github.com/niieani/gpt-tokenizer
- **License:** MIT
- **Language:** TypeScript, browser-ready (no WASM/native dependencies)

The go-to tokenizer for JavaScript/TypeScript environments. Runs in browsers,
Node.js, Deno, and edge runtimes. Critical for web-based coding tools.

**Key features:**
- `isWithinTokenLimit(text, limit)` — early-exit check without full encoding
- `estimateCost(text, model)` — direct cost estimation
- Streaming encode/decode via async iterators

```typescript
import { encode, isWithinTokenLimit } from 'gpt-tokenizer';

// Quick count
const tokens = encode("Hello, world!");
console.log(tokens.length); // 4

// Early-exit: returns false immediately if limit exceeded
// Does NOT encode the entire string — stops as soon as limit is hit
const fits = isWithinTokenLimit("...very long text...", 4096);
if (!fits) {
  console.log("Text exceeds context window");
}
```

**Key pattern:** client-side token counting in web UIs. Cursor, Continue, and
similar tools use this pattern to show token counts in real-time without
round-tripping to a server.

### llama-tokenizer-js

- **URL:** https://github.com/belladoreai/llama-tokenizer-js
- **License:** MIT
- **Language:** JavaScript, pure client-side

Client-side tokenization for LLaMA/Llama family models. Runs in ~1ms for
typical inputs.

**Critical insight:** LLaMA and OpenAI token counts differ by ~20% for the same
text. Code that is 1,000 tokens in cl100k_base might be 1,200 tokens in LLaMA's
tokenizer. Agents supporting multiple providers must use the correct tokenizer
or risk overflow/underutilization.

---

## 3. Code Intelligence & Parsing Tools

Raw text tokenization tells you *how much* something costs. Code intelligence
tells you *what* to include. These tools understand code structure — functions,
classes, imports, call graphs — enabling agents to make informed decisions about
which pieces of context matter most.

### tree-sitter

- **URL:** https://github.com/tree-sitter/tree-sitter
- **License:** MIT
- **Language:** C core, bindings for Python, Rust, JavaScript, Go, Java, Swift, and more
- **Stars:** 18k+ (as of 2025)

The foundational parsing library for the coding agent ecosystem. Provides
incremental, error-tolerant parsing for 100+ programming languages.

**Core properties:**
- **General:** one API for any language via grammar plugins
- **Fast:** parses on every keystroke (sub-millisecond for incremental updates)
- **Robust:** produces useful ASTs even with syntax errors (critical for in-progress code)
- **Dependency-free:** no external toolchains required

**Language support:** C, C++, C#, Python, JavaScript, TypeScript, Java, Go, Rust,
Ruby, PHP, Swift, Kotlin, Scala, Haskell, OCaml, Elixir, HTML, CSS, JSON, YAML,
TOML, Markdown, Bash, SQL, and 80+ more.

**How agents use it:**

1. **Aider:** builds repo-map by extracting all definitions and references, then
   constructs a dependency graph ranked by PageRank. This map tells the LLM which
   files and symbols exist without including full source code.

2. **Cursor:** code intelligence for navigation, symbol extraction, and
   understanding code structure for context assembly.

3. **GitHub Copilot:** code understanding for suggestion generation, using
   tree-sitter to parse surrounding code and identify relevant context.

**Key patterns:**

```python
import tree_sitter_python as tspython
from tree_sitter import Language, Parser

# Initialize parser
PY_LANGUAGE = Language(tspython.language())
parser = Parser(PY_LANGUAGE)

source = b"""
class UserAuth:
    def login(self, username, password):
        token = self.generate_jwt(username)
        return token

    def logout(self, token):
        self.revoke(token)
"""

tree = parser.parse(source)

# Extract all function definitions
def extract_functions(node):
    if node.type == "function_definition":
        name_node = node.child_by_field_name("name")
        yield name_node.text.decode(), node.start_point, node.end_point
    for child in node.children:
        yield from extract_functions(child)

for name, start, end in extract_functions(tree.root_node):
    print(f"{name}: lines {start[0]+1}-{end[0]+1}")
# login: lines 3-5
# logout: lines 7-8
```

### ast-grep

- **URL:** https://github.com/ast-grep/ast-grep
- **License:** MIT
- **Language:** Rust, CLI tool with WASM playground

Structural code search and replace built on top of tree-sitter. Uses pattern
syntax with `$MATCH` wildcards instead of regex, so searches are AST-aware
rather than text-based.

**Why it matters for context management:** agents can find structurally relevant
code (all callers of a function, all implementations of an interface) rather
than relying on text grep, which produces false positives.

```bash
# Find all functions that call .save()
ast-grep -p '$FUNC.save()' --lang python

# Find all try/except blocks that catch generic Exception
ast-grep -p 'try: $$$ except Exception as $E: $$$' --lang python

# Find React components with useState
ast-grep -p 'const [$STATE, $SETTER] = useState($INIT)' --lang tsx

# Structural replace: rename a method across the codebase
ast-grep -p '$OBJ.old_method($$$ARGS)' -r '$OBJ.new_method($$$ARGS)' --lang python
```

### Aider repo-map (grep-ast)

- **URL:** https://github.com/Aider-AI/aider
- **License:** Apache 2.0

Aider's repo-map is the gold standard for structural code understanding in agents.
It provides a concise, ranked overview of a repository that fits within a token budget.

**Three-step pipeline:**
1. **tree-sitter parsing** — extract all definitions (classes, functions, methods)
   and references (identifiers, calls) from every file
2. **Dependency graph construction** — build a graph where nodes are files and
   edges represent cross-file references (file A references symbol defined in file B)
3. **PageRank scoring** — rank files by importance using the same algorithm Google
   uses for web pages. Files referenced by many others rank higher.

**Default budget:** 1,024 tokens for the map, expandable up to 8,192 tokens
for larger repos. The map is regenerated on each prompt, focused on files most
relevant to the current conversation.

**Output format:**
```
src/auth/jwt.py
│ class JWTManager
│   def generate_token(self, user_id, claims)
│   def validate_token(self, token)
│   def refresh_token(self, token)
src/auth/middleware.py
│ class AuthMiddleware
│   def __call__(self, request)
│   def _extract_token(self, request)
```

This gives the LLM a structural understanding of the codebase without
consuming tokens on implementation details.

### Sourcegraph / SCIP

- **URL:** https://sourcegraph.com
- **SCIP URL:** https://github.com/sourcegraph/scip
- **License:** Apache 2.0 (SCIP), proprietary (Sourcegraph platform)

Production-grade code intelligence at scale. Sourcegraph's architecture layers
three levels of code understanding:

1. **Precise indexers** (SCIP) — compiler-level accuracy for Go, Java, TypeScript, Python
2. **tree-sitter** — syntax-level intelligence for 30+ additional languages
3. **Text search** — fallback for everything else (Zoekt trigram index)

**How agents use it:** Cody (Sourcegraph's AI assistant) uses this layered
approach for cross-repository code navigation and context assembly. When you
ask "how does authentication work?", it can follow import chains across
multiple repositories.

---

## 4. Memory & Long-Context Systems

These tools manage what stays in the context window and what gets evicted,
summarized, or stored externally. They represent the "working memory" layer
of coding agents.

### Letta / MemGPT

- **URL:** https://github.com/letta-ai/letta
- **License:** Apache 2.0
- **Stars:** 13k+ (as of 2025)

The most radical approach to context management: treat the context window like
RAM in an operating system, and let the agent manage its own memory via tool calls.

**Memory hierarchy:**
- **core_memory** — always in-context, like CPU registers. Contains essential
  facts about the user and current task. Read/write via tools.
- **archival_memory** — vector-backed persistent storage, like a hard drive.
  Agent can insert and search. Unlimited capacity.
- **conversation_search** — recall buffer for past conversation turns.
  Agent can search by keyword or semantic similarity.

```python
# These are tools the AGENT calls (not the developer)
# The agent decides when to save/retrieve information

# Agent stores a preference it learned
core_memory_append(
    section="human",
    content="Prefers TypeScript over JavaScript for new projects"
)

# Agent archives detailed technical context
archival_memory_insert(
    content="The auth module uses JWT tokens stored in HttpOnly cookies. "
            "Refresh tokens are rotated on each use. See src/auth/jwt.ts."
)

# Agent retrieves context when needed
results = archival_memory_search(query="authentication approach", count=5)

# Agent manages its own context budget
core_memory_replace(
    section="task",
    old_content="Working on: initial project setup",
    new_content="Working on: implementing OAuth2 flow"
)
```

**Key insight:** Letta inverts the typical pattern. Instead of the *system*
deciding what to keep (sliding window, summarization), the *agent itself*
makes memory management decisions. This allows for much more intelligent
retention — the agent knows what's important.

### LangChain / LangGraph

- **URL:** https://github.com/langchain-ai/langchain
- **License:** MIT
- **Stars:** 100k+ (as of 2025)

LangChain provides a menu of composable memory strategies. LangGraph adds
checkpoint-based persistence for stateful agent workflows.

**Memory strategies (LangChain):**

| Strategy | Mechanism | Token Cost | Best For |
|---|---|---|---|
| `ConversationBufferMemory` | Keep everything | O(n) growing | Short conversations |
| `ConversationBufferWindowMemory` | Sliding window (k turns) | O(k) fixed | Chat interfaces |
| `ConversationSummaryMemory` | LLM summarization | O(1) compressed | Long conversations |
| `ConversationSummaryBufferMemory` | Recent turns + summary | O(k + summary) | Balanced approach |
| `ConversationTokenBufferMemory` | Token-based window | O(budget) fixed | Budget-constrained |
| `ConversationEntityMemory` | Entity tracking | O(entities) | Character/concept tracking |
| `VectorStoreRetrieverMemory` | RAG-based recall | O(k retrieved) | Large knowledge bases |

**LangGraph persistence:**

```python
from langgraph.checkpoint.sqlite import SqliteSaver

# Checkpoint-based persistence: every agent state is saved
checkpointer = SqliteSaver.from_conn_string("agent_state.db")

# Agent can resume from any checkpoint
# Enables: undo/redo, branching conversations, crash recovery
```

**Key pattern:** composable memory — chain multiple strategies. Use token buffer
for recent context + summary for older context + vector retrieval for long-term facts.

### OpenHands Condensers

- **URL:** https://github.com/All-Hands-AI/OpenHands
- **License:** MIT

The most comprehensive compaction system in the coding agent ecosystem.
OpenHands implements ~10 pluggable condenser strategies that can be chained
into pipelines.

**Available condensers:**
- `ObservationMaskingCondenser` — redact large outputs (command results, logs)
- `RecentEventsCondenser` — keep only the last N events
- `LLMSummarizingCondenser` — use an LLM to summarize older events
- `LLMAttentionCondenser` — LLM selects which events are most important
- `BrowserOutputCondenser` — specialized compaction for browser observations
- `AmortizedForgettingCondenser` — gradual decay of older events
- `LLMCondenser` — generic LLM-based compression
- Pipeline combinators — chain condensers sequentially

**Pipeline pattern example:**
```
Raw events
  → ObservationMaskingCondenser (redact verbose tool outputs)
  → AmortizedForgettingCondenser (decay old events)
  → LLMSummarizingCondenser (summarize when budget exceeded)
  → Final context (fits within token budget)
```

---

## 5. Code RAG & Retrieval

Retrieval-Augmented Generation for code. These tools find and inject relevant
code context on demand, rather than trying to keep everything in the window.

### LlamaIndex

- **URL:** https://github.com/run-llama/llama_index
- **License:** MIT
- **Stars:** 38k+ (as of 2025)

A data framework with first-class support for code. Its `CodeSplitter` does
AST-aware chunking — splitting code at function/class boundaries rather than
arbitrary line counts.

```python
from llama_index.core.node_parser import CodeSplitter

# AST-aware chunking: splits at function/class boundaries
splitter = CodeSplitter(
    language="python",
    chunk_lines=40,        # target chunk size
    chunk_lines_overlap=5, # overlap between chunks
    max_chars=1500         # hard limit
)

# Hierarchical indices: broad-to-narrow search
# Level 1: file-level summaries (cheap to search)
# Level 2: class-level chunks (medium granularity)
# Level 3: function-level chunks (precise retrieval)
```

**Key features:**
- **Hierarchical indices** — broad-to-narrow search (file → class → function)
- **Hybrid retrieval** — combine vector similarity with keyword search
- **Metadata filtering** — filter by language, file path, symbol type
- **Streaming ingestion** — process repos incrementally

### Voyage AI (voyage-code-3)

- **URL:** https://github.com/voyage-ai/voyageai-python
- **Documentation:** https://docs.voyageai.com

Code-specific embedding model that outperforms general-purpose embeddings
by 15–30% on code retrieval benchmarks.

**Specifications:**
- **Context:** 32,000 tokens (can embed entire files)
- **Dimensions:** 1024 (configurable: 256, 512, 1024)
- **Training:** fine-tuned on code with understanding of syntax, semantics, and documentation

```python
import voyageai

client = voyageai.Client()  # uses VOYAGE_API_KEY env var

# Embed code documents
doc_embeddings = client.embed(
    texts=["def hello(): pass", "class UserAuth: ..."],
    model="voyage-code-3",
    input_type="document"
)

# Embed a query (different input_type for asymmetric search)
query_embedding = client.embed(
    texts=["authentication handler"],
    model="voyage-code-3",
    input_type="query"
)

# Reranking for precision (second pass after vector search)
reranked = client.rerank(
    query="authentication handler",
    documents=candidate_chunks,
    model="rerank-2",
    top_k=5
)
```

**Why code-specific embeddings matter:** general embeddings treat code as text.
Code embeddings understand that `authenticate_user` and `verify_credentials`
are semantically related even though they share no words. They also understand
that `def foo(x):` and `function foo(x)` are the same concept in different languages.

### ChromaDB

- **URL:** https://github.com/chroma-core/chroma
- **License:** Apache 2.0
- **Stars:** 16k+ (as of 2025)

Embeddable vector database with a minimal 4-function API. Designed to be the
"SQLite of vector databases" — zero-config, runs in-process.

```python
import chromadb

client = chromadb.Client()  # in-memory; use PersistentClient for disk
collection = client.create_collection("codebase")

# Add code chunks with metadata
collection.add(
    documents=["def login(user, pw): ...", "class AuthMiddleware: ..."],
    ids=["auth-login", "auth-middleware"],
    metadatas=[
        {"lang": "python", "file": "src/auth.py", "type": "function"},
        {"lang": "python", "file": "src/middleware.py", "type": "class"}
    ]
)

# Query with metadata filtering
results = collection.query(
    query_texts=["how does authentication work?"],
    n_results=5,
    where={"lang": "python"}  # filter by language
)
```

**Key pattern for agents:** metadata filtering by file path, language, and
symbol type. When an agent knows it's working on Python auth code, it can
scope retrieval to `where={"lang": "python", "file": {"$contains": "auth"}}`.

### Sweep

- **URL:** https://github.com/sweepai/sweep
- **License:** BUSL-1.1

Multi-pass retrieval pipeline purpose-built for coding agents:

1. **Lexical search** — fast keyword matching (file names, identifiers)
2. **Embedding search** — semantic similarity via vector embeddings
3. **LLM re-ranking** — final pass where an LLM scores relevance

**Additional features:**
- AST-aware chunking (tree-sitter based)
- Incremental indexing on git changes (only re-index modified files)
- Context-aware chunk expansion (include surrounding code for coherence)

### Greptile

- **URL:** https://greptile.com
- **License:** Proprietary (API service), self-hosted option available
- **Compliance:** SOC 2 Type II

Codebase understanding as a service. Indexes entire GitHub/GitLab repositories
and provides a natural language API for querying code.

**Unique feature:** learns team standards and patterns from PR review comments.
Over time, it understands not just what the code does but how the team prefers
to write code.

### Cursor Codebase Indexing

- **URL:** https://docs.cursor.com/context/codebase-indexing

The most successful consumer implementation of code RAG. Cursor indexes your
entire codebase using embeddings and provides hybrid retrieval combining
vector similarity with structural code understanding.

**Features:**
- Automatic re-indexing on file changes
- `.cursorignore` for scoping (exclude node_modules, build artifacts, etc.)
- Hybrid search: embeddings + tree-sitter structural analysis
- Incremental updates (only re-embed changed files)

---

## 6. Persistence & Memory Stores

Long-term memory that survives across sessions. These tools let agents remember
user preferences, project context, and past decisions.

### SQLite Patterns

SQLite is the dominant choice for single-agent persistence. Zero-config,
single-file, ACID-compliant, and available everywhere.

**How agents use SQLite:**

| Agent | SQLite Usage |
|---|---|
| OpenCode | `sessions` and `messages` tables for conversation history |
| Aider | `diskcache` (SQLite-backed) for tree-sitter tag caching |
| LangGraph | `SqliteSaver` for agent state checkpointing |
| Claude Code | Session persistence and conversation history |

**Common schema pattern:**
```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSON
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES sessions(id),
    role TEXT NOT NULL,       -- 'user', 'assistant', 'system', 'tool'
    content TEXT NOT NULL,
    token_count INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_messages_session ON messages(session_id, created_at);
```

### Mem0

- **URL:** https://github.com/mem0ai/mem0
- **License:** Apache 2.0
- **Stars:** 24k+ (as of 2025)

Self-improving memory layer that automatically extracts facts from conversations,
deduplicates them, and resolves conflicts.

**Multi-level memory:**
- **User level** — preferences, expertise, communication style
- **Session level** — current task context, decisions made
- **Agent level** — learned patterns, tool preferences

**Storage:** hybrid vector + graph. Facts are stored as embeddings for semantic
search AND as graph nodes for relational queries ("what does this user's auth
module depend on?").

**Claimed benchmarks:** +26% accuracy vs OpenAI Memory, 91% faster retrieval,
90% fewer tokens consumed.

### Zep

- **URL:** https://github.com/getzep/zep
- **License:** Apache 2.0

Long-term memory with automatic summarization and entity extraction.

**Key features:**
- **Auto-summarization** — progressively summarizes older conversation turns
- **Named entity extraction** — identifies people, projects, technologies mentioned
- **Temporal awareness** — understands when facts were learned and can detect stale info
- **Native LangChain integration** — drop-in memory backend
- **Hybrid search** — vector similarity + metadata filtering

---

## 7. Retrieval Approach Comparison

| Approach | Speed | Semantic Accuracy | Setup Cost | Best For |
|---|---|---|---|---|
| Text grep | ★★★★★ | ★★☆☆☆ | None | Known identifiers, exact matches |
| tree-sitter structural | ★★★★☆ | ★★★☆☆ | Low | Symbol extraction, code navigation |
| Sparse retrieval (BM25) | ★★★★☆ | ★★★☆☆ | Low | Keyword-heavy queries |
| Dense retrieval (embeddings) | ★★★☆☆ | ★★★★☆ | Medium | Semantic code search |
| Code-specific embeddings | ★★★☆☆ | ★★★★★ | Medium | Cross-language code search |
| Hybrid (sparse + dense) | ★★★☆☆ | ★★★★★ | Medium | Production code search |
| LLM re-ranking | ★★☆☆☆ | ★★★★★ | High | Final-pass precision |
| Aider repo-map | ★★★★☆ | ★★★★☆ | Low | Repository overview, navigation |
| Multi-pass (Sweep-style) | ★★☆☆☆ | ★★★★★ | High | Complex code understanding |

**Key insight:** most production agents use layered retrieval. Cheap methods
(grep, tree-sitter) filter broadly, then expensive methods (embeddings, LLM
re-ranking) refine. This balances speed and accuracy.

---

## 8. Memory Store Comparison

| Store | Auto-Extract | Summarization | Multi-Level | Search | Integration |
|---|---|---|---|---|---|
| SQLite (raw) | ✗ | ✗ | Manual | SQL queries | Universal |
| Mem0 | ✓ | ✓ | ✓ (user/session/agent) | Vector + Graph | Python SDK |
| Zep | ✓ | ✓ | ✓ (user/session) | Vector + Metadata | LangChain native |
| Letta | ✓ (agent-driven) | ✓ (agent-driven) | ✓ (core/archival/recall) | Vector | REST API |
| ChromaDB | ✗ | ✗ | Manual | Vector + Metadata | Python/JS SDK |
| LangGraph Checkpoints | ✗ | ✗ | ✗ | By checkpoint ID | LangChain native |
| Redis (LangChain) | ✗ | ✗ | ✗ | Key-value + Vector | LangChain |

**Trade-offs:**
- **SQLite** is the pragmatic default — add complexity only when you need it.
- **Mem0** is best when you want automatic fact extraction without building it yourself.
- **Zep** is best when LangChain integration and temporal awareness matter.
- **Letta** is best when the agent should manage its own memory (most autonomous).
- **ChromaDB** is best for pure retrieval without memory management overhead.

---

## 9. How to Choose Tools

### Decision Framework

**Step 1: What's your model provider?**
- OpenAI only → tiktoken
- Multiple providers → Hugging Face tokenizers
- Browser/client-side → gpt-tokenizer or llama-tokenizer-js

**Step 2: How big is the codebase?**
- Small (< 50 files) → full file inclusion, no RAG needed
- Medium (50–500 files) → tree-sitter repo-map (Aider-style)
- Large (500+ files) → full RAG pipeline (embeddings + vector store)
- Monorepo / multi-repo → Sourcegraph or Greptile

**Step 3: What kind of memory do you need?**
- No persistence → in-memory buffers (LangChain ConversationBufferMemory)
- Session persistence → SQLite
- Cross-session learning → Mem0 or Zep
- Agent-managed memory → Letta

**Step 4: What's your budget?**
- Zero cost → tiktoken + tree-sitter + SQLite + ChromaDB (all open-source, local)
- Low cost → add Voyage AI embeddings ($0.06/1M tokens)
- Medium cost → add LLM-based summarization and re-ranking
- Full budget → Greptile or Sourcegraph for managed infrastructure

### Common Tool Combinations

**Minimal (Aider-style):**
tiktoken + tree-sitter + SQLite → accurate counting, structural maps, session persistence

**Mid-range (Continue-style):**
tiktoken + tree-sitter + ChromaDB + code embeddings → adds semantic retrieval

**Full pipeline (Cursor-style):**
tokenizer + tree-sitter + custom embeddings + vector store + LLM re-ranking → maximum retrieval quality

**Autonomous agent (Letta-style):**
tokenizer + Letta memory system + vector store → agent manages its own context

### Final Recommendations

1. **Always start with accurate token counting.** Everything else depends on it.
2. **tree-sitter is non-negotiable** for any serious code agent. The structural
   understanding it provides is too valuable to skip.
3. **SQLite is the right default** for persistence. Upgrade to Mem0/Zep only when
   you need automatic fact extraction or cross-session learning.
4. **Embeddings are worth it** for repos over ~100 files. The retrieval quality
   improvement justifies the cost and complexity.
5. **Layer your retrieval.** Cheap methods first (grep, tree-sitter), expensive
   methods second (embeddings, LLM re-ranking). Never start with the most
   expensive approach.
