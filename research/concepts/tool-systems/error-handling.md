---
title: "Error Handling"
---

# Error Handling

When tools fail — how coding agents detect, recover from, and learn from errors across the tool execution lifecycle.

## The Error Taxonomy

Tool failures in coding agents fall into distinct categories, each requiring different detection and recovery strategies. Understanding this taxonomy is essential because the model's ability to self-correct depends entirely on how errors are surfaced.

### Categories of Failure

**Validation errors** occur before execution begins. The tool call itself is malformed — invalid JSON in arguments, missing required parameters, wrong types, or referencing a tool that doesn't exist. These are the cheapest errors to handle because no side effects have occurred.

**Execution errors** happen during tool operation. A command returns a non-zero exit code, a file path doesn't exist, permissions are denied, or the operation hits a logical impossibility (like replacing text that appears multiple times). The tool ran but couldn't complete its job.

**Timeout errors** arise when tools exceed their time budget. A build that hangs, a test suite that runs forever, or a shell command waiting on input that never arrives. These require process management — terminating the hung operation and reporting back cleanly.

**Output errors** occur when the tool succeeds but its output is problematic. Too much output floods the context window. Binary content can't be meaningfully processed. Encoding issues corrupt the text. The tool worked, but the result can't be consumed.

**Infrastructure errors** are environmental failures. The sandbox crashes, a Docker container dies, network connectivity drops, or the filesystem becomes read-only. These are the hardest to handle because they may affect the agent's ability to function at all.

### Why Graceful Handling Matters

The fundamental principle across all coding agents: **errors are information for the LLM, not crashes for the agent**. When a tool fails, the error message becomes part of the conversation. The model reads it, understands what went wrong, and issues a corrected tool call. This only works if:

1. The error message is clear and actionable
2. The agent doesn't crash before delivering the message
3. The error is surfaced as content the model can read, not metadata it might miss
4. The agent maintains enough state to continue the conversation

Every agent in the ecosystem has internalized this principle, though they implement it very differently.

---

## Validation Errors

Validation errors are the first line of defense. Most agents return these as tool results rather than crashing — the model needs to know what it did wrong so it can try again.

### OpenCode: Go Error Patterns

OpenCode distinguishes between infrastructure failures (which are Go errors propagated up the call stack) and tool-level failures (which are reported back to the LLM as content). This separation is critical:

```go
// Tool dispatch in OpenCode's message loop
if tool == nil {
    toolResults[i] = message.ToolResult{
        Content: fmt.Sprintf("Tool not found: %s", toolCall.Name),
        IsError: true,
    }
    continue  // Not fatal — reported back to LLM
}

// Actual tool execution
result, err := tool.Execute(ctx, args)
if err != nil {
    // Infrastructure error — propagate up
    return fmt.Errorf("tool execution failed: %w", err)
}
if result.IsError {
    // Tool-level error — report to model
    toolResults[i] = result
}
```

The `IsError: true` flag tells the model "this didn't work" while the content explains why. The model then adjusts its next call.

### Ante: Typed Rust Errors

Ante uses Rust's type system to enforce exhaustive error handling through a dedicated `ToolError` enum:

```rust
pub enum ToolError {
    InvalidArgs(String),      // Malformed parameters
    Execution(String),        // Runtime failure
    Timeout(Duration),        // Exceeded time limit
    NotFound(String),         // Tool doesn't exist
    PermissionDenied(String), // Sandbox restriction
}

// Tool execution returns Result<Value, ToolError>
pub async fn execute(&self, args: Value) -> Result<Value, ToolError> {
    let params: WriteFileParams = serde_json::from_value(args)
        .map_err(|e| ToolError::InvalidArgs(format!("Invalid params: {}", e)))?;
    // ...
}
```

This approach guarantees at compile time that every error path is handled. The Rust compiler enforces that no `ToolError` variant goes unmatched.

### OpenHands: Clean Failure Messages

OpenHands' `str_replace_editor` demonstrates thoughtful validation error design. When the model provides text to replace that matches multiple locations:

```
ERROR: Multiple occurrences of `old_str` found in file.
Please ensure it is unique within the file.
Consider including more surrounding context to make the match unique.
```

The error doesn't just say "failed" — it tells the model exactly what to do differently. This instructional approach to error messages dramatically improves self-correction rates.

### ForgeCode: Pre-Execution Correction

ForgeCode introduces the most sophisticated validation layer — a tool-call correction system that intercepts and repairs calls before execution:

```
Three failure classes handled:
1. Wrong tool selected → redirects to correct tool
2. Correct tool, wrong arguments → auto-corrects parameters
3. Correct call, wrong sequencing → enforces ordering constraints
```

This is validation-as-correction rather than validation-as-rejection. Instead of telling the model it failed, ForgeCode fixes the call silently and proceeds. This reduces round-trips and avoids wasting context on error-correction loops.

---

## Execution Errors

When a tool runs but fails during operation, agents must capture the failure and present it as useful context for the model's next attempt.

### Command Failures

Non-zero exit codes are the most common execution error. Every agent that runs shell commands must handle them:

```python
# Common pattern across agents
result = subprocess.run(cmd, capture_output=True, timeout=timeout)
if result.returncode != 0:
    return ToolResult(
        content=f"Command failed (exit code {result.returncode}):\n"
                f"stdout: {result.stdout}\n"
                f"stderr: {result.stderr}",
        is_error=True
    )
```

The key design decision: include both stdout and stderr. Some tools write useful information to stdout before failing, and stderr often contains the actual error message. Omitting either reduces the model's ability to diagnose the problem.

### File Operation Errors

File-not-found and permission-denied errors require clear messaging:

```go
// OpenCode pattern for file reads
content, err := os.ReadFile(path)
if os.IsNotExist(err) {
    return ToolResult{
        Content: fmt.Sprintf("File not found: %s. Use list_files to see available files.", path),
        IsError: true,
    }
}
if os.IsPermission(err) {
    return ToolResult{
        Content: fmt.Sprintf("Permission denied: %s. This file is outside the allowed workspace.", path),
        IsError: true,
    }
}
```

Notice the pattern: the error message includes a **suggestion for what to do next**. "Use list_files to see available files" gives the model a concrete next step rather than leaving it to guess.

### The Error-as-Information Principle

Across all agents, execution errors share a common design principle: they are returned as tool result content, never as exceptions that crash the agent loop. This ensures:

- The conversation continues even after failures
- The model accumulates error context across multiple attempts
- Error patterns become part of the model's learning within the session
- The agent can track error frequency and intervene if needed

---

## Timeout Handling

Long-running tools can stall the entire agent loop. Every serious coding agent implements timeout management, but strategies vary significantly.

### Timeout Configuration Across Agents

| Agent | Default Timeout | Maximum | Termination Behavior |
|-------|----------------|---------|----------------------|
| OpenCode | 60s | 10 min | End process, return partial output |
| OpenHands | Configurable | Per-parameter | Force terminate sandbox command |
| Codex | Configurable | Per-policy | Terminate + detailed report |
| mini-SWE-agent | Configurable | Per-config | `subprocess.run` timeout kwarg |
| Ante | 30s | Configurable | `ToolError::Timeout` with duration |
| ForgeCode | 300s | `FORGE_TOOL_TIMEOUT` env | Terminate hung commands with signal |
| Goose | Per-extension | 300s default | MCP protocol timeout |
| Aider | None (shell) | User interrupt | Manual Ctrl+C |
| Claude Code | Per-tool | Configurable | Process group termination |
| Cursor | Per-tool | Session-based | Workspace timeout |

### Graceful vs Hard Termination

Two strategies emerge for handling timeouts:

**Graceful shutdown** sends SIGTERM first, waits briefly, then escalates to SIGKILL. This allows processes to clean up temporary files, flush buffers, and release locks:

```python
# Graceful timeout pattern
try:
    result = subprocess.run(cmd, timeout=default_timeout)
except subprocess.TimeoutExpired:
    process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=5)  # Grace period
    except subprocess.TimeoutExpired:
        process.terminate()  # Force stop after grace period
```

**Hard termination** sends SIGKILL immediately. This is faster but risks leaving zombie processes, locked files, or corrupted state. Agents running in disposable sandboxes (like Codex) favor this approach since the entire sandbox can be reset.

### Reporting Timeout Errors

The timeout message to the model must include enough context for correction:

```
Command timed out after 60 seconds: `npm test`
Partial output (last 50 lines):
  PASS src/auth.test.js (12.5s)
  PASS src/api.test.js (8.2s)
  RUNNING src/integration.test.js...

Consider: Run specific test files instead of the full suite.
```

Including partial output helps the model understand what was happening when the timeout hit, enabling smarter retries.

---

## Output Truncation

