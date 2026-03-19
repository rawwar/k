# Droid — Unique Patterns

> What makes Droid architecturally and strategically distinct: interface agnosticism, enterprise-native design, model/vendor independence, and async delegation across surfaces.

## 1. Interface-Agnostic Architecture

**The most distinctive pattern in Droid's design is that the same agent operates across every developer surface.**

While the industry has fragmented into surface-specific agents — Cursor for IDE, Claude Code for terminal, Devin for web workspaces, various bots for Slack/CI — Factory built a single agent core that deploys to all of them:

| Interface | Use Pattern |
|-----------|-------------|
| **IDE** | Inline coding assistance, real-time pair programming |
| **Web** | Cloud workspaces for complex tasks, accessible from anywhere |
| **CLI** | Terminal-native for power users, local dev, scripting |
| **Slack** | Delegate tasks from team chat without context switching |
| **Linear** | Pick up tasks directly from project management |
| **Microsoft Teams** | Enterprise communication integration |
| **CI/CD** | Non-interactive pipeline automation (auto-review, repair) |

### Why This Matters

This isn't just a convenience feature — it's an architectural decision with deep implications:

1. **Session continuity across surfaces**: A task started in Slack can be monitored in web and finalized in CLI. The compaction engine preserves context across transitions.

2. **Unified analytics**: Factory Analytics captures tool usage, token spend, and productivity metrics uniformly regardless of interface. This enables apples-to-apples comparison of delegation patterns across surfaces.

3. **Enterprise deployment simplicity**: IT teams deploy one platform, not seven different tools with separate security, audit, and compliance configurations.

4. **Workflow-native delegation**: Developers delegate tasks where they encounter them. See a bug in PagerDuty → delegate via Slack. Review a spec in Linear → delegate implementation. This reduces the friction of "switching to the AI tool."

### Activity Tracking by Client

Factory Analytics breaks down usage by client type:
- Terminal CLI
- Non-interactive CI mode
- Web workspaces

**Stickiness** (DAU/MAU ratio) reveals whether AI tooling is embedded in daily workflow or used occasionally — a critical enterprise adoption metric.

## 2. Enterprise-Native Design (Not Enterprise-Bolted)

Most coding agents were built for individual developers and later added enterprise features. Factory built enterprise-first:

### Factory Analytics

A full observability platform built on **OpenTelemetry** — not an afterthought dashboard:

| Analytics View | Metrics |
|---------------|---------|
| **Tokens** | Consumption by model, user, date; input/output split; cache efficiency |
| **Tools** | Tool calls by type; skills/slash commands/MCP adoption; autonomy ratio |
| **Activity** | DAU/WAU/MAU; sessions/messages; client breakdown; stickiness |
| **Productivity** | Files created/edited; commits; PRs; language distribution |
| **Users** | Per-person breakdowns; days active; tokens; tool calls; sessions |

The Analytics API enables a powerful second-order pattern: **Droids generating their own ROI reports**. Droids can:
- Call `/analytics/activity`, `/analytics/tokens`, `/analytics/productivity`, `/analytics/tools`
- Join with Jira/Linear data
- Generate narratives like "team doubled PR merge rate in the sprint they ramped Droid usage"
- Package these into leadership-ready reports automatically

### Audit and Compliance

- **ISO 42001**: Among the first organizations worldwide to adopt the AI management system standard.
- **SOC 2 Type I**: Achieved certification for security and privacy controls.
- **Droid Shield**: Proprietary system for detecting and removing security vulnerabilities, bugs, or IP breaches from generated code.
- **Audit logging**: Comprehensive, configurable, exportable to SIEM systems.
- **No training on customer data**: Explicit commitment that customer code is never used as model training data.
- **Single-tenant VPC**: Each organization gets isolated, sandboxed compute.

### Enterprise Security Stack

| Feature | Implementation |
|---------|---------------|
| Authentication | SSO + SAML + OIDC (Google Workspace, Okta, Azure AD) |
| Provisioning | SCIM automated user lifecycle |
| Compute | Dedicated allocation per organization |
| Data at rest | AES-256 encryption |
| Data in transit | TLS 1.2+ |
| Permissions | Mirrors source application access controls |
| Hosting | Single-tenant sandboxed VPC |
| Code safety | Droid Shield vulnerability/IP detection |

