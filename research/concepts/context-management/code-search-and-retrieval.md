# Code Search and Retrieval: RAG for Codebases

## 1. Introduction: Why RAG for Code

Large codebases contain millions of lines of code spread across thousands of files.
No LLM context window—not even at 200K tokens—can hold an entire production
codebase. The fundamental problem: an AI agent must find the RIGHT code to reason
about before it can do anything useful.

**Retrieval-Augmented Generation (RAG)** solves this by:
1. Pre-indexing the codebase into searchable representations
2. Retrieving the most relevant code snippets for a given query
3. Injecting those snippets into the LLM context alongside the prompt

This is fundamentally different from repo-map approaches (structural summaries like
tree-sitter outlines or dependency graphs). Repo-maps give you the SHAPE of the code;
RAG gives you the actual CONTENT. Both are complementary—structure tells you where
to look, retrieval gives you what's there.

**The retrieval quality directly determines agent effectiveness.** If retrieval misses
a critical function, the agent hallucinates. If retrieval returns too much irrelevant
code, the agent gets confused. The precision/recall balance is everything.

Key challenges unique to code retrieval (vs. document retrieval):
- **Multi-language**: a single repo may contain Python, TypeScript, SQL, YAML
- **Structural semantics**: indentation, nesting, scope matter
- **Cross-file dependencies**: understanding one function requires its imports
- **Identifier-heavy**: variable/function names carry critical semantic meaning
- **Rapidly changing**: code evolves with every commit

---

## 2. Vector Embeddings for Code

Embeddings convert code into dense vector representations where semantically similar
code maps to nearby points in vector space. The quality of embeddings determines the
ceiling of retrieval quality.

### Voyage AI voyage-code-3

The current state-of-the-art for code-specific embeddings. Trained on millions of
code + documentation pairs across dozens of programming languages.

**Key specifications:**
- Context length: 32,000 tokens
- Embedding dimensions: 1024 (default), also supports 256, 512, 2048
- Outperforms general-purpose embeddings by 15–30% on code retrieval benchmarks
- Supports `input_type` parameter for asymmetric retrieval

**Asymmetric retrieval** is critical for code search. The query ("how does auth work?")
looks nothing like the document (actual authentication code). Voyage handles this by
prepending different internal prompts for queries vs documents:

```python
import voyageai

client = voyageai.Client()  # uses VOYAGE_API_KEY env var

# Embed code snippets as documents
doc_result = client.embed(
    [
        "def calculate_tax(income, rate):\n    return income * rate",
        "class TaxCalculator:\n    def __init__(self, brackets):\n        self.brackets = brackets",
        "def validate_tax_id(tax_id: str) -> bool:\n    return len(tax_id) == 9 and tax_id.isdigit()",
    ],
    model="voyage-code-3",
    input_type="document",
)
# doc_result.embeddings is a list of 1024-dim vectors

# Embed a natural-language query
query_result = client.embed(
    ["how to compute tax on income"],
    model="voyage-code-3",
    input_type="query",
)
# query_result.embeddings[0] is a 1024-dim vector

# Compute cosine similarity to find best match
import numpy as np
similarities = [
    np.dot(query_result.embeddings[0], doc_emb)
    / (np.linalg.norm(query_result.embeddings[0]) * np.linalg.norm(doc_emb))
    for doc_emb in doc_result.embeddings
]
# similarities: [0.87, 0.72, 0.54] — calculate_tax is the best match
```

**Dimension reduction**: For large-scale deployments, use 256-dim embeddings to reduce
storage and speed up similarity search, at a modest quality cost (~5% degradation).

### OpenAI Embeddings

- **text-embedding-3-small**: 1536 dimensions, cheapest option
- **text-embedding-3-large**: 3072 dimensions, can truncate to 256/512/1024
- General-purpose, not code-optimized
- Native dimension truncation via `dimensions` parameter (Matryoshka embeddings)
- Significantly cheaper per token than Voyage but lower code retrieval quality

