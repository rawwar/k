---
title: CI/CD Integration
status: complete
---

# CI/CD Integration

> How coding agents participate in continuous integration and delivery pipelines—creating
> pull requests, diagnosing CI failures, triaging issues, and operating autonomously in
> automated workflows without human interaction.

---

## Why CI/CD Integration Matters

Coding agents are no longer confined to the interactive terminal session. The most impactful
shift in agent design over the past year is the move from **developer tool** to **development
pipeline participant**. An agent that can only write code when a human is watching is
fundamentally limited; an agent that can respond to CI failures at 3 AM, triage incoming
issues, review pull requests, and push fixes autonomously is a qualitatively different tool.

CI/CD integration enables agents to:

- **Create pull requests** from issue descriptions or feature requests
- **Respond to CI failures** by analyzing logs, diagnosing root causes, and pushing fixes
- **Perform automated code review** with inline comments and suggestions
- **Triage issues** with labeling, prioritization, and initial analysis
- **Run scheduled maintenance** tasks on cron triggers
- **Operate in event-driven pipelines** triggered by webhooks, comments, or pushes

```
    Traditional Agent Workflow          CI/CD-Integrated Agent Workflow
    ┌───────────────────────┐           ┌──────────────────────────────────┐
    │                       │           │                                  │
    │   Human ──► Agent     │           │   Event ──► Agent ──► PR/Fix    │
    │     │         │       │           │     ▲                   │       │
    │     │         ▼       │           │     │                   ▼       │
    │     │      Terminal   │           │     │        ┌──── CI Pipeline  │
    │     │      Output     │           │     │        │         │        │
    │     ▼                 │           │     │        │         ▼        │
    │   Human reviews       │           │   Webhook ◄──┘    Pass/Fail    │
    │   locally             │           │     │                   │       │
    │                       │           │     └───────────────────┘       │
    └───────────────────────┘           │     (autonomous loop)          │
                                        └──────────────────────────────────┘
```

