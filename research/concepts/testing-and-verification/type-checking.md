---
title: Type Checking
status: complete
---

# Type Checking

Type checkers are the highest-signal verification tool available to coding agents. Linters catch style violations. Tests catch behavioral regressions. But type checkers catch **structural impossibilities** — calling a function with the wrong argument count, accessing a property that doesn't exist, returning a string where a number is expected. These are not opinions. They are mathematical proofs of incorrectness. For agents working on typed codebases — TypeScript, Rust, Go, Java — type errors are simultaneously the most common failure mode and the most fixable one, because the error message itself contains the answer.

---

## Why Type Checking Matters for Agents

A linter tells you "this line is too long." A type checker tells you "this code **will crash at runtime**." The distinction is critical for coding agents because type errors represent the intersection of two properties:

1. **High severity** — type errors are not warnings. In compiled languages they prevent the program from running entirely. In gradually typed languages like TypeScript or Python-with-mypy, they indicate logic errors that will manifest as runtime exceptions.

2. **High fixability** — unlike test failures (which require understanding intended behavior) or linter warnings (which may require subjective judgment), type errors come with machine-precise diagnostics. The error message tells you exactly what type was expected, what type was provided, and where the mismatch occurred.

```
┌─────────────────────────────────────────────────────────────────────┐
│                  VERIFICATION SIGNAL QUALITY                        │
│                                                                     │
│   Tool            Severity    Fixability    Signal Clarity          │
│   ─────────────   ────────    ──────────    ──────────────          │
│   Linter          Low-Med     High          Style opinion            │
│   Type Checker    High        Very High     Structural proof         │
│   Unit Tests      Very High   Medium        Behavioral spec          │
│   Integration     Very High   Low           System-level             │
│                                                                     │
│   Type checkers hit the sweet spot: high severity + high fixability │
│   This makes them ideal for automated fix loops.                    │
└─────────────────────────────────────────────────────────────────────┘
```

Consider a typical agent edit — adding a new parameter to a function. The agent updates the function signature but forgets to update three of the seven call sites. A linter sees nothing wrong. Tests *might* catch it if coverage is good. But the type checker catches it **instantly**, with exact file/line/column for every broken call site. The agent reads those errors and fixes each one. No guessing. No re-reading source code. Pure mechanical correction.

This is why agents perform measurably better on typed codebases. More constraints means fewer valid solutions, which means less opportunity for the LLM to hallucinate a plausible-but-wrong answer. TypeScript strict mode effectively narrows the solution space; the type system eliminates entire categories of mistakes before the code ever runs.

---

## The Type Checker Landscape

Not all type checkers are created equal. Their speed, strictness, and integration patterns vary enormously — and these differences have direct implications for how agents use them.

### TypeScript: `tsc`

The canonical type checker for the web ecosystem. Two primary modes:

```bash
# Full compilation (type-check + emit JavaScript)
tsc

# Check-only mode — no output files, just errors (what agents want)
tsc --noEmit

# Incremental mode — reuses previous compilation state
tsc --noEmit --incremental

# Project references — check only affected projects in a monorepo
tsc --build --noEmit
```

**Speed**: Slow for large projects. A 500-file TypeScript project can take 10-30 seconds for a full `tsc --noEmit`. Incremental mode reduces this to 2-5 seconds for subsequent checks. This latency matters — agents that run type checks after every edit pay a steep time tax.

**Strictness**: Configurable via `tsconfig.json`. Agents benefit from stricter settings (`strict: true`, `noImplicitAny: true`) because they produce more errors — which sounds bad but is actually good. More errors means more feedback, which means fewer silent failures that only surface later.

### Python: mypy, pyright, pytype

Python's type checking story is fragmented across three major tools:

| Tool | Author | Speed | Strictness | Key Strength |
|------|--------|-------|------------|--------------|
| **mypy** | Dropbox/Community | Moderate | Configurable | The standard; widest adoption |
| **pyright** | Microsoft | Fast | Strict by default | VS Code integration; inference engine |
| **pytype** | Google | Slow | Lenient | Infers types from runtime behavior |

