---
title: "Permission Systems"
---

# Permission Systems

How coding agents gate tool access — from simple user confirmation to sophisticated multi-tier inspection pipelines.

---

## 1. Why Permission Systems Matter

Coding agents operate by translating LLM-generated plans into concrete actions: editing files,
executing shell commands, making network requests, and interacting with external services. Every
one of these actions carries real-world consequences. Without permission systems, a single
hallucinated command — `rm -rf /`, `git push --force`, or `curl | bash` from a malicious
URL — can destroy data, corrupt repositories, or compromise security.

The fundamental tension is between **autonomy** and **safety**. Developers want agents that
work independently without constant interruption, but they also need guarantees that the agent
will not take destructive or unauthorized actions.

### The Autonomy Spectrum

```
Fully Manual          Semi-Automated           Fully Autonomous
     |                      |                        |
Every action        Approve categories           Agent decides
needs approval      of actions once              everything
     |                      |                        |
High safety         Balanced                     High speed
Low speed           trade-off                    Low safety
```

Most production agents sit in the middle — they pre-approve safe operations (reading files,
running tests) while gating dangerous ones (deleting files, network access, system commands).

### Trust Bootstrapping

How do you trust an agent with increasing autonomy? The pattern across agents is remarkably
consistent:

1. **Start restrictive** — require approval for everything
2. **Build trust through observation** — user sees the agent making good decisions
3. **Selectively grant autonomy** — pre-approve specific tools or command patterns
4. **Maintain kill switches** — even in autonomous mode, certain actions remain gated

This mirrors how organizations onboard human engineers: limited access initially, expanded
permissions as trust is established, but always with audit trails and hard limits.

The permission system is the **primary security boundary** between an LLM's generated intent
and real-world execution. Get it wrong, and you have an uncontrolled agent with shell access.

---

## 2. Claude Code's 5 Permission Modes

Claude Code implements one of the most sophisticated permission systems among coding agents,
with five distinct operating modes and a layered rule evaluation engine.

### Permission Modes

| Mode               | Behavior                                               | Use Case                        |
|--------------------|---------------------------------------------------------|---------------------------------|
| `default`          | Prompts for first use of each tool                      | Normal interactive development  |
| `acceptEdits`      | Auto-accepts file edits; asks for Bash commands         | Trusted editing, cautious exec  |
| `plan`             | Read-only — analyze and suggest but never modify        | Code review, architecture       |
| `dontAsk`          | Auto-denies unless pre-approved via allowlist           | CI/CD, headless operation       |
| `bypassPermissions`| Skips all prompts (requires `--dangerously-skip-permissions`) | Sandboxed environments only |

The `default` mode tracks which tools the user has already approved during a session, so the
same tool does not prompt repeatedly. The `dontAsk` mode is designed for non-interactive
contexts where there is no human to respond to prompts — everything not explicitly allowed
is denied.

### Permission Rule Syntax

Rules use tool-specific glob patterns:

```json
{
  "permissions": {
    "allow": [
      "Bash(npm run *)",
      "Bash(npx jest *)",
      "Bash(git status)",
      "Bash(git diff *)",
      "Read(src/**)",
      "Read(tests/**)",
      "Write(src/**)",
      "WebFetch(domain:docs.example.com)",
      "WebFetch(domain:api.github.com)"
    ],
    "deny": [
      "Bash(rm -rf *)",
      "Bash(curl * | bash)",
      "Bash(sudo *)",
      "Write(.env*)",
      "WebFetch(domain:*.malware.com)"
    ]
  }
}
```

Each rule specifies a **tool name** and a **pattern** for its arguments. The glob patterns
support `*` (single segment) and `**` (recursive) matching.

### Evaluation Order

Claude Code evaluates permission rules in a strict priority order:

```
deny rules → ask rules → allow rules
```

**First match wins.** If a deny rule matches, the action is blocked regardless of any allow
rules that might also match. This ensures that safety-critical denials cannot be accidentally
overridden by overly broad allow patterns.

### Configuration Hierarchy

Settings are managed at three levels with a clear override chain:

```
Enterprise Managed Settings (highest priority)
    ↓ cannot be overridden
Project Settings (.claude/settings.json)
    ↓ can be overridden by
User Settings (~/.claude/settings.json)
```

