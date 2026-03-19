# Codex CLI — Tool System & Sandboxing

## Overview

Codex CLI's tool system is the most complex and security-critical part of the
architecture. It combines a **tool router** (dispatching model-generated calls),
a **tool orchestrator** (approval + sandbox + execution pipeline), an
**execution policy engine** (command allowlisting), and **OS-native sandboxing**
(Seatbelt, bubblewrap+seccomp, Windows ACLs).

## Tools Exposed to the Model

### Built-in Tools

| Tool | Type | Purpose |
|---|---|---|
| `shell` / `local_shell` | LocalShellCall | Execute shell commands |
| `apply_patch` | FunctionCall | Apply unified diffs to files |
| `web_search` | WebSearchCall | Search the web (cached/live) |
| `codex` | FunctionCall | Spawn a sub-agent |
| `codex-reply` | FunctionCall | Continue a sub-agent conversation |
| `image_generation` | ImageGenerationCall | Generate images |
| `js_repl` | FunctionCall | JavaScript REPL execution |
| MCP tools | FunctionCall | Tools from connected MCP servers |
| Connector tools | CustomToolCall | ChatGPT app/plugin tools |

### MCP Server Integration

When acting as an **MCP server** (`codex mcp-server`), Codex exposes two tools:

```json
{
  "name": "codex",
  "description": "Run a Codex session",
  "inputSchema": {
    "properties": {
      "prompt": { "type": "string" },
      "model": { "type": "string" },
      "profile": { "type": "string" },
      "cwd": { "type": "string" },
      "approval-policy": {
        "enum": ["untrusted", "on-failure", "on-request", "never"]
      },
      "sandbox": {
        "enum": ["read-only", "workspace-write", "danger-full-access"]
      },
      "config": { "type": "object" },
      "base-instructions": { "type": "string" },
      "developer-instructions": { "type": "string" }
    }
  }
}
```

```json
{
  "name": "codex-reply",
  "description": "Continue a Codex conversation by providing the thread id and prompt",
  "inputSchema": {
    "properties": {
      "threadId": { "type": "string" },
      "prompt": { "type": "string" }
    }
  }
}
```

### Tool Call Flow (MCP Server Mode)

```
MCP client sends tools/call
  → message_processor.rs dispatches to codex_tool_runner.rs
    → run_codex_tool_session() creates CodexThread via ThreadManager
      → Prompt submitted as Op::UserInput
      → Events streamed via next_event() loop:
          ExecApprovalRequest → exec_approval.rs (auto-approve)
          ApplyPatchApprovalRequest → patch_approval.rs
          TurnComplete → send CallToolResult { threadId, content }
          Error → send error response
          Other events → forwarded as MCP notifications
```

## Tool Router

The `ToolRouter` maps model-generated `ResponseItem` variants to executable
`ToolCall` objects:

```rust
pub struct ToolRouter {
    registry: ToolRegistry,
    specs: Vec<ConfiguredToolSpec>,
    model_visible_specs: Vec<ToolSpec>,  // sent to model as available tools
}

impl ToolRouter {
    pub fn build_tool_call(&self, item: &ResponseItem) -> ToolCall {
        match item {
            ResponseItem::FunctionCall { name, .. } => {
                // 1. Check MCP tool registry
                // 2. Fall back to built-in function handlers
            }
            ResponseItem::LocalShellCall { .. } => {
                // Route to shell execution handler
            }
            ResponseItem::CustomToolCall { .. } => {
                // Route to custom/connector tool handler
            }
            ResponseItem::ToolSearchCall { .. } => {
                // Route to tool search handler
            }
            _ => unreachable!()
        }
    }
}
```

## Tool Orchestrator — The Approval → Sandbox → Execute Pipeline

Every tool call goes through a three-phase pipeline:

### Phase 1: Approval Check

```rust
pub enum ExecApprovalRequirement {
    Skip,                                    // Auto-approved
    Forbidden { reason: String },            // Blocked
    NeedsApproval {
        call_id: String,
        command: Vec<String>,
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
        proposed_network_amendments: Option<Vec<NetworkPolicyAmendment>>,
    },
}
```

