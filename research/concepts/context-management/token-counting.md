# Token Counting for Coding Agents

Token counting is the metrological foundation of context management. Every decision an
agent makes about what to include, when to compact, and how much budget remains depends
on knowing—or estimating—how many tokens a piece of text consumes. This document surveys
the theory, tooling, and practical strategies used across production coding agents.

---

## 1. Why Token Counting Matters

Every LLM has a finite context window measured in tokens. Coding agents must continuously
decide what fits and what doesn't. Token counting underpins:

- **Budget allocation**: Dividing context capacity across system prompts, file contents,
  conversation history, tool definitions, and response reserves.
- **Compaction triggers**: Knowing when the conversation is at 80% capacity so
  summarization can fire before hitting the hard limit.
- **Cost estimation**: Input and output tokens determine API cost. Without counting,
  agents can't display cost to users or enforce spending limits.
- **Truncation decisions**: When a file is too large, the agent needs to know how many
  tokens it would consume to decide whether to include it, truncate it, or summarize it.
- **Cache optimization**: Prompt caching (Anthropic, OpenAI) is token-position-sensitive.
  Accurate counting helps maintain cache-friendly prompt prefixes.

Without accurate token counting, an agent is flying blind. It might silently truncate
important context, overshoot the window and get an API error, or waste money by
underutilizing available capacity.

The fundamental tension: **exact counting requires running the actual tokenizer, which
adds latency and dependencies. Approximate counting is fast but introduces error.** Every
agent resolves this tension differently.

---

## 2. BPE Tokenization Deep-Dive

### What is Byte Pair Encoding?

Byte Pair Encoding (BPE) is a subword tokenization algorithm originally developed for
data compression. Its adoption in NLP (Sennrich et al., 2016) and subsequent use in
GPT-2, GPT-3, GPT-4, Claude, and most modern LLMs makes it the dominant tokenization
strategy.

### How BPE Works, Step by Step

1. **Start with bytes**: The vocabulary begins with all 256 possible byte values. Every
   string can be represented as a sequence of bytes—this guarantees the encoding is
   lossless and works on arbitrary text, including code, binary-like content, and any
   Unicode script.

2. **Count pair frequencies**: Scan the training corpus and count how often each adjacent
   pair of tokens appears. Initially these are byte pairs, e.g., `(104, 101)` for "he".

3. **Merge the most frequent pair**: Replace all occurrences of the most frequent pair
   with a new token. Add this merged token to the vocabulary.

4. **Repeat**: Continue merging until the vocabulary reaches the desired size (e.g.,
   100,000 tokens for cl100k_base, 200,000 for o200k_base).

5. **Result**: A merge table that deterministically maps any byte sequence to a sequence
   of tokens. Common words become single tokens ("function", "return"). Rare strings
   decompose into smaller pieces or individual bytes.

### Why BPE Dominates

- **Reversible and lossless**: Every token maps to a specific byte sequence. Decoding is
  trivial concatenation.
- **Open vocabulary**: Any input can be encoded, including novel words, code syntax, and
  Unicode. No "unknown token" problem.
- **Good compression**: Common patterns (English words, code keywords) compress to single
  tokens, while rare patterns degrade gracefully to byte-level encoding.
- **Deterministic**: Given the same merge table, encoding is always the same. No
  ambiguity (assuming tie-breaking rules are fixed).

### Token Density Varies by Content Type

For English prose, the average is roughly **4 bytes per token** (equivalently, ~0.75
words per token or ~4 characters per token). But code is structurally different:

- **Operators and brackets** (`{`, `}`, `(`, `)`, `=>`, `===`) are often individual
  tokens or short multi-character tokens.
- **Indentation** consumes tokens: 4 spaces might be 1 token, but inconsistent
  indentation wastes token budget.
- **String literals** tokenize differently from code—quoted content follows natural
  language patterns while the quotes themselves are separate tokens.
- **JSON** is particularly token-expensive because of its high density of structural
  punctuation: colons, commas, braces, brackets, and mandatory double quotes around
  every key.

