---
title: Build Verification
status: complete
---

# Build Verification

> An agent can pass linting and type checks but still produce code that doesn't link,
> can't resolve its imports, or blows up at compile time. Build verification is the
> definitive "does this code work as a whole" gate — and most agents treat it as
> an afterthought.

**Build verification** is the practice of running the project's actual build system after
every code change to confirm that the entire codebase still compiles, links, and packages
correctly. It sits between lightweight checks (lint, type-check) and heavyweight ones
(integration tests, end-to-end tests) in the verification hierarchy. Among the 17 agents
studied, only a handful treat it as a first-class concern — and the ones that do show
measurably better outcomes on multi-file editing tasks.

---

## Why Build Verification Matters

Linting catches syntax errors. Type-checking catches mismatched signatures. Neither
catches these failure modes:

- **Linking errors** — a function is declared but never defined; the linker fails
- **Missing dependencies** — an import is added but the package was never installed
- **Import cycles** — module A imports B which imports A; detected only at build time
- **Resource resolution** — embedded assets, config files, or generated code absent
- **Cross-module type mismatches** — a change in module A breaks module B's assumptions
  that the type-checker in isolation can't see
- **Build configuration drift** — Makefile targets, Gradle tasks, or Webpack configs
  that no longer match the source tree

The cost of skipping build verification compounds across turns. Research from the
edit-apply-verify analysis shows that LLMs produce syntactically or semantically broken
code **15–25% of the time**. Without a build gate, these errors cascade — by turn five,
an agent typically has 3–4 compounding failures that are far harder to diagnose than the
original error.

```
Verification Hierarchy (cost vs. coverage)

  Coverage ▲
           │
     ┌─────┴──────────────────────────────────────────┐
     │  E2E / Integration Tests                       │  Minutes
     ├────────────────────────────────────────────────┤
     │  Unit Tests                                    │  Seconds–Minutes
     ├────────────────────────────────────────────────┤
     │  ██ BUILD VERIFICATION ██                      │  Seconds–Minutes
     ├────────────────────────────────────────────────┤
     │  Type Checking                                 │  Seconds
     ├────────────────────────────────────────────────┤
     │  Linting / Formatting                          │  Milliseconds
     └─────┬──────────────────────────────────────────┘
           │
     Cost  ▼
```

**The key insight:** build verification is the cheapest check that exercises the *entire*
dependency graph. A lint pass tells you one file is syntactically valid. A build tells you
every file, together, produces a working artifact.

---

## Build System Detection

Before an agent can verify a build, it must answer: *what builds this project?* The
detection problem maps marker files to build commands:

| Marker File | Build System | Typical Build Command |
|---|---|---|
| `package.json` (with `scripts.build`) | npm / yarn / pnpm | `npm run build` |
| `Cargo.toml` | Cargo (Rust) | `cargo build` |
| `Makefile` | Make | `make` |
| `build.gradle` / `build.gradle.kts` | Gradle (Java/Kotlin) | `gradle build` |
| `pom.xml` | Maven (Java) | `mvn compile` |
| `CMakeLists.txt` | CMake | `cmake --build .` |
| `go.mod` | Go toolchain | `go build ./...` |
| `pyproject.toml` | Python build backends | `python -m build` / `pip install -e .` |
| `*.sln` / `*.csproj` | .NET | `dotnet build` |
| `Justfile` | Just | `just build` |

### Detection Logic (Pseudocode)

```python
BUILD_MARKERS = [
    ("Cargo.toml",       "cargo build"),
    ("package.json",     "npm run build"),      # if scripts.build exists
    ("go.mod",           "go build ./..."),
    ("Makefile",         "make"),
    ("build.gradle",     "gradle build"),
    ("build.gradle.kts", "gradle build"),
    ("pom.xml",          "mvn compile"),
    ("CMakeLists.txt",   "cmake --build build/"),
    ("pyproject.toml",   "pip install -e ."),
    ("*.sln",            "dotnet build"),
]

def detect_build_system(project_root: str) -> str | None:
    """Walk markers in priority order. First match wins."""
    for marker, command in BUILD_MARKERS:
        if (project_root / marker).exists():
            # Special case: package.json needs a build script
            if marker == "package.json":
                pkg = json.load(open(project_root / marker))
                if "build" not in pkg.get("scripts", {}):
                    continue
            return command
    return None  # No recognized build system
```