```python
from openai import OpenAI

client = OpenAI()
response = client.embeddings.create(
    input="def calculate_tax(income, rate): return income * rate",
    model="text-embedding-3-large",
    dimensions=1024,  # truncate from 3072 to 1024
)
embedding = response.data[0].embedding  # 1024-dim vector
```

### Code-Specific vs General-Purpose: Performance Comparison

| Metric | voyage-code-3 | text-embedding-3-large | text-embedding-3-small |
|---|---|---|---|
| Code-to-code retrieval (MRR@10) | 0.82 | 0.68 | 0.61 |
| NL-to-code retrieval (MRR@10) | 0.78 | 0.65 | 0.58 |
| Cross-language retrieval | Strong | Weak | Weak |
| Dimension (default) | 1024 | 3072 | 1536 |
| Context window | 32K tokens | 8K tokens | 8K tokens |
| Cost per 1M tokens | ~$0.06 | ~$0.13 | ~$0.02 |

**What code embeddings understand that general embeddings miss:**
- Function signatures and their semantic intent
- Import patterns and dependency relationships
- API usage idioms (e.g., `with open(...) as f` pattern)
- Programming language-specific constructs
- Variable naming conventions as semantic signals
- Cross-language equivalence (Python `dict` ↔ TypeScript `Record`)

---

## 3. Chunking Strategies for Code

How you split code into chunks for embedding is arguably MORE important than which
embedding model you use. Bad chunking destroys semantic coherence.

### Function-Level Chunking

Split code at function/method boundaries. Each chunk is one complete function.

```python
import tree_sitter_python as tspython
from tree_sitter import Language, Parser

PY_LANGUAGE = Language(tspython.language())
parser = Parser(PY_LANGUAGE)

def extract_functions(source_code: str) -> list[dict]:
    tree = parser.parse(bytes(source_code, "utf-8"))
    functions = []
    for node in tree.root_node.children:
        if node.type in ("function_definition", "class_definition"):
            functions.append({
                "text": source_code[node.start_byte:node.end_byte],
                "name": node.children[1].text.decode(),
                "start_line": node.start_point[0],
                "end_line": node.end_point[0],
            })
    return functions
```

**Pros:** Each chunk is a complete semantic unit; natural retrieval granularity.
**Cons:** Long functions exceed chunk limits; module-level imports/constants lost;
inner functions may lack outer context.

**Enhancement:** Prepend the file path and import block to each function chunk:
```
# File: src/services/tax_calculator.py
# Imports: from decimal import Decimal; from .models import TaxBracket
def calculate_tax(income: Decimal, brackets: list[TaxBracket]) -> Decimal:
    ...
```

### File-Level Chunking

Embed entire files as single chunks.

**Pros:** Full context within each chunk—imports, constants, relationships all present.
**Cons:** Many files exceed embedding model context limits; retrieval returns entire
files when only one function is relevant; wastes context window budget.

**When to use:** Small utility files, configuration files, type definition files.

### Semantic Chunking (AST-Aware)

The gold standard. Uses the Abstract Syntax Tree to split code at meaningful boundaries.

```python
from llama_index.core.node_parser import CodeSplitter

splitter = CodeSplitter(
    language="python",
    chunk_lines=40,        # target chunk size in lines
    chunk_lines_overlap=15, # overlap between chunks
    max_chars=1500,        # hard limit on characters per chunk
)
nodes = splitter.get_nodes_from_documents(documents)
# Each node respects function/class boundaries
# Never splits mid-function unless the function exceeds max_chars
```

**How it works:**
1. Parse code into AST using tree-sitter
2. Walk the AST to find natural split points (top-level definitions)
3. Group adjacent small definitions together up to chunk_lines
4. Split oversized definitions at nested boundaries (methods within classes)
5. Add overlap so context isn't lost at boundaries

**Language support:** Python, JavaScript, TypeScript, Go, Rust, Java, C, C++, Ruby,
and 30+ other languages via tree-sitter grammars.

### Sliding Window with Overlap

Fixed-size chunks with configurable overlap. Language-agnostic.

```
Chunk 1: lines 1-50
Chunk 2: lines 40-90    (10-line overlap)
Chunk 3: lines 80-130   (10-line overlap)
```

