# Droid — Architecture

> How Factory's agent-native platform separates interface, agent core, model routing, and integrations into a layered architecture.

## High-Level Architecture

Droid's architecture is designed around a central principle: **the agent core is interface-agnostic**. The same agent logic executes regardless of whether the developer interacts via IDE, web, CLI, Slack, Linear, or CI/CD. This is achieved through a layered architecture that cleanly separates concerns.

### Layer 1: Interface Layer (Developer Surfaces)

The outermost layer handles the diverse surfaces through which developers interact with Droid:

- **CLI**: Terminal-based interactive agent for local development. Supports synchronous interaction and non-interactive CI/CD mode for pipeline automation.
- **Web Workspaces**: Cloud-hosted development environments at `app.factory.ai` with full browser-based interaction.
- **Desktop IDE**: Plugin-based integration into existing IDEs.
- **Slack / Microsoft Teams**: Conversational interface for delegating tasks from chat. Enables developers to kick off tasks ("refactor the auth module") without switching tools.
- **Linear**: Direct integration with project management — Droids can pick up tasks from Linear issues.
- **CI/CD Pipelines**: Non-interactive mode where Droids run as part of build/deploy pipelines (e.g., auto-reviewing PRs, repairing GitHub Actions failures).

Each interface adapter translates surface-specific interactions (Slack messages, IDE commands, terminal input) into a unified internal protocol that the agent core consumes.

### Layer 2: Droid Agent Core

The central processing layer contains:

- **Compaction Engine**: Factory's proprietary context management system that enables multi-week sessions. Uses incremental compression with anchor points to preserve critical context while staying within token limits.
- **Model Router**: Vendor-agnostic model dispatch that routes requests to the appropriate LLM (OpenAI, Anthropic, Google, xAI, open-source, local). Teams can configure which models handle which task types.
- **Tool System**: Standardized tool interface (Read, Edit, Execute, Grep, Glob, and custom tools). Tracks tool calls for analytics (autonomy ratio, delegation patterns).
- **Specification Mode**: A planning capability that generates detailed implementation specs with mixed model support — reasoning models for spec generation, execution models for implementation.
- **Session Manager**: Maintains session state across interactions, enabling a developer to start a task in Slack and continue it in the CLI or web workspace.

### Layer 3: Integration Layer

The deepest layer connects Droid to the developer's existing engineering ecosystem:

- **Source Code Management** (required): GitHub Cloud, GitLab — Droid reads/writes code, creates PRs, reviews diffs.
- **Project Management** (recommended): Jira, Linear, Asana — Droid understands task context, links commits to tickets.
- **Knowledge Management** (recommended): Notion, Confluence, Google Drive — Droid references documentation and design decisions.
- **Incident Management** (recommended): Sentry, PagerDuty — Droid can respond to incidents, diagnose errors, and propose fixes.
- **CI/CD & Infrastructure**: Docker, CircleCI, Google Cloud, Azure — pipeline integration and deployment awareness.
- **Analytics & Observability**: Codecov, DX — code quality and developer experience metrics.

## Configuration Architecture

Droid behavior is configured via `.droid.yaml` at the repository root. This file controls:

```yaml
review:
  guidelines:
    - path: "**/api/**"
      guideline: "All API endpoints must have input validation"
  auto_review:
    enabled: true
    draft: false
    bot: false
    ignore_title_keywords: ["WIP", "DO NOT MERGE"]
    ignore_labels: ["droid-skip"]
  pr_summary: true
  file_summaries: true
  tips: true
  github_action_repair: true
  path_filters:
    - "!**/*.log"
```

Key configuration capabilities:
- **Path-specific review guidelines**: Different rules for different parts of the codebase using fnmatch patterns.
- **Auto-review triggers**: Configurable conditions for automatic PR review (skip drafts, skip bot PRs, ignore WIP titles).
- **GitHub Actions repair**: Automatically suggest fixes when CI fails.
- **Path filtering**: Include/exclude files from review scope.

## Compute Architecture

Factory uses a **single-tenant, sandboxed architecture**:

- Each organization gets a dedicated VPC (Virtual Private Cloud) with isolated compute resources.
- **Dedicated compute allocation** ensures consistent performance regardless of platform load.
- All data encrypted at rest (AES-256) and in transit (TLS 1.2+).
- Strict permissions enforcement — Droid only accesses what the user already has permission to see.

## Observability Architecture (Factory Analytics)

Built on **OpenTelemetry**, the analytics system captures:

- Token consumption (by model, user, date range, input/output split, cache efficiency)
- Tool calls (by type: Read, Edit, Execute, Grep, Glob)
- Activity metrics (DAU/WAU/MAU, session duration, client breakdown)
- Productivity output (files created/edited, commits, PRs, language distribution)
- Per-user breakdowns (days active, tokens consumed, tool calls)

Data flows to either Factory's hosted dashboards (`app.factory.ai/analytics`) or exports via OTLP to Datadog, Grafana, New Relic, Splunk, or any compatible collector.

### Analytics API

Four core endpoints power both dashboards and AI-generated reports:

| Endpoint | Question Answered |
|----------|-------------------|
| `/analytics/activity` | Is my org using Droid? |
| `/analytics/tokens` | What are we spending? |
| `/analytics/productivity` | What is being shipped? |
| `/analytics/tools` | How is Droid being used? |

Droids can consume these endpoints directly to generate ROI reports, compare team adoption, or correlate AI usage with project delivery (e.g., linking token spend on a Linear project to story points completed).

## Authentication & Identity

- **SSO/SAML/OIDC**: Integrates with any identity provider (Google Workspace, Okta, Azure AD).
- **SCIM provisioning**: Automated user lifecycle management.
- **Repository permissions**: Fine-grained access control mirroring source code management permissions.
- **Audit logging**: Comprehensive, configurable logging exportable to SIEM systems.