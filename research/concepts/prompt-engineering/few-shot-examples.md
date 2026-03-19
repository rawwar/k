# Few-Shot Examples in Coding Agent Prompts

## Abstract

Few-shot prompting — providing worked examples within the prompt to guide model
behavior — is one of the most widely adopted techniques in coding agent design.
Yet its application in agentic coding systems reveals nuances absent from
standard NLP few-shot literature. Coding agents must teach models not just *what*
to output but *how to interact with tools*, *how to format edits*, and *how to
orchestrate multi-step workflows*. This document examines how production coding
agents deploy few-shot examples, when the technique helps vs. actively hurts
performance, and the architectural patterns that have emerged for managing the
substantial token costs involved.

---

## 1. What Are Few-Shot Examples in the Coding Agent Context?

In classical NLP, few-shot examples are input-output pairs prepended to a prompt
so the model learns the task pattern in-context. In coding agents, the concept
extends across several dimensions:

| Dimension | Classical Few-Shot | Coding Agent Few-Shot |
|-----------|-------------------|----------------------|
| **Content** | Input → Output pairs | Tool invocations, edit formats, shell commands, multi-turn workflows |
| **Purpose** | Task demonstration | Format adherence, tool-use training, workflow orchestration |
| **Scope** | Single inference | Potentially dozens of tool calls across a long trajectory |
| **Token cost** | 100–500 tokens | 500–5,000+ tokens per example set |
| **Failure mode** | Wrong answer | Malformed edits, broken files, hallucinated tool calls |

A coding agent's few-shot examples typically answer three questions
simultaneously:

1. **What format should my output take?** (e.g., unified diff, search/replace
   blocks, function calls)
2. **What tools are available and how do I invoke them?** (e.g., shell commands,
   file operations, browser actions)
3. **What does a good workflow look like?** (e.g., read before edit, test after
   change, commit when green)

### The Multi-Layered Example Stack

Production agents rarely use a single example. They compose examples at multiple
layers:

```
┌─────────────────────────────────────┐
│  System Prompt                      │
│  ┌───────────────────────────────┐  │
│  │ Edit Format Examples          │  │  ← How to format code changes
│  │ (search/replace, diff, whole) │  │
│  └───────────────────────────────┘  │
│  ┌───────────────────────────────┐  │
│  │ Tool Use Examples             │  │  ← How to invoke available tools
│  │ (shell, file ops, search)     │  │
│  └───────────────────────────────┘  │
│  ┌───────────────────────────────┐  │
│  │ Workflow Examples             │  │  ← Multi-step task patterns
│  │ (debug loop, refactor, test)  │  │
│  └───────────────────────────────┘  │
│  ┌───────────────────────────────┐  │
│  │ Domain Knowledge Examples     │  │  ← Framework-specific patterns
│  │ (React, Django, Rust, etc.)   │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```

---

## 2. When Few-Shot Helps vs. Hurts

### The Aider Discovery: Format Complexity × Few-Shot = Degradation

Aider's extensive benchmarking revealed a counterintuitive finding: few-shot
examples for complex output formats can *reduce* performance. Specifically,
function-call edit formats (whole-func, diff-func) performed worse than
plain-text equivalents despite including worked examples:

> "The additional cognitive overhead of producing valid JSON hurt both code
> quality and format adherence."

This suggests a **complexity ceiling** for few-shot effectiveness:

```
Performance
    │
    │     ╭──── Plain text formats
    │    ╱      (search/replace, diff)
    │   ╱
    │  ╱  ╭──── Structured formats
    │ ╱  ╱      (JSON, function calls)
    │╱  ╱
    │  ╱
    │ ╱
    │╱
    └──────────────────────────────
         Format Complexity →
```

**When the model must simultaneously**:
1. Solve the coding problem
2. Produce syntactically valid JSON
3. Follow the function-call schema exactly
4. Match the patterns shown in examples

...the cognitive load of format compliance competes with the cognitive load of
code reasoning. The few-shot examples that were meant to help with (3) and (4)
actually exacerbate the problem by consuming context that could have been used
for reasoning.

### The Helpfulness Spectrum

```
STRONGLY HELPS                              ACTIVELY HURTS
◄──────────────────────────────────────────────────────────►

Simple format    Tool invocation   Complex JSON    Over-specified
demonstration    patterns          schemas         workflow
                                                   templates

Shell commands   Search/replace    Function-call   Multi-step
with flags       block syntax      edit formats    recipes for
                                                   novel tasks
```