### How Agents Approach Detection

**Junie-CLI** has the most sophisticated build system detection of any agent studied.
Its `detect_build_system()` examines project files to identify the build system and
returns appropriate commands for build, test, and lint. It handles ambiguous cases —
for instance, when both `pom.xml` and `build.gradle` are present. From its tool system:

```
detect_build_system(project_root) → build_system
run_build(project_root, target?) → result
install_dependencies(project_root) → result
```

Junie parses `pom.xml`, `build.gradle`, `package.json`, `Cargo.toml`, `go.mod`, and
`pyproject.toml` to extract not just build commands but dependencies, module structures,
plugin configs, and test frameworks.

**Aider** takes the opposite approach — no automatic detection at all. Users configure
build commands explicitly via `--test-cmd`, which can chain build and test steps:

```bash
aider --test-cmd "dotnet build && dotnet test"
aider --test-cmd "cargo build 2>&1 && cargo test"
```

**Codex** maintains dual build systems for its own codebase — Cargo for development,
Bazel for CI/release — managed through a `Justfile`:

```makefile
# Codex's own build orchestration
just fmt      # Format code
just fix      # Apply lint fixes
just test     # Run test suite
just bazel-lock-update  # Sync Bazel lockfile
```

**Claude Code** relies on the LLM itself to discover build commands by reading
`package.json`, `Makefile`, or CI configs. Build commands learned during a session
get persisted to project memory (`~/.claude/projects/<project>/memory/MEMORY.md`),
so they don't need rediscovery on subsequent sessions.

**Most agents** (mini-SWE-agent, OpenHands, Ante, TongAgents) delegate entirely to the
LLM — the model reads the project structure through file listings and decides what shell
command to run. This works surprisingly well for common ecosystems but fails on
non-standard build setups.

---

## Running Builds After Changes

The canonical pattern is **edit → build → diagnose → fix → rebuild**:

```
┌─────────────────────────────────────────────────────┐
│                  Build-Verify Loop                   │
│                                                      │
│   ┌──────┐    ┌───────┐    ┌──────────┐             │
│   │ Edit │───►│ Build │───►│ Success? │──► Done     │
│   └──────┘    └───────┘    └────┬─────┘             │
│       ▲                         │ No                 │
│       │                         ▼                    │
│       │                   ┌───────────┐              │
│       │                   │ Parse     │              │
│       │                   │ Errors    │              │
│       │                   └─────┬─────┘              │
│       │                         │                    │
│       │                   ┌─────▼─────┐              │
│       │                   │ Feed to   │              │
│       └───────────────────│ LLM       │              │
│                           └───────────┘              │
└─────────────────────────────────────────────────────┘
```

### Agent-Specific Patterns

**Junie-CLI** runs a three-check verification after every implementation phase:
(1) test execution, (2) code inspections / static analysis, (3) **compilation
validation**. All three must pass. If compilation fails, a diagnostic sub-loop
retries up to 3–5 iterations before escalating. Build status is tracked in
structured context:

```json
{
  "compilation": {
    "errors": 0,
    "warnings": 3,
    "last_successful_build": "2025-01-15T10:30:00Z"
  }
}
```

**Warp** has a unique approach — live terminal buffer monitoring. When a user says
"start the dev server and fix any compilation errors," Warp runs the command,
monitors the terminal output stream in real time, detects compilation errors as they
appear, edits source files, and observes hot-reload success — all without a
discrete build-verify loop:

```
User:   "Start dev server and fix compilation errors"
Agent:  [Runs: npm run dev]
        [Monitors terminal buffer]
        [Detects: ERROR in ./src/App.tsx - Module not found]
        [Edits src/App.tsx to fix import]
        [Observes: Compiled successfully]
        [Reports fix to user]
```

