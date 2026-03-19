# Aider — Edit Format System (The "Tool System")

## Overview

Aider doesn't have a traditional tool system like autonomous agents (no shell execution tool, no file-read tool, no web-search tool). Instead, its **edit formats** serve as the interface between the LLM and the file system. The edit format is the "tool" — it's the structured protocol through which the LLM expresses code changes.

This is one of Aider's most significant contributions to the field. Paul Gauthier's extensive benchmarking revealed that **the format you ask an LLM to use for code edits dramatically affects both code quality and edit reliability**. A complex format taxes the model's attention, leaving less capacity for the actual coding task.

## Edit Formats

### 1. `whole` — Full File Replacement

The simplest format. The LLM returns the entire updated file inside a fenced code block.

**Format:**
````
filename.py
```python
# entire file content here
def greeting(name):
    print("Hello", name)
```
````

**Characteristics:**
- **Simplest cognitive load** — the LLM just writes code naturally
- **Most reliable** — minimal parsing complexity, hard for the LLM to get wrong
- **Expensive** — the LLM must re-emit the entire file, even for one-line changes
- **Slow** — streaming a large file takes time, especially with slower models
- **Best for**: Weaker models (GPT-3.5, small local models), small files

**Used by default for**: GPT-3.5 models, Gemini 2.0 Flash, some smaller models

### 2. `diff` — Search/Replace Blocks

The LLM specifies edits as pairs of SEARCH (original) and REPLACE (new) text blocks.

**Format:**
````
filename.py
```python
<<<<<<< SEARCH
from flask import Flask
=======
import math
from flask import Flask
>>>>>>> REPLACE
```
````

**Characteristics:**
- **Efficient** — only changed portions are transmitted
- **Requires precision** — the SEARCH block must match existing code exactly
- **Fuzzy matching fallback** — aider tries progressively looser matching if exact match fails:
  1. Exact match
  2. Strip trailing whitespace
  3. Ignore blank lines
  4. Normalized whitespace matching
- **Multiple blocks per file** — can make several changes in one response
- **New files** — an empty SEARCH block signals file creation
- **Best for**: Most capable models (Claude, GPT-4, DeepSeek)

**Used by default for**: Claude 3.5/3.7 Sonnet, GPT-4o, GPT-4.1, DeepSeek V3/R1, o3, o3-mini

### 3. `diff-fenced` — Fenced Variant for Gemini

Identical to `diff` semantically, but the filename is placed **inside** the fence instead of before it.

**Format:**
````
```
filename.py
<<<<<<< SEARCH
from flask import Flask
=======
import math
from flask import Flask
>>>>>>> REPLACE
```
````

**Why it exists**: Gemini models consistently fail to conform to the standard `diff` fencing approach (placing the filename outside the fence). This variant was created specifically because Gemini would put the filename inside regardless of instructions. Rather than fight the model, aider adapted the format.

**Used by default for**: All Gemini 2.5 Pro and Flash models

### 4. `udiff` — Unified Diff Format

Based on the standard unified diff format, simplified for LLM use.

**Format:**
````
```diff
--- filename.py
+++ filename.py
@@ ... @@
-class MathWeb:
+import sympy
+
+class MathWeb:
```
````

**Characteristics:**
- **Familiar format** — based on universal diff syntax
- **Reduced lazy coding** — specifically designed to combat GPT-4 Turbo's tendency to elide code with "# ... rest of code here ..." comments
- **Fragile line numbers** — LLMs sometimes get line numbers wrong in `@@` headers
- **Best for**: GPT-4 Turbo models specifically

**Historical note**: This format was a targeted fix for GPT-4 Turbo's laziness problem and is less commonly used today.

### 5. `architect` — Two-Model Pipeline

Not really an edit format itself, but a **meta-mode** that chains two models.

**How it works:**
1. The **architect model** receives the user's request and describes the solution in plain text
2. The **editor model** receives the architect's description and produces structured edits (using `editor-diff` or `editor-whole` sub-formats)

**Architect prompt** (simplified):
> "Describe how to solve this coding problem. Don't write code edits — just explain what needs to change."

**Editor prompt** (simplified):
> "The architect has described these changes. Now produce the specific file edits using SEARCH/REPLACE blocks."

**Sub-formats:**
- `editor-diff` — Streamlined diff format with simpler prompts focused solely on edit mechanics
- `editor-whole` — Streamlined whole-file format for the editor

**Characteristics:**
- **Separation of concerns** — reasoning model reasons, editing model edits
- **SOTA results** — o1-preview + DeepSeek/o1-mini achieved 85% (the original SOTA)
- **Higher cost** — two LLM calls per turn
- **Higher latency** — sequential model calls
- **Best for**: Reasoning models (o1, o3, R1) that struggle with structured edit output

**Key configurations:**
```bash
# o3 as architect, GPT-4.1 as editor
aider --model o3 --architect

# DeepSeek R1 as architect, Claude Sonnet as editor
aider --architect --model r1 --editor-model sonnet

# Same model as both architect and editor (still helps!)
aider --model sonnet --architect
```

### 6. `whole-func` and `diff-func` — Function Call Variants

Early experiments using OpenAI's function calling API to structure edits as JSON.

**Outcome**: Benchmarking showed these performed **worse** than plain text formats. The additional cognitive overhead of producing valid JSON hurt both code quality and format adherence. These are historical artifacts, rarely used today.

## The Prompts

Each edit format has a carefully crafted system prompt. These are in `aider/coders/*_prompts.py`:

- `editblock_prompts.py` — diff format instructions
- `wholefile_prompts.py` — whole format instructions  
- `udiff_prompts.py` — unified diff instructions
- `editor_editblock_prompts.py` — editor-diff for architect mode
- `editor_whole_prompts.py` — editor-whole for architect mode

The prompts are **extensively tuned** through benchmark iteration. Small changes to wording can move benchmark scores by several percentage points. Key prompt engineering insights:

1. **Concrete examples** — Each prompt includes worked examples of the format
2. **Explicit rules** — "Every SEARCH block must exactly match existing file content"
3. **Common mistakes** — Prompts explicitly warn against known failure modes
4. **Minimal complexity** — Simpler prompts → better code (the benchmark proved this)

## Edit Format Selection Logic

Aider automatically selects the best edit format per model:

```
Model                          → Default Format
─────────────────────────────────────────────
GPT-5                          → diff
GPT-4.1                        → diff
GPT-4o                         → diff
GPT-3.5 Turbo                  → whole
o3, o3-pro                     → diff
o1                             → diff (architect recommended)
Claude 3.5/3.7 Sonnet          → diff
Claude Opus                    → diff
Gemini 2.5 Pro                 → diff-fenced
Gemini 2.5 Flash               → diff-fenced
DeepSeek V3, R1                → diff
Local models (Ollama)          → whole (usually safest)
```

Users can override with `--edit-format <format>`.

## The Key Insight

Aider's benchmark data reveals a profound finding about LLM tool use:

> **The simpler the output format, the better the LLM's actual coding performance.**

This has implications far beyond Aider. It suggests that autonomous agent frameworks should minimize the syntactic overhead of tool calls. When an LLM spends attention on formatting, it has less attention for reasoning.

The function-call formats (`whole-func`, `diff-func`) were expected to be more reliable (that was the whole point of OpenAI's function calling API), but they actually performed worse. Plain text with simple delimiters beat structured JSON every time.

This finding was instrumental in shaping how the industry thinks about LLM tool interfaces.