This variance is why a single bytes-per-token ratio is always an approximation. The
actual ratio depends heavily on what kind of text you're encoding.

---

## 3. tiktoken Deep-Dive

[tiktoken](https://github.com/openai/tiktoken) is OpenAI's official tokenizer library.
It's implemented in Rust with Python bindings, making it both fast and accessible.

### Encodings

| Encoding       | Vocab Size | Models                            |
|----------------|------------|-----------------------------------|
| `cl100k_base`  | ~100K      | GPT-4, GPT-3.5-turbo, text-embedding-ada-002 |
| `o200k_base`   | ~200K      | GPT-4o, GPT-4o-mini, o1, o3      |
| `p50k_base`    | ~50K       | Codex, text-davinci-002/003       |
| `r50k_base`    | ~50K       | GPT-3 (davinci, curie, etc.)      |

The progression from 50K → 100K → 200K vocabulary sizes reflects a trend toward larger
vocabularies that compress common patterns more aggressively, at the cost of larger
embedding matrices.

### Performance

tiktoken is **3-6x faster** than the HuggingFace tokenizers library for equivalent
operations, primarily because it uses a regex-based pre-tokenization step implemented in
Rust that avoids the overhead of the more general HuggingFace pipeline.

### Python Usage

```python
import tiktoken

# Direct encoding selection
enc = tiktoken.get_encoding("cl100k_base")
tokens = enc.encode("def hello_world():\n    print('Hello')")
print(f"Token count: {len(tokens)}")  # 11 tokens
print(f"Tokens: {tokens}")            # [755, 24748, 23097, 4658, 397, 262, ...]

# Decode back to string
text = enc.decode(tokens)
assert text == "def hello_world():\n    print('Hello')"

# Model-specific encoding (resolves model name to encoding)
enc = tiktoken.encoding_for_model("gpt-4o")  # Returns o200k_base
tokens = enc.encode("def hello_world():\n    print('Hello')")
print(f"Token count with o200k: {len(tokens)}")  # May differ from cl100k

# Counting tokens for chat messages (includes message framing overhead)
def count_chat_tokens(messages, model="gpt-4o"):
    enc = tiktoken.encoding_for_model(model)
    tokens_per_message = 3  # <|start|>role<|end|> framing
    num_tokens = 0
    for message in messages:
        num_tokens += tokens_per_message
        for key, value in message.items():
            num_tokens += len(enc.encode(value))
    num_tokens += 3  # reply priming
    return num_tokens
```

### Educational Submodule

tiktoken ships with `tiktoken._educational` which implements BPE in pure Python for
learning purposes. It exposes the merge process step by step, making it invaluable for
understanding how tokenization actually works under the hood.

---

## 4. Hugging Face Tokenizers

The [tokenizers](https://github.com/huggingface/tokenizers) library is a Rust-based
tokenization framework with bindings for Python, Node.js, and Ruby.

### Supported Algorithms

- **BPE**: Same algorithm as tiktoken, used by GPT-2, GPT-Neo, LLaMA, etc.
- **WordPiece**: Used by BERT and its derivatives. Greedy longest-match instead of
  merge-based.
- **Unigram**: Probabilistic model (Kudo, 2018). Starts with a large vocabulary and
  prunes. Used by T5, ALBERT.
- **SentencePiece**: Treats input as a raw byte stream (no pre-tokenization). Used by
  LLaMA, Mistral, Gemma.

### Alignment Tracking

The killer feature for coding agents is **offset mapping**. Each token carries the
character offsets it maps to in the original string:

```python
from tokenizers import Tokenizer

tokenizer = Tokenizer.from_pretrained("gpt2")
output = tokenizer.encode("def hello():\n    return 42")

print(output.tokens)
# ['def', ' hello', '():', '\n', '   ', ' return', ' 42']

print(output.offsets)
# [(0, 3), (3, 9), (9, 12), (12, 13), (13, 16), (16, 23), (23, 26)]

# Map token index back to source character span
for token, (start, end) in zip(output.tokens, output.offsets):
    print(f"  Token '{token}' -> chars [{start}:{end}]")
```

This alignment is valuable for code because it lets agents:
- Map token budgets to exact line ranges in source files.
- Implement precise truncation that respects syntactic boundaries.
- Debug tokenization issues by seeing exactly which source characters map to which tokens.

### Performance Comparison

For bulk tokenization of large codebases, HuggingFace tokenizers are competitive with
tiktoken in throughput when using batch encoding. The gap narrows further with the
library's parallelized batch mode that distributes work across CPU cores.

---

## 5. Codex's Byte-Based Heuristic

Codex (OpenAI's open-source coding agent) takes the most pragmatic approach: **it
doesn't use a tokenizer at all**.

### The Constant

```rust
/// Approximate number of bytes per token for estimation purposes.
const APPROX_BYTES_PER_TOKEN: usize = 4;
```

That's it. Token count is estimated as `text.len() / 4`. The estimation is combined with
**server-reported token counts** from the most recent API response to stay calibrated.

### How It Works in Practice

1. After each API call, Codex records the actual token count from the response's `usage`
   field.
2. For new content added since the last API call (new user messages, tool results), Codex
   estimates tokens using the byte heuristic.
3. The total estimated context size is: `last_known_actual + bytes_since_last_call / 4`.
4. When this estimate approaches the context limit, compaction triggers.

### Why This Works

The byte heuristic is intentionally paired with **conservative compaction thresholds**.
If compaction triggers at 80% capacity, and the estimate has ±10% error, the agent still
has a comfortable buffer before actually hitting the limit. The cost of occasional
premature compaction (wasting a small amount of context) is far lower than the cost of
overshooting (API error, lost context).

### Trade-offs

| Advantage                  | Disadvantage                                     |
|----------------------------|--------------------------------------------------|
| Zero dependencies          | Less accurate for non-English text               |
| Near-zero latency          | Symbol-heavy code underestimates token count     |
| No version coupling        | JSON content significantly underestimates        |
| Simple to audit            | Can't provide precise cost estimates client-side  |

For a coding agent that processes primarily English-language code with English comments,
the 4 bytes/token heuristic is surprisingly robust. It degrades for CJK text (~2 bytes
per token), heavily punctuated formats like JSON (~2.5-3 bytes per token), or minified
code.

---

## 6. Server-Reported Token Counts

Every major LLM API returns token usage in the response:

```json
{
  "usage": {
    "prompt_tokens": 1842,
    "completion_tokens": 356,
    "total_tokens": 2198
  }
}
```

### OpenCode's Approach: Pure Server-Side Counting

OpenCode (the Go-based coding agent) takes this to its logical extreme: it performs **no
client-side token counting whatsoever**. All token tracking comes from API responses.

```go
// From OpenCode's token tracking
type TokenUsage struct {
    InputTokens         int64
    OutputTokens        int64
    CacheCreationTokens int64
    CacheReadTokens     int64
}
```

The `CacheCreationTokens` and `CacheReadTokens` fields reflect Anthropic's prompt
caching system, where cached tokens are billed at reduced rates.

### Advantages

- **Zero client complexity**: No tokenizer library, no version management, no encoding
  mismatches.
- **Always accurate**: The server uses the actual model's tokenizer, so counts are exact.
- **Handles all models**: Works identically for OpenAI, Anthropic, Google, and local
  models—as long as they report usage.

### Disadvantages

- **Reactive, not proactive**: You only learn the token count after sending the request.
  If the prompt is too large, you've already paid for (and waited for) a failed or
  truncated request.
- **No pre-flight checks**: Can't answer "will this file fit?" without making an API
  call.
- **No granular attribution**: The server tells you total prompt tokens, but not how many
  came from the system prompt vs. file contents vs. conversation history.

### Hybrid Patterns

Most production agents combine server-reported counts with some form of client-side
estimation. The server counts serve as ground truth for calibrating client estimates
and for accurate cost tracking.

---

## 7. Aider's Hybrid Approach

[Aider](https://github.com/paul-gauthier/aider) implements perhaps the most
sophisticated token counting strategy: exact counting for short texts, statistical
sampling for long texts.

### The Algorithm

```python
def token_count(self, text):
    """Count tokens with adaptive precision based on text length."""
    if len(text) < 200:
        # Short text: exact count is fast enough
        return self.main_model.token_count(text)

    # Long text: sample and extrapolate
    lines = text.splitlines(keepends=True)
    step = len(lines) // 100 or 1  # Sample ~100 lines
    sample = lines[::step]
    sample_text = "".join(sample)
    sample_tokens = self.main_model.token_count(sample_text)

    # Extrapolate: assume token density is uniform across the file
    ratio = sample_tokens / len(sample_text) if sample_text else 0
    return int(ratio * len(text))
```

### Why Sampling Works

The insight is that **token density within a single file is relatively uniform**. A
Python file's token-per-byte ratio doesn't vary dramatically between its top and bottom
halves. By sampling every 100th line, Aider captures the overall density while running
the tokenizer on only ~1% of the text.

### Performance Impact

For a 100KB file (~2,500 lines):
- **Exact counting**: Tokenize all 100KB → ~5-10ms with tiktoken.
- **Sampled counting**: Tokenize ~1KB sample → ~0.1ms with tiktoken.

This is a **~50-100x speedup** per file. When an agent is scanning an entire repository
to build a repo map (potentially hundreds of files), the cumulative savings are
substantial.

### Accuracy

In practice, the sampling approach introduces roughly **2-5% error** for typical source
files. This is well within the tolerance needed for budget allocation decisions. The error
increases for files with highly non-uniform density (e.g., a Python file with a large
base64-encoded string literal), but such cases are rare.

---

## 8. Token Counting for Different Content Types

Token density varies significantly by content type. Understanding these differences is
critical for agents that manage context budgets.

### Approximate Token Ratios

| Content Type    | Chars/Token | Bytes/Token | Notes                                  |
|-----------------|-------------|-------------|----------------------------------------|
| English prose   | ~4.0        | ~4.0        | Baseline; well-compressed by BPE       |
| Python code     | ~3.5        | ~3.5        | Symbols and indentation add overhead   |
| TypeScript/JS   | ~3.3        | ~3.3        | Type annotations add tokens            |
| JSON            | ~2.5-3.0    | ~2.5-3.0    | Very expensive: quotes, colons, commas |
| Minified JS     | ~3.0        | ~3.0        | No whitespace savings, dense symbols   |
| Markdown        | ~4.0        | ~4.0        | Similar to prose, headers are cheap    |
| Shell output    | ~3.5        | ~3.5        | Paths and flags fragment into tokens   |
| YAML            | ~3.8        | ~3.8        | Less punctuation overhead than JSON    |
| HTML            | ~3.0        | ~3.0        | Tags and attributes are expensive      |
| CJK text        | ~1.5-2.0    | ~3.0-4.0    | Multi-byte chars, fewer merges         |
| Base64          | ~2.5        | ~2.5        | Random-looking, defeats BPE merges     |

### Practical Implications

A **10KB JSON file** uses roughly **3,300-4,000 tokens**, while a **10KB Python file**
uses roughly **2,800-3,000 tokens**. This 20-30% difference matters when agents are
making inclusion/exclusion decisions at the margin.

Tool call results are particularly expensive because they often contain JSON-formatted
data. An agent that includes raw JSON tool results in context pays a significant token
premium compared to one that extracts and reformats the relevant information.

This is why several agents (Claude Code, Codex) implement tool result truncation or
summarization as a compaction strategy—tool outputs are disproportionately expensive
in token terms relative to their informational content.

---

## 9. Pre-Computed Token Budgets

Production agents don't count tokens on the fly and then decide what to include. They
pre-allocate budgets across categories, treating the context window like a financial
budget with line items.

### Aider's Budget Structure

- **Repository map**: 1,024 tokens by default, expandable up to 8,192 tokens for complex
  tasks. The map is a ranked summary of relevant files and symbols.
- **Chat history**: Grows dynamically, compacted via summarization when it exceeds budget.
- **File contents**: Remainder after other allocations. This is the flexible portion.
- **Response reserve**: Minimum 4,096 tokens reserved for model output.

### Junie CLI's Allocation

Junie CLI (JetBrains) allocates roughly:
- **~2K tokens**: System prompt and instructions
- **~20K tokens**: Active file contents (files being edited)
- **~10K tokens**: Related file contents (imported modules, tests)
- **~2K tokens**: Build and test output
- **Response reserve**: Scales with model's max output capability

### Warp's Explicit Budget System

Warp implements the most explicit budget allocation, with named categories that compete
for space:
- System prompt allocation
- Codebase context allocation
- Conversation history allocation
- User message allocation
- Tool definition allocation
- Response reserve allocation

Each category has a minimum and maximum, with a priority ordering for when total demand
exceeds supply.

### The Financial Budget Metaphor

This pattern is best understood as **departmental budgeting**: each category (system
prompt, files, history, tools, response) is a department requesting funds from a fixed
total. The agent acts as CFO, making allocation decisions based on priorities and current
needs.

Static budgets are simpler but wasteful—if the system prompt is allocated 2K tokens but
only uses 1.5K, 500 tokens go unused. Dynamic budgets reallocate unused capacity but
require more sophisticated accounting.

---

## 10. Cost Tracking and Budgeting

Token counting directly enables cost tracking. With per-token pricing, agents can display
real-time cost information to users.

### OpenCode's Cost Computation

```go
func calculateCost(usage TokenUsage, model ModelConfig) float64 {
    cost := model.CostPer1MInCached/1e6*float64(usage.CacheCreationTokens) +
        model.CostPer1MOutCached/1e6*float64(usage.CacheReadTokens) +
        model.CostPer1MIn/1e6*float64(usage.InputTokens) +
        model.CostPer1MOut/1e6*float64(usage.OutputTokens)
    return cost
}
```

Note the four-tier pricing: cached input creation, cached input reads, regular input,
and output. Anthropic's caching makes input tokens up to 90% cheaper on cache hits, so
tracking cache utilization is essential for accurate cost reporting.

### Claude Code's `/context` Command

Claude Code provides a `/context` slash command that displays current context usage:
- Total tokens used vs. available
- Breakdown by category (system, conversation, tool results)
- Estimated cost so far
- Warning when approaching limits

### Codex's Cumulative Tracking

Codex tracks cumulative token usage across the entire session:

```
Session tokens: 45,231 input / 12,847 output
Estimated cost: $0.23
```

This running total helps users understand the cost of their coding session and make
decisions about when to start fresh vs. continue in the current context.

### Budget Alerting Patterns

Common alerting thresholds across agents:
- **70% capacity**: Informational. Some agents begin pre-emptive context optimization.
- **80% capacity**: Warning. Most agents trigger compaction at this level.
- **90% capacity**: Critical. Aggressive compaction or conversation reset.
- **95%+ capacity**: Emergency. Drop non-essential context, summarize everything.

These thresholds are typically configurable, with the defaults tuned to balance context
richness against the risk of overflow.

---

## 11. Code Examples Across Languages

### TypeScript with gpt-tokenizer

```typescript
import { encode, decode, isWithinTokenLimit } from 'gpt-tokenizer';

// Basic token counting
const code = 'function hello() { return "world"; }';
const tokens = encode(code);
console.log(`Tokens: ${tokens.length}`);  // 9
console.log(`Decoded: ${decode(tokens)}`);

// Token limit checking (short-circuits for efficiency)
const longText = readFileSync('large-file.ts', 'utf-8');
if (!isWithinTokenLimit(longText, 4096)) {
    console.log('Text exceeds 4096 token limit');
    // Truncate or summarize
}

// Streaming token count for incremental building
let totalTokens = 0;
for (const chunk of textChunks) {
    totalTokens += encode(chunk).length;
    if (totalTokens > MAX_CONTEXT * 0.8) {
        triggerCompaction();
        break;
    }
}
```

### Go with tiktoken-go

```go
package main

import (
    "fmt"
    "github.com/pkoukk/tiktoken-go"
)

func countTokens(text string, model string) (int, error) {
    enc, err := tiktoken.EncodingForModel(model)
    if err != nil {
        return 0, fmt.Errorf("encoding for model %s: %w", model, err)
    }
    tokens := enc.Encode(text, nil, nil)
    return len(tokens), nil
}

func main() {
    code := "func main() {\n\tfmt.Println(\"Hello\")\n}"
    count, err := countTokens(code, "gpt-4o")
    if err != nil {
        panic(err)
    }
    fmt.Printf("Token count: %d\n", count)  // ~15 tokens
}
```

### Rust with tiktoken-rs

```rust
use tiktoken_rs::{cl100k_base, o200k_base};

fn main() {
    // Using cl100k_base encoding (GPT-4)
    let bpe = cl100k_base().unwrap();
    let code = "fn main() {\n    println!(\"Hello\");\n}";
    let tokens = bpe.encode_with_special_tokens(code);
    println!("Token count: {}", tokens.len());  // ~14 tokens

    // Using o200k_base encoding (GPT-4o)
    let bpe = o200k_base().unwrap();
    let tokens = bpe.encode_with_special_tokens(code);
    println!("Token count (o200k): {}", tokens.len());

    // Byte-heuristic alternative (what Codex does)
    let approx_tokens = code.len() / 4;
    println!("Approximate tokens: {}", approx_tokens);
}
```

---

## 12. Comparison Table

| Approach            | Used By        | Accuracy   | Speed       | Dependencies       | Proactive? |
|---------------------|----------------|------------|-------------|--------------------|------------|
| Exact tokenizer     | Aider (short)  | ~100%      | ~5-10ms/10KB| tiktoken or equiv. | Yes        |
| Sampled tokenizer   | Aider (long)   | ~95-98%    | ~0.1ms/10KB | tiktoken or equiv. | Yes        |
| Byte heuristic      | Codex          | ~85-95%    | ~0.001ms    | None               | Yes        |
| Server-reported     | OpenCode       | 100%       | N/A (async) | None               | No         |
| Hybrid (heuristic + server) | Codex  | ~90-100%   | ~0.001ms    | None               | Partial    |
| Chat framing calc   | Custom agents  | ~99%       | ~5-10ms     | tiktoken           | Yes        |

### Key Observations

1. **No agent uses exact tokenization for everything.** The cost of running a full
   tokenizer on every piece of text at every decision point is too high for interactive
   agents.

2. **Server-reported counts are universally used for cost tracking**, even by agents that
   do client-side estimation for budget management.

3. **The byte heuristic is underrated.** Codex's approach of `len / 4` combined with
   server calibration is remarkably effective for English-centric coding tasks.

4. **Aider's sampling approach is the best middle ground** for agents that need
   per-file token counts without the full cost of tokenization.

5. **Proactive counting matters more than accuracy.** An agent that estimates tokens at
   85% accuracy before sending a request outperforms one that knows the exact count only
   after the request fails.

---

## Summary

Token counting sits at the intersection of theory and engineering pragmatism. The
theoretical foundation—BPE tokenization—is well-understood, but production agents make
widely varying trade-offs between accuracy, speed, and complexity.

The spectrum runs from Codex's zero-dependency byte heuristic to Aider's adaptive
sampling to OpenCode's pure server-side approach. Each reflects different priorities:
Codex optimizes for simplicity and speed, Aider for accuracy without latency, and
OpenCode for correctness without client complexity.

For agent developers, the key insight is that **token counting accuracy only needs to
exceed the margin of your safety buffers**. If you trigger compaction at 80% capacity,
a ±10% estimation error is perfectly acceptable. If you trigger at 95%, you need near-
exact counts. The compaction threshold and the counting precision must be co-designed.
