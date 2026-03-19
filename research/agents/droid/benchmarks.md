# Droid — Benchmarks

> Terminal-Bench scores, enterprise productivity metrics, and what they reveal about Droid's model-agnostic performance.

## Terminal-Bench 2.0

Terminal-Bench is a benchmark for evaluating terminal-based coding agents on real-world software engineering tasks. Droid appears with multiple model configurations, demonstrating its model-agnostic architecture.

### Droid Scores

| Model Configuration | Rank | Score | Notes |
|--------------------|------|-------|-------|
| Droid + GPT-5.3-Codex | #6 | 77.3% | Best Droid result; OpenAI frontier model |
| Droid + Claude Opus 4.6 | #16 | 69.9% | Anthropic frontier model |
| Droid + GPT-5.2 | #23 | 64.9% | Previous-gen OpenAI model |

### Analysis

**Model-agnostic consistency**: Droid achieves competitive scores across different model families. The ~7.4 percentage point spread between its best (GPT-5.3-Codex, 77.3%) and middle (Opus 4.6, 69.9%) results is notable — the agent platform adds consistent value regardless of which model powers it.

**Rank #6 with GPT-5.3-Codex**: Placing in the top 6 of Terminal-Bench 2.0 demonstrates that Droid's agent infrastructure (tool system, context management, task planning) effectively leverages frontier model capabilities.

**Multi-model presence**: Having three entries in a single benchmark leaderboard is unusual. Most agents appear once (optimized for their best model). Droid's multiple entries reflect its vendor-agnostic positioning and provide useful signal about how much the agent platform vs. the underlying model contributes to performance.

## Terminal-Bench 1.0

| Model Configuration | Rank | Score |
|--------------------|------|-------|
| Droid + Claude Opus 4.1 | #5 | 58.8% |

The improvement from 58.8% (TB 1.0) to 77.3% (TB 2.0) reflects both model improvements (Opus 4.1 → GPT-5.3-Codex) and likely agent platform improvements over time.

## Enterprise Productivity Metrics

Factory reports enterprise-level productivity benchmarks from customer deployments. These are not synthetic benchmarks but claimed real-world outcomes:

| Metric | Value | Context |
|--------|-------|---------|
| Feature delivery speed | **7x faster** | End-to-end from task to merge |
| Migration time reduction | **96.1% reduction** | Time to complete codebase migrations |
| On-call resolution time | **95.8% time saved** | Incident response and fix deployment |

### Chainguard Case Study Metrics

| Metric | Value |
|--------|-------|
| Continuous session length | **2 weeks** |
| Repositories managed | **6** |
| Packages built | **80** |
| Pattern: Teach once → replicate | **~2 teaching iterations → independent execution** |

## Analytics-Derived Benchmarks

Factory Analytics enables organizations to generate their own benchmarks:

### Autonomy Ratio
- Measures tool calls per user message.
- Example: **13x** means 13 autonomous tool calls per human interaction.
- Higher ratios indicate more effective delegation and greater agent autonomy.

### Productivity Output (Example Org)
Factory's analytics blog post cites example aggregate outputs:
- **4,500 files created**
- **129,000 files edited**
- **3,500 commits**
- **484 pull requests**

These are tracked over time and correlated with adoption curves — as teams ramp up Droid usage, code output increases in patterns matching the adoption curve.

### Cost Efficiency
- Per-token costs tracked by model
- Per-seat economics derivable from user analytics
- **Per-story-point AI cost**: Teams can calculate cost of AI assistance per Linear/Jira story point completed by joining analytics data with project management data.

## Benchmark Methodology Observations

### What Terminal-Bench Measures
Terminal-Bench evaluates agents on their ability to solve software engineering tasks using terminal tools. This aligns well with Droid's capabilities since it has a full CLI mode with the same agent core.

### What Enterprise Metrics Measure
Factory's enterprise metrics focus on **business outcomes** rather than benchmark puzzles:
- Time-to-delivery (feature delivery speed)
- Migration efficiency
- Incident response time

This reflects Factory's enterprise positioning — they optimize for metrics that matter to engineering leadership and procurement, not just developer satisfaction.

### Model Contribution vs. Agent Contribution
The multi-model Terminal-Bench entries allow rough estimation of agent vs. model contribution:
- GPT-5.3-Codex alone vs. Droid + GPT-5.3-Codex → agent overhead/benefit
- Same agent across multiple models → consistency of agent platform value

The ~7-12 point variation across models with the same agent platform suggests the agent contributes a meaningful but not overwhelming portion of the total score — the model still matters significantly.