The approval requirement is determined by:
1. **Execution policy** — static rules matching the command
2. **Approval policy** — user's configured approval level
3. **Sandbox policy** — whether the action exceeds sandbox bounds
4. **Tool annotations** — MCP tools can declare `destructive` hints

### Phase 2: Sandbox Selection

```rust
let initial_sandbox = self.sandbox.select_initial(
    &tool_ctx.sandbox_policy,
    tool.requested_permissions(&req),
);

pub enum SandboxPolicy {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}
```

### Phase 3: Execution with Retry

```rust
// First attempt under selected sandbox
let result = Self::run_attempt(tool, req, initial_sandbox, ..).await;

// On SandboxErr::Denied → retry with escalation
match result {
    Err(SandboxErr::Denied { .. }) if tool.escalate_on_failure() => {
        // Request user approval to retry without sandbox
        let escalated = SandboxAttempt { sandbox: None, .. };
        Self::run_attempt(tool, req, escalated, ..).await
    }
    other => other,
}
```

## Execution Policy Engine

### New Policy Engine (`codex-rs/execpolicy/`)

The modern policy engine uses a three-valued decision system:

```rust
pub enum Decision {
    Allow,     // Command runs without approval
    Prompt,    // Requires explicit user approval
    Forbidden, // Blocked entirely
}

pub struct Policy {
    rules_by_program: MultiMap<String, RuleRef>,
    network_rules: Vec<NetworkRule>,
    host_executables_by_name: HashMap<String, Arc<[AbsolutePathBuf]>>,
}
```

Policy evaluation:
1. Extract the first token (program name) from the command
2. Look up matching rules in `rules_by_program`
3. Run prefix-pattern matching against arguments
4. Fall back to a heuristics function if no rules match
5. Return `Decision` with matched rule references

```rust
pub struct Evaluation {
    pub decision: Decision,
    pub matched_rules: Vec<RuleMatch>,
}

impl Policy {
    pub fn check(&self, cmd: &[String], heuristics_fallback: bool) -> Evaluation;
    pub fn add_prefix_rule(&mut self, prefix: &[String], decision: Decision);
    pub fn add_network_rule(&mut self, host: &str, protocol: NetworkRuleProtocol,
                             decision: Decision, justification: Option<String>);
    pub fn merge_overlay(&self, overlay: &Policy) -> Policy;
    pub fn compiled_network_domains(&self) -> (Vec<String>, Vec<String>);
}
```

### Network Rules

```rust
pub struct NetworkRule {
    pub host: String,
    pub protocol: NetworkRuleProtocol,
    pub decision: Decision,
    pub justification: Option<String>,
}
```

The `compiled_network_domains()` method returns `(allowed, denied)` domain
lists for configuring the sandbox's network proxy.

### Legacy Policy Engine (`codex-rs/execpolicy-legacy/`)

The legacy system uses a Python-like DSL (`default.policy`) with an explicit
command allowlist:

```
# default.policy (simplified)
program ls:
    flags: -1 -a -l -A -F -h -S -t -r -R
    args: ARG_RFILES_OR_CWD

program cat:
    flags: -b -n -t -v
    args: ARG_RFILES

program cp:
    flags: -r -R -p -a
    args: ARG_RFILES ARG_WFILE

program rg:
    flags: --json --count --files --glob --type --hidden ...
    args: ARG_RFILES

program sed:
    args: ARG_SED_COMMAND ARG_RFILES  # blocks 'e' flag (executes as shell)
```

Special argument types:
- `ARG_RFILES` — Read-only file paths
- `ARG_WFILE` — Writable file path (must be in writable root)
- `ARG_RFILES_OR_CWD` — Files or CWD
- `ARG_SED_COMMAND` — sed command with safety check (blocks `e` flag)
- `ARG_OPAQUE_VALUE` — Any string value
- `ARG_POS_INT` — Positive integer

