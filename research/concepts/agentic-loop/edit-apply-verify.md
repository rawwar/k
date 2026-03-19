# Edit-Apply-Verify

## Overview

The **edit-apply-verify** cycle is an orthogonal pattern that can be layered onto
any agentic loop — ReAct, plan-and-execute, or purely conversational. It sits
*after* the LLM has decided what code to write and *before* the agent declares
success. The three-step sequence is deceptively simple:

1. **Edit** — the LLM produces structured changes (diffs, whole-file rewrites,
   tool calls).
2. **Apply** — the runtime patches the working tree.
3. **Verify** — lint, compile, and/or test the result; feed failures back.

The cycle repeats on failure, creating a tight automated feedback sub-loop that
dramatically reduces the chance of shipping broken code to the user.

Key representatives that embody this pattern:

| Agent | How it implements E-A-V |
|-------|------------------------|
| **Aider** | `--auto-lint` + `--auto-test` flags; 6 edit formats; git auto-commit |
| **ForgeCode** | Programmatic enforcement — agent *must* call verification skill |
| **Junie CLI** | 5-phase loop with dedicated Verify + Diagnose stages |
| **Claude Code** | Edit tool → bash lint/test → adaptive retry |

---

## Why Verification After Editing Matters

### The Error Rate Problem

LLMs produce syntactically invalid code roughly **15–25 %** of the time,
depending on model, language, and edit complexity. The error rate rises sharply
with:

- Unfamiliar languages or frameworks
- Edits spanning multiple files
- Long context windows where the model loses track of indentation
- Diff-format edits (matching search blocks is error-prone)

Without verification, each broken edit compounds across turns. By turn 5 the
codebase may have accumulated three or four cascading failures that are far
harder to fix than the original mistake.

### The Feedback Loop Insight

Verification creates a tight **error → fix → verify** loop:

```
LLM generates edits
        │
        ▼
   Apply to files
        │
        ▼
   Run linter ──── errors? ──► feed errors back to LLM ──► re-edit
        │                                                       │
        ▼ (pass)                                                │
   Run tests ──── failures? ─► feed failures back to LLM ──► re-edit
        │                                                       │
        ▼ (pass)                                                │
     ✅ Done                                          (bounded retries)
```

The critical insight, validated by Anthropic's research on tool-using agents, is
that agents perform **"dramatically better"** when they can observe and react to
the consequences of their actions. Telling the model "please verify your code"
in the system prompt does not reliably produce verification behavior. What works
is **enforcement at the runtime level** — the agent framework itself runs the
linter and tests, feeds errors back, and refuses to proceed until they pass (or
retries are exhausted).

ForgeCode's TermBench experiments quantified this: under token pressure, models
routinely skip optional verification steps. Making verification mandatory was
the **single biggest score improvement** in their benchmark suite.

---

## Aider's Core Loop: The Gold Standard

Aider is the most mature open-source implementation of the edit-apply-verify
pattern. Its core loop looks like this:

```
User message
    ↓
Context assembly (system prompt + repo map + files + history)
    ↓
LLM call → raw response text
    ↓
Parse edits (detect format, extract SEARCH/REPLACE or whole-file blocks)
    ↓
Apply edits to working tree
    ↓
Git commit (auto, with generated message)
    ↓
Lint (if --auto-lint enabled)
    │
    ├── errors → send to LLM → re-edit → re-apply → re-lint
    │
    ↓ (pass)
Test (if --auto-test enabled)
    │
    ├── failures → send first 50 lines → LLM → re-edit → re-apply → re-test
    │
    ↓ (pass)
Report success to user
```

### Context Assembly

Every LLM call in Aider is constructed from six layers, assembled in order:

1. **System prompt** — contains the edit format instructions, rules about how to
   produce SEARCH/REPLACE blocks, and behavioral guidelines. This is the most
   important piece — it teaches the model how to speak Aider's edit language.