**Managed settings** are deployed by enterprise administrators and cannot be overridden by
individual users or project configurations. This allows organizations to enforce security
policies across all developers.

### Example `.claude/settings.json`

```json
{
  "permissions": {
    "allow": [
      "Bash(npm run build)",
      "Bash(npm run test)",
      "Bash(npm run lint)",
      "Bash(npx prettier --write *)",
      "Read(**)",
      "Write(src/**)",
      "Write(tests/**)"
    ],
    "deny": [
      "Bash(npm publish *)",
      "Bash(git push *)",
      "Write(package-lock.json)",
      "Write(.github/**)"
    ]
  },
  "mode": "acceptEdits"
}
```

This configuration allows the agent to freely read all files, edit source and test files,
run build/test/lint commands, but blocks publishing, pushing, and modifying CI configuration
or the lockfile. File edits are auto-accepted while bash commands follow the allow/deny rules.

---

## 3. Codex's Execution Policy Engine

OpenAI's Codex CLI takes a distinctly systems-programming approach to permissions, implementing
a Rust-based execution policy engine with deep shell command parsing.

### Core Decision Type

```rust
pub enum Decision {
    Allow,    // Execute without prompting
    Prompt,   // Ask user before executing
    Forbidden // Block entirely
}
```

Every command evaluation resolves to one of these three outcomes. There is no ambiguity —
the policy engine always produces a deterministic decision.

### Four Approval Levels

Codex defines four escalating levels of trust:

| Level         | Description                                                 |
|---------------|-------------------------------------------------------------|
| `untrusted`   | Ask before every action (maximum safety)                    |
| `on-failure`  | Auto-approve unless command fails, then ask before retry    |
| `on-request`  | Auto-approve most actions, ask only for flagged categories  |
| `never`       | Never ask — auto-approve everything (sandbox required)      |

The `on-failure` level is particularly interesting: it assumes the agent's commands are
generally correct, but if something fails, it pauses to let the human assess before the
agent retries (preventing infinite failure loops).

### Shell Command Parser

The shell command parser is one of the most complex components in the Codex codebase —
at approximately 84KB, it is the largest single file. It needs to decompose arbitrarily
complex shell expressions into individual commands for per-command policy evaluation.

```bash
# This compound command:
cd /tmp && npm install && npm run build | tee build.log; echo "done"

# Gets decomposed into individual evaluations:
# 1. cd /tmp          → evaluate against policy
# 2. npm install      → evaluate against policy
# 3. npm run build    → evaluate against policy
# 4. tee build.log    → evaluate against policy (writes file)
# 5. echo "done"      → evaluate against policy
```

Each sub-command is independently evaluated. If any sub-command in a compound expression
is `Forbidden`, the entire expression is blocked. If any is `Prompt`, the user is asked
about the full expression.

### Rule Matching

Rules match by **program name** and **argument prefix patterns**:

```rust
struct Rule {
    program: String,         // e.g., "npm", "git", "python"
    arg_prefix: Vec<String>, // e.g., ["run", "test"] matches "npm run test ..."
    decision: Decision,
}
```

### Special Argument Types

Codex recognizes semantic argument types for fine-grained control:

| Argument Type    | Meaning                              | Example                    |
|------------------|--------------------------------------|----------------------------|
| `ARG_RFILES`    | Arguments that are read file paths   | `cat FILE`, `grep PATTERN FILE` |
| `ARG_WFILE`     | Arguments that are write file paths  | `tee FILE`, `cp SRC DEST` |
| `ARG_SED_COMMAND`| sed expressions (can modify files)  | `sed -i 's/old/new/' FILE` |

This allows the policy engine to understand not just *what program* is being run, but
*what it is doing* — reading vs writing, and which files are affected.

### Sandbox Escalation

When a command fails due to sandbox restrictions (e.g., network access denied), Codex
can offer **sandbox escalation**:

```
Command failed: npm install axios
Reason: Network access blocked by sandbox

Options:
  [1] Allow network access for this command
  [2] Allow network access for npm install
  [3] Skip this command
  [4] Abort
```

This provides a just-in-time permission upgrade without requiring the user to preconfigure
every possible network dependency.