### Shell Command Parser (`codex-rs/shell-command/`)

The largest single file in the codebase is `parse_command.rs` at **84KB** —
a comprehensive shell command parser that decomposes complex shell expressions
(pipes, redirections, subshells, command substitutions) into individual
executable invocations for policy evaluation.

```rust
// shell-command/src/lib.rs
pub fn is_dangerous_command(cmd: &str) -> bool;
pub fn is_safe_command(cmd: &str) -> bool;
```

Supports:
- Bash command parsing (`bash.rs`)
- PowerShell command parsing (`powershell.rs`)
- Shell detection (`shell_detect.rs`)
- Command safety classification (`command_safety/`)

## Sandboxing — Deep Dive

### Linux Sandbox (`codex-rs/linux-sandbox/`)

The Linux sandbox has a **three-layer architecture**:

#### Layer 1: Bubblewrap (Filesystem Isolation — Default)

The primary sandbox creates a mount namespace using bubblewrap:

```bash
bwrap \
  --ro-bind / /                           # FS is read-only by default
  --bind <writable_root> <writable_root>  # explicit writable dirs
  --ro-bind .git .git                     # sensitive subpaths re-applied read-only
  --unshare-user                          # user namespace isolation
  --unshare-pid                           # PID namespace isolation
  --unshare-net                           # network namespace isolation
  --proc /proc                            # fresh /proc mount
```

Protected subpaths (`.git`, `.codex`, resolved `gitdir:`) are always read-only
even when the parent directory is writable. Overlapping split-policy entries
are applied in **path-specificity order**.

The launcher prefers `/usr/bin/bwrap` if present, then falls back to a vendored
bubblewrap binary compiled into the Codex release:

```rust
pub(crate) fn exec_bwrap(argv: Vec<String>, preserved_files: Vec<File>) -> ! {
    match preferred_bwrap_launcher() {
        BubblewrapLauncher::System(program) => exec_system_bwrap(..),
        BubblewrapLauncher::Vendored => exec_vendored_bwrap(..),
    }
}
```

#### Layer 2: seccomp (Syscall Filtering)

The seccomp filter operates in two modes:

```rust
enum NetworkSeccompMode {
    Restricted,    // Block all network syscalls except AF_UNIX
    ProxyRouted,   // Allow AF_INET/AF_INET6, block AF_UNIX socketpair
}
```

**Restricted mode** (default) denies these syscalls unconditionally:

| Category | Blocked Syscalls |
|---|---|
| **Debug** | `ptrace` |
| **Async I/O** | `io_uring_setup`, `io_uring_enter`, `io_uring_register` |
| **Network** | `connect`, `accept`, `accept4`, `bind`, `listen` |
| **Network** | `getpeername`, `getsockname`, `shutdown` |
| **Network** | `sendto`, `sendmmsg`, `recvmmsg` |
| **Network** | `getsockopt`, `setsockopt` |
| **Socket** | `socket` (only `AF_UNIX` allowed; all others return `EPERM`) |
| **Socket** | `socketpair` (only `AF_UNIX` allowed) |

**ProxyRouted mode** (for managed proxy networking):
- Allows `AF_INET`/`AF_INET6` sockets (to reach local TCP bridge)
- Blocks `AF_UNIX` socketpair creation (prevents bridge bypass)

`PR_SET_NO_NEW_PRIVS` is always set before seccomp to prevent setuid escalation.

#### Layer 3: Landlock (Legacy Fallback)

Available via `features.use_legacy_landlock = true`:

```rust
fn install_filesystem_landlock_rules_on_current_thread(
    writable_roots: Vec<AbsolutePathBuf>,
) {
    Ruleset::default()
        .handle_access(AccessFs::from_all(ABI::V5))
        .create()
        .add_rules(path_beneath_rules(&["/"], access_ro))        // read-only everywhere
        .add_rules(path_beneath_rules(&["/dev/null"], access_rw)) // /dev/null writable
        .add_rules(path_beneath_rules(&writable_roots, access_rw)) // explicit writable roots
        .restrict_self()
}
```