Long outputs are a silent destroyer of agent performance. A single `cat` of a large file or a verbose test run can consume the entire context window, leaving no room for reasoning.

### OpenCode: Balanced Truncation

OpenCode keeps the first and last halves of output, preserving both the beginning (which often contains headers and initial errors) and the end (which contains summaries and final status):

```go
const MaxOutputLength = 30000

func truncateOutput(content string) string {
    if len(content) <= MaxOutputLength {
        return content
    }
    halfLength := MaxOutputLength / 2
    lines := strings.Split(content, "\n")
    truncatedLines := len(lines) - countLines(content[:halfLength]) -
        countLines(content[len(content)-halfLength:])

    return fmt.Sprintf("%s\n\n... [%d lines truncated] ...\n\n%s",
        content[:halfLength],
        truncatedLines,
        content[len(content)-halfLength:])
}
```

The 30,000 character limit balances information preservation with context budget management.

### mini-SWE-agent: Head + Tail with Explicit Elision

mini-SWE-agent uses a template-based approach with clear structural markers:

```xml
<output_head>{{ output.output[:5000] }}</output_head>
<elided_chars>{{ elided_chars }} characters elided</elided_chars>
<output_tail>{{ output.output[-5000:] }}</output_tail>
```

The 10,000 character total limit (5,000 head + 5,000 tail) is more aggressive than OpenCode's approach, reflecting mini-SWE-agent's focus on smaller models where context is more precious.

### ForgeCode: Truncation Signals in Content

ForgeCode embeds truncation markers directly in the output text rather than in metadata:

```
[OUTPUT TRUNCATED - showing first 2000 and last 2000 of 45000 characters]

--- BEGIN OUTPUT ---
npm test
> project@1.0.0 test
> jest --verbose
...

--- TRUNCATED (41000 characters omitted) ---

...
Test Suites: 3 passed, 1 failed, 4 total
Tests:       47 passed, 2 failed, 49 total
--- END OUTPUT ---
```

This design reflects a critical insight: **models reliably read content but often miss metadata fields**. Putting truncation information inline in the text ensures the model knows it's seeing incomplete output.

### Why Truncation Strategy Matters

Different truncation strategies serve different purposes:

| Strategy | Preserves | Loses | Best For |
|----------|-----------|-------|----------|
| Head only | Initial errors, headers | Final status | Build logs |
| Tail only | Final status, summary | Initial context | Test results |
| Head + Tail | Both ends | Middle context | General purpose |
| Sampled | Distribution | Continuity | Very long outputs |
| Summarized | Semantics | Raw data | Structured output |

The head + tail approach dominates because most useful information appears at the boundaries of output — error messages at the top, summaries at the bottom.

---

## Retry Strategies

When errors occur, agents need strategies beyond simple "try again." The most effective agents implement intelligent retry mechanisms that change their approach based on the failure type.

### Codex: Sandbox Escalation Retry

Codex implements a unique retry strategy tied to its sandbox permission model. When a tool fails due to sandbox restrictions, it can automatically retry with elevated permissions:

```rust
// Codex sandbox escalation pattern
match self.run_attempt(tool, req, current_sandbox).await {
    Ok(result) => Ok(result),
    Err(SandboxErr::Denied { action, .. }) if tool.escalate_on_failure() => {
        // Permission denied — retry with escalated sandbox
        let escalated = current_sandbox.escalate_for(action);
        self.run_attempt(tool, req, escalated).await
    }
    Err(e) => {
        // Non-permission error — report to model
        Ok(ToolResult::error(format!("Tool failed: {}", e)))
    }
}
```

This is particularly elegant because it handles the common case where a tool needs slightly more permissions than initially granted, without requiring the model to understand sandbox policy details.

### ForgeCode: Tool-Call Correction Layer

ForgeCode's correction layer is arguably the highest-impact innovation in agent error handling. It operates as a middleware between the model's tool calls and actual execution:

```
┌─────────┐     ┌──────────────┐     ┌───────────┐
│  Model   │────>│  Correction  │────>│   Tool    │
│  Output  │     │    Layer     │     │ Execution │
└─────────┘     └──────────────┘     └───────────┘
                       │
                 3 failure classes:
                 1. Wrong tool → redirect
                 2. Wrong args → repair
                 3. Wrong order → resequence
```

The correction layer handles three classes of failures:

1. **Wrong tool selected**: The model calls `write_file` when it should call `str_replace`. The layer detects the intent and redirects to the correct tool.
2. **Correct tool, wrong arguments**: The model provides a relative path when an absolute path is required. The layer repairs the argument.
3. **Correct call, wrong sequencing**: The model tries to edit a file before reading it. The layer enforces the read-then-edit ordering.

A critical safeguard: `max_tool_failure_per_turn: 3` prevents infinite correction loops. After three failed corrections in a single turn, the error is returned to the model for manual resolution.

### mini-SWE-agent: Format Error Correction

mini-SWE-agent handles a common failure mode — the model generating incorrectly formatted tool calls — with corrective template messages:

```python
# When the model's output doesn't match expected format
corrective_message = (
    "Your previous response was not in the correct format.\n"
    "Tool calls must use this exact structure:\n\n"
    "<tool_name>\n"
    "<param_name>value</param_name>\n"
    "</tool_name>\n\n"
    "Please try again with the correct format."
)
```

This approach works because format errors are the most common failure mode with smaller models. By providing explicit correction templates, mini-SWE-agent gets the model back on track without wasting attempts.

### Goose: Repetition Detection

Goose implements a `RepetitionInspector` that detects when the model is stuck in a loop — calling the same tool repeatedly without making progress:

```
RepetitionInspector Logic:
1. Track tool calls within a sliding window
2. If same tool called N times with similar arguments:
   - Return DECLINED_RESPONSE
   - Inject message: "You appear to be repeating the same action.
     Please try a different approach."
3. Reset counter on successful progress
```

This is a meta-level retry strategy — it doesn't retry the failing operation but instead forces the model to try something different entirely.

### Aider: Progressive Fuzzy Matching

Aider's `str_replace` implementation uses progressively relaxed matching when exact matches fail:

```
Match Progression:
1. Exact match → apply directly
2. Strip trailing whitespace → retry match
3. Ignore blank lines → retry match
4. Normalize all whitespace → retry match
5. All fail → report error to model with context
```

Each relaxation level catches a different class of model error — trailing spaces, extra blank lines, inconsistent indentation. This dramatically reduces false-negative matches without sacrificing precision.

---

## Feedback Loops: Error → Model → Corrected Call

The most sophisticated error handling doesn't just report failures — it creates tight feedback loops where errors lead to immediate, actionable corrections.

### LSP Integration for Immediate Feedback

Several agents integrate with Language Server Protocol to get instant feedback after edits:

```
Edit Flow with LSP:
1. Model generates code edit
2. Agent applies edit to file
3. Agent waits for LSP diagnostics (100-500ms)
4. Diagnostics returned as tool result:
   "Edit applied. LSP reports 2 issues:
    - Line 15: Type 'string' is not assignable to type 'number'
    - Line 23: Property 'foo' does not exist on type 'Bar'"
5. Model immediately corrects in next turn
```

OpenCode and OpenHands both implement this pattern. The model gets type errors and warnings without running a separate build step, dramatically tightening the feedback loop.

### Warp: Active AI Error Monitoring

Warp takes a different approach — monitoring terminal output in real-time and proactively suggesting fixes when errors are detected. Rather than waiting for the model to request help, Warp identifies error patterns and offers one-click corrections.

### Droid: CI Failure Analysis

Droid's `github_action_repair` tool represents the most automated feedback loop — it analyzes CI/CD failures, identifies the root cause, and creates repair pull requests:

```
CI Failure Loop:
1. GitHub Action fails → webhook triggers Droid
2. Droid fetches failure logs
3. Analyzes error patterns against known fixes
4. Generates repair PR with fix
5. Runs CI again to verify
```

### Junie CLI: Structured Test Parsing

Junie CLI parses test results across 13+ frameworks, extracting structured data that the model can act on:

```
Extracted fields per test:
- test_name: "should authenticate user with valid token"
- file: "src/auth.test.ts"
- line: 47
- status: FAILED
- error_type: AssertionError
- expected: "{ authenticated: true }"
- actual: "{ authenticated: false }"
- stack_trace: [truncated to relevant frames]
```

This structured extraction means the model doesn't have to parse raw test output — it gets precisely the information needed to fix the failing test.

### Test Execution as Error Detection

Most agents follow a common post-edit pattern:

```
1. Model makes code changes
2. Agent runs relevant tests
3. Test failures returned as tool results
4. Model reads failures and corrects code
5. Repeat until tests pass (with attempt limit)
```