**Pros:** Simple, no parsing required, works for any file type.
**Cons:** Splits mid-function, mid-statement, mid-string. The overlap helps but
doesn't guarantee coherent chunks.

**Use case:** When you need to index non-code files (documentation, configs) or
when tree-sitter support isn't available for an exotic language.

### Hierarchical Chunking

Multiple levels of granularity, enabling broad-to-narrow search.

```
Level 0: Repository summary
Level 1: File summaries (docstrings + signatures)
Level 2: Class-level chunks (full class definitions)
Level 3: Method-level chunks (individual methods)
```

**Search strategy:**
1. Query at Level 1 to find relevant files
2. Query at Level 2 within those files to find relevant classes
3. Query at Level 3 to find the exact methods

This dramatically reduces the search space at each level. LlamaIndex supports this
via `HierarchicalNodeParser` and `AutoMergingRetriever`.

### Chunking Strategy Comparison

| Strategy | Code-Aware | Granularity | Complexity | Best For |
|---|---|---|---|---|
| Function-level | Yes | Medium | Medium | Most codebases |
| File-level | No | Coarse | Low | Small files, configs |
| AST-aware | Yes | Adaptive | High | Production systems |
| Sliding window | No | Fixed | Low | Non-code, fallback |
| Hierarchical | Yes | Multi-level | High | Large codebases |

---

## 4. Retrieval Approaches

### Keyword/BM25 Search

Traditional information retrieval using term frequency–inverse document frequency.

**How it works:** Tokenize query and documents into terms. Score documents by how
many query terms they contain, weighted by term rarity (IDF) and frequency (TF).

**Strengths for code:**
- Exact identifier matching: searching `calculateTax` finds it immediately
- Error messages: stack traces contain exact strings
- Fast: sub-second on million-line codebases
- No pre-computation beyond tokenization

**Weaknesses:**
- No semantic understanding: "compute tax" won't find `calculateTax`
- No cross-language retrieval
- Sensitive to naming conventions

**Implementation options:**
- ripgrep (`rg`): blazing fast, regex-capable, used by most editors
- ElasticSearch/OpenSearch: full BM25 with analyzers, scalable
- Tantivy (Rust): embeddable full-text search engine

### Semantic Search (Vector)

Embed everything, then find nearest neighbors.

```python
import chromadb

client = chromadb.PersistentClient(path="./chroma_db")
collection = client.get_or_create_collection(
    name="codebase",
    metadata={"hnsw:space": "cosine"},  # cosine similarity
)

# Index code chunks
collection.add(
    documents=[
        "def calculate_tax(income, rate):\n    return income * rate",
        "def validate_email(email):\n    return '@' in email and '.' in email",
        "class UserService:\n    def authenticate(self, username, password): ...",
    ],
    metadatas=[
        {"file": "tax.py", "language": "python", "type": "function"},
        {"file": "validators.py", "language": "python", "type": "function"},
        {"file": "services.py", "language": "python", "type": "class"},
    ],
    ids=["chunk-1", "chunk-2", "chunk-3"],
)

# Query with natural language
results = collection.query(
    query_texts=["how does the system verify user identity"],
    n_results=3,
    where={"language": "python"},  # metadata filter
)
# Returns: UserService.authenticate (highest similarity)
```

**Strengths:** Semantic understanding, natural language queries, cross-language.
**Weaknesses:** Can miss exact identifiers; embedding quality is the bottleneck;
requires pre-computation of all embeddings.

### Hybrid Search

Combine keyword and semantic search for the best of both worlds.

```
Query: "calculateTax function"

BM25 results:          Semantic results:
1. tax.py:calculate_tax    1. tax.py:calculate_tax
2. tax_test.py:test_calc   2. billing.py:compute_amount
3. README.md (mentions)    3. tax.py:TaxBracket

Hybrid (RRF fusion):
1. tax.py:calculate_tax     (rank 1 in both → top result)
2. tax_test.py:test_calc    (rank 2 in BM25)
3. billing.py:compute_amount (rank 2 in semantic)
```