#### Proxy Routing (`proxy_routing.rs`)

For managed networking, Codex creates a TCP → UDS → TCP routing bridge:

```
Sandboxed Process (network namespace)
    │
    ├── AF_INET connect to 127.0.0.1:PORT
    │       │
    │       └── TCP bridge → UDS socket → parent namespace
    │                                         │
    │                                         └── TCP to configured proxy
    │
    └── After bridge is live, seccomp blocks new AF_UNIX/socketpair
```

This allows tool traffic to reach only configured proxy endpoints while
maintaining complete network isolation.

### macOS Sandbox (Seatbelt)

Commands are wrapped with `sandbox-exec -p <profile>`:

```
sandbox-exec -p "
  (version 1)
  (deny default)
  (allow file-read* (subpath \"/\"))
  (deny file-write* (subpath \"/\"))
  (allow file-write* (subpath \"$PWD\"))
  (allow file-write* (subpath \"$TMPDIR\"))
  (allow file-write* (subpath \"~/.codex\"))
  (deny network*)
" -- <command>
```

When restricted read access enables platform defaults, Codex appends a curated
macOS platform policy (instead of broadly allowing `/System`) to preserve
common tool compatibility.

### Windows Sandbox (`codex-rs/windows-sandbox-rs/`)

Windows uses a fundamentally different approach with **27 source files**:

#### Architecture

1. **Sandbox User Accounts** (`sandbox_users.rs`) — Creates dedicated Windows
   user accounts for sandboxed commands

2. **ACL Deny ACEs** (`acl.rs`, `workspace_acl.rs`) — Restricts filesystem
   access using Windows Deny ACEs. Only whitelisted paths are writable:

   ```rust
   // Only workspace root gets write access
   set_workspace_acl(workspace_path, sandbox_sid, AccessPermission::Write);
   // Everything else gets explicit Deny ACEs
   ```

3. **Windows Firewall** (`firewall.rs`) — Creates per-SID outbound block rules:

   ```rust
   const OFFLINE_BLOCK_RULE_NAME: &str = "codex_sandbox_offline_block_outbound";
   // Blocks ALL outbound IP protocols for the sandbox user
   rule.SetAction(NET_FW_ACTION_BLOCK);
   rule.SetDirection(NET_FW_RULE_DIR_OUT);
   rule.SetProtocol(NET_FW_IP_PROTOCOL_ANY);
   rule.SetLocalUserAuthorizedList(&local_user_spec); // scoped to sandbox SID
   ```

4. **Alternate Desktop** (`desktop.rs`) — Isolates sandboxed processes visually
   and from window message attacks

5. **Restricted Tokens** (`token.rs`, `process.rs`) — Processes launched with
   stripped-down security tokens

6. **DPAPI Protection** (`dpapi.rs`) — Credential protection for stored secrets

#### Windows Sandbox Modes

```toml
[windows]
sandbox = "unelevated"  # or "elevated"
sandbox_private_desktop = true  # default
```

### Process Hardening (`codex-rs/process-hardening/`)

Applied **pre-main** via `#[ctor::ctor]`:

| Platform | Hardening |
|---|---|
| **Linux** | `prctl(PR_SET_DUMPABLE, 0)` — disable ptrace & core dumps |
| **Linux** | `setrlimit(RLIMIT_CORE, 0)` — prevent core files |
| **Linux** | Remove all `LD_*` env vars — prevent shared-library injection |
| **macOS** | `ptrace(PT_DENY_ATTACH)` — block debugger attach |
| **macOS** | `setrlimit(RLIMIT_CORE, 0)` |
| **macOS** | Remove all `DYLD_*` env vars |
| **FreeBSD/OpenBSD** | `setrlimit(RLIMIT_CORE, 0)` + strip `LD_*` vars |

### Shell Escalation (`codex-rs/shell-escalation/`)

For commands that need elevated privileges within the sandbox:

```rust
pub use unix::EscalateServer;
pub use unix::EscalationDecision;
pub use unix::EscalationPolicy;
pub use unix::ShellCommandExecutor;
pub use unix::main_execve_wrapper;
pub use unix::ESCALATE_SOCKET_ENV_VAR;
```

The escalation server listens on a Unix domain socket. The `main_execve_wrapper`
is used as an `arg0` dispatch target to execute commands with elevated
privileges within the sandbox constraints.

## Approval Policies

### Policy Levels

| Policy | Behavior |
|---|---|
| `on-request` (default) | Ask for approval on sensitive operations |
| `untrusted` | Only auto-approve known-safe read operations |
| `never` | Never ask — auto-reject anything needing approval |
| Granular | Per-category control |

### Granular Approval

```toml
approval_policy = { granular = {
    sandbox_approval = true,        # sandbox-escape requests
    rules = true,                   # execpolicy rule prompts
    mcp_elicitations = true,        # MCP tool elicitations
    request_permissions = false,    # permission escalation
    skill_approval = false          # skill script execution
} }
```

### Approval Workflow

```
Model generates tool call
  → ExecPolicy checks command
    ├── Allow → auto-execute
    ├── Forbidden → reject
    └── Prompt → emit ExecApprovalRequestEvent to UI
         │
         ├── User approves → execute
         ├── User approves + amends policy → execute + update rules
         ├── User denies → reject
         └── User aborts → abort turn
```

### Sandbox + Approval Combinations

| Intent | Flags | Effect |
|---|---|---|
| Auto (default) | `--full-auto` | Edit/run in workspace; ask for outside/network |
| Safe read-only | `--sandbox read-only -a on-request` | Read only; ask for everything else |
| CI read-only | `--sandbox read-only -a never` | Read only; never ask |
| Auto edit, ask commands | `--sandbox workspace-write -a untrusted` | Edit files; ask for untrusted commands |
| Dangerous full | `--yolo` | No sandbox; no approvals |

## Hooks System

### Hook Points

| Hook | When | Can Do |
|---|---|---|
| `SessionStart` | Session begins | Stop session, inject system message |
| `UserPromptSubmit` | User sends prompt | Block with reason |
| `Stop` | Turn ends | Block continuation |

### Programmatic Hooks

```rust
enum HookToolKind { Function, Custom, LocalShell, Mcp }

enum HookToolInput {
    Function { arguments: String },
    Custom { input: String },
    LocalShell { params: ShellParams },
    Mcp { server: String, tool: String, arguments: String },
}

enum HookEvent {
    AfterAgent {
        thread_id: String,
        turn_id: String,
        input_messages: Vec<..>,
        last_assistant_message: String,
    },
    AfterToolUse {
        turn_id: String,
        call_id: String,
        tool_name: String,
        tool_kind: HookToolKind,
        tool_input: HookToolInput,
        executed: bool,
        success: bool,
        duration_ms: u64,
        mutating: bool,
        sandbox: bool,
        sandbox_policy: String,
        output_preview: String,
    },
}
```

## End-to-End: Tool Call → Sandboxed Execution

```
1. LLM generates LocalShellCall { command: ["npm", "test"] }

2. ToolRouter.build_tool_call() → ToolCall { tool: "local_shell", .. }

3. ToolOrchestrator.run():
   a. ExecPolicy.check(["npm", "test"]) → Decision::Prompt
   b. Emit ExecApprovalRequest to UI
   c. Wait for Op::ExecApproval { decision: Approved }

4. SandboxManager.select_initial(WorkspaceWrite) → LinuxSandbox

5. Spawn sandboxed process:
   Linux: bubblewrap
     → mount namespace (/ read-only, $PWD writable)
     → user namespace isolation
     → PID namespace isolation
     → network namespace isolation
     → seccomp filter (block ptrace, io_uring, network syscalls)
     → PR_SET_NO_NEW_PRIVS
     → exec("npm", ["test"])

6. Capture stdout/stderr

7. Build FunctionCallOutput { call_id, output: "..." }

8. Inject into ContextManager

9. Call model again with result → next iteration
```