2. **Repo map** — a tree-sitter-powered ranked summary of the repository. Aider
   builds a tag index of every definition and reference, then uses a PageRank-
   inspired algorithm to surface the most relevant symbols for the current task.
   The map is token-budgeted: it expands or shrinks to fit the available context
   window.

3. **Added files** — full content of files the user has `/add`'d to the chat.
   These are the files the LLM is allowed to edit.

4. **Read-only files** — context files added via `/read-only`. The LLM can see
   them but cannot produce edits targeting them.

5. **Chat history** — previous turns in the conversation. When history grows too
   large, Aider summarizes older turns using a cheaper model to stay within the
   context budget.

6. **User's new message** — the current request.

### Aider's 6 Edit Formats

Aider supports six distinct ways for the LLM to express code changes:

#### 1. `diff` (search/replace)

The default and most commonly used format. The LLM produces pairs of blocks:

```
<<<<<<< SEARCH
def greet(name):
    print("hello")
=======
def greet(name: str) -> None:
    print(f"hello, {name}")
>>>>>>> REPLACE
```

**Pros:** Compact, handles surgical edits well, works with most models.
**Cons:** The LLM must reproduce the SEARCH block exactly (or close enough for
fuzzy matching).

#### 2. `whole`

Replace the entire file content. The LLM outputs the complete new file.

```python
# filename: src/utils.py
def greet(name: str) -> None:
    print(f"hello, {name}")
```

**Pros:** Simple, no matching failures possible.
**Cons:** Wasteful for small changes in large files. Used when the diff would be
larger than the file itself.

#### 3. `udiff` (unified diff)

Standard unified diff format familiar to developers:

```diff
--- a/src/utils.py
+++ b/src/utils.py
@@ -1,2 +1,2 @@
-def greet(name):
-    print("hello")
+def greet(name: str) -> None:
+    print(f"hello, {name}")
```

**Pros:** Familiar format, compact.
**Cons:** LLMs frequently get line numbers and context lines wrong, making this
the least reliable format in practice.

#### 4. `diff-fenced`

Search/replace blocks wrapped in markdown fenced code blocks. Useful for models
that aggressively produce markdown.

#### 5. `editor-diff`

Used in **architect mode**. The architect model writes prose instructions, and
the editor model translates them into search/replace edits.

#### 6. `editor-whole`

Also used in architect mode, but the editor model outputs whole-file rewrites
instead of diffs.

### How Each Format Is Applied

**For `diff` (search/replace):**

```python
def apply_search_replace(file_content, search_text, replace_text):
    # Step 1: Try exact match
    if search_text in file_content:
        return file_content.replace(search_text, replace_text, 1)
    
    # Step 2: Fuzzy matching cascade
    match_pos = fuzzy_find(file_content, search_text)
    if match_pos is not None:
        return splice(file_content, match_pos, len(search_text), replace_text)
    
    raise SearchMatchError(f"Could not find SEARCH block in file")
```

Multiple edits to the same file are applied in order, each one operating on the
result of the previous edit.

**For `whole`:** Simply overwrite the file. No matching needed.

### Fuzzy Matching Strategy

When the exact SEARCH text doesn't match (common — LLMs frequently alter
whitespace or make minor transcription errors), Aider applies a cascade of
increasingly loose matching strategies:

1. **Strip leading/trailing whitespace** from both sides
2. **Ignore blank lines** — collapse runs of empty lines
3. **Partial line matching** — allow the SEARCH block to match a subset of
   contiguous lines
4. **Normalize whitespace** — collapse all whitespace to single spaces
5. **Character-level fuzzy match** — last resort, using difflib's
   `SequenceMatcher` with a similarity threshold

This cascade handles the vast majority of LLM transcription mistakes gracefully,
turning what would be hard failures into successful applies.

---

## Lint Integration

### Built-in Linters

Aider ships with built-in linting for most popular languages, powered by
tree-sitter. The built-in linter doesn't check style — it checks **parse
validity**. If tree-sitter can't parse the file, the edit produced invalid
syntax.