**Reciprocal Rank Fusion (RRF)** merges ranked lists:
```
RRF_score(doc) = Σ 1 / (k + rank_i(doc))
```
where `k` is a constant (typically 60) and `rank_i` is the document's rank in list `i`.

Most production code search systems use hybrid retrieval. It handles both the "I know
the exact function name" and "find code that does X" cases gracefully.

### Multi-Pass Retrieval

The highest-quality approach, pioneered by Sweep for code search.

```
Pass 1: Lexical Search (BM25/ripgrep)
  - Fast, broad sweep over entire codebase
  - Returns ~100 candidates
  - Cost: milliseconds

Pass 2: Embedding Search (vector similarity)
  - Semantic matching over Pass 1 candidates
  - Narrows to ~20 candidates
  - Cost: seconds (embedding the query + similarity computation)

Pass 3: LLM Re-ranking
  - Send top candidates to an LLM
  - "Which of these code snippets are most relevant to: {query}?"
  - Returns final ~5 results, precisely ranked
  - Cost: LLM API call (most expensive step)
```

**Why this works:** Each pass uses a more sophisticated (and expensive) model.
Lexical search is cheap enough to scan everything. Embedding search adds semantic
understanding. LLM re-ranking brings deep reasoning about relevance.

**The re-ranking step is critical.** Studies show that LLM re-ranking improves
precision@5 by 20–40% over embedding-only retrieval. The cost is an additional
LLM call, but the candidate set is small enough (10–20 items) that this is affordable.

---

## 5. Platform Deep-Dives

### Sweep

Open-source AI code search and modification tool. One of the first to implement
multi-pass retrieval specifically for codebases.

**Architecture:**
1. **Indexing**: AST-aware chunking → embeddings stored in vector DB
2. **Incremental updates**: Watches git changes, re-indexes only modified files
3. **Retrieval pipeline**: lexical → embedding → LLM rerank (3-pass)
4. **Context assembly**: Retrieved chunks + repo-map + file metadata → prompt

**Key design decisions:**
- Uses tree-sitter for language-aware chunking
- Stores chunk metadata (file path, function name, class, line numbers)
- Maintains a "relevance cache" to avoid re-ranking identical queries
- License: BUSL-1.1 (Business Source License)

### Cursor Codebase Indexing

The most successful consumer-facing implementation of code RAG.

**How it works:**
1. User opens a project → Cursor scans all files
2. Files are chunked and embedded (runs locally or via Cursor servers)
3. Embeddings stored in local index
4. On query, hybrid retrieval combines vector search + structural understanding
5. `.cursorignore` lets users scope what gets indexed

**Incremental re-indexing:** File watcher detects changes → only re-embeds modified
chunks. This keeps the index fresh without full rebuilds.

**What makes it effective:**
- Tight integration with the editor (knows which files are open, recent edits)
- Combines retrieval with structural context (imports, references)
- Fast: queries return results in <500ms even for large repos
- Transparent: users can see which files were retrieved via "@codebase"

### Greptile

Codebase understanding as a managed service. Takes a different approach: instead of
running locally, it indexes your GitHub/GitLab repos server-side.

**Capabilities:**
- Indexes entire repositories including PR history and review comments
- Learns team coding conventions and patterns from merged PRs
- API-based: `POST /query` with natural language, returns relevant code + explanation
- SOC 2 Type II compliant; self-hosted option for enterprise

**Reported metrics:**
- Median time-to-merge: 20h → 1.8h (10x improvement)
- Code review accuracy: catches 60% of issues human reviewers catch
- Onboarding time reduction: new developers productive 3x faster

**API usage:**
```bash
curl -X POST https://api.greptile.com/v2/query \
  -H "Authorization: Bearer $GREPTILE_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [{"role": "user", "content": "How does authentication work?"}],
    "repositories": [{"remote": "github", "repository": "org/repo", "branch": "main"}]
  }'
```

### LlamaIndex for Code

Python data framework providing building blocks for code RAG pipelines.