**Aider** integrates build verification into its edit-apply-lint-test loop. When
`--auto-lint` or `--auto-test` triggers a failure, the error output is sent back
to the LLM, which produces new edits. The cycle repeats with bounded retries
(default: 2 attempts).

**ForgeCode** takes the strongest stance: verification is **enforced at runtime**.
The agent cannot claim a task is complete without running verification checks. This
was motivated by the finding that models skip optional verification ~40% of the
time — making enforcement the single largest benchmark improvement.

---

## Iterative Fix Loops

When a build fails, agents enter a fix loop. The strategies vary in sophistication,
but all share a core structure:

### The First-Error Strategy

Compilers often produce cascading errors — one missing semicolon generates dozens
of follow-on diagnostics. The effective strategy is:

1. **Fix the first error only** — it's most likely the root cause
2. **Rebuild** — many subsequent errors will vanish
3. **Repeat** — address the new first error if one remains

Attempting to fix all errors simultaneously leads to over-correction, where the agent
"fixes" cascading errors that would have resolved themselves.

### Bounded Retries

Every agent that implements fix loops caps the iteration count:

| Agent | Max Build-Fix Iterations | Escalation Strategy |
|---|---|---|
| **Junie-CLI** | 3–5 | Escalate to meta-agent |
| **Aider** | 2 | Return error to user |
| **ForgeCode** | 3 | Block completion, report |
| **Claude Code** | Unbounded (model decides) | Model self-terminates |
| **OpenHands** | Stuck detection triggers | Break pattern or terminate |

**Ante** delegates build errors to sub-agents. If a sub-agent's code change breaks
the build, the sub-agent handles the fix internally before reporting back to the
meta-agent. This isolation prevents build failures in one sub-task from polluting
the context of the broader plan.

**OpenHands** uses a `StuckDetector` with four strategies to detect agents trapped
in build-fix loops: identical action repetition, alternating ping-pong edits,
consecutive error streaks, and empty response detection. When triggered, it either
breaks the pattern with a context hint or terminates the loop entirely.

### Cascading Error Example (Rust)

```rust
// Agent adds a function call with the wrong type
fn process(data: Vec<u8>) -> Result<String, Error> {
    let parsed = parse_input(data)?;  // ERROR 1: parse_input expects &[u8]
    let result = transform(parsed);    // ERROR 2: cascading — parsed type wrong
    format_output(result)              // ERROR 3: cascading — result type wrong
}
```

The compiler produces three errors, but only the first is actionable. An effective
agent fixes `data` to `&data` on line 3, rebuilds, and finds all three errors resolved.

---

## Compilation Error Parsing

Build tools produce error output in predictable formats. Agents that parse these
formats can extract **file path**, **line number**, **column**, and **error message** —
giving the LLM precise targets for fixes rather than asking it to interpret raw output.

### Common Error Formats

```
GCC/Clang:     src/main.c:42:10: error: use of undeclared identifier 'foo'
rustc:         error[E0308]: mismatched types
                 --> src/lib.rs:15:5
TypeScript:    src/app.ts(23,7): error TS2304: Cannot find name 'Widget'.
javac:         src/Main.java:18: error: cannot find symbol
Go:            ./main.go:12:5: undefined: processData
.NET:          Program.cs(10,25): error CS1002: ; expected
```

### Structured Parsing (Pseudocode)

```python
import re

ERROR_PATTERNS = {
    "gcc":        r"^(?P<file>[^:]+):(?P<line>\d+):(?P<col>\d+): error: (?P<msg>.+)$",
    "rustc":      r"^\s+--> (?P<file>[^:]+):(?P<line>\d+):(?P<col>\d+)$",
    "typescript": r"^(?P<file>[^(]+)\((?P<line>\d+),(?P<col>\d+)\): error (?P<msg>.+)$",
    "go":         r"^(?P<file>[^:]+):(?P<line>\d+):(?P<col>\d+): (?P<msg>.+)$",
}

def parse_build_errors(output: str, compiler: str) -> list[dict]:
    """Extract structured error info from compiler output."""
    pattern = ERROR_PATTERNS.get(compiler)
    if not pattern:
        return []  # Fall back to feeding raw output to LLM

    errors = []
    for line in output.splitlines():
        match = re.match(pattern, line)
        if match:
            errors.append(match.groupdict())
    return errors
```