### Agent-Readiness Program

Factory offers an **Agent-Readiness Improvement Program** for enterprise customers — suggesting they actively consult on how to optimize organizations for AI-native development, not just sell software.

## 3. Model/Vendor Agnostic Design

Droid works with **any model provider**: OpenAI, Anthropic, Google, xAI, open-source, and local models.

### Strategic Implications

- **No vendor lock-in**: As model capabilities shift (and they shift rapidly), enterprises aren't trapped on a single provider.
- **Cost optimization**: Teams can route different task types to different models:
  - Frontier models (Opus 4.6, o3-pro) for complex reasoning, spec generation
  - Cost-efficient models (GLM-5, Kimi-K2.5) for routine execution
  - Open-source/local models for sensitive code that can't leave the network
- **Day-one model access**: Factory's pricing page promises "fast and direct priority routing, the day the model is available."
- **Mixed-model Specification Mode**: Planning with one model, execution with another.

### Terminal-Bench Evidence

Droid's benchmark results demonstrate model flexibility:
- GPT-5.3-Codex: 77.3% (rank #6)
- Claude Opus 4.6: 69.9% (rank #16)
- GPT-5.2: 64.9% (rank #23)
- Claude Opus 4.1 (v1.0): 58.8% (rank #5)

The agent platform maintains competitive performance across model families rather than being optimized for a single provider.

## 4. Long-Running Async Task Delegation

Droid handles tasks that span from minutes to weeks:

### Short-Lived Tasks (Minutes)
- Inline code review comments
- Quick refactors
- Test generation for a single file

### Medium Tasks (Hours)
- Full PR reviews with guideline compliance
- CI failure diagnosis and repair
- Cross-file refactoring

### Long-Running Tasks (Days to Weeks)
This is where Droid's compaction engine becomes critical:
- **Multi-repository migrations** (Chainguard: 6 repos, 2 weeks, 80 packages)
- **Feature development from spec to PR**
- **Iterative design → implement → review → revise cycles**

The key architectural pattern: **session state is decoupled from any single interface session**. The Droid session persists server-side, and any interface can attach to it. This means:

1. Developer delegates a migration task via Slack.
2. Droid begins executing, running for hours in background.
3. Developer checks progress in web workspace the next day.
4. Developer provides feedback via CLI.
5. Droid continues, eventually opening PRs across 6 repos.
6. Developer reviews PRs in GitHub, provides review comments.
7. Droid addresses review feedback and merges.

All within a single continuous session, with full context preservation.

## 5. "Droid Shield" — Code Safety System

A unique pattern in the enterprise security space:

- Performs **industry-leading techniques** to detect and remove security vulnerabilities from generated code.
- Scans for bugs and IP breaches in generated output.
- Acts as a guardrail between the LLM's output and the codebase.
- Addresses the enterprise concern: "how do we trust AI-generated code?"

## 6. The Wipro Pattern — Enterprise Channel Strategy

Factory's partnership with Wipro (NYSE: WIT) reveals a distinctive go-to-market pattern:

- Wipro integrates Factory into its **WEGA agent-native delivery platform**.
- Rollout across **tens of thousands of engineers**.
- Factory becomes embedded in Wipro's service delivery to clients across banking, healthcare, manufacturing, retail, and technology.
- Wipro Ventures participated in Factory's funding round.

This is notable because most AI coding tools sell direct-to-developer or direct-to-enterprise. Factory is also selling through **systems integrators** — a classic enterprise software distribution pattern rarely seen in the AI agent space.

## 7. Productivity Metrics as First-Class Citizens

Factory doesn't just track whether developers use Droid — it tracks **engineering outcomes**:

| Claimed Metric | Value |
|---------------|-------|
| Feature delivery speed | 7x faster |
| Migration time reduction | 96.1% |
| On-call resolution time saved | 95.8% |

These metrics flow into the Analytics platform, where they can be correlated with AI usage data, creating a feedback loop:
- More delegation → measure impact → optimize delegation patterns → more effective delegation.

This is a pattern unique to enterprise-focused agents. Individual developer tools measure "did the suggestion get accepted?" Factory measures "did the project ship faster?"