```bash
# mypy — the standard
mypy src/ --strict

# mypy daemon — persistent process, incremental checking
dmypy run -- src/ --strict

# pyright — faster, stricter
pyright src/

# pytype — Google's type checker (lenient, infers from runtime)
pytype src/
```

**mypy** is what most agents will encounter because it's the most widely configured in `pyproject.toml` and `mypy.ini` files. Its daemon mode (`dmypy`) is particularly relevant for agents — it maintains a persistent analysis cache that makes subsequent checks near-instant.

**pyright** is increasingly important because of its VS Code integration. Agents that operate within VS Code (like **Gemini CLI** in companion mode) get pyright diagnostics for free through the editor's language server.

### Go: Built Into the Compiler

Go's type checker is not a separate tool — it's the compiler itself. `go build` and `go vet` perform type checking as part of compilation:

```bash
# Type-check + compile (the standard)
go build ./...

# Type-check + additional static analysis (stricter)
go vet ./...
```

**Speed**: Extremely fast. Go was designed for fast compilation. Even large projects type-check in seconds. This makes Go an ideal language for agent edit-check loops — the feedback cycle is tight.

**Implication for agents**: There is no separate "type check" step for Go. The agent simply runs `go build` and parses the errors. This means compilation errors and type errors are interleaved in the output, which is actually simpler for agents to handle (one command, one error stream).

### Rust: `cargo check`

Rust's type system is famously strict — ownership, borrowing, lifetimes, trait bounds. `cargo check` runs the compiler front-end (type checking, borrow checking) without generating machine code:

```bash
# Type-check only — skips codegen, ~2-5x faster than cargo build
cargo check

# With all warnings enabled
cargo check 2>&1

# Check a specific package in a workspace
cargo check -p my-crate
```

**Speed**: `cargo check` is significantly faster than `cargo build` because it skips LLVM code generation. For a medium Rust project: `cargo check` ~5-10s vs `cargo build` ~30-60s. This difference is critical for agents — it means the feedback loop is 3-6x tighter.

**Error quality**: Rust's compiler errors are famously helpful. They include not just what went wrong, but often suggest the exact fix:

```
error[E0308]: mismatched types
  --> src/main.rs:15:20
   |
15 |     let count: i32 = get_name();
   |                ---   ^^^^^^^^^^ expected `i32`, found `String`
   |                |
   |                expected due to this
   |
help: consider using a different type
   |
15 |     let count: String = get_name();
   |                ~~~~~~
```

This "help" annotation is gold for LLMs — the compiler literally tells the agent what to type.

### Java: `javac` and Beyond

Java's type system is enforced by `javac`, but the ecosystem has additional static analysis tools:

```bash
# Standard compilation (includes type checking)
javac -sourcepath src -d out src/**/*.java

# With Gradle (most common in practice)
./gradlew compileJava

# With Maven
mvn compile
```

**SpotBugs** and **Error Prone** extend Java's type checking with deeper analysis (null safety, concurrency bugs, API misuse), but agents rarely encounter these unless the project is already configured to use them.

---

## How Agents Integrate Type Checkers

The integration patterns fall into three categories: **shell-based** (most common), **LSP-based** (most powerful), and **IDE-based** (most context-rich).

### Pattern 1: Shell Execution (Most Common)

The overwhelming majority of agents run type checkers as shell commands via their Bash tool. The workflow is straightforward:

```
┌──────────────────────────────────────────────────────────────────┐
│              SHELL-BASED TYPE CHECKING LOOP                       │
│                                                                   │
│   1. Agent edits file(s) via file-editing tool                    │
│   2. Agent calls Bash tool: "tsc --noEmit" / "mypy src/"         │
│   3. Agent parses stdout/stderr for error messages                │
│   4. Agent maps errors to files/lines it just edited              │
│   5. Agent applies fixes                                          │
│   6. Agent re-runs type checker to verify                         │
│   7. Repeat until clean (or give up after N iterations)           │
│                                                                   │
│   Used by: Most agents (ForgeCode, Codex, OpenCode, Aider,       │
│            Goose, Warp, mini-SWE-agent, OpenHands, Capy,          │
│            Pi Coding Agent, Sage Agent, Ante, Droid, TongAgents)  │
└──────────────────────────────────────────────────────────────────┘
```