### Example Policy Configuration

```toml
[policy]
approval_level = "on-request"

[[rules]]
program = "npm"
args = ["run", "*"]
decision = "allow"

[[rules]]
program = "npm"
args = ["install", "*"]
decision = "prompt"

[[rules]]
program = "git"
args = ["status"]
decision = "allow"

[[rules]]
program = "git"
args = ["push", "*"]
decision = "forbidden"

[[rules]]
program = "rm"
args = ["-rf", "*"]
decision = "forbidden"
```

---

## 4. Goose's 4-Tier Inspection Pipeline

Goose implements the most defense-in-depth permission architecture among coding agents.
Every tool call passes through **all four inspectors sequentially**, even when running
in autonomous mode. This is not a single gate — it is a pipeline.

### Pipeline Architecture

```
                    Tool Call
                       │
                       ▼
            ┌─────────────────────┐
            │  SecurityInspector  │──── BLOCK ──→ Action Denied
            │  (pattern matching) │
            └──────────┬──────────┘
                       │ PASS
                       ▼
            ┌─────────────────────┐
            │ AdversaryInspector  │──── BLOCK ──→ Action Denied
            │ (injection detect)  │
            └──────────┬──────────┘
                       │ PASS
                       ▼
            ┌─────────────────────┐
            │ PermissionInspector │──── BLOCK ──→ User Prompt
            │ (per-tool rules)    │
            └──────────┬──────────┘
                       │ PASS
                       ▼
            ┌─────────────────────┐
            │ RepetitionInspector │──── BLOCK ──→ Declined Response
            │  (loop detection)   │
            └──────────┬──────────┘
                       │ PASS
                       ▼
                  Tool Executes
```

### Tier 1: SecurityInspector

The SecurityInspector pattern-matches tool arguments against known dangerous commands:

```python
DANGEROUS_PATTERNS = [
    r"rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|--force\s+).*(/|\*)",  # rm -rf / or rm -rf *
    r"mkfs\.",                                                  # filesystem formatting
    r"dd\s+.*of=/dev/",                                        # raw disk writes
    r"chmod\s+(-[a-zA-Z]*R[a-zA-Z]*\s+)?777",                # world-writable perms
    r"curl\s+.*\|\s*(bash|sh|zsh)",                            # pipe URL to shell
    r"wget\s+.*\|\s*(bash|sh|zsh)",                            # pipe download to shell
    r">\s*/dev/sd[a-z]",                                       # redirect to raw disk
    r":(){ :\|:& };:",                                         # fork bomb
    r"shutdown|reboot|halt|poweroff",                           # system control
]
```

These patterns catch the most catastrophic commands regardless of what the permission
system says. Even in fully autonomous mode, `rm -rf /` is blocked.

### Tier 2: AdversaryInspector

The AdversaryInspector looks for prompt injection attacks embedded in tool arguments.
This catches cases where malicious content in files or web pages tries to hijack the
agent:

```python
INJECTION_PATTERNS = [
    r"ignore\s+(all\s+)?previous\s+instructions",
    r"you\s+are\s+now\s+a\s+different\s+AI",
    r"system\s*:\s*you\s+must",
    r"<\|im_start\|>system",
    r"ADMIN\s*OVERRIDE",
    r"IMPORTANT:\s*ignore\s+the\s+above",
]
```

This is particularly important when the agent processes untrusted input — reading files
from unknown repositories, fetching web content, or processing user-provided data that
might contain embedded instructions.

### Tier 3: PermissionInspector

The PermissionInspector implements four per-tool permission levels:

| Level        | Behavior                                                    |
|--------------|-------------------------------------------------------------|
| `Autonomous` | Always allowed without prompting                            |
| `Manual`     | Always requires explicit user confirmation                  |
| `Smart`      | Uses heuristics to decide (safe patterns auto-approve)      |
| `Chat-Only`  | Tool cannot be invoked by the agent, only by user directly  |

The `Smart` level is unique to Goose. It applies heuristics based on the specific tool
and arguments — for example, `git status` might be auto-approved under Smart mode, while
`git push --force` would require confirmation.