### Rust's Rich Error Messages

Rust deserves special attention because `rustc` produces the richest compiler
output of any mainstream language — errors include **explanations**, **suggestions**,
and sometimes complete fix code:

```
error[E0382]: borrow of moved value: `data`
  --> src/main.rs:10:20
   |
8  |     let result = process(data);
   |                          ---- value moved here
9  |
10 |     println!("{:?}", data);
   |                      ^^^^ value borrowed here after move
   |
help: consider cloning the value
   |
8  |     let result = process(data.clone());
   |                              ++++++++
```

This is why Rust-based agents like **Ante** and **Goose** benefit from the compiler
itself acting as a code reviewer — the agent can often apply the `help:` suggestion
directly.

### Output Truncation

Build output can be enormous — a full `cargo build` on a large project might produce
thousands of lines. Agents truncate before feeding to the LLM:

| Agent | Truncation Strategy | Limit |
|---|---|---|
| **Aider** | First N lines of error output | ~50 lines |
| **Claude Code** | Adaptive — keeps relevant portions | Dynamic |
| **Junie-CLI** | First failure only | 1 error block |
| **ForgeCode** | Structured fields (file, line, msg) | Parsed |
| **OpenCode** | Middle truncation (head + tail) | 30K chars |
| **mini-SWE-agent** | Head + tail with instruction hint | 10K chars |

**Design principle:** too much compiler output *hurts* model performance. The LLM
performs better with the first error and 10 lines of context than with 500 lines of
cascading diagnostics.

---

## CI-Like Local Verification

The most reliable way to verify a build is to run exactly what CI runs — locally.
Several agents support headless or non-interactive modes designed for this purpose.

### Claude Code: Headless Mode

The `-p` flag enables non-interactive execution with no permission prompts — designed
for CI/CD pipelines and scripting:

```bash
# In a GitHub Actions workflow
claude -p "Run the full build and test suite, fix any failures" \
  --allowedTools "Bash(npm run build)" "Bash(npm run test)" \
  --output-format json
```

With `bypassPermissions` mode, Claude Code can operate fully autonomously in CI
without human confirmation gates.

### Codex: Exec Mode

Codex provides `codex exec` for non-interactive CI/scripting:

```bash
# CI pipeline step
codex exec "Build the project and run all tests" \
  --sandbox read-only \
  -a never
```

The `--sandbox read-only` flag prevents writes to the filesystem (the agent can
only report findings), and `-a never` disables approval prompts.

### Droid: Pipeline Automation

Droid operates across multiple interfaces — IDE, CLI, Slack, Linear, and CI/CD
pipelines. In CI mode, it's triggered by events like PR creation or CI failure.
Its `github_action_repair` tool specifically analyzes GitHub Actions failures
and suggests fixes:

```yaml
# .droid.yaml
triggers:
  - event: ci_failure
    action: github_action_repair
    auto_fix: true
```

### ForgeCode: Enforced Non-Interactive Mode

ForgeCode's non-interactive mode is used for benchmark evaluation and CI/CD. The
enforcement mechanism prevents the agent from skipping verification even in
automated contexts — a critical design decision since the tendency to skip
verification increases in non-interactive mode.

### Local CI Verification Script

A generalized pattern that several agents approximate:

```bash
#!/bin/bash
# Reproduce CI locally before committing
set -euo pipefail

echo "=== Step 1: Install Dependencies ==="
if [ -f "package.json" ]; then
    npm ci --quiet
elif [ -f "Cargo.toml" ]; then
    cargo fetch --quiet
elif [ -f "requirements.txt" ]; then
    pip install -r requirements.txt --quiet
fi

echo "=== Step 2: Lint ==="
if [ -f "package.json" ]; then
    npm run lint --if-present
elif [ -f "Cargo.toml" ]; then
    cargo clippy -- -D warnings
fi

echo "=== Step 3: Build ==="
if [ -f "package.json" ]; then
    npm run build
elif [ -f "Cargo.toml" ]; then
    cargo build
elif [ -f "Makefile" ]; then
    make
fi

echo "=== Step 4: Test ==="
if [ -f "package.json" ]; then
    npm test
elif [ -f "Cargo.toml" ]; then
    cargo test
elif [ -f "Makefile" ]; then
    make test
fi

echo "=== All checks passed ==="
```

---

## Build Caching and Incremental Builds

Agent iteration speed is directly gated by build time. If a build takes 30 seconds,
the agent gets ~2 fix attempts per minute. If it takes 2 seconds, it gets ~30.
Incremental and cached builds are therefore critical infrastructure for effective
agent operation.

### Incremental Build Strategies

| Language/Tool | Incremental Mode | Speedup | Notes |
|---|---|---|---|
| Rust | `cargo check` (skip codegen) | 2–5x | Catches type errors without producing binary |
| TypeScript | `tsc --incremental` | 3–10x | Reuses `.tsbuildinfo` across runs |
| Gradle | Daemon + build cache | 2–4x | Persistent JVM, task-level caching |
| Maven | Daemon (`mvnd`) | 2–3x | Less mature than Gradle daemon |
| C/C++ | `ccache` | 5–20x | Hash-based object file caching |
| Go | Built-in build cache | 2–10x | `go build` caches compiled packages |
| Bazel | Content-addressed cache | 10–100x | Only rebuilds changed targets |

### Why `cargo check` Is Agent-Optimal

For Rust projects, `cargo check` is the ideal agent verification command:

```
Full build:    cargo build    →  compile + link + codegen  →  ~30s
Check only:    cargo check    →  compile + type-check      →  ~5s
```

`cargo check` catches every error that matters for code correctness — type
mismatches, borrow checker violations, missing imports — without spending time
on code generation and linking. Since agents rarely need to *run* the binary
(they just need to know if the code is valid), `cargo check` provides the same
signal at a fraction of the cost.

**Codex** benefits from Bazel's content-addressed caching for its CI builds —
only targets affected by a change are rebuilt. For a project of Codex's size,
this reduces CI build time from minutes to seconds on typical changes.

**No agent currently implements automatic build cache management** — none will
suggest switching from `cargo build` to `cargo check`, or enabling `tsc
--incremental`. This is a significant optimization gap. An agent that
automatically chose the fastest verification command for each ecosystem would
iterate substantially faster.

---

## Dependency Resolution During Builds

A common agent workflow: edit code, add an import, build fails because the
dependency isn't installed. The agent must then install the dependency and retry.

### The Dependency Installation Loop

```
┌──────────┐     ┌───────┐     ┌─────────────────┐
│ Add      │────►│ Build │────►│ "Module not      │
│ import   │     │       │     │  found: axios"   │
└──────────┘     └───────┘     └────────┬─────────┘
                                        │
                                        ▼
                                 ┌──────────────┐
                                 │ npm install   │
                                 │ axios         │
                                 └──────┬───────┘
                                        │
                                        ▼
                                 ┌──────────────┐
                                 │ Rebuild       │──► Success
                                 └──────────────┘
```

### Sandbox Implications

Dependency installation requires network access — which conflicts with sandboxing.
Agents handle this tension differently:

**Codex** uses a TCP-to-UDS network proxy with domain allowlists. The proxy permits
traffic to package registries (PyPI, npm, crates.io) while blocking all other
network access. This enables `npm install` and `pip install` within the sandbox
without allowing data exfiltration:

```
┌──────────────────────────────────────────────┐
│                Codex Sandbox                  │
│                                              │
│  Agent ──► npm install axios                 │
│              │                               │
│              ▼                               │
│  ┌─────────────────────┐                     │
│  │ Network Proxy        │                    │
│  │ ✅ registry.npmjs.org │                    │
│  │ ✅ pypi.org           │                    │
│  │ ❌ evil-server.com    │                    │
│  └─────────────────────┘                     │
└──────────────────────────────────────────────┘
```