The key insight is that this is **not special-cased**. Agents don't have a dedicated "type check" action. They run `tsc --noEmit` or `cargo check` through the same Bash tool they use for everything else. The LLM decides *when* to type-check based on its understanding of the project — it sees a `tsconfig.json` and knows to run `tsc`, sees a `Cargo.toml` and knows to run `cargo check`.

### Pattern 2: LSP Integration (Most Powerful)

A few agents go beyond shell commands and integrate with **Language Server Protocol** for real-time type feedback:

**Claude Code** supports LSP plugins that provide type checking diagnostics. Instead of running `tsc --noEmit` and parsing text output, Claude Code can receive structured diagnostic objects from the TypeScript language server:

```typescript
// LSP diagnostic (what Claude Code can receive via plugin)
{
  "uri": "file:///src/utils.ts",
  "diagnostics": [
    {
      "range": { "start": { "line": 14, "character": 8 },
                 "end": { "line": 14, "character": 22 } },
      "severity": 1,  // Error
      "code": 2322,
      "source": "ts",
      "message": "Type 'string' is not assignable to type 'number'."
    }
  ]
}
```

This structured data is superior to parsing text because:
- **Exact ranges**: the agent knows exactly which characters are wrong
- **Error codes**: enables programmatic classification of error types
- **No parsing ambiguity**: JSON vs regex-matching compiler output

### Pattern 3: IDE Integration (Most Context-Rich)

**Junie CLI** operates in two distinct modes with fundamentally different type checking capabilities:

- **IDE mode** (running within JetBrains): Full access to the JetBrains inspections engine, which includes the **PSI tree** (Program Structure Interface) — JetBrains' language-aware AST. This provides not just type errors but deep semantic analysis: unreachable code, redundant casts, missing null checks, incorrect generic bounds. The PSI tree is essentially a running compiler that updates incrementally as the agent edits code.

- **CLI mode** (standalone terminal): Falls back to shell-based type checking, same as most other agents. The quality gap is significant — IDE mode catches errors that CLI mode misses entirely.

**Gemini CLI** takes a different approach through its VS Code companion extension. When running inside VS Code, the extension shares the editor's diagnostics (which include type errors from TypeScript, pyright, and other language extensions) with the agent:

```
┌───────────────────────────────────────────────────────────────┐
│                GEMINI CLI + VS CODE                            │
│                                                                │
│   VS Code                          Gemini CLI                  │
│   ┌──────────────┐                 ┌──────────────┐           │
│   │ TypeScript   │  diagnostics    │              │           │
│   │ Language     │ ───────────────>│  Agent sees   │           │
│   │ Server       │  (via ext.)     │  type errors  │           │
│   └──────────────┘                 │  in real time │           │
│   ┌──────────────┐                 │              │           │
│   │ Pyright      │  diagnostics    │  No need to   │           │
│   │ Extension    │ ───────────────>│  run tsc or   │           │
│   └──────────────┘                 │  mypy manually│           │
│                                    └──────────────┘           │
└───────────────────────────────────────────────────────────────┘
```

This is the most seamless integration: the agent gets type errors as a side effect of the editor already running language servers. No extra commands, no parsing, no latency.

---

## Incremental Type Checking

Running a full type check on every edit is prohibitively expensive for large projects. A 2000-file TypeScript monorepo takes 30-60 seconds for `tsc --noEmit`. If an agent makes 15 edits in a session, that's 7-15 minutes spent just type-checking. Incremental strategies are essential.

### TypeScript Incremental Modes

```bash
# --incremental saves a .tsbuildinfo file with compilation state
# Subsequent runs only re-check changed files and their dependents
tsc --noEmit --incremental

# --build mode for project references (monorepo-aware)
# Only rebuilds projects affected by changes
tsc --build --noEmit
```

**Agent implication**: An agent that runs `tsc --noEmit --incremental` on its first check pays the full cost once, then gets near-instant feedback on subsequent checks. This makes "check after every edit" viable even for large projects.

### mypy Daemon

```bash
# Start the daemon (stays resident in memory)
dmypy start

# Check files (uses daemon's cached analysis)
dmypy run -- src/ --strict

# Status check
dmypy status
```