The `Chat-Only` level prevents the agent from using certain tools entirely, reserving them
for direct user invocation. This is useful for tools that should only be triggered by
explicit human intent (e.g., deployment tools).

### Tier 4: RepetitionInspector

The RepetitionInspector solves one of the most common agent failure modes: infinite retry
loops. When an agent encounters an error, it often tries the same command again with
minor variations, getting stuck in a cycle.

```python
class RepetitionInspector:
    def __init__(self, max_repeats=3, window_size=10):
        self.recent_calls = deque(maxlen=window_size)
        self.max_repeats = max_repeats

    def inspect(self, tool_call):
        signature = self.normalize(tool_call)
        count = self.recent_calls.count(signature)
        self.recent_calls.append(signature)

        if count >= self.max_repeats:
            return InspectionResult.DECLINED_RESPONSE
        return InspectionResult.PASS
```

When repetition is detected, the inspector returns `DECLINED_RESPONSE` — a special signal
that tells the LLM the action was declined and it should try a fundamentally different
approach. This breaks the retry loop by forcing the agent to reconsider.

---

## 5. OpenCode's Permission System

OpenCode takes a pragmatic approach to permissions, focused on keeping the agent loop
running smoothly while gating genuinely dangerous operations.

### Permission Blocking

Tools that are marked as requiring permission **block the agent loop** until the user
responds. This is a hard stop — the agent cannot proceed with any other actions until
the permission decision is made.

```go
type PermissionRequest struct {
    ToolName    string
    Arguments   map[string]interface{}
    Description string
}

type PermissionResponse int

const (
    AllowOnce    PermissionResponse = iota  // Allow this specific invocation
    AllowSession                             // Allow this tool for rest of session
    Deny                                     // Block this invocation
    AutoApprove                              // Non-interactive: approve everything
)
```

### User Options

When a permission prompt appears, users have four choices:

1. **Allow Once** — permits this specific tool call only
2. **Allow for Session** — permits all future calls to this tool during the current session
3. **Deny** — blocks this specific tool call
4. **Auto-Approve** — (non-interactive mode) approves all tool calls without prompting

### Banned Commands

The Bash tool maintains an explicit blocklist of commands that are never allowed:

```go
var BannedCommands = []string{
    "curl", "wget",           // Network downloads
    "nc", "netcat", "ncat",   // Network connections
    "ssh", "scp", "sftp",     // Remote access
    "ftp", "tftp",            // File transfer
    "telnet",                 // Remote access
    "nmap",                   // Network scanning
    "dig", "nslookup",        // DNS queries
    "traceroute", "mtr",      // Network diagnostics
}
```

### Safe Command Whitelist

Conversely, certain read-only commands are always permitted:

```go
var SafeCommands = []string{
    "ls", "cat", "head", "tail",    // File reading
    "find", "grep", "rg",           // Searching
    "wc", "sort", "uniq",           // Text processing
    "pwd", "echo", "date",          // System info
    "git status", "git log",        // Git read operations
    "git diff", "git show",         // Git read operations
}
```

### Permission Service

The permission service sits between the tool executor and the UI layer:

```
Agent Loop → Tool Executor → Permission Service → UI Prompt → User Decision
                                     ↓
                              Session Cache
                           (remembered decisions)
```

Session-level approvals are cached so the user does not have to approve the same tool
repeatedly within a single coding session.

---

## 6. Gemini CLI's Multi-Tier Permissions

Gemini CLI organizes permissions around the **type of operation** rather than the specific
tool, creating a clean three-tier hierarchy.

### Permission Tiers

| Tier   | Operations                          | Permission Required |
|--------|-------------------------------------|---------------------|
| Tier 0 | Read file, list directory, search   | None                |
| Tier 1 | Write file, edit, delete, move      | Explicit approval   |
| Tier 2 | Computer Use (screen, keyboard)     | Highest permission  |

This tiered approach means that exploratory operations — reading code, searching for
patterns, listing directories — never interrupt the agent's flow. Only operations that
modify state require human approval.

### Tool Invocation Priority

Gemini CLI also defines a priority order for tool selection, preferring safer and more
targeted tools:

```
1. Built-in file tools     (read/write/edit files directly)
2. Shell execution         (bash commands)
3. LSP integration         (language server features)
4. Web search/fetch        (internet access)
5. MCP tools               (external tool servers)
6. Computer Use            (screen/keyboard control)
```

Lower-priority tools are only used when higher-priority tools cannot accomplish the task.
This implicitly reduces the frequency of permission prompts by preferring tools that
require fewer permissions.

### Configuration

Gemini CLI uses JSON-based configuration for permission rules:

```json
{
  "tools": {
    "shell": {
      "allowedCommands": ["npm run *", "git status", "git diff"],
      "blockedCommands": ["rm -rf *", "sudo *"],
      "requireConfirmation": true
    },
    "fileSystem": {
      "readOnly": ["node_modules/**", ".git/**"],
      "writable": ["src/**", "tests/**"],
      "blocked": [".env", "*.key", "*.pem"]
    },
    "network": {
      "allowedDomains": ["docs.google.com", "github.com"],
      "blocked": true
    }
  }
}
```

### Sandbox Integration

Gemini CLI supports sandbox environments where permissions are relaxed:

- In sandbox mode, Tier 1 operations (writes) are auto-approved
- Tier 2 operations (Computer Use) still require explicit permission
- Network access follows sandbox-specific policies
- Container isolation provides the safety net instead of permission prompts

---

## 7. Junie CLI's 4-Level Permission System

JetBrains' Junie CLI implements a numbered level system that maps directly to risk severity.

### Permission Levels

| Level | Name              | Behavior                        | Examples                          |
|-------|-------------------|---------------------------------|-----------------------------------|
| L0    | No confirmation   | Always allowed silently         | Read file, list dir, search code  |
| L1    | First-use confirm | Ask once, then remember         | Write file, run tests             |
| L2    | Always confirm    | Ask every time                  | Delete file, run arbitrary shell  |
| L3    | Explicit opt-in   | Disabled by default             | Network access, system commands   |

### Level Progression

The levels create a clear mental model for users:

```
L0: Safe operations (read-only)
 ↓ increasing risk
L1: Moderate operations (first-use gate)
 ↓ increasing risk
L2: Dangerous operations (always gated)
 ↓ increasing risk
L3: Sensitive operations (disabled by default)
```

### Security Pattern Filtering

Junie applies regex-based pattern filtering at L2 and L3 to catch dangerous commands
even within allowed tool categories:

```kotlin
val securityPatterns = listOf(
    Pattern.compile("rm\\s+-[a-z]*r[a-z]*f"),     // recursive force delete
    Pattern.compile("chmod\\s+777"),                // world-writable
    Pattern.compile("eval\\s*\\("),                 // dynamic code execution
    Pattern.compile("exec\\s*\\("),                 // process execution
    Pattern.compile(">\\.env"),                     // overwrite env file
    Pattern.compile("curl.*\\|.*sh"),               // pipe to shell
)
```

Commands matching these patterns are automatically escalated to L2 (always confirm)
regardless of their original level assignment. This provides a safety net against
seemingly benign tools being used in dangerous ways.

### IDE Integration

Being a JetBrains product, Junie integrates permission prompts directly into the IDE:

- Permission dialogs appear as IDE notifications, not terminal prompts
- Users can configure permission levels per-project in IDE settings
- L3 operations show a security warning dialog with detailed risk explanation
- All permission decisions are logged in the IDE's event log

---

## 8. Capability-Based Security Model

### Theory: Capabilities vs ACLs

Traditional Access Control Lists (ACLs) associate permissions with **subjects** (users,
processes). Capability-based security instead associates permissions with **tokens** that
grant specific access rights. The holder of a capability can exercise it; no capability
means no access.

In the agent context:

- **ACL approach**: "Agent X can use tools A, B, C" — permissions tied to identity
- **Capability approach**: "This token grants write access to src/**" — permissions are
  transferable, composable, and revocable

### Application to Agent Tools

Most coding agents implicitly use a capability-like model:

```
Session Start
    │
    ├── Agent receives: read capability (all files)
    ├── Agent receives: write capability (src/**)
    ├── Agent receives: execute capability (npm run *)
    │
    └── Agent does NOT receive: network capability
        Agent does NOT receive: system capability
```