The **autonomous loop** is the key architectural difference. In a CI/CD context, the agent
must handle the full cycle without a human in the loop: receive trigger → understand context
→ execute changes → verify via CI → iterate if needed. This requires robust
[non-interactive modes](#headlessnon-interactive-modes), structured output, and reliable
error handling.

---

## GitHub Actions Integration

GitHub Actions is the dominant platform for agent-driven CI/CD workflows. Several agents
ship first-party GitHub Actions that wrap their CLI tools for use in automated pipelines.

### Claude Code: claude-code-action

The `anthropics/claude-code-action` action is a **general-purpose GitHub Action** that
enables Claude Code to participate in PR reviews, issue triage, and code implementation
directly from GitHub events.

**Key capabilities:**
- Responds to `@claude` mentions in PR comments and issue threads
- Performs automated code review with inline diff comments
- Implements changes requested in issues and creates PRs
- Supports Anthropic API, AWS Bedrock, and Google Vertex AI backends
- Uses `/install-github-app` for streamlined repository setup

```yaml
# .github/workflows/claude-code.yml
name: Claude Code Agent
on:
  issue_comment:
    types: [created]
  pull_request_review_comment:
    types: [created]
  issues:
    types: [opened, labeled]

jobs:
  claude:
    if: |
      (github.event_name == 'issue_comment' && contains(github.event.comment.body, '@claude')) ||
      (github.event_name == 'pull_request_review_comment' && contains(github.event.comment.body, '@claude')) ||
      (github.event_name == 'issues' && contains(github.event.issue.labels.*.name, 'claude'))
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pull-requests: write
      issues: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 1

      - uses: anthropics/claude-code-action@v1
        with:
          anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
          # Optional: use Bedrock instead
          # use_bedrock: "true"
          # aws_access_key_id: ${{ secrets.AWS_ACCESS_KEY }}
          # aws_secret_access_key: ${{ secrets.AWS_SECRET_KEY }}
          # aws_region: "us-east-1"
```

The action's architecture is notable: it **wraps the full Claude Code CLI** in a container,
meaning the agent has access to the same tool suite (file editing, bash execution, search)
as in interactive mode. The key difference is the permission model—in CI, the agent runs
with `--dangerously-skip-permissions` or equivalent bypass flags, since there is no human
to approve tool calls.

### Gemini CLI: gemini-cli-action

Google's `google-github-actions/run-gemini-cli` action provides similar functionality for
the Gemini CLI agent, with particular strength in **PR review** and **issue triage**.

```yaml
# .github/workflows/gemini-review.yml
name: Gemini CLI Review
on:
  pull_request:
    types: [opened, synchronize]
  issue_comment:
    types: [created]

jobs:
  review:
    if: |
      github.event_name == 'pull_request' ||
      (github.event_name == 'issue_comment' && contains(github.event.comment.body, '@gemini-cli'))
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: write
      issues: write
    steps:
      - uses: actions/checkout@v4

      - uses: google-github-actions/run-gemini-cli@main
        with:
          gemini_api_key: ${{ secrets.GEMINI_API_KEY }}
          prompt: |
            Review this pull request for correctness, security issues,
            and adherence to project conventions. Provide inline comments
            on specific lines where improvements are needed.
```

Gemini CLI's action supports **custom workflow prompts**, allowing teams to define
domain-specific review criteria. The `--output-format json` flag enables structured
output parsing for downstream pipeline steps.

### Droid: First-Class CI/CD Citizen

[Droid](../../agents/droid/) distinguishes itself by treating CI/CD as a **primary design
goal** rather than an afterthought. Its `.droid.yaml` configuration file defines automated
behaviors declaratively:

```yaml
# .droid.yaml
version: 1

auto_review:
  enabled: true
  trigger:
    - on: pull_request
      actions: [opened, synchronize]
  filter:
    paths:
      - "src/**"
      - "!src/generated/**"
    labels:
      exclude: ["skip-review"]
  review:
    focus:
      - correctness
      - security
      - performance
    severity_threshold: warning

ci_repair:
  enabled: true
  trigger:
    - on: check_suite
      conclusion: failure
  repair:
    max_attempts: 3
    auto_push: true
    require_ci_pass: true

issue_triage:
  enabled: true
  trigger:
    - on: issues
      actions: [opened]
  triage:
    auto_label: true
    priority_assessment: true
    assign_to_team: true

scheduled:
  - cron: "0 9 * * 1"
    task: dependency_audit
  - cron: "0 0 * * *"
    task: stale_branch_cleanup
```

Droid's `auto_review` loop follows a structured pipeline: **trigger on PR open → filter
by path and label conditions → analyze diff → generate review comments → optionally
trigger CI repair** if the review identifies issues. The `ci_repair` section enables
Droid's most distinctive feature: **automatic GitHub Actions failure diagnosis and repair**
(covered in [CI Failure Diagnosis and Repair](#ci-failure-diagnosis-and-repair)).

### OpenHands: openhands-resolver

[OpenHands](../../agents/openhands/) provides `openhands-resolver`, a purpose-built tool
for **automated issue resolution** in GitHub repositories:

```yaml
# .github/workflows/openhands-resolver.yml
name: OpenHands Issue Resolver
on:
  issues:
    types: [labeled]

jobs:
  resolve:
    if: contains(github.event.issue.labels.*.name, 'openhands')
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run OpenHands Resolver
        uses: all-hands-ai/openhands-resolver@main
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          llm_api_key: ${{ secrets.LLM_API_KEY }}
          llm_model: "anthropic/claude-sonnet-4-20250514"
          max_iterations: 30
```

The resolver operates by: reading the issue description → setting up a sandboxed workspace
→ running the full OpenHands agent loop → creating a PR with the proposed fix. It is
designed for **batch processing**: multiple issues can be resolved in parallel by labeling
them simultaneously.

---

## Headless/Non-Interactive Modes

For CI/CD pipelines, agents **must** operate without human interaction. A prompt asking
"Should I proceed?" in a GitHub Actions runner will hang indefinitely and eventually
timeout. Every serious CI/CD-capable agent provides a non-interactive mode.

```
    Interactive Mode                Non-Interactive Mode
    ┌──────────────────┐            ┌─────────────────────────┐
    │  Agent: "Edit     │            │  Agent receives task    │
    │  foo.js?"         │            │       │                 │
    │     │             │            │       ▼                 │
    │     ▼             │            │  Executes all tools     │
    │  Human: "Yes"     │            │  (permissions bypassed) │
    │     │             │            │       │                 │
    │     ▼             │            │       ▼                 │
    │  Agent edits      │            │  Returns structured     │
    │     │             │            │  output (JSON/exit code)│
    │     ▼             │            └─────────────────────────┘
    │  Agent: "Also     │
    │  modify bar.js?"  │
    │     │             │
    │     ▼             │
    │  Human: "No"      │
    └──────────────────┘
```

### Agent-by-Agent Non-Interactive Support

| Agent | Flag/Mode | Behavior | Output Format |
|-------|-----------|----------|---------------|
| [Claude Code](../../agents/claude-code/) | `-p` flag | Full agentic loop, no permission prompts | Streaming text or JSON |
| [Codex](../../agents/codex/) | `exec` mode / `exec-server` | Non-interactive over JSON-RPC/WebSocket | Structured JSON |
| [ForgeCode](../../agents/forgecode/) | Non-Interactive Mode | Rewrites system prompt to prohibit clarification | Text |
| [Gemini CLI](../../agents/gemini-cli/) | `-p` + `--output-format json` | Non-interactive with structured output | JSON |
| [Goose](../../agents/goose/) | Recipe mode | `RetryManager` handles failures automatically | Text/structured |
| [Pi Coding Agent](../../agents/pi-coding-agent/) | Print/JSON mode | CI pipeline output | JSON |
| [Droid](../../agents/droid/) | CI runner mode | Triggered by `.droid.yaml` events | Structured |
| [OpenHands](../../agents/openhands/) | Resolver mode | Headless issue resolution | PR + comments |
| [Warp](../../agents/warp/) | Plan execution | Runs plan steps with exit code observation | Text |

### Implementation Approaches

**Claude Code's `-p` flag** is the canonical example. When active, it runs the full agentic
loop (tool calls, file edits, bash execution) without pausing for confirmation. Combined
with `--dangerously-skip-permissions`, it opts out of the
[permission system](../tool-systems/permission-systems.md) entirely.

```bash
# Run Claude Code non-interactively in a CI pipeline
claude -p "Fix the failing test in src/auth.test.ts" \
  --output-format json \
  --dangerously-skip-permissions
```

**Codex's `exec-server`** maintains a warm JSON-RPC/WebSocket process that handles multiple
requests without cold-start overhead—suited for batch CI operations.

**ForgeCode's prompt rewriting** takes an unusual approach: it rewrites the system prompt
to prohibit clarification requests, a **prompt-level guarantee** that relies on model
instruction-following rather than architectural enforcement.

---

## PR Workflow Integration

Pull requests are the **natural interface** between agents and human development workflows.
They provide code review, CI verification, and a structured approval process that maps
cleanly to agent capabilities.

### The Agent-Driven PR Lifecycle

```
    ┌──────────────────────────────────────────────────────────────────┐
    │                    Agent-Driven PR Lifecycle                     │
    │                                                                  │
    │   ┌─────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
    │   │ Trigger  │───►│  Agent   │───►│  CI      │───►│  Human   │  │
    │   │ (issue,  │    │ Creates  │    │ Pipeline │    │ Review   │  │
    │   │  event,  │    │ PR       │    │ Runs     │    │          │  │
    │   │  mention)│    │          │    │          │    │          │  │
    │   └─────────┘    └──────────┘    └────┬─────┘    └────┬─────┘  │
    │                                       │               │         │
    │                                  Pass │          Approve│        │
    │                                       │               │         │
    │                                       ▼               ▼         │
    │                                  ┌──────────┐    ┌──────────┐  │
    │                                  │  Fail    │    │  Merge   │  │
    │                                  │          │    │          │  │
    │                                  └────┬─────┘    └──────────┘  │
    │                                       │                         │
    │                                       ▼                         │
    │                                  ┌──────────┐                   │
    │                                  │  Agent   │                   │
    │                                  │ Diagnoses│──── (loops back   │
    │                                  │ & Fixes  │     to CI)        │
    │                                  └──────────┘                   │
    └──────────────────────────────────────────────────────────────────┘
```

### Automated Code Review

Agents perform code review by analyzing PR diffs and posting inline comments:

- **Droid's auto_review loop**: Trigger on PR open → filter by path/label → analyze diff →
  generate review comments with severity levels → post as GitHub review → optionally trigger
  `ci_repair` if issues are found.
- **Claude Code**: Leverages the full agent toolset—checks out the branch, runs tests,
  reads related files for context, then posts a review. More heavyweight but contextually
  richer than diff-only analysis.
- **Gemini CLI**: Structured analysis with configurable review criteria (security,
  performance, style) specified in the workflow configuration.

### Pre-Merge Verification

A powerful pattern enabled by CI/CD integration: **run the agent as a pre-merge check**.

```yaml
# Use agent as a required status check
name: Agent Verification
on:
  pull_request:
    types: [opened, synchronize]
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Agent Review
        run: |
          claude -p "Review this PR for bugs, security issues, and \
            breaking changes. Exit with code 1 if critical issues found." \
            --output-format json > review.json
          
          # Parse result and set check status
          if jq -e '.has_critical_issues' review.json; then
            echo "::error::Agent found critical issues"
            exit 1
          fi
```

This makes the agent a **required status check** in branch protection rules, meaning
agent approval is required alongside human review and CI passage before merging.

---

## CI Failure Diagnosis and Repair

One of the highest-value CI/CD integrations is **automated failure diagnosis**. When a CI
pipeline fails, the agent can analyze logs, identify the root cause, and propose (or even
push) a fix.

### The CI-Fix Loop

```
    Push ──► CI Runs ──► Failure Detected
                              │
                              ▼
                     ┌─────────────────┐
                     │  Fetch CI Logs  │
                     │  (stdout/stderr,│
                     │   exit codes,   │
                     │   test output)  │
                     └────────┬────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │  Agent Analyzes │
                     │  (parse errors, │
                     │   match to code,│
                     │   identify fix) │
                     └────────┬────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │  Push Fix       │
                     │  (commit to     │
                     │   same branch)  │
                     └────────┬────────┘
                              │
                              ▼
                     CI Runs Again ──► Pass? ──► Done
                         │
                         ▼
                     Fail? ──► Loop (max N attempts)
```

### Droid's github_action_repair

[Droid's](../../agents/droid/) `ci_repair` feature is the most complete implementation of
automated CI repair:

1. **Trigger**: GitHub check suite completes with `failure` conclusion
2. **Fetch**: Download workflow run logs via GitHub API
3. **Analyze**: Parse logs for error patterns, stack traces, test failures
4. **Diagnose**: Map errors to source code locations
5. **Fix**: Generate and apply code changes
6. **Push**: Commit fix to the failing branch
7. **Verify**: Wait for CI to re-run; retry up to `max_attempts` times

The `max_attempts` configuration is critical—without a bound, the agent could enter an
infinite fix-break loop. Droid defaults to 3 attempts before escalating to a human.

### Warp's Exit Code Observation

[Warp](../../agents/warp/) takes a simpler approach: its execution plans include explicit
test steps, and Warp observes exit codes to determine success or failure. When a step
fails, Warp can re-plan with the failure information included in context. While not a
full CI integration, this pattern is the building block for CI-aware behavior.

### Common Failure Patterns

| Failure Type | Detection Signal | Typical Agent Fix |
|-------------|-----------------|-------------------|
| Compilation error | Non-zero exit code + error message | Fix syntax/type errors |
| Test failure | Test framework output (FAIL, ✗) | Update test or fix code |
| Lint violation | Linter output with file:line format | Apply auto-fix or manual fix |
| Dependency issue | Resolution/install errors | Update lockfile or versions |
| Timeout | CI timeout signal | Optimize slow tests or increase limits |
| Permission denied | Auth/access error messages | Fix configuration, not code |

---

## Automated Issue Resolution

The most ambitious CI/CD integration pattern: **agents that automatically resolve issues**.

### OpenHands Resolver

[OpenHands](../../agents/openhands/) provides the most mature automated issue resolver.
The workflow:

1. **Issue labeled** with `openhands` tag (or configured trigger label)
2. **Resolver spins up** a sandboxed workspace with the repository
3. **Agent reads** issue title, description, and comments for context
4. **Agent executes** the full OpenHands loop: explore codebase → plan → edit → test
5. **Agent creates PR** with the proposed fix, linking to the original issue
6. **CI runs** on the PR; if it fails, the agent can iterate

This is **fully autonomous**: no human touches the keyboard between issue creation and PR
submission. The human's role shifts to **review and approval** rather than implementation.
### Gemini CLI Issue Triage

[Gemini CLI](../../agents/gemini-cli/) focuses on the **triage** side of issue management:

- **Automated labeling**: Analyze issue content and apply appropriate labels
- **Priority assessment**: Estimate severity and urgency from description
- **Duplicate detection**: Compare against existing open issues
- **Initial analysis**: Post a comment with preliminary analysis and suggested approach

This is lower-autonomy than OpenHands' full resolution but provides value without the
risk of autonomous code changes.

### Claude Code Issue Response

[Claude Code](../../agents/claude-code/) can be triggered from issue comments via
`claude-code-action`. When a user mentions `@claude` in an issue, the action:

1. Reads the issue context (title, description, conversation)
2. Checks out the repository
3. Runs Claude Code with the issue as the prompt
4. Creates a PR or posts a comment with analysis

---

## Status Checks and Branch Protection

Agent-generated PRs must pass the **same quality gates** as human-generated ones. This is
both a correctness concern and a security boundary.

### How Agents Interact with Required Checks

```
    Agent Creates PR
          │
          ▼
    ┌───────────────────────────────────────────────┐
    │              Branch Protection Rules            │
    │                                                 │
    │   ☑ Require status checks to pass              │
    │     ├── Unit tests        ✓ / ✗                │
    │     ├── Lint               ✓ / ✗                │
    │     ├── Type check         ✓ / ✗                │
    │     └── Agent review       ✓ / ✗  (optional)   │
    │                                                 │
    │   ☑ Require pull request reviews                │
    │     └── At least 1 human approval              │
    │                                                 │
    │   ☑ Require signed commits (optional)          │
    │     └── Agent must have GPG key configured     │
    │                                                 │
    │   ☑ Restrict who can push                      │
    │     └── Agent's GitHub App or bot account      │
    │                                                 │
    └───────────────────────────────────────────────┘
```

Key design principle: **agent-generated code is untrusted code**. Even if an agent created
a PR, human review should be required before merge. Branch protection rules enforce this
by requiring at least one human approval regardless of how the PR was created.

### Agent as Status Check

Some teams add the agent itself as a **required status check**. This creates a two-layer
review: the agent reviews human PRs, and humans review agent PRs. Both must pass before
merge.

---

## Pipeline Automation Patterns

Beyond PR workflows, agents integrate into broader automation patterns.

### Event-Driven Patterns

| Trigger | Event | Agent Action | Example |
|---------|-------|-------------|---------|
| Push to branch | `push` | Run analysis, check for regressions | Lint new code |
| PR opened | `pull_request` | Full code review | Droid auto_review |
| PR comment | `issue_comment` | Respond to requests | `@claude fix this` |
| Issue opened | `issues` | Triage or resolve | OpenHands resolver |
| CI failure | `check_suite` | Diagnose and repair | Droid ci_repair |
| Scheduled | `cron` | Maintenance tasks | Dependency updates |
| Webhook | External event | Custom automation | Slack → agent → PR |

### Scheduled Maintenance

[Droid's](../../agents/droid/) cron-triggered tasks demonstrate **proactive agent behavior**:

```yaml
# Droid scheduled maintenance
scheduled:
  - cron: "0 9 * * 1"        # Every Monday at 9am
    task: dependency_audit     # Check for outdated/vulnerable deps
  - cron: "0 0 * * *"        # Every day at midnight
    task: stale_branch_cleanup # Remove merged/stale branches
  - cron: "0 6 * * 3"        # Every Wednesday at 6am
    task: code_quality_report  # Generate quality metrics
```

### Batch Operations

Multiple issues can be resolved in parallel by labeling them simultaneously. OpenHands
resolver supports this natively—each labeled issue spawns an independent agent instance.
This enables **throughput scaling**: a team can label 10 issues and get 10 PRs.

### Webhook-Driven Workflows

Claude Code supports integration channels including Telegram, Discord, and custom webhooks.
This enables patterns like:

```
    Slack message: "Can someone fix the auth timeout bug?"
         │
         ▼
    Webhook ──► GitHub Action ──► Claude Code ──► PR created
         │
         ▼
    Slack notification: "PR #142 created to fix auth timeout"
```

---

## Security Considerations

CI/CD integration introduces significant security surface area. An agent operating
autonomously in a pipeline has access to secrets, can push code, and operates without
real-time human oversight.

### API Key Management

| Concern | Best Practice | Risk if Violated |
|---------|--------------|------------------|
| API key storage | GitHub Secrets / vault | Key leakage in logs |
| Key rotation | Regular rotation schedule | Compromised long-lived keys |
| Key scoping | Minimum required permissions | Over-privileged agent |
| Audit logging | Log all API key usage | Undetected misuse |

### Least-Privilege Principle

Agents in CI/CD should operate with **minimal permissions**:

- **Repository access**: Read + write to contents and PRs only
- **No admin access**: Agents should not manage repository settings
- **Scoped tokens**: Use fine-grained GitHub tokens, not classic PATs
- **Time-limited**: Tokens should expire; avoid permanent credentials

### Sandboxed Execution

[Codex](../../agents/codex/) provides the strongest sandboxing model for CI/CD: all agent
code execution occurs in an isolated sandbox with restricted network access and filesystem
isolation. This prevents malicious code generated by the LLM from escaping the sandbox to
compromise the CI environment.

```
    ┌─────────────────────────────────────────┐
    │            CI Runner (Host)              │
    │                                         │
    │   ┌─────────────────────────────────┐   │
    │   │        Agent Sandbox            │   │
    │   │                                 │   │
    │   │   ┌─────────┐  ┌────────────┐  │   │
    │   │   │  Agent   │  │  Code      │  │   │
    │   │   │  Process │  │  Execution │  │   │
    │   │   └─────────┘  └────────────┘  │   │
    │   │                                 │   │
    │   │   No network  │  Filesystem    │   │
    │   │   access      │  isolated      │   │
    │   └─────────────────────────────────┘   │
    │                                         │
    │   Secrets injected via env vars only    │
    └─────────────────────────────────────────┘
```

### Commit Signing

Agent-generated commits should be **signed** to maintain audit trails:

- GitHub Apps automatically sign commits with the app's identity
- Bot accounts can be configured with GPG keys
- Unsigned agent commits may fail branch protection rules that require signing
- Signed commits provide **non-repudiation**: you can verify which agent made which change

### Pull Request Security

Agent-generated code is **untrusted code**. Critical guardrails:

1. **Never auto-merge** agent PRs without human review
2. **Run the full CI suite** on agent PRs (same as human PRs)
3. **Review for prompt injection** — adversarial issue descriptions could manipulate the agent
4. **Restrict file scope** — agents should not modify CI configuration, secrets, or workflows
5. **Monitor for exfiltration** — ensure the agent doesn't leak secrets through code changes

---

## Cross-Agent CI/CD Capabilities

The following table summarizes CI/CD integration capabilities across all 17 agents
studied in this research library.

| Agent | GitHub Action | Non-Interactive Mode | PR Review | CI Repair | Issue Resolution | Scheduled Tasks |
|-------|:------------:|:-------------------:|:---------:|:---------:|:----------------:|:--------------:|
| [Aider](../../agents/aider/) | — | ✅ CLI flags | — | — | — | — |
| [Ante](../../agents/ante/) | — | ⚠️ Limited | — | — | — | — |
| [Capy](../../agents/capy/) | — | ⚠️ Limited | — | — | — | — |
| [Claude Code](../../agents/claude-code/) | ✅ anthropics/claude-code-action | ✅ `-p` flag | ✅ Inline | ⚠️ Via action | ✅ Via action | — |
| [Codex](../../agents/codex/) | — | ✅ exec/exec-server | — | — | — | — |
| [Droid](../../agents/droid/) | ✅ Built-in | ✅ .droid.yaml | ✅ auto_review | ✅ ci_repair | ✅ Issue triage | ✅ Cron |
| [ForgeCode](../../agents/forgecode/) | — | ✅ Non-Interactive Mode | — | — | — | — |
| [Gemini CLI](../../agents/gemini-cli/) | ✅ google-github-actions/run-gemini-cli | ✅ `-p` + `--output-format json` | ✅ PR review | — | ✅ Issue triage | — |
| [Goose](../../agents/goose/) | — | ✅ Recipe mode | — | — | — | — |
| [Junie CLI](../../agents/junie-cli/) | — | ⚠️ Limited | — | — | — | — |
| [Mini SWE Agent](../../agents/mini-swe-agent/) | — | ✅ Headless | — | — | ✅ Issue-driven | — |
| [OpenCode](../../agents/opencode/) | — | ⚠️ Limited | — | — | — | — |
| [OpenHands](../../agents/openhands/) | ✅ openhands-resolver | ✅ Resolver mode | — | — | ✅ Full resolver | — |
| [Pi Coding Agent](../../agents/pi-coding-agent/) | — | ✅ Print/JSON mode | — | — | ✅ Issue-driven | — |
| [Sage Agent](../../agents/sage-agent/) | — | ✅ Headless | — | — | ✅ Issue-driven | — |
| [TongAgents](../../agents/tongagents/) | — | ⚠️ Limited | — | — | — | — |
| [Warp](../../agents/warp/) | — | ✅ Plan execution | — | ⚠️ Exit code aware | — | — |

**Legend:** ✅ = Full support | ⚠️ = Partial/limited | — = Not supported

### Maturity Tiers

The agents cluster into three tiers of CI/CD maturity:

**Tier 1 — Full CI/CD Citizens** (Droid, Claude Code, Gemini CLI, OpenHands):
First-party GitHub Actions, automated PR workflows, event-driven triggers, and structured
non-interactive output. These agents are designed to operate as autonomous pipeline
participants.

**Tier 2 — CI-Capable** (Codex, ForgeCode, Goose, Pi Coding Agent, Aider, Warp,
Mini SWE Agent, Sage Agent):
Robust non-interactive modes that enable CI/CD integration via custom wrapper scripts.
These agents can participate in pipelines but require more setup and orchestration.

**Tier 3 — Interactive-First** (Ante, Capy, Junie CLI, OpenCode, TongAgents):
Primarily designed for interactive use. CI/CD integration is possible but requires
significant custom tooling and may lack structured output formats.

---

## Design Principles for CI/CD Integration

Building on patterns observed across the 17 agents, several design principles emerge:

### 1. Fail Loudly, Not Silently

In CI/CD, a silent failure is worse than a loud one. Agents should:
- Exit with non-zero codes on failure
- Produce structured error output parseable by CI systems
- Never silently swallow exceptions in non-interactive mode

### 2. Bound Autonomous Behavior

Every autonomous loop needs bounds:
- **Max iterations** for CI repair loops (Droid: 3 attempts)
- **Max time** for agent execution (prevent runaway costs)
- **Max scope** for changes (prevent agents from rewriting entire codebases)

### 3. Maintain Auditability

Every agent action in CI/CD should be traceable:
- Signed commits with agent identity
- Structured logs with tool call traces
- PR descriptions that explain agent reasoning
- Links from PRs to triggering issues/events

### 4. Treat Agent Output as Untrusted

Agent-generated code, reviews, and suggestions should go through the same review process
as human contributions. The pipeline should enforce this through branch protection rules,
required reviews, and full CI suite execution.

### 5. Support Graceful Degradation

When the LLM API is down, rate-limited, or returns errors, the CI pipeline should not
break. Agent steps should be **optional checks** that degrade gracefully, not blocking
gates that halt all development.

---

## Real-World Implementations

| Agent | CI/CD Source | Key File/Component | Notes |
|-------|-------------|-------------------|-------|
| [Aider](../../agents/aider/) | CLI flags | `--yes`, `--no-git` | Enables scripted execution |
| [Ante](../../agents/ante/) | — | — | Interactive-first design |
| [Capy](../../agents/capy/) | — | — | Interactive-first design |
| [Claude Code](../../agents/claude-code/) | `anthropics/claude-code-action` | `-p`, `--dangerously-skip-permissions` | Most widely adopted CI action |
| [Codex](../../agents/codex/) | `exec` / `exec-server` | JSON-RPC/WebSocket API | Server mode for batch CI |
| [Droid](../../agents/droid/) | `.droid.yaml` | `auto_review`, `ci_repair` | Most complete CI/CD native |
| [ForgeCode](../../agents/forgecode/) | Non-Interactive Mode | System prompt rewrite | Prompt-level guarantee |
| [Gemini CLI](../../agents/gemini-cli/) | `google-github-actions/run-gemini-cli` | `-p`, `--output-format json` | Strong triage capabilities |
| [Goose](../../agents/goose/) | Recipe mode | `RetryManager` | Automated retry on failure |
| [Junie CLI](../../agents/junie-cli/) | — | — | JetBrains ecosystem focus |
| [Mini SWE Agent](../../agents/mini-swe-agent/) | Headless execution | Issue-to-PR pipeline | Lightweight resolver |
| [OpenCode](../../agents/opencode/) | — | — | TUI-focused design |
| [OpenHands](../../agents/openhands/) | `openhands-resolver` | Sandboxed workspace | Full issue-to-PR automation |
| [Pi Coding Agent](../../agents/pi-coding-agent/) | Print/JSON mode | Structured output | Research-oriented agent |
| [Sage Agent](../../agents/sage-agent/) | Headless execution | Issue-to-PR pipeline | Multi-agent architecture |
| [TongAgents](../../agents/tongagents/) | — | — | Multi-agent research system |
| [Warp](../../agents/warp/) | Plan execution | Exit code observation | Foundation for CI awareness |

---

*This analysis covers CI/CD integration patterns as implemented in publicly available
open-source coding agents as of mid-2025. GitHub Actions workflows, agent capabilities,
and CI/CD features may change between versions.*