The mypy daemon keeps the full analysis graph in memory. When a file changes, it only re-analyzes that file and its transitive dependents. For a large Python project, this reduces check time from 30+ seconds to under 2 seconds.

**Agent challenge**: Agents typically don't persist the daemon across invocations. Each new agent session starts cold. This is a missed optimization — a smarter agent runtime could pre-start `dmypy` when it detects a Python project with mypy configuration.

### pyright: Fast by Default

pyright is architecturally fast — it uses a custom TypeScript-based analysis engine that's optimized for incremental checking. Even without daemon mode, pyright checks a large Python project in 2-5 seconds. This makes it the preferred choice for agent integration when speed matters more than ecosystem compatibility.

### cargo check: Skip the Codegen

```bash
# cargo check skips LLVM codegen — the slowest part of Rust compilation
# Typical speedup: 3-6x over cargo build
cargo check 2>&1
```

For Rust agents, always preferring `cargo check` over `cargo build` is a critical optimization. The type checker and borrow checker run during the front-end analysis phase; code generation is irrelevant for verification purposes.

---

## Type Error Resolution Loops

The core agent workflow for type errors is a tight loop: edit → check → fix → re-check. But the details of this loop matter enormously for success rates.

### Common Type Errors Agents Produce

Agents generate a predictable distribution of type errors. The most common categories:

| Error Category | Frequency | Typical Cause | Fix Difficulty |
|---------------|-----------|---------------|----------------|
| **Wrong argument types** | Very High | LLM guesses parameter types | Easy — error says expected vs actual |
| **Missing interface members** | High | Incomplete implementation of interface/trait | Easy — error lists missing members |
| **Incompatible return types** | High | Function returns wrong variant | Easy — change return value or type annotation |
| **Nullable access** | Medium | Forgetting to handle `null`/`undefined`/`Option` | Medium — requires control flow change |
| **Wrong generic params** | Medium | Complex generic types confuse LLM | Hard — may require understanding type relationships |
| **Incompatible union types** | Low | Type narrowing mistakes | Hard — requires understanding discriminated unions |
| **Lifetime/borrow errors** | Low (Rust) | Ownership semantics | Very Hard — may require structural redesign |

### The Fix Loop in Practice

Here's a concrete TypeScript example of an agent's type-checking loop:

```typescript
// Step 1: Agent writes this code (has a type error)
interface User {
  id: number;
  name: string;
  email: string;
}

function formatUser(user: User): string {
  return `${user.name} <${user.mail}>`;  // ← 'mail' doesn't exist on User
}

function getUserAge(user: User): number {
  return user.name;  // ← Type 'string' not assignable to 'number'
}
```

```bash
# Step 2: Agent runs type checker
$ tsc --noEmit

src/users.ts(9,29): error TS2551: Property 'mail' does not exist on type
  'User'. Did you mean 'email'?
src/users.ts(13,3): error TS2322: Type 'string' is not assignable to
  type 'number'.
```

```typescript
// Step 3: Agent reads errors and applies fixes
// Fix 1: 'mail' → 'email' (compiler even suggested the fix!)
// Fix 2: return user.name → need to rethink (no 'age' field exists)

function formatUser(user: User): string {
  return `${user.name} <${user.email}>`;  // ← Fixed: mail → email
}

// Agent realizes User has no 'age' — must add it or change return type
interface User {
  id: number;
  name: string;
  email: string;
  age: number;  // ← Added missing field
}

function getUserAge(user: User): number {
  return user.age;  // ← Fixed: returns number from number field
}
```

```bash
# Step 4: Agent re-runs type checker
$ tsc --noEmit
# (no output — clean!)
```

### Cascading Type Errors

The trickiest scenario is **cascading type changes** — fixing one type error creates new ones elsewhere. Consider:

```python
# Agent changes return type of get_user()
def get_user(id: int) -> User | None:  # was: -> User
    ...

# Now every call site that doesn't handle None is broken:
# mypy output:
# app.py:45: error: Item "None" of "User | None" has no attribute "name"
# app.py:67: error: Argument 1 to "send_email" has incompatible type
#   "User | None"; expected "User"
# app.py:89: error: Item "None" of "User | None" has no attribute "id"
```