For deeper linting, users can specify a custom command:

```bash
aider --lint-cmd "python:ruff check --fix" --lint-cmd "js:eslint"
```

The format is `language:command`. Aider runs the appropriate linter based on the
file extension of each modified file.

### The Lint Feedback Loop

```python
def lint_and_fix(modified_files, llm, max_retries=2):
    for attempt in range(max_retries):
        errors = []
        for fpath in modified_files:
            lint_output = run_linter(fpath)
            if lint_output:
                errors.append((fpath, lint_output))
        
        if not errors:
            return True  # All clean
        
        # Format errors for the LLM
        error_context = format_lint_errors(errors)
        
        # Ask LLM to fix
        fix_edits = llm.generate(
            system="Fix these lint errors.",
            user=error_context
        )
        apply_edits(fix_edits)
    
    return False  # Gave up after max_retries
```

Key design decisions:
- **Only lint modified files** — don't surface pre-existing warnings
- **Parse error output** — extract file, line, message for structured feedback
- **Bounded retries** — prevent infinite lint-fix loops (default: 2 attempts)

---

## Test-Driven Verification

### The Test Feedback Loop

The test loop follows the same pattern as linting, but with different economics:
tests are slower and produce richer error output.

```python
def test_and_fix(test_cmd, llm, max_attempts=2):
    for attempt in range(max_attempts):
        result = subprocess.run(test_cmd, capture_output=True, timeout=120)
        
        if result.returncode == 0:
            return True  # Tests pass
        
        # Truncate to first 50 lines — more than enough for the model,
        # and avoids blowing the context window on massive test suites
        error_output = truncate(result.stderr + result.stdout, lines=50)
        
        llm.add_message(
            role="user",
            content=f"Tests failed. Fix the code.\n\n{error_output}"
        )
        
        edits = llm.generate()
        apply_edits(edits)
    
    return False
```

**Why 50 lines?** Aider's benchmarks showed that sending the first 50 lines of
test output captures the relevant assertion error or traceback in virtually all
cases, while keeping the LLM focused. Sending thousands of lines of output
actually *hurts* performance — the model gets lost in noise.

**Why only 2 attempts?** Diminishing returns. If the model can't fix it in one
retry, a second retry rarely helps. Better to show the user what happened and
let them guide the fix.

### The Benchmark Loop

Aider's benchmark (`aider --benchmark`) uses the edit-apply-verify cycle as its
core evaluation methodology:

```
1. Send coding exercise prompt + stub file to LLM
2. Parse and apply the LLM's edits
3. Run the exercise's test suite
4. If FAIL:
   a. Capture first 50 lines of test output
   b. Send to LLM with "fix the failing tests" instruction
   c. Parse and apply second attempt
   d. Run tests again
5. Record pass/fail result
```

This two-shot benchmark (initial attempt + one fix) directly measures how well a
model performs within the edit-apply-verify loop. It is how Aider evaluates every
new model release.

---

## Architect Mode: Two-Model Split

Aider's architect mode separates *reasoning* from *code editing* into two
distinct model roles:

```
User Request
    ↓
ARCHITECT MODEL (o3, DeepSeek R1, Gemini 2.5 Pro)
    │
    │  Produces: prose solution description, pseudocode,
    │  step-by-step plan for what to change and why
    │
    ↓
EDITOR MODEL (Claude Sonnet, GPT-4o)
    │
    │  Receives: architect's prose + file contents
    │  Produces: structured SEARCH/REPLACE edits
    │
    ↓
Apply edits → Git commit → Lint → Test
```

**Why this works:** Reasoning models (o3, R1) are excellent at *thinking through
problems* but often produce poorly formatted edits. Code-editing models
(Sonnet, GPT-4o) are excellent at producing well-formatted, syntactically correct
diffs but may not reason as deeply about the problem. The architect pattern lets
each model play to its strengths.