**Capy** runs in isolated Ubuntu VMs with full network access — the Build agent
has sudo privileges and can install any dependency. The VM-level isolation provides
security without restricting the agent's ability to resolve dependencies.

**OpenHands** uses Docker containers. Dependencies can be pre-installed in custom
base images, or installed at runtime with full network access inside the container.

**Gemini-CLI** classifies package installation as "high risk" and always executes
it in a sandboxed context, while file reads are "low risk" and run unsandboxed.

**Supply chain risk** is a real concern: `npm install` and `pip install` execute
arbitrary code from package registries. When an agent installs dependencies
autonomously, it becomes a potential supply chain attack vector. Codex's allowlist
proxy is the most sophisticated mitigation studied.

---

## Build Timeout and Process Management

Large builds can run for minutes. Agents must handle timeouts, process cleanup,
and the possibility of builds that never terminate.

### Timeout Defaults

| Agent | Build Timeout | Behavior on Timeout |
|---|---|---|
| **Codex** | 120s | Process tree terminated |
| **OpenHands** | 120s | `SandboxTimeoutError` — agent can retry |
| **Goose** | 300s | Graceful shutdown |
| **mini-SWE-agent** | None | Dangerous — can hang indefinitely |

### Process Tree Management

Terminating a build shell doesn't necessarily stop its child processes. A `make`
command spawns compiler subprocesses, which spawn linker subprocesses. Agents
use `setpgid` and negative-PID signaling to stop entire process groups:

```python
# Simplified: stop the entire build process tree
import os, signal

def stop_build(pid: int):
    """Stop build process and all its children."""
    try:
        os.killpg(os.getpgid(pid), signal.SIGTERM)
    except ProcessLookupError:
        pass  # Already exited
```

### Interactive Prompt Hazards

Build tools sometimes prompt for input — `[Y/n]` during package installation,
`Accept license? (yes/no)` during SDK setup. An agent that doesn't handle this
will hang indefinitely waiting for input that never comes. Solutions include:

- **`yes |` prefix**: `yes | npm install` auto-accepts prompts
- **`--yes` / `-y` flags**: `apt-get install -y`, `pip install --yes`
- **`CI=true` environment variable**: Many tools detect CI mode and skip prompts
- **Timeout detection**: If no output for N seconds, assume it's waiting for input

---

## The LSP Alternative: Build-Free Verification

**OpenCode** demonstrates an alternative to running builds: using LSP (Language
Server Protocol) diagnostics as a zero-cost build proxy. After every file edit,
the LSP server produces type errors, missing import warnings, and unresolved
reference diagnostics — providing much of the same signal as a build, in
milliseconds rather than seconds:

```
Edit file ──► LSP analyzes ──► Diagnostics returned
                                 • error: Cannot find module 'axios'
                                 • error: Type 'string' not assignable to 'number'
                                 • warning: Unused variable 'tmp'
```

**Limitation:** LSP diagnostics don't cover linking, resource bundling, or
build-system-specific transformations. They're a fast approximation, not a
replacement. The ideal strategy is LSP for fast feedback during editing, with
a full build as a gate before committing.

---

## Real-World Implementations