A naive agent fixes each error independently. A smart agent recognizes the pattern: "I changed a return type to be nullable — I need to add null checks at every call site." The difference between these strategies is the difference between 3 fix iterations and 1.

### mypy Error Parsing

Agents parse mypy output to extract structured error information:

```bash
# mypy output format:
# file.py:LINE: error: MESSAGE  [error-code]

$ mypy src/ --strict
src/api/routes.py:23: error: Incompatible return value type
  (got "str", expected "int")  [return-value]
src/api/routes.py:45: error: Missing named argument "timeout"
  for "fetch_data"  [call-arg]
src/models/user.py:12: error: "User" has no attribute "full_name"
  [attr-defined]
Found 3 errors in 2 files (checked 47 source files)
```

The structured format — `file:line: error: message [code]` — is easy for agents to parse. The error codes (`return-value`, `call-arg`, `attr-defined`) help the agent categorize errors and apply appropriate fix strategies.

### cargo check Output Parsing

Rust's compiler output is more verbose but exceptionally informative:

```bash
$ cargo check 2>&1
error[E0599]: no method named `push_str` found for type `Vec<String>`
              in the current scope
  --> src/builder.rs:42:14
   |
42 |     self.items.push_str(&new_item);
   |                ^^^^^^^^ method not found in `Vec<String>`
   |
   = help: items of similar names exist: `push`

error[E0308]: mismatched types
  --> src/config.rs:18:24
   |
18 |     let timeout: u64 = config.get("timeout");
   |                  ---   ^^^^^^^^^^^^^^^^^^^^^^
   |                  |     expected `u64`, found `Option<&str>`
   |                  expected due to this
   |
help: consider using `Option::map` to convert
   |
18 |     let timeout: u64 = config.get("timeout").map(|v| v.parse().unwrap());
   |                        ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

error: aborting due to 2 previous errors
```

Note how Rust's compiler goes beyond stating the problem — it suggests `push` instead of `push_str`, and even provides a complete code suggestion for the type conversion. LLMs leverage these suggestions directly, often copying the compiler's recommendation verbatim.

---

## Typed vs Untyped Codebases

The presence or absence of type annotations fundamentally changes how well an agent performs. This is not a minor difference — it's one of the strongest predictors of agent success.

### The Constraint Hypothesis

Type systems work as **constraint satisfaction**. In an untyped codebase, the agent must infer what types are expected from context, naming conventions, and runtime behavior. In a typed codebase, the constraints are explicit:

```
┌─────────────────────────────────────────────────────────────────┐
│              SOLUTION SPACE VISUALIZATION                        │
│                                                                  │
│   Untyped (JavaScript):                                          │
│   ┌─────────────────────────────────────────────┐               │
│   │                                             │               │
│   │   Any value can flow anywhere.              │               │
│   │   Enormous solution space.                  │               │
│   │   Agent must guess from context.            │               │
│   │                                             │               │
│   │        ╔═══════╗                            │               │
│   │        ║Correct║ (tiny region)              │               │
│   │        ╚═══════╝                            │               │
│   └─────────────────────────────────────────────┘               │
│                                                                  │
│   Typed (TypeScript strict):                                     │
│   ┌──────────────┐                                               │
│   │              │  Solution space is much smaller.              │
│   │  ╔═══════╗   │  Type checker rejects invalid solutions.     │
│   │  ║Correct║   │  Agent has fewer wrong options.              │
│   │  ╚═══════╝   │                                               │
│   └──────────────┘                                               │
└─────────────────────────────────────────────────────────────────┘
```

### Empirical Evidence

While rigorous benchmarks are still emerging, practitioners consistently report that agents make significantly fewer errors on TypeScript projects compared to equivalent JavaScript projects. The mechanism is straightforward:

1. **Early detection**: type errors surface immediately after editing, not at runtime
2. **Precise feedback**: "expected number, got string" vs "TypeError: undefined is not a function"
3. **Compiler-guided fixes**: the agent doesn't need to reason about what went wrong — the type checker tells it
4. **Interface contracts**: types serve as documentation that the agent can read mechanically

### The Gradual Typing Challenge