The agent can only exercise capabilities it has been granted. New capabilities can be
granted at runtime (user approves a new tool), and capabilities can be revoked (user
changes configuration mid-session).

### Principle of Least Privilege

The most secure agents follow least privilege strictly:

1. Start with **no capabilities** (or read-only)
2. Grant capabilities **only as needed** for the current task
3. **Revoke capabilities** when no longer needed
4. **Scope capabilities narrowly** (specific files, specific commands)

This is why Claude Code's `plan` mode and Goose's `Chat-Only` level exist — they
provide a way to run agents with minimal capabilities for tasks that do not require them.

---

## 9. Rule Syntax and Configuration

Different agents use different syntaxes for expressing permission rules. Here is a
comparison of the major approaches.

### Glob Patterns (Claude Code)

```
Tool(pattern)

Bash(npm run *)           # match npm run with any argument
Read(src/**)              # match any file under src/ recursively
Write(src/**/*.ts)        # match TypeScript files under src/
WebFetch(domain:*.com)    # match any .com domain
```

**Pros**: Familiar to developers, expressive, readable.
**Cons**: Limited to pattern matching, cannot express complex conditions.

### Regex Patterns (Goose SecurityInspector)

```python
patterns = [
    r"rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|--force\s+).*(/|\*)",
    r"curl\s+.*\|\s*(bash|sh)",
]
```

**Pros**: Maximum expressiveness, can match complex patterns.
**Cons**: Hard to read, easy to get wrong, potential for ReDoS.

### Program + Args Matching (Codex)

```toml
[[rules]]
program = "npm"
args = ["run", "test"]
decision = "allow"
```

**Pros**: Structured, unambiguous, easy to validate.
**Cons**: Verbose, limited to exact prefix matching.

### YAML Configuration (mini-SWE-agent style)

```yaml
permissions:
  tools:
    file_read:
      level: autonomous
      paths: ["**/*.py", "**/*.md"]
    file_write:
      level: manual
      paths: ["src/**"]
      blocked_paths: ["src/config/**"]
    shell:
      level: manual
      allowed: ["python -m pytest *", "pip install *"]
      blocked: ["rm *", "curl *"]
    network:
      level: disabled
```

**Pros**: Human-readable, hierarchical, supports complex configurations.
**Cons**: YAML parsing edge cases, no standard schema.

### JSON Configuration (Claude Code, Gemini CLI)

```json
{
  "permissions": {
    "allow": ["Bash(npm run *)"],
    "deny": ["Bash(rm -rf *)"]
  }
}
```

**Pros**: Universal format, strict parsing, easy tooling.
**Cons**: Verbose, no comments (unless JSONC).

---

## 10. Audit Logging

### Why Audit Matters

For enterprise adoption, knowing *what the agent did* is as important as controlling
*what it can do*. Audit logs provide:

- **Accountability**: trace every action back to a specific agent session
- **Forensics**: investigate incidents after the fact
- **Compliance**: demonstrate that agents operated within approved boundaries
- **Debugging**: understand why an agent made specific decisions

### OpenHands: Event Stream as Audit Log

OpenHands records every Action and Observation in a structured event stream:

```python
# Every action is logged before execution
event_stream.append(ActionEvent(
    action_type="CmdRunAction",
    command="npm run test",
    timestamp="2024-01-15T10:30:00Z",
    permission_decision="allowed",
    permission_source="session_cache"
))

# Every result is logged after execution
event_stream.append(ObservationEvent(
    observation_type="CmdOutputObservation",
    exit_code=0,
    output="All 247 tests passed",
    timestamp="2024-01-15T10:30:05Z"
))
```

This creates a complete, replayable record of everything the agent did.

### Codex: Policy Decision Logging

Codex logs every policy evaluation decision:

```
[POLICY] npm run test → ALLOW (rule: npm run * → allow)
[POLICY] git push origin main → FORBIDDEN (rule: git push * → forbidden)
[POLICY] curl https://api.example.com → PROMPT (no matching rule, default: prompt)
[POLICY] rm -rf node_modules → ALLOW (rule: rm -rf node_modules → allow)
```

Each log entry includes the command, the decision, and which rule produced the decision.
This makes it straightforward to debug why a command was allowed or blocked.