The editor model uses `editor-diff` or `editor-whole` format, which includes the
architect's instructions in the prompt alongside the file contents.

---

## Git Commit Integration

Aider's git integration serves as both a safety net and an audit trail:

### Auto-Commit Workflow

```
1. Before editing: check for dirty files in the working tree
2. If dirty files overlap with files to edit:
   a. Auto-commit dirty files with message "wip: pre-edit checkpoint"
   b. This protects user's uncommitted work
3. Apply LLM edits
4. Auto-commit with a generated message

Commit message generation:
   - Uses the "weak model" (cheaper, faster — e.g., GPT-4o-mini)
   - Conventional Commits format: "feat:", "fix:", "refactor:", etc.
   - Includes a brief summary of what changed
```

### Why This Matters for E-A-V

- **Each commit is an undo checkpoint.** If verification fails and the fix
  attempt makes things worse, `git checkout` reverts to the last known state.
- **Dirty-file protection** prevents the agent from accidentally destroying
  user work that hasn't been committed yet.
- **The commit history** provides a clear record of what the agent did, making
  it easy to review, cherry-pick, or revert individual changes.

---

## ForgeCode's Verification Enforcement

ForgeCode takes a fundamentally different approach to verification: instead of
*offering* verification as an optional step, the runtime **programmatically
requires** it.

### How Enforcement Works

```python
class VerificationEnforcer:
    def __init__(self, agent):
        self.agent = agent
        self.verification_called = False
    
    def on_tool_call(self, tool_name, args):
        if tool_name == "verify":
            self.verification_called = True
        return self.agent.execute_tool(tool_name, args)
    
    def on_turn_end(self):
        if not self.verification_called:
            # Inject a mandatory verification reminder
            self.agent.add_system_message(
                "REQUIRED: You must call the verify tool before completing "
                "this task. Review your changes against the requirements."
            )
            self.agent.force_continue()  # Don't let the agent stop
            return False  # Turn not complete
        return True
```

### The Verification Checklist

When the verify tool is called, ForgeCode generates a structured checklist:

| Field | Description |
|-------|-------------|
| **Requested** | What the user asked for |
| **Done** | What was actually implemented |
| **Evidence** | Lint output, test results, manual checks |
| **Missing** | Gaps between requested and done |

### Why Enforcement Beats Prompting

ForgeCode's key insight, validated on their TermBench benchmark:

- **Prompting** ("please verify your work"): Models skip verification ~40% of
  the time, especially under token pressure or when confident in their solution.
- **Enforcement** (runtime requires verification call): Models verify 100% of
  the time, because the framework won't accept the turn as complete without it.

This single change — making verification non-optional — was the largest score
improvement across all of ForgeCode's benchmark experiments.

---

## Junie CLI's Structured Verification

JetBrains' Junie takes a more structured approach, organizing the entire agent
workflow into five explicit phases:

### 5-Phase Loop

```
┌─────────────┐
│  Understand  │  Analyze the request, read relevant files
└──────┬──────┘
       ↓
┌─────────────┐
│    Plan      │  Create a step-by-step implementation plan
└──────┬──────┘
       ↓
┌─────────────┐
│  Implement   │  Write code changes according to the plan
└──────┬──────┘
       ↓
┌─────────────┐
│   Verify     │  Run tests, inspections, compilation checks
└──────┬──────┘
       │
       ├── pass → Done ✅
       │
       ↓ fail
┌─────────────┐
│  Diagnose    │  Analyze failures, fix, re-verify (up to 3-5 iterations)
└─────────────┘
```

### Verification Phase Details

Junie's verification phase runs three categories of checks:

1. **Test execution** — run the project's test suite (or a targeted subset)
2. **Code inspections** — JetBrains-style static analysis (null safety, type
   mismatches, unused imports, unreachable code)
3. **Compilation validation** — ensure the project builds successfully

All three must pass for verification to succeed.

### Diagnostic Loop