Real codebases are rarely fully typed or fully untyped. Most TypeScript projects have some `any` types; most Python projects with type hints have gaps in coverage. This creates a **partial verification** problem — the type checker validates some paths but not others:

```python
# Partially typed Python — mypy catches some errors but not all
def process_data(items: list[dict]) -> list[str]:
    # mypy knows items is list[dict] and return is list[str]
    results = []
    for item in items:
        # But mypy doesn't know what keys 'item' has
        # This will NOT be caught:
        results.append(item["naem"])  # typo in key name
    return results
```

For agents, gradual typing means the type checker is a **necessary but insufficient** verification layer. It catches structural type errors but misses semantic ones in untyped regions. Smart agents combine type checking with other strategies — running tests, using linters, or asking the LLM to self-review — to cover the gaps.

---

## LSP Integration for Type Feedback

The Language Server Protocol provides the richest type feedback available, but few agents fully exploit it. Understanding why requires examining what LSP offers and what it costs.

### What LSP Provides

An LSP-connected agent receives:

```typescript
// Pseudocode: LSP diagnostic integration
interface TypeDiagnostic {
  file: string;
  range: {
    startLine: number;
    startCol: number;
    endLine: number;
    endCol: number;
  };
  severity: "error" | "warning" | "info" | "hint";
  code: number | string;
  source: string;  // "ts", "pyright", "rust-analyzer"
  message: string;
  relatedInformation?: {
    location: { file: string; range: Range };
    message: string;
  }[];
}

// Agent receives these after every edit — no explicit "run tsc" needed
function onDiagnosticsReceived(diagnostics: TypeDiagnostic[]) {
  const errors = diagnostics.filter(d => d.severity === "error");
  if (errors.length > 0) {
    // Feed errors back to LLM for fixing
    const errorContext = errors.map(e =>
      `${e.file}:${e.range.startLine}: ${e.message}`
    ).join("\n");
    return fixErrors(errorContext);
  }
}
```

### LSP Advantages Over Shell-Based Checking

| Aspect | Shell (`tsc --noEmit`) | LSP (tsserver) |
|--------|----------------------|----------------|
| **Latency** | 5-30s full check | <1s incremental |
| **Precision** | File:line:column | Exact character ranges |
| **Structured data** | Text parsing required | JSON objects |
| **Incremental** | With `--incremental` flag | Always incremental |
| **Additional info** | Error message only | Related locations, fix suggestions |
| **Setup cost** | None (just run command) | Language server must be running |

### Why Most Agents Don't Use LSP

Despite its advantages, most agents stick to shell-based type checking for practical reasons:

1. **Startup latency**: Language servers take 3-30 seconds to initialize and index a project. For a quick agent task, this exceeds the type-checking time saved.
2. **Complexity**: Running an LSP client requires managing a child process, handling JSON-RPC messages, tracking document state, and handling server crashes. Shell commands are one line.
3. **Portability**: `tsc --noEmit` works everywhere. LSP requires the right language server to be installed and configured for the specific project.
4. **Sufficient signal**: For most agent tasks, the text output of `tsc` or `mypy` is perfectly adequate. The agent doesn't need character-level precision — file-and-line is enough to locate and fix the error.

The agents that *do* use LSP — **Claude Code** (via plugins), **Junie CLI** (via JetBrains platform), **Gemini CLI** (via VS Code extension) — do so because they operate within environments where the language server is already running. They don't start LSP for type checking; they piggyback on an existing IDE integration.

---

## Type Checking as Build Verification

For compiled languages, the distinction between "type checking" and "building" is blurred or nonexistent. This affects how agents decide when and how to verify their changes.

### Compiled Languages: Type Check IS Build

```bash
# Go: compilation includes type checking — there's no separate step
go build ./...

# Rust: cargo check is type-check-only; cargo build includes codegen
cargo check     # fast: type + borrow check only
cargo build     # slow: type + borrow check + LLVM codegen

# Java: javac performs type checking during compilation
javac -d out src/**/*.java

# C/C++: gcc/clang type-check as part of compilation
gcc -fsyntax-only src/*.c    # syntax + type check only
gcc -c src/*.c               # full compilation to object files
```