### Conditions Where Few-Shot Reliably Helps

1. **Novel output formats** the model hasn't seen in training (e.g., Aider's
   specific search/replace block syntax)
2. **Tool invocation patterns** where the model needs to learn a specific API
   (e.g., Mini-SWE-Agent's shell command patterns)
3. **Disambiguation** when multiple valid approaches exist and you want a
   specific one
4. **Error prevention** when the model has known failure modes in a domain

### Conditions Where Few-Shot Hurts or Is Unnecessary

1. **Standard code generation** — models already have strong priors from training
2. **Complex structured output** — JSON schema adherence competes with reasoning
3. **Open-ended tasks** — examples can anchor the model to demonstrated patterns,
   reducing creativity
4. **Sufficient training data** — as ForgeCode observes: "Models have strong
   priors from training about what tool calls should look like"

---

## 3. Types of Few-Shot Examples for Coding Agents

### 3.1 Edit Format Examples

The most common type. Every agent that defines a custom edit format must teach
the model that format through examples.

**Aider's search/replace format example (effective):**

```
Here is an example of how to edit a file:

path/to/file.py
<<<<<<< SEARCH
def old_function():
    return 1
=======
def old_function():
    return 2
>>>>>>> REPLACE
```

**Why this works:** The format is simple text with clear delimiters. The model
doesn't need to escape characters, count brackets, or produce valid JSON. The
few-shot example teaches only the delimiter pattern — a small cognitive addition.

**Contrast with a function-call format (less effective):**

```json
{
  "tool": "edit_file",
  "arguments": {
    "path": "path/to/file.py",
    "changes": [
      {
        "search": "def old_function():\n    return 1",
        "replace": "def old_function():\n    return 2"
      }
    ]
  }
}
```

**Why this hurts:** The model must handle JSON escaping of newlines, proper
quoting, nested object structure, and array syntax — all while reasoning about
the actual code change. Every escaped newline is a potential failure point.

### 3.2 Command Examples

Teaching the model which shell commands to use and how to use them.

**Mini-SWE-Agent's command examples:**

```bash
### Create a new file:
cat <<'EOF' > newfile.py
import numpy as np
print("hello")
EOF

### Edit files with sed:
sed -i 's/old_string/new_string/g' filename.py

### View file content:
nl -ba filename.py | sed -n '10,20p'
```

These examples serve multiple purposes:
- **Tool vocabulary**: establishes which commands the agent should use
- **Idiom selection**: `cat <<'EOF'` over `echo`, `nl -ba` over `cat -n`
- **Pattern anchoring**: the agent learns to reach for `sed` for edits rather
  than rewriting entire files

### 3.3 Workflow Examples

Multi-step patterns that demonstrate how to chain actions.

**Junie CLI's refactoring checklist (embedded in prompts):**

```
When extracting a method, ensure:
- All used variables are passed as parameters
- Modified variables are returned
- Exception handling is preserved
- Access modifiers are appropriate
```

This isn't a traditional input-output example — it's a *procedural* few-shot
pattern. It teaches the model a checklist-style workflow derived from JetBrains'
20+ years of IDE framework knowledge. The insight is that refactoring is a
domain where human experts follow predictable steps, and encoding those steps
as examples prevents the model from skipping critical considerations.

### 3.4 Domain-Specific Knowledge Examples

Framework and language-specific patterns that inject expertise.

```python
# Example: Django model migration workflow
# When adding a field to a Django model:
# 1. Add the field to the model class
# 2. Run: python manage.py makemigrations
# 3. Review the generated migration file
# 4. Run: python manage.py migrate
# 5. Update serializers/forms that reference the model
# 6. Update tests
```

---

## 4. Static vs. Dynamic Example Selection

### Static Examples

Fixed examples embedded in the system prompt, loaded for every interaction.

```
┌─────────────────────────┐
│  System Prompt           │
│                          │
│  [Always-loaded examples]│
│  - Edit format           │
│  - Basic tool use        │
│  - Core workflow         │
│                          │
│  Cost: Fixed ~2000 tokens│
└─────────────────────────┘
```

**Advantages:**
- Predictable token cost
- Consistent behavior
- Simple implementation

**Disadvantages:**
- Wastes tokens when examples are irrelevant
- Cannot adapt to task type
- Context window pressure grows with example count

### Dynamic Examples

Selected based on the current task, user input, or conversation state.

```
┌─────────────────────────────────────┐
│  Task: "Add React component"        │
│                                      │
│  Selected examples:                  │
│  ✓ React component creation pattern │
│  ✓ JSX file editing format          │
│  ✓ Test file co-location pattern    │
│  ✗ Python migration workflow        │  ← Not selected
│  ✗ Rust borrow-checker patterns     │  ← Not selected
│                                      │
│  Cost: ~800 tokens (task-specific)  │
└─────────────────────────────────────┘
```

**Selection strategies:**

| Strategy | Description | Used By |
|----------|-------------|---------|
| **Keyword matching** | Match user input keywords to example tags | OpenHands microagents |
| **Embedding similarity** | Semantic search over example corpus | Custom RAG pipelines |
| **Rule-based** | If language=X, load X-specific examples | Most agents |
| **LLM-selected** | Ask a cheaper model to pick relevant examples | Advanced pipelines |

---

## 5. Progressive and On-Demand Example Loading

### Gemini CLI's Skill Architecture

Gemini CLI pioneered a progressive disclosure pattern for few-shot examples
disguised as "skills":

```
Phase 1: System Prompt (~500 tokens)
┌────────────────────────────────────────────┐
│  Available skills (metadata only):          │
│  - web_search: "Search the web for info"    │
│  - code_review: "Review code for issues"    │
│  - test_gen: "Generate test cases"          │
│  - refactor: "Refactor code safely"         │
│                                             │
│  Use activate_skill(name) to load a skill.  │
└────────────────────────────────────────────┘

Phase 2: On-Demand Loading (~2000 tokens when activated)
┌────────────────────────────────────────────┐
│  activate_skill("code_review")              │
│                                             │
│  [Full skill content loaded]:               │
│  - Detailed review checklist                │
│  - Example review comments                  │
│  - Severity classification guide            │
│  - Common anti-patterns to flag             │
└────────────────────────────────────────────┘
```

**The key insight:** skills function as stored few-shot examples or domain
expertise, but they avoid paying the token cost upfront. The ~500-token metadata
index lets the model decide *when* it needs more detailed guidance.

**Token economics:**

```
Approach                    Tokens at start    Tokens when needed
─────────────────────────────────────────────────────────────────
All examples upfront        8,000              8,000
Gemini-style progressive      500              500 + 2,000 = 2,500
No examples                     0              0 (but more errors)
```

### OpenHands' Knowledge Microagents

OpenHands implements keyword-triggered expertise injection — what they call a
"lightweight form of RAG that requires no embedding model or vector database."

```
Trigger: User message contains "github"
  → Inject: GitHub API patterns, authentication examples, common operations

Trigger: User message contains "docker"
  → Inject: Dockerfile best practices, common commands, debugging patterns

Trigger: User message contains "postgres"
  → Inject: Connection patterns, migration examples, query optimization tips
```

This is few-shot examples implemented as a reactive system rather than a static
prompt section. The examples are identical in structure to traditional few-shot
— worked input-output pairs — but the delivery mechanism is event-driven.

**Comparison of loading strategies:**

```
                    Static        Progressive      Keyword-Triggered
                    ──────        ───────────      ─────────────────
Token efficiency    Low           High             High
Implementation      Simple        Moderate         Moderate
Relevance           Mixed         Model-chosen     Heuristic
Latency             None          One round-trip   None (rule-based)
Reliability         High          Depends on       Depends on
                                  model judgment   keyword coverage
```

---

## 6. Code-Specific Few-Shot Patterns

### 6.1 File Creation Patterns

**Good pattern (Mini-SWE-Agent style):**

```bash
# Creating a new Python module with proper structure
cat <<'EOF' > src/auth/middleware.py
"""Authentication middleware for the application."""

from functools import wraps
from flask import request, jsonify

def require_auth(f):
    @wraps(f)
    def decorated(*args, **kwargs):
        token = request.headers.get("Authorization")
        if not token:
            return jsonify({"error": "Missing token"}), 401
        return f(*args, **kwargs)
    return decorated
EOF
```

**Bad pattern (overly abstract):**

```
To create a file, write the contents to the specified path.
Make sure to include proper imports and documentation.
```

The bad pattern provides guidance without demonstrating execution — it tells the
model *what* to do without showing *how*. The good pattern is a concrete,
copy-adaptable template.

### 6.2 File Editing Patterns

**Good pattern (search/replace with realistic context):**

```
To fix the off-by-one error in the pagination logic:

src/api/pagination.py
<<<<<<< SEARCH
    start = page * page_size
    end = start + page_size
    return items[start:end]
=======
    start = (page - 1) * page_size
    end = start + page_size
    return items[start:end]
>>>>>>> REPLACE
```

**Bad pattern (trivial example that doesn't teach edge cases):**

```
example.py
<<<<<<< SEARCH
hello
=======
goodbye
>>>>>>> REPLACE
```

The trivial example teaches the *syntax* but not the *judgment* — it doesn't
show how much context to include in the SEARCH block, how to handle indentation,
or what a realistic edit looks like.

### 6.3 Search and Navigation Patterns

```bash
# Find all files that import a specific module
grep -r "from auth import" --include="*.py" -l

# Find function definitions
grep -rn "def process_payment" --include="*.py"

# View context around a match
grep -n "class UserModel" src/ -r -A 20
```

### 6.4 Testing Patterns

```bash
# Run specific test file
python -m pytest tests/test_auth.py -v

# Run tests matching a pattern
python -m pytest -k "test_login" -v

# Run with coverage for changed files
python -m pytest tests/test_auth.py --cov=src/auth --cov-report=term-missing
```

---

## 7. Few-Shot for Tool Use vs. Few-Shot for Code Generation

These are fundamentally different applications of the same technique, and
conflating them leads to poor prompt design.

### Tool-Use Few-Shot

**Purpose:** Teach the model *how to invoke* a tool correctly.
**Focus:** Syntax, parameter names, expected return values.
**Stability:** High — the tool API rarely changes.

```
Example tool call:
<tool_call>
  <name>read_file</name>
  <param name="path">src/main.py</param>
  <param name="start_line">10</param>
  <param name="end_line">25</param>
</tool_call>

Response will contain the file contents between lines 10 and 25.
```

### Code-Generation Few-Shot

**Purpose:** Teach the model *what code to produce*.
**Focus:** Patterns, idioms, architectural decisions.
**Stability:** Low — varies by language, framework, and project.

```python
# Example: Creating a FastAPI endpoint with proper error handling
@router.post("/users", response_model=UserResponse, status_code=201)
async def create_user(
    user_data: UserCreate,
    db: AsyncSession = Depends(get_db),
):
    try:
        user = await UserService(db).create(user_data)
        return user
    except DuplicateEmailError:
        raise HTTPException(
            status_code=409,
            detail="Email already registered",
        )
```

### Why the Distinction Matters

Tool-use examples should be **minimal and precise** — they teach API surface
area. Including unnecessary code logic in tool-use examples wastes tokens and
can confuse the model about what the example is demonstrating.

Code-generation examples should be **realistic and contextual** — they teach
patterns and judgment. Stripping them to minimal syntax defeats their purpose.

```
                    Tool-Use Examples       Code-Gen Examples
                    ─────────────────       ─────────────────
Token budget        50–200 per example      200–1000 per example
Number needed       One per tool            Varies by task
Update frequency    When tools change       When best practices evolve
Risk of staleness   Low                     High
Overfitting risk    Low                     High (anchoring)
```

---

## 8. The Token Cost Tradeoff

Every token spent on few-shot examples is a token *not* available for:
- User context (file contents, error messages)
- Conversation history (multi-turn reasoning)
- Model reasoning (chain-of-thought)
- Output generation (the actual code)

### Token Budget Analysis

Consider a 128K context window with a typical coding agent session:

```
┌──────────────────────────────────────────────┐
│  128K Context Window                          │
│                                               │
│  System prompt + examples:  4,000–12,000     │
│  Tool descriptions:         2,000–6,000      │
│  Conversation history:      20,000–60,000    │
│  Current file contents:     10,000–40,000    │
│  Available for reasoning:   20,000–92,000    │
│                                               │
│  With aggressive few-shot:                    │
│  Examples:                  12,000            │
│  Other fixed costs:         8,000             │
│  History + files:           80,000            │
│  Remaining for reasoning:   28,000            │
│                                               │
│  With progressive loading:                    │
│  Examples (metadata only):  500               │
│  Activated examples:        2,000             │
│  Other fixed costs:         8,000             │
│  History + files:           80,000            │
│  Remaining for reasoning:   37,500            │
└──────────────────────────────────────────────┘
```

That 9,500-token difference between aggressive and progressive approaches can
represent ~25 additional lines of code context or significantly more reasoning
space. Over a long session with many tool calls, this compounds.

### The Diminishing Returns Curve

```
Format
Adherence
  100% │                    ·····················
       │               ····
       │           ···
   90% │        ··
       │      ·
       │    ·
   80% │   ·
       │  ·
       │ ·
   70% │·
       │
       └─────────────────────────────────────────
        0    500   1000  1500  2000  2500  3000
              Tokens spent on examples →
```

Most of the benefit comes from the first 500–1000 tokens of examples. Beyond
that, returns diminish sharply while costs remain linear.

### Cost-Effective Strategies

1. **One good example > three mediocre ones.** A single realistic, well-chosen
   example often outperforms multiple trivial ones.

2. **Show the hard case, not the easy one.** If the model already handles simple
   edits well, don't waste tokens demonstrating them. Show the tricky edge case
   (multi-file edit, indentation-sensitive change, handling of special
   characters).

3. **Front-load format, defer domain knowledge.** The edit format example must
   always be present (the model needs it every turn). Domain-specific patterns
   can be loaded on demand.

---

## 9. Knowledge-Triggered Examples (OpenHands' Microagent Pattern)

OpenHands' KnowledgeMicroagent pattern deserves dedicated analysis because it
represents a distinct architectural approach to few-shot example delivery.

### Architecture

```
User Message: "Fix the GitHub Actions workflow"
                │
                ▼
        ┌───────────────┐
        │ Keyword Scan   │
        │ "github"  ──── │──► GitHub microagent knowledge
        │ "actions" ──── │──► CI/CD microagent knowledge
        │ "workflow" ─── │──► (covered by above)
        └───────────────┘
                │
                ▼
        ┌───────────────────────────────────┐
        │ Injected Context (~2000+ tokens): │
        │                                    │
        │ GitHub Actions best practices:     │
        │ - Use pinned action versions       │
        │ - Cache dependencies               │
        │ - Use matrix for multi-version     │
        │                                    │
        │ Common workflow patterns:           │
        │ - CI: lint → test → build → deploy │
        │ - PR: label, review, merge checks  │
        │                                    │
        │ Debugging patterns:                │
        │ - Check runner logs first           │
        │ - Validate YAML syntax             │
        │ - Test locally with `act`          │
        └───────────────────────────────────┘
```

### Why "Lightweight RAG" Is an Apt Description

Traditional RAG requires:
- Embedding model for query vectorization
- Vector database for storage and retrieval
- Similarity search at inference time
- Chunking and indexing pipeline

OpenHands' keyword-triggered approach requires:
- A keyword → knowledge mapping (simple dictionary)
- String matching on user input
- Direct injection of pre-written knowledge

The tradeoff is precision vs. simplicity. Keyword matching will sometimes inject
irrelevant knowledge (e.g., "github" mentioned in passing) and sometimes miss
relevant knowledge (e.g., describing a CI problem without saying "github"). But
the implementation complexity is orders of magnitude lower.

### Designing Effective Knowledge Triggers

**Good trigger design:**

```python
triggers = {
    "react": {
        "keywords": ["react", "jsx", "tsx", "component", "hook", "useEffect"],
        "knowledge": "...",
        "priority": "high",
    },
    "django": {
        "keywords": ["django", "manage.py", "migration", "queryset", "ORM"],
        "knowledge": "...",
        "priority": "medium",
    },
}
```

**Bad trigger design:**

```python
triggers = {
    "web": {
        "keywords": ["web"],  # Too broad — triggers on "web" in any context
        "knowledge": "...",
    },
    "python_advanced_metaclass_patterns": {
        "keywords": ["metaclass"],  # Too narrow — rarely triggered
        "knowledge": "...",  # 3000 tokens of rarely-useful content
    },
}
```

---

## 10. Anti-Patterns and Pitfalls

### Anti-Pattern 1: The Trivial Example

```
# BAD: Example too simple to teach anything useful
Example:
User: Change x to y
Assistant: I'll change x to y.
file.txt
<<<<<<< SEARCH
x
=======
y
>>>>>>> REPLACE
```

**Why it fails:** The model already knows how to substitute single characters.
This wastes ~100 tokens teaching nothing. The model needs to see examples with
realistic complexity — multi-line edits, proper indentation, sufficient search
context for unique matching.

### Anti-Pattern 2: The Kitchen Sink

```
# BAD: Loading every possible example upfront
System prompt includes:
- 5 edit format examples (various languages)
- 10 shell command examples
- 8 workflow examples
- 6 framework-specific patterns
- 4 error-handling examples
Total: ~8,000 tokens of examples
```

**Why it fails:** Most examples are irrelevant to any given task. The model must
process all of them, and research suggests that irrelevant few-shot examples
can actually *degrade* performance by introducing noise.

### Anti-Pattern 3: The Contradictory Example

```
# BAD: Example contradicts the instructions

Instructions: "Always use the search/replace format for edits."

Example:
To fix the bug, I'll rewrite the entire file:
```python
# Complete new file contents...
```

**Why it fails:** The example demonstrates whole-file rewriting while the
instructions mandate search/replace. Models will often follow the *example*
over the *instruction* when they conflict ("show, don't tell" cuts both ways).

### Anti-Pattern 4: The Outdated Example

```
# BAD: Example uses deprecated patterns

Example:
```javascript
// Creating a React component
class MyComponent extends React.Component {
  render() {
    return <div>{this.props.name}</div>;
  }
}
```

**Why it fails:** The model may follow the demonstrated class-component pattern
even though functional components with hooks are the modern standard. Few-shot
examples have a powerful anchoring effect on model output.

### Anti-Pattern 5: Over-Specifying the Workflow

```
# BAD: Rigid multi-step example for a task that needs flexibility

Example workflow for ANY bug fix:
1. Read the error message
2. Search for the error string in the codebase
3. Open the file containing the error
4. Read 50 lines of context around the error
5. Identify the root cause
6. Write a fix
7. Run the test suite
8. Commit the fix

```

**Why it fails:** Some bugs require different investigation strategies. The
rigid example can cause the model to mechanically follow steps 1–8 even when
the task calls for a different approach (e.g., starting from a failing test
rather than an error message).

### Anti-Pattern 6: Format Example Without Error Recovery

```
# BAD: Only showing the happy path

Example:
<<<<<<< SEARCH
old code
=======
new code
>>>>>>> REPLACE

# BETTER: Also showing what to do when search fails

If the SEARCH block doesn't match exactly, you may need to:
1. View the file to see current contents
2. Copy the exact text including whitespace
3. Retry with the correct SEARCH block
```

Production agents encounter malformed edits frequently. Teaching the model how
to *recover* from format errors is as important as teaching the format itself.

---

## 11. Best Practices

### Practice 1: Match Example Complexity to Task Complexity

```
Simple format teaching     →  1 concise example (~200 tokens)
Tool invocation patterns   →  1 example per tool (~100 tokens each)
Complex workflow patterns  →  1 realistic example (~500 tokens)
Domain-specific knowledge  →  Load on demand (0 tokens until needed)
```

### Practice 2: Use Realistic, Non-Trivial Examples

**Instead of:**
```
file.py
<<<<<<< SEARCH
a = 1
=======
a = 2
>>>>>>> REPLACE
```

**Use:**
```
src/utils/parser.py
<<<<<<< SEARCH
def parse_config(filepath):
    with open(filepath) as f:
        data = json.load(f)
    return data
=======
def parse_config(filepath):
    with open(filepath) as f:
        try:
            data = json.load(f)
        except json.JSONDecodeError as e:
            raise ConfigError(f"Invalid config at {filepath}: {e}") from e
    return data
>>>>>>> REPLACE
```

The realistic example teaches proper search context size, indentation handling,
and the kind of edit the model will actually need to make.

### Practice 3: Implement Progressive Disclosure

Follow Gemini CLI's pattern:

```
Level 0 (always loaded):   Format syntax + 1 core example     ~500 tokens
Level 1 (task-triggered):  Tool-specific examples              ~1000 tokens
Level 2 (domain-triggered): Framework/language patterns        ~2000 tokens
Level 3 (error-triggered):  Recovery and debugging examples    ~1000 tokens
```

### Practice 4: Separate Format Examples from Logic Examples

Don't mix "how to format your output" with "how to solve this kind of problem"
in a single example. This makes it harder for the model to extract the relevant
lesson and harder for you to update one without affecting the other.

```
# GOOD: Separate concerns

## Output Format
Your edits must use this format:
[single format example]

## Approach Guidance
When fixing bugs:
[workflow guidance without format details]
```

### Practice 5: Test Examples Against the Target Model

Different models respond differently to the same examples. An example that works
perfectly with Claude may confuse GPT-4 and vice versa. Key testing dimensions:

- **Format adherence rate**: Does the model follow the demonstrated format?
- **Anchoring effect**: Does the model over-copy from the example?
- **Generalization**: Can the model adapt the example to novel situations?
- **Interference**: Do examples for Task A hurt performance on Task B?

### Practice 6: Version and Iterate Examples

Treat few-shot examples as code — version them, test them, and iterate:

```
examples/
  v1/
    edit_format.md      # Original examples
    tool_use.md
  v2/
    edit_format.md      # After benchmarking: simplified, more realistic
    tool_use.md
  experiments/
    minimal.md          # Testing with fewer examples
    verbose.md          # Testing with more detailed examples
```

### Practice 7: Consider the Correction-Layer Alternative

ForgeCode's approach challenges the assumption that few-shot examples are always
needed. Their correction layer compensates for model errors rather than
preventing them:

```
Model Output → Correction Layer → Valid Output
                    │
                    ├── Fix JSON syntax errors
                    ├── Repair malformed diffs
                    ├── Normalize file paths
                    └── Validate tool call schemas
```

**When to prefer correction over few-shot:**
- The model already produces nearly-correct output
- The error patterns are mechanical (syntax, formatting)
- Token budget is extremely constrained
- You need to support multiple models with different failure modes

**When to prefer few-shot over correction:**
- The errors are semantic (wrong approach, missing steps)
- The output format is genuinely novel
- Correction is computationally expensive
- You need the model to "think in" the correct format

### Practice 8: Leverage the Goose Recipe Pattern

Goose's recipe system provides worked examples for specific task patterns that
users can invoke by name. This is an explicit, user-controlled form of few-shot
selection:

```yaml
# .goose/recipes/add-api-endpoint.yaml
name: Add API Endpoint
description: Add a new REST API endpoint with tests
steps:
  - "Create the route handler in src/routes/"
  - "Add request/response validation schemas"
  - "Write integration tests"
  - "Update the API documentation"
example: |
  For adding a GET /users/:id endpoint:
  1. Created src/routes/users.ts with getUser handler
  2. Added UserParams schema with id validation
  3. Wrote tests in tests/routes/users.test.ts
  4. Updated openapi.yaml with new endpoint spec
```

This shifts example selection from automated heuristics to human judgment —
the user knows which recipe applies to their current task. The tradeoff is that
it requires user awareness and explicit invocation.

---

## Summary: Decision Framework

When designing few-shot examples for a coding agent, use this decision tree:

```
Is the output format novel to the model?
├── Yes → Include 1-2 format examples (always loaded)
└── No  → Skip format examples, rely on model priors

Does the task involve custom tools?
├── Yes → Include 1 example per tool (always loaded, minimal)
└── No  → Skip tool examples

Is domain expertise required?
├── Yes → Load domain examples on demand (keyword or skill trigger)
└── No  → Skip domain examples

Are there known failure modes?
├── Yes → Include recovery/error-handling examples
└── No  → Skip recovery examples

Is the output format complex (JSON, function calls)?
├── Yes → Consider correction layer instead of more examples
└── No  → Few-shot examples should work well
```

The most effective coding agents don't treat few-shot examples as a monolithic
prompt section. They treat them as a *managed resource* — loaded strategically,
measured for effectiveness, and continuously refined against real-world
performance data.

---

## References

- Aider edit format benchmarking and the finding that function-call formats
  underperform plain-text with worked examples
- Mini-SWE-Agent system prompt with concrete shell command examples
- Gemini CLI's progressive skill disclosure architecture
- OpenHands KnowledgeMicroagent keyword-triggered expertise injection
- Goose recipe and skill system via Summon
- ForgeCode's correction-layer approach as an alternative to few-shot
- Junie CLI's JetBrains-derived refactoring checklists in prompts