```python
from llama_index.core import VectorStoreIndex, Settings
from llama_index.core.node_parser import CodeSplitter
from llama_index.readers.file import SimpleDirectoryReader
from llama_index.embeddings.voyageai import VoyageEmbedding

# Configure code-optimized embeddings
Settings.embed_model = VoyageEmbedding(
    model_name="voyage-code-3",
    voyage_api_key="...",
)

# Load and chunk code files
documents = SimpleDirectoryReader(
    input_dir="./src",
    recursive=True,
    required_exts=[".py", ".ts", ".js"],
).load_data()

splitter = CodeSplitter(language="python", chunk_lines=40, chunk_lines_overlap=15)
nodes = splitter.get_nodes_from_documents(documents)

# Build searchable index
index = VectorStoreIndex(nodes)

# Query
query_engine = index.as_query_engine(similarity_top_k=10)
response = query_engine.query("How does the authentication module work?")
print(response)
```

**Strengths:** Composable building blocks; supports hybrid retrieval via
`QueryFusionRetriever`; hierarchical indices for multi-level search;
MIT license; active ecosystem of integrations.

---

## 6. Vector Databases for Code

### ChromaDB

Embeddable, Python-native vector database with a 4-function API.

```python
import chromadb

# Persistent storage (survives restarts)
client = chromadb.PersistentClient(path="./codebase_index")

collection = client.get_or_create_collection(
    name="project_code",
    metadata={"hnsw:space": "cosine"},
)

# Add chunks with rich metadata
collection.add(
    documents=chunks,
    metadatas=[
        {"file": "auth.py", "language": "python", "type": "function", "name": "login"},
        {"file": "auth.py", "language": "python", "type": "function", "name": "logout"},
    ],
    ids=["auth-login", "auth-logout"],
)

# Query with metadata filtering
results = collection.query(
    query_texts=["user session management"],
    n_results=5,
    where={"language": "python"},
    where_document={"$contains": "session"},  # keyword filter on content
)
```

**Best for:** Prototyping, single-developer tools, small-to-medium codebases (<100K files).
Apache 2.0 license. Handles embedding automatically if you configure an embedding function.

### Pinecone

Managed vector database service. No infrastructure to maintain.

**Key features:**
- Serverless and pod-based deployment options
- Namespace isolation (separate indices per repo/branch)
- Metadata filtering with complex boolean expressions
- Scales to billions of vectors
- 99.95% uptime SLA

**Best for:** Production multi-tenant systems, large organizations, teams that don't
want to manage infrastructure.

### Qdrant

High-performance vector database with flexible deployment.

**Key features:**
- Written in Rust for performance
- Rich filtering with payload indices
- Supports sparse vectors (for hybrid BM25 + dense retrieval)
- Docker, Kubernetes, or Qdrant Cloud deployment
- Apache 2.0 license

### Vector Database Comparison

| Database | Type | Embedding | Scale | Hybrid Search | License | Best For |
|---|---|---|---|---|---|---|
| ChromaDB | Embedded | Built-in | Small–Medium | Limited | Apache 2.0 | Prototyping, single-agent |
| Pinecone | Managed | External | Large | Yes | Proprietary | Production, multi-tenant |
| Qdrant | Self-hosted/Cloud | External | Medium–Large | Native sparse+dense | Apache 2.0 | Flexible deployment |
| Weaviate | Self-hosted/Cloud | Built-in | Medium–Large | Native BM25+vector | BSD-3 | Hybrid search focus |
| pgvector | PostgreSQL ext. | External | Medium | Via SQL | PostgreSQL | Existing Postgres infra |

---

## 7. Retrieval Approach Comparison

| Approach | Speed | Semantic Accuracy | Setup Complexity | Cost | Best For |
|---|---|---|---|---|---|
| Keyword/BM25 | <100ms | Low | Minimal | Free | Known identifiers, grep-like |
| Embedding (general) | 200–500ms | Medium | Moderate | $ | Natural-language queries |
| Embedding (code-specific) | 200–500ms | High | Moderate | $$ | Code search, cross-language |
| Hybrid (keyword + vector) | 300–700ms | High | High | $$ | Production systems |
| Multi-pass (lexical→vector→LLM) | 1–5s | Highest | Highest | $$$ | Complex retrieval tasks |