When verification fails, Junie enters a diagnostic sub-loop:

```python
def diagnose_and_fix(failures, max_iterations=5):
    for i in range(max_iterations):
        # Analyze: what went wrong and why?
        diagnosis = diagnostic_model.analyze(failures)
        
        # Fix: generate targeted edits
        fixes = implementation_model.fix(diagnosis)
        apply(fixes)
        
        # Re-verify
        new_failures = verify()
        if not new_failures:
            return True  # Fixed!
        
        # Check if we're making progress
        if len(new_failures) >= len(failures):
            # Not improving — escalate to stronger model
            diagnostic_model = upgrade_model(diagnostic_model)
        
        failures = new_failures
    
    return False  # Exhausted retries
```

### Multi-Model Delegation

Junie uses different models for different phases, optimizing for cost and
quality:

| Phase | Model Tier | Rationale |
|-------|-----------|-----------|
| Understand | Mid (Sonnet) | Needs good comprehension |
| Plan | Mid (Sonnet) | Needs reasoning ability |
| Implement (boilerplate) | Fast (Flash) | Mechanical code generation |
| Implement (complex) | Mid (Sonnet) | Needs care and precision |
| Diagnose | Strong (Opus) | Hardest task — debugging |

### IDE vs CLI Execution Paths

Junie runs in two modes with different capabilities:

**IDE mode** (IntelliJ, WebStorm):
- Uses PSI (Program Structure Interface) for refactoring
- Rename operations update all references project-wide
- Extract method/variable uses semantic understanding
- Import management is automatic
- Result: semantically correct refactoring, not just text replacement

**CLI mode** (standalone):
- Falls back to text-based search-and-replace
- Similar to Aider's diff format
- No semantic understanding of the code
- Works without an IDE but loses refactoring precision

---

## The Feedback Loop: Failed Verification → Retry

The core pattern is remarkably consistent across all agents. Here is a
generalized implementation:

```python
def edit_apply_verify(
    llm,
    context: Context,
    lint_cmd: str | None = None,
    test_cmd: str | None = None,
    max_retries: int = 3,
) -> Result:
    """
    The universal edit-apply-verify loop.
    
    This pattern appears in Aider, ForgeCode, Junie, Claude Code,
    and virtually every serious coding agent.
    """
    for attempt in range(max_retries):
        # 1. EDIT — LLM produces changes
        edits = llm.generate_edits(context)
        
        # 2. APPLY — patch the working tree
        modified_files = apply(edits)
        
        # 3. VERIFY — lint then test
        
        # 3a. Lint check (fast, catches syntax errors)
        if lint_cmd:
            lint_errors = run_lint(lint_cmd, modified_files)
            if lint_errors:
                context.append_feedback(
                    f"Lint errors on attempt {attempt + 1}:\n"
                    f"{format_errors(lint_errors)}\n"
                    f"Fix these errors."
                )
                continue  # Back to EDIT
        
        # 3b. Test check (slower, catches semantic errors)
        if test_cmd:
            test_result = run_tests(test_cmd, timeout=120)
            if test_result.failed:
                context.append_feedback(
                    f"Test failures on attempt {attempt + 1}:\n"
                    f"{truncate(test_result.output, lines=50)}\n"
                    f"Fix the failing tests."
                )
                continue  # Back to EDIT
        
        # All checks passed
        return Result(success=True, attempts=attempt + 1)
    
    # Exhausted retries
    return Result(success=False, attempts=max_retries)
```

### Why Lint Before Test?

The ordering is deliberate:

1. **Lint is fast** (milliseconds) vs tests (seconds to minutes)
2. **Lint catches syntax errors** that would cause test failures anyway
3. **Fixing lint errors first** often resolves test failures as a side effect
4. **Reduces wasted test runs** on code that won't even parse

### Error Truncation Strategy

All agents truncate error output before feeding it back to the LLM:

| Agent | Truncation | Rationale |
|-------|-----------|-----------|
| Aider | First 50 lines | Captures the relevant traceback |
| Claude Code | Adaptive | Scales with context window |
| Junie | First failure | Focuses on one error at a time |
| ForgeCode | Structured | Parsed into fields, not raw text |

Sending too much error output is counterproductive — the model loses focus in
thousands of lines of test output and produces worse fixes.

---

## Comparison Table

| Feature | Aider | ForgeCode | Junie CLI | Claude Code |
|---------|-------|-----------|-----------|-------------|
| **Edit format** | 6 formats (diff, whole, udiff, etc.) | Tool-based (file_edit) | PSI or text-based | Edit tool (search/replace) |
| **Lint integration** | `--auto-lint`, built-in + custom | Verification skill | JetBrains inspections | Via bash tool |
| **Test integration** | `--auto-test`, custom command | Verification skill | IDE test runner or CLI | Via bash tool |
| **Max retries** | 2 (lint: 2, test: 2) | Configurable | 3–5 | Unbounded (adaptive) |
| **Git integration** | Auto-commit every edit | Not built-in | Not built-in | Optional (`--git`) |
| **Verification** | Opt-in via flags | **Enforced by runtime** | First-class phase | Adaptive (model decides) |
| **Fuzzy matching** | Yes (5-level cascade) | N/A (tool-based) | PSI-aware in IDE | Exact match only |
| **Architect mode** | Yes (2-model split) | No | Multi-model delegation | No (single model) |
| **Commit messages** | Weak model, Conventional Commits | N/A | N/A | Model-generated |

---

## Best Practices

### 1. Always Lint After Edits

Linting catches syntax errors for virtually zero cost (milliseconds). There is
no reason to skip this step. Even a basic tree-sitter parse check catches the
most common LLM failure mode: malformed code.

### 2. Run Tests If Available

Tests catch **semantic** errors that linting cannot: wrong logic, incorrect API
usage, broken integrations. If the project has a test suite, use it.

### 3. Bound Your Retries

Without a retry limit, a confused model can enter an infinite fix loop — each
"fix" introduces a new error, which triggers another fix, ad infinitum. Two to
three retries is the sweet spot for most agents.

### 4. Feed Structured Error Output

Don't just tell the model "it failed." Give it the actual error message, the
file and line number, and ideally the surrounding code. The more specific the
feedback, the more targeted the fix.

```python
# Bad: "Tests failed. Fix it."
# Good:
"""
Test FAILED: test_user_login (tests/test_auth.py:42)
AssertionError: expected status 200, got 401

The relevant code in src/auth.py:
  def login(username, password):
      user = db.find_user(username)
      if user.check_password(password):  # line 15
          return Response(status=200)
      return Response(status=403)  # Should this be 401?
"""
```

### 5. Commit Before and After

Git commits serve as undo checkpoints. Committing before edits protects the
user's work; committing after records what the agent did. If a fix attempt makes
things worse, `git revert` cleanly undoes it.

### 6. Use Different Models for Different Phases

The architect pattern recognizes that reasoning and code editing are different
skills. A strong reasoning model (o3, R1) paired with a precise editing model
(Sonnet, GPT-4o) consistently outperforms using a single model for both tasks.

### 7. Enforce, Don't Prompt

If verification is important (and it always is), don't rely on the model to do
it voluntarily. Build it into the runtime. ForgeCode's experience shows that
enforcement is dramatically more reliable than prompting.

---

## Summary

The edit-apply-verify cycle is the **essential quality gate** in any coding
agent. Without it, LLM-generated code is a gamble — sometimes correct, sometimes
subtly broken. With it, the agent becomes a self-correcting system that catches
and fixes its own mistakes before the user ever sees them.

The pattern is simple: generate edits, apply them, check the result, and retry
on failure. But the implementation details matter enormously — edit format
design, fuzzy matching, error truncation, retry bounds, and verification
enforcement all contribute to the difference between a toy demo and a production
coding agent.