This creates a natural TDD-like feedback loop where the test suite serves as an automated oracle for correctness.

---

## Comparison Table

| Agent | Validation Strategy | Timeout | Retry Strategy | Output Handling | Feedback Loop |
|-------|-------------------|---------|----------------|-----------------|---------------|
| **OpenCode** | `IsError: true` in ToolResult | 60s default, 10min max | Error returned to LLM | Head+tail, 30K char limit | LSP diagnostics after edits |
| **OpenHands** | Instructional error messages | Configurable per-param | Error as tool content | Configurable truncation | LSP integration, test runs |
| **Codex** | Sandbox policy errors | Per-policy configurable | Sandbox escalation retry | Policy-based truncation | Sandbox re-execution |
| **Claude Code** | Tool-specific validation | Per-tool timeout | Model-driven retry | Context-aware truncation | Built-in test running |
| **Cursor** | IDE-integrated validation | Session-based | Model-guided correction | IDE output management | Real-time LSP feedback |
| **Aider** | Progressive fuzzy matching | User interrupt (Ctrl+C) | 4-level match relaxation | Git-diff based output | Linter + test integration |
| **mini-SWE-agent** | Format correction templates | subprocess.run timeout | Corrective messages | Head+tail, 10K char limit | Test execution loop |
| **Ante** | Typed `ToolError` enum (Rust) | 30s default, configurable | Result-based error flow | Structured truncation | Compile-time error paths |
| **ForgeCode** | Pre-execution correction layer | 300s, `FORGE_TOOL_TIMEOUT` | 3-class tool correction | Inline truncation markers | Per-tool micro-evaluations |
| **Goose** | MCP protocol validation | 300s per-extension | RepetitionInspector | Extension-managed | Plugin-based feedback |
| **Droid** | GitHub API validation | CI-timeout aware | `github_action_repair` | Log analysis + truncation | CI failure → repair PR loop |
| **Warp** | Terminal output monitoring | Session-based | AI-suggested corrections | Terminal-native display | Real-time error monitoring |
| **Junie CLI** | Multi-framework parsing | Per-framework timeout | Structured retry with context | 13+ framework parsers | Structured test result extraction |
| **Cline** | VSCode integration | Editor-based | Model-driven retry | Editor output management | Diagnostic integration |
| **Roo Code** | Mode-specific validation | Per-mode timeout | Mode-switching on failure | Mode-aware truncation | Multi-mode feedback |
| **Devin** | Cloud sandbox validation | Cloud-managed | Infrastructure retry | Cloud output management | Full environment feedback |
| **Amazon Q** | AWS service validation | Service-dependent | Service-level retry | AWS log integration | CloudWatch integration |
| **Windsurf** | Cascade-integrated | Flow-based | Cascade retry logic | Flow-managed output | Cascade feedback loop |

---

## Key Design Patterns

### Pattern 1: Error Messages as Instructions

The most effective error messages don't just describe what went wrong — they tell the model what to do differently:

```
Bad:  "Error: file not found"
Good: "Error: file 'src/auth.ts' not found. Available files in src/:
       index.ts, config.ts, utils.ts. Did you mean one of these?"
```

### Pattern 2: Fail Fast, Recover Faster

Validation before execution prevents wasted computation. Correction before validation prevents wasted round-trips. The evolution:

```
Generation 1: Execute → Fail → Report → Model retries
Generation 2: Validate → Fail → Report → Model retries
Generation 3: Validate → Correct → Execute → Succeed
```

ForgeCode's correction layer represents Generation 3 — errors are fixed before they happen.

### Pattern 3: Bounded Retry with Escalation

Every retry mechanism needs bounds to prevent infinite loops:

```
Attempt 1: Try with current parameters
Attempt 2: Try with relaxed matching (Aider)
Attempt 3: Try with elevated permissions (Codex)
Attempt N (max): Report comprehensive error to model
```

The bound prevents resource waste while the escalation maximizes success probability.

### Pattern 4: Errors as Context, Not Exceptions

The universal principle across all agents: errors flow through the same channel as successful results. They are content for the model to read, not exceptions that break the agent loop. This architectural decision enables the self-correcting behavior that makes coding agents useful.

---

## Summary

Error handling in coding agents is not about preventing failures — it's about making failures productive. The best agents treat every error as a learning opportunity within the session, surfacing clear information that enables the model to self-correct. The evolution from simple error reporting to pre-execution correction layers represents one of the most important advances in agent reliability.