**Recommendation by use case:**
- **IDE integration** (Cursor-like): Hybrid search. Users expect both exact and semantic.
- **AI coding agent**: Multi-pass. Agent quality depends on retrieval quality; worth the cost.
- **Code review bot**: Hybrid with metadata filtering. Scope to changed files + dependencies.
- **Documentation Q&A**: Embedding-only with code-specific model. Queries are natural language.

---

## 8. Practical Considerations

### Index Update Strategy

**Full rebuild:** Re-embed the entire codebase. Simple but expensive.
- When: initial setup, embedding model change, major refactor
- Cost: proportional to codebase size (minutes to hours for large repos)

**Incremental indexing:** Watch for git changes, re-embed only modified files.
- Track via git diff: `git diff --name-only HEAD~1` → list of changed files
- Delete old chunks for changed files, insert new chunks
- Most production systems use this approach
- Complexity: handling file renames, deletions, moved code

**Hybrid approach:** Incremental for day-to-day; scheduled full rebuilds weekly.

### Embedding Cost Analysis

Embedding is a one-time cost per chunk. Querying is cheap (vector similarity).

```
Example: 100,000 lines of Python code
  → ~2,500 function-level chunks
  → ~1.5M tokens to embed

Voyage AI voyage-code-3: ~$0.06 per 1M tokens → $0.09 total
OpenAI text-embedding-3-small: ~$0.02 per 1M tokens → $0.03 total

Re-indexing daily (assume 5% changes): ~$0.005/day with Voyage
```

The cost of embedding is negligible compared to LLM inference costs. Don't optimize
for embedding cost—optimize for retrieval quality.

### Chunk Size vs Retrieval Quality

**Too small** (10–20 lines): Loses context. A function split across chunks is useless.
**Too large** (200+ lines): Returns too much irrelevant code. Wastes context window.
**Sweet spot** (30–60 lines): Captures complete functions with some surrounding context.

The optimal chunk size depends on the codebase:
- Utility libraries (many small functions): 20–30 lines
- Application code (medium functions): 40–60 lines
- Legacy code (large functions): 60–100 lines with overlap

### Re-Ranking: The Precision Multiplier

LLM re-ranking is the single most impactful improvement you can make to a RAG pipeline.

**Without re-ranking:** Top-5 precision ~60%. The embedding model retrieves
approximately relevant code, but ranking is noisy.

**With LLM re-ranking:** Top-5 precision ~85%. The LLM understands the query intent
deeply and can judge relevance with much higher accuracy.

**Cost:** One additional LLM call with ~2K–5K tokens (the candidate chunks + query).
At current pricing, this is $0.005–$0.02 per query. Worth it for agent workflows
where a wrong retrieval means a wrong code edit.

### Context Window Budget

A typical agent prompt contains:
- System prompt: ~500 tokens
- User instruction: ~200 tokens
- Retrieved code: ??? tokens
- Repo-map/structure: ~500 tokens
- Output space: ~2,000 tokens

For a 128K context model, you have ~124K tokens for retrieved code. That's roughly
50–60 file-sized chunks. But MORE is not always BETTER:

**The "lost in the middle" problem:** LLMs attend less to content in the middle of
long contexts. Place the most relevant chunks at the beginning and end.

**Practical budget:** 5–15 highly relevant chunks outperform 50 loosely relevant ones.
Focus on precision over recall.

### End-to-End Pipeline Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Git Watcher  │────▶│  Chunker     │────▶│  Embedder    │
│  (file diffs) │     │  (tree-sitter)│     │  (voyage-3)  │
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                  │
                                                  ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  LLM Context │◀────│  Re-Ranker   │◀────│  Vector DB   │
│  Assembly    │     │  (LLM-based) │     │  (ChromaDB)  │
└──────┬───────┘     └──────────────┘     └──────────────┘
       │
       ▼
┌──────────────┐
│  LLM Agent   │
│  (generation)│
└──────────────┘
```

This pipeline—incremental indexing, AST-aware chunking, code-optimized embeddings,
hybrid retrieval with LLM re-ranking—represents the current best practice for
production code RAG systems.
