# Codex CLI — Unique Patterns & Differentiators

## Overview

Codex CLI stands apart from other terminal coding agents through its Rust-native
implementation, OS-level sandboxing without containers, the SQ/EQ architecture
pattern, and deep integration with OpenAI's model ecosystem. This document
catalogs the patterns that make Codex CLI architecturally distinct.

## 1. OS-Native Sandboxing (No Containers, No VMs)

### The Key Differentiator

While Claude Code relies on **no sandboxing by default** (trusting the user to
run in a safe environment) and other agents use Docker containers, Codex CLI
implements **OS-native sandboxing** that requires zero additional infrastructure:

| Agent | Sandbox Approach |
|---|---|
| **Codex CLI** | OS-native: Seatbelt (macOS), bubblewrap+seccomp (Linux), ACLs+Firewall (Windows) |
| **Claude Code** | No sandbox by default; user-managed Docker optional |
| **Aider** | No sandbox; git-based safety net |
| **Cline** | VS Code extension sandbox only |
| **Cursor Agent** | Editor-level sandbox |

### Three-Layer Linux Security

Codex's Linux sandbox is the most sophisticated in the open-source agent space:

**Layer 1 — Bubblewrap (mount namespace isolation)**
```
--ro-bind / /              # Entire filesystem read-only
--bind $PWD $PWD           # Only workspace writable
--ro-bind .git .git        # Git dir always protected
--unshare-user             # User namespace isolation
--unshare-pid              # PID namespace isolation
--unshare-net              # Network namespace isolation
```

**Layer 2 — seccomp (syscall filtering)**
- Blocks `ptrace`, `io_uring_*` (async I/O bypass), all network syscalls
- Socket creation limited to `AF_UNIX` only
- `PR_SET_NO_NEW_PRIVS` prevents privilege escalation

**Layer 3 — Process hardening (pre-main)**
- `#[ctor::ctor]` runs before `main()`
- Disables core dumps (`RLIMIT_CORE=0`)
- Blocks debugger attachment (`PT_DENY_ATTACH` on macOS, `PR_SET_DUMPABLE=0` on Linux)
- Strips `LD_*`/`DYLD_*` environment variables (prevents shared-library injection)

### Windows Sandbox Innovation

The Windows implementation is entirely unique in the agent space — no other
coding agent has a Windows-native sandbox:

1. Creates dedicated **sandbox user accounts**
2. Applies **Windows ACL Deny ACEs** for filesystem restriction
3. Creates **per-SID Windows Firewall rules** for network blocking
4. Runs on an **alternate desktop** for visual + message isolation
5. Uses **restricted security tokens** for process creation
6. **DPAPI protection** for credential storage

### Vendored Bubblewrap

Codex ships a **vendored bubblewrap binary** as a fallback when the system
`/usr/bin/bwrap` isn't available. This means sandbox protection works on any
Linux system without additional package installation.

## 2. SQ/EQ Architecture (vs. Direct Function Calls)

### The Pattern

Most agents use direct function calls between UI and core logic. Codex uses
a formal **Submission Queue / Event Queue** message-passing pattern:

```
┌──────────┐       SQ (Op variants)        ┌──────────┐
│   UI     │ ────────────────────────────► │   Core   │
│ (TUI,    │                               │ (Session,│
│  exec,   │ ◄──────────────────────────── │  Agent)  │
│  app-srv)│       EQ (EventMsg variants)  │          │
└──────────┘                               └──────────┘
```

### Why This Matters

1. **Multiple frontends**: TUI, `codex exec`, IDE extension (app-server), and
   MCP server all share the same core — just different SQ/EQ consumers
2. **Async decoupling**: UI never blocks on model calls or tool execution
3. **Testability**: Core can be tested with synthetic SQ inputs and EQ assertions
4. **Distribution**: The exec-server extends SQ/EQ over JSON-RPC/WebSocket
5. **Tracing**: W3C trace context propagates through submissions for OTel

### Comparison

| Agent | UI↔Core Communication |
|---|---|
| **Codex CLI** | SQ/EQ channels (typed Rust enums) |
| **Claude Code** | Direct async function calls |
| **Aider** | Synchronous Python method calls |
| **Continue** | VS Code extension message passing |