### Best Practices for Audit Logging

1. **Log before and after** — record the intent and the outcome
2. **Include context** — which task, which file, which conversation turn
3. **Log denials** — blocked actions are as important as allowed ones
4. **Structured format** — use JSON or structured logs for queryability
5. **Tamper-resistant** — agent should not be able to modify its own audit logs

---

## 11. Comparison Table

### Permission Model Overview

| Agent       | Architecture        | Modes/Levels | Config Format  | Evaluation Order      |
|-------------|---------------------|-------------|----------------|-----------------------|
| Claude Code | Layered rules       | 5 modes     | JSON           | Deny → Ask → Allow    |
| Codex       | Policy engine       | 4 levels    | TOML           | Per-command evaluation |
| Goose       | 4-tier pipeline     | 4 per-tool  | Python config  | Sequential pipeline   |
| OpenCode    | Allow/deny lists    | 4 responses | Go config      | Blocklist → Safelist  |
| Gemini CLI  | Tiered operations   | 3 tiers     | JSON           | Tier-based            |
| Junie CLI   | Numbered levels     | 4 levels    | Kotlin/IDE     | Level-based + patterns|

### Safety Features

| Agent       | Banned Commands | Network Control | Injection Detection | Loop Detection | Sandbox Support |
|-------------|:--------------:|:---------------:|:-------------------:|:--------------:|:---------------:|
| Claude Code | Via deny rules | Via WebFetch rules | No              | No             | Via bypass mode |
| Codex       | Via forbidden  | Via sandbox     | No                  | Via on-failure | Built-in        |
| Goose       | SecurityInspector | Per-tool     | AdversaryInspector  | RepetitionInspector | No         |
| OpenCode    | Explicit list  | Banned commands | No                  | No             | No              |
| Gemini CLI  | Via config     | Domain allowlist| No                  | No             | Container       |
| Junie CLI   | Pattern filter | L3 opt-in      | No                  | No             | IDE sandbox     |

### Granularity

| Agent       | File-Level | Command-Level | Argument-Level | Domain-Level | Semantic (R/W) |
|-------------|:----------:|:-------------:|:--------------:|:------------:|:--------------:|
| Claude Code | Yes        | Yes           | Via globs      | Yes          | Via tool name   |
| Codex       | Via args   | Yes           | Yes (prefix)   | Via sandbox  | Yes (ARG_RFILES)|
| Goose       | Per-tool   | Per-tool      | Via inspector  | Per-tool     | No              |
| OpenCode    | No         | Yes           | No             | Banned list  | Via safe list   |
| Gemini CLI  | Yes        | Yes           | Via patterns   | Yes          | Yes (tiers)     |
| Junie CLI   | No         | Yes           | Via regex      | L3 gated     | Yes (levels)    |

### Key Differentiators

| Agent       | Unique Strength                                                    |
|-------------|--------------------------------------------------------------------|
| Claude Code | Enterprise managed settings with unoverridable deny rules          |
| Codex       | Deep shell parsing decomposes compound commands for per-cmd policy |
| Goose       | Only agent with prompt injection detection in tool arguments       |
| OpenCode    | Clean session-based permission caching with minimal configuration  |
| Gemini CLI  | Operation-type tiers create intuitive mental model                 |
| Junie CLI   | Deep IDE integration makes permissions feel native to the editor   |

---

## Summary

Permission systems in coding agents have evolved rapidly from simple approve/deny prompts
to sophisticated multi-layered architectures. The key design patterns that have emerged:

1. **Defense in depth** — multiple independent checks (Goose's 4-tier pipeline)
2. **Deny-first evaluation** — safety rules always take priority (Claude Code)
3. **Semantic awareness** — understanding what a command *does*, not just what it *is* (Codex)
4. **Graduated autonomy** — multiple modes from fully manual to fully autonomous
5. **Session memory** — remembering user decisions to reduce prompt fatigue
6. **Audit trails** — logging every decision for accountability

The trend is toward more granular, configurable, and context-aware permission systems that
balance developer productivity with security guarantees. As agents take on more complex
tasks, permission systems will need to evolve further — potentially incorporating runtime
behavior analysis, task-scoped capability grants, and cross-agent permission delegation.