| Agent | Build Verification Approach | Build System Detection | CI/CD Mode | Reference |
|---|---|---|---|---|
| **Junie-CLI** | 3-check verification (tests + inspections + compilation); 3–5 retry loop | Automated detection for 7+ build systems | Space (CI/CD) integration | [`../agents/junie-cli/`](../agents/junie-cli/) |
| **Claude Code** | Bash tool + model judgment; build commands in project memory | Model reads project files; learned commands persist | Headless mode (`-p` flag) | [`../agents/claude-code/`](../agents/claude-code/) |
| **Codex** | Sandboxed shell execution; dual build (Cargo + Bazel) | Own project: Justfile orchestration | `codex exec` for non-interactive | [`../agents/codex/`](../agents/codex/) |
| **Aider** | Edit-apply-lint-test loop; `--test-cmd` supports build chaining | User-configured via `--test-cmd` | ❌ | [`../agents/aider/`](../agents/aider/) |
| **Droid** | Shell commands + GitHub Actions repair tool | Via CI config analysis | Non-interactive pipeline mode | [`../agents/droid/`](../agents/droid/) |
| **Warp** | Live terminal buffer monitoring for compilation errors | Terminal-native; reads command output | "Always allow" automation mode | [`../agents/warp/`](../agents/warp/) |
| **ForgeCode** | Enforced verification — model cannot skip build checks | General shell execution | Non-interactive benchmark/CI mode | [`../agents/forgecode/`](../agents/forgecode/) |
| **Capy** | Build agent executes in isolated VM with sudo | VM-based; full dependency control | Cloud-native (always automated) | [`../agents/capy/`](../agents/capy/) |
| **OpenCode** | LSP diagnostics as fast build proxy; shell for full builds | LSP-based + user config in OpenCode.md | ❌ | [`../agents/opencode/`](../agents/opencode/) |
| **Gemini-CLI** | Shell execution with risk-tiered sandboxing | User config in GEMINI.md | Headless mode for GitHub Actions | [`../agents/gemini-cli/`](../agents/gemini-cli/) |
| **OpenHands** | Docker sandbox execution; macro templates for build-deploy | Sandbox-based with pre-built images | ❌ | [`../agents/openhands/`](../agents/openhands/) |
| **Goose** | MCP-native shell execution; conversation reset on failure | Via MCP tool servers | CI/CD tutorial documented | [`../agents/goose/`](../agents/goose/) |
| **Ante** | Sub-agent delegation; Rust compile-time safety for own code | Shell commands via sub-agents | ❌ | [`../agents/ante/`](../agents/ante/) |
| **Pi-Coding-Agent** | Extension-based; primitives over built-in features | Via skills/extensions | Print/JSON/RPC modes for CI | [`../agents/pi-coding-agent/`](../agents/pi-coding-agent/) |
| **mini-SWE-agent** | Raw bash — LM decides build commands | LM reads project files via bash | ❌ | [`../agents/mini-swe-agent/`](../agents/mini-swe-agent/) |
| **TongAgents** | Bash in Docker containers; make/cmake awareness | Build system coverage in benchmarks | ❌ | [`../agents/tongagents/`](../agents/tongagents/) |
| **Sage-Agent** | No build verification found | ❌ | ❌ | [`../agents/sage-agent/`](../agents/sage-agent/) |

---

## Key Takeaways

1. **Build verification is the cheapest whole-program check.** Lint and type-check
   validate individual files; builds validate the entire dependency graph. Agents that
   skip builds accumulate cascading errors across turns.

2. **Automatic build system detection is rare.** Only Junie-CLI does it well. Most
   agents rely on user configuration or the LLM's ability to read project files — a
   fragile approach for non-standard setups.

3. **Enforced verification outperforms optional verification.** ForgeCode's biggest
   benchmark gain came from making verification mandatory. Models skip optional checks
   ~40% of the time.

4. **Fix the first error, not all errors.** Cascading compiler output misleads agents
   into over-correction. Truncating to the first error block yields better fix rates.

5. **Build speed determines iteration budget.** An agent with 2-second builds gets 15x
   more fix attempts per minute than one with 30-second builds. No agent currently
   optimizes for incremental builds — a clear opportunity.

6. **Dependency installation is a sandbox design problem.** Agents need network access
   for `npm install` / `pip install`, but unrestricted network access is a security
   risk. Codex's allowlist proxy is the most sophisticated solution studied.

7. **LSP diagnostics complement but don't replace builds.** OpenCode shows that LSP
   provides fast, cheap feedback — but it can't catch linking errors, resource issues,
   or build-configuration problems.

*This analysis covers the 17 agents studied as of mid-2025. Agent capabilities evolve
rapidly; build verification support may change between versions.*