## 3. Rust-Native Implementation

### Zero Dependencies at Runtime

Codex CLI ships as a **single static binary** with no runtime dependencies:
- No Node.js, Python, or JVM required
- No Docker daemon needed
- Sandbox primitives are OS-native
- Vendored bubblewrap for Linux

### Compile-Time Security

Security measures that happen **before `main()` even runs**:

```rust
#[ctor::ctor]
fn harden_process() {
    // Runs at process load time, before main()
    #[cfg(target_os = "linux")]
    {
        prctl(PR_SET_DUMPABLE, 0);       // No ptrace
        setrlimit(RLIMIT_CORE, 0);       // No core dumps
        remove_ld_env_vars();             // No LD_PRELOAD
    }
    #[cfg(target_os = "macos")]
    {
        ptrace(PT_DENY_ATTACH);           // No debugger
        setrlimit(RLIMIT_CORE, 0);
        remove_dyld_env_vars();           // No DYLD_INSERT_LIBRARIES
    }
}
```

### API Key Security

The Responses API proxy (`responses-api-proxy`) demonstrates extreme care:
- API key read from **stdin** (never environment variables)
- Key memory locked with **`mlock(2)`** (never swapped to disk)
- Key leaked to `'static` lifetime via `.leak()` for `HeaderValue::from_static()`
- Header marked as **`.set_sensitive(true)`** (excluded from debug output)
- Only `POST /v1/responses` forwarded — no other endpoints exposed

## 4. Multi-Model Support with Provider Abstraction

### Built-in Providers

| Provider | Default URL | Auth |
|---|---|---|
| `openai` | `api.openai.com/v1` | OAuth / API key |
| `ollama` | `localhost:11434/v1` | None |
| `lmstudio` | `localhost:1234/v1` | None |

### Custom Provider Configuration

```toml
[model_providers.my_provider]
base_url = "https://my-api.example.com/v1"
env_key = "MY_API_KEY"
wire_api = "responses"
request_max_retries = 4
stream_max_retries = 5
stream_idle_timeout_ms = 300000
```

### OpenAI Model Lineup

The recommended models for Codex CLI:
- **GPT-5.4** — Recommended default (frontier coding + reasoning)
- **GPT-5.3-Codex** — Specialized coding model
- **GPT-5.3-Codex-Spark** — Fast mode (Pro subscribers only)
- **GPT-5.2-Codex** — Previous generation
- Legacy: `codex-mini-latest`, `o4-mini`, etc.

### Responses API Only

Codex has **dropped Chat Completions entirely** — only the Responses API wire
format is supported. Attempting `wire_api = "chat"` produces an error with
migration instructions. This is a bold commitment to the newer API.

## 5. Execution Policy System

### Two-Generation Policy Engine

Codex maintains both a new and legacy policy system:

**New Engine** — Prefix-rule matching with three-valued decisions:
```rust
enum Decision { Allow, Prompt, Forbidden }

// Policy stored as:
rules_by_program: MultiMap<String, RuleRef>  // cmd → rules
network_rules: Vec<NetworkRule>               // domain → decision
```

**Legacy Engine** — Python-like DSL with typed argument matching:
```
program ls:
    flags: -1 -a -l
    args: ARG_RFILES_OR_CWD

program sed:
    args: ARG_SED_COMMAND ARG_RFILES
    # Blocks 'e' flag which would execute shell commands
```

The **84KB shell command parser** (`parse_command.rs`) is the largest single
file in the codebase — it decomposes complex shell expressions (pipes,
redirections, subshells, command substitutions) into individual executable
invocations for policy evaluation.

### Dynamic Policy Amendment

When users approve a command, they can optionally add an **execution policy
amendment** that auto-approves similar commands in the future:

```rust
ExecApprovalRequestEvent {
    proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    proposed_network_policy_amendments: Option<Vec<NetworkPolicyAmendment>>,
}
```

## 6. Enterprise-Grade Configuration

### Layered Config with MDM Support

```
MDM (lowest) → System → User → Project → SessionFlags (highest)
```

Enterprise administrators can enforce constraints via MDM (Mobile Device
Management) or system-level requirements:

```rust
pub struct ConfigRequirements {
    pub approval_policy: Option<...>,
    pub sandbox_policy: Option<...>,
    pub web_search_mode: Option<...>,
    pub mcp_servers: Vec<McpServerRequirement>,
    pub exec_policy: Option<...>,
    pub enforce_residency: Option<ResidencyRequirement>,
    pub network: Option<NetworkConstraints>,
}
```

### Custom CA Certificates

```bash
export CODEX_CA_CERTIFICATE=/path/to/ca-bundle.pem
# Falls back to SSL_CERT_FILE, then system roots
```

Enterprise proxies that intercept TLS are fully supported.

### OpenTelemetry Auditing

```toml
[otel]
environment = "staging"
exporter = { otlp-http = {
    endpoint = "https://otel.example.com/v1/logs",
    headers = { "x-otlp-api-key" = "${OTLP_TOKEN}" }
}}
log_user_prompt = false  # redact by default
```

Events: `codex.conversation_starts`, `codex.api_request`, `codex.tool_decision`,
`codex.tool_result` with corresponding metrics.

## 7. Sub-Agent System

### Resource-Controlled Multi-Agent

```rust
pub(crate) struct Guards {
    active_agents: Mutex<ActiveAgents>,
    total_count: AtomicUsize,  // CAS-based max enforcement
}
```

Sub-agents:
- Have their own context, model calls, and tool execution
- Share the parent's sandbox manager (enforced by same OS mechanisms)
- Can be spawned with different roles (`explorer`, `worker`)
- Support session resume from persisted rollout files
- Are resource-limited via atomic counter guards

### Role-Based Agent Configuration

```rust
// Built-in roles
"explorer" → codebase Q&A, may use faster model
"worker"   → execution tasks, file ownership semantics
"default"  → standard agent behavior
```

Custom roles can be defined in config with per-role model, reasoning effort,
and developer instructions.

## 8. Session Persistence & Resume

### JSONL Rollout Files

Every session is persisted as JSONL rollout files under `~/.codex/sessions/`.
Each line is a serialized `ResponseItem` or `Op` that can be replayed:

```bash
codex resume              # Interactive picker
codex resume --last       # Most recent session
codex resume <SESSION_ID> # Specific session
codex exec resume --last "Fix the issues"  # Non-interactive resume
```

Sub-agent rollouts are stored alongside parent rollouts and recursively
resumed.

### SQLite State Database

```rust
pub const STATE_DB_VERSION: i32 = 5;
pub const LOGS_DB_VERSION: i32 = 1;
```

Stores thread metadata, agent job state, and structured logs. Versioned
schema for forward compatibility.

## 9. Web Search with Safety Controls

Three modes with escalating risk:

| Mode | Behavior | Risk |
|---|---|---|
| `cached` (default) | Pre-indexed OpenAI cache | Low (no live fetch) |
| `live` | Real-time web browsing | Higher (prompt injection risk) |
| `disabled` | No web search | None |

The cached mode is a unique safety feature — web results come from an
OpenAI-maintained index rather than live pages, reducing prompt injection
surface area.

## 10. Managed Network Proxy

For environments that need controlled network access (not just allow/deny),
Codex implements a **proxy bridge**:

```
Sandboxed Process (isolated network namespace)
  └── AF_INET → 127.0.0.1:PORT (TCP bridge)
       └── UDS socket → parent namespace
            └── TCP to configured proxy endpoint
```

After the bridge is established, seccomp blocks new `AF_UNIX`/`socketpair`
creation, preventing bypass. This allows fine-grained network control per
domain through the execution policy.

## Summary: What Makes Codex CLI Unique

1. **OS-native sandboxing** — No containers/VMs needed; works on bare metal
2. **SQ/EQ architecture** — Clean separation enables multiple frontends
3. **Rust single binary** — Zero runtime dependencies
4. **Pre-main hardening** — Security before code executes
5. **Windows-native sandbox** — Only agent with real Windows sandboxing
6. **Execution policy DSL** — Fine-grained command allowlisting
7. **Enterprise MDM support** — Corporate-managed security requirements
8. **Cached web search** — Reduced prompt injection via pre-indexed results
9. **API key mlock** — Key never touches disk or environment
10. **Responses API only** — Committed to OpenAI's modern API format