**Agent strategy**: For Go and Java, the agent should always build after editing — the build *is* the type check. For Rust, prefer `cargo check` over `cargo build` to skip unnecessary codegen. For TypeScript, `tsc --noEmit` avoids generating JavaScript files that might interfere with the project's build system.

### Detecting When to Type Check

Agents don't hard-code type checker commands. Instead, they infer the appropriate command from project configuration files:

```
┌────────────────────────────────────────────────────────────────┐
│              PROJECT DETECTION → TYPE CHECKER                   │
│                                                                 │
│   Config File Found         Type Check Command                  │
│   ─────────────────         ──────────────────                  │
│   tsconfig.json        →    tsc --noEmit                        │
│   pyproject.toml +          mypy src/ (or pyright)              │
│     [tool.mypy]                                                 │
│   mypy.ini             →    mypy .                              │
│   Cargo.toml           →    cargo check                         │
│   go.mod               →    go build ./...                      │
│   pom.xml              →    mvn compile                         │
│   build.gradle         →    ./gradlew compileJava               │
│   .eslintrc +               (linting, not type checking —       │
│     no tsconfig             agent should NOT run tsc)            │
└────────────────────────────────────────────────────────────────┘
```

This detection is typically done by the LLM itself — it reads the project root, sees configuration files in its context, and decides which verification commands to run. More sophisticated agents like **ForgeCode** and **OpenHands** have explicit project detection logic that identifies the build system and configures verification commands programmatically.

---

## The Edit-Check-Fix Strategy

The highest-level question is: **when should an agent run the type checker?** Three strategies exist, with different trade-offs:

### Strategy 1: Check After Every Edit

```
edit file A → tsc → fix errors → tsc → edit file B → tsc → fix errors → tsc → done
```

**Pros**: Errors are caught immediately, before they cascade. Each fix is small and targeted.
**Cons**: Expensive in time. N edits × type-check latency adds up fast.
**Best for**: Small changes, compiled languages with fast type checkers (Go, Rust with `cargo check`).

### Strategy 2: Check After All Edits

```
edit file A → edit file B → edit file C → tsc → fix all errors → tsc → done
```

**Pros**: Only 2 type-checker invocations (check + verify). Maximally efficient.
**Cons**: Errors may interact. Fixing one error may create others. Harder to attribute errors to specific edits.
**Best for**: Confident changes, agents with strong type reasoning, small scoped tasks.

### Strategy 3: Check at Milestones

```
edit A → edit B → tsc → fix → edit C → edit D → tsc → fix → done
```

**Pros**: Balanced approach. Groups related edits, checks between logical units.
**Cons**: Requires the agent to identify "milestone" boundaries.
**Best for**: Multi-file refactorings, tasks with natural phase boundaries.

Most production agents use **Strategy 1** or **Strategy 3**. The time cost of type checking is almost always worth the error-prevention benefit. Agents that defer all checking to the end frequently enter long fix loops where each fix introduces new errors — the very cascade effect that incremental checking prevents.

---

## Cross-Agent Type Checking Comparison

How do the 17 studied agents approach type checking? The table below synthesizes each agent's strategy:

| Agent | Primary Method | LSP/IDE Integration | Incremental Support | Type-Check Trigger | Notes |
|-------|---------------|--------------------|--------------------|-------------------|-------|
| [**ForgeCode**](../agents/forgecode/) | Shell (Bash tool) | No | Via project build | After verification step | Service-oriented; verification is a required pipeline stage |
| [**Claude Code**](../agents/claude-code/) | Shell + LSP plugins | Yes (LSP plugins) | Via LSP | After edits + on-demand | "Check for type errors via LSP" is an explicit plugin capability |
| [**Codex**](../agents/codex/) | Shell (sandboxed) | No | Via `--incremental` flag | After edits | Runs in Docker sandbox; full `tsc`/`cargo check` via Bash |
| [**Droid**](../agents/droid/) | Shell (Bash tool) | No | Project-dependent | Agent-decided | General-purpose shell execution for verification |
| [**Ante**](../agents/ante/) | Shell (Bash tool) | No | Project-dependent | Agent-decided | Minimal tool set; relies on LLM judgment for when to check |
| [**OpenCode**](../agents/opencode/) | Shell (persistent) | No | Via persistent shell state | Agent-decided | Persistent bash session preserves incremental build state |
| [**OpenHands**](../agents/openhands/) | Shell (sandboxed) | No | Via sandbox state | After edits | Docker sandbox; type checker state persists within session |
| [**Warp**](../agents/warp/) | Shell (Bash tool) | No | Project-dependent | Agent-decided | Terminal-native; strong shell integration |
| [**Gemini CLI**](../agents/gemini-cli/) | Shell + VS Code | Yes (VS Code ext.) | Via editor LS | Automatic (VS Code) | Companion extension shares editor diagnostics with agent |
| [**Goose**](../agents/goose/) | Shell (Bash tool) | No | Project-dependent | Agent-decided | Extensible via plugins; could add LSP via MCP |
| [**Junie CLI**](../agents/junie-cli/) | Shell + JetBrains | Yes (PSI tree) | Via IDE engine | Automatic (IDE mode) | IDE mode: full inspections; CLI mode: shell fallback |
| [**mini-SWE-agent**](../agents/mini-swe-agent/) | Shell (only tool) | No | No | Agent-decided | Single bash tool; `tsc`/`mypy` via shell only |
| [**Pi Coding Agent**](../agents/pi-coding-agent/) | Shell (Bash tool) | No | Project-dependent | Agent-decided | Standard shell-based verification |
| [**Aider**](../agents/aider/) | Shell (lint cmd) | No | Via `--lint-cmd` config | After every edit | Configurable lint/type-check command runs automatically |
| [**Sage Agent**](../agents/sage-agent/) | Shell (Bash tool) | No | Project-dependent | Agent-decided | Research-oriented; standard shell verification |
| [**TongAgents**](../agents/tongagents/) | Shell (Bash tool) | No | No | Agent-decided | Multi-agent; type checking delegated to coding agent |
| [**Capy**](../agents/capy/) | Shell (Bash tool) | No | Project-dependent | Agent-decided | Standard shell-based verification pattern |

### Key Observations

1. **Shell dominates**: 14 of 17 agents use shell-only type checking. The Bash tool is the universal type-checker interface.

2. **IDE integration is rare but powerful**: Only **Claude Code**, **Junie CLI**, and **Gemini CLI** have native IDE/LSP integration, and each approaches it differently (plugins, PSI tree, VS Code extension).

3. **Aider is unique**: It's the only agent with a dedicated `--lint-cmd` configuration that automatically runs a user-specified type-check command after every edit. This is the closest to "type checking as a first-class feature."

4. **No agent runs multiple type checkers**: No agent automatically runs both `mypy` and `pyright`, or both `tsc` and `eslint`. Agents pick one and stick with it.

5. **Incremental checking is incidental**: No agent explicitly manages incremental type-checking state. Agents that benefit from incrementality (OpenCode's persistent shell, OpenHands' sandbox) do so as a side effect of their execution environment, not by design.

---

## Implications and Future Directions

Type checking integration in coding agents is surprisingly shallow. Most agents treat type checkers as just another shell command — no different from `ls` or `cat`. This works, but it leaves significant value on the table.

**What deeper integration would look like:**

1. **Pre-started language servers**: Agent runtimes could start `tsserver` or `pyright` when they detect a typed project, so the first type check is instant instead of cold-start.

2. **Structured error consumption**: Instead of parsing text output with regex, agents could consume compiler output in JSON format (`tsc --pretty false`, `mypy --output json`) for reliable error extraction.

3. **Type-aware editing**: An agent that understands the type system could predict type errors before making edits — "if I change this return type, these 7 call sites will break" — and fix them proactively.

4. **Multi-checker pipelines**: Running `tsc` + `eslint` + custom type guards in sequence, with each layer catching different error classes.

5. **Cross-file type propagation**: When an agent changes a type definition, automatically identifying and updating all dependent code without waiting for the type checker to report errors one at a time.

The agents that solve these problems first — making type checking instantaneous, structured, and proactive rather than slow, text-parsed, and reactive — will have a measurable advantage on typed codebases. The signal is already there. The question is who integrates it most deeply.