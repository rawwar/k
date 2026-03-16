---
title: Cost Optimization
description: Strategies for reducing LLM API costs including prompt caching, model routing, token-aware message pruning, and usage tracking with budget controls.
---

# Cost Optimization

> **What you'll learn:**
> - How prompt caching (Anthropic's cache_control, OpenAI's cached prompts) can dramatically reduce costs for repeated context
> - Model routing strategies that use cheaper models for simple tasks and reserve expensive models for complex reasoning
> - Building usage tracking and budget controls that alert users to spending and enforce per-session or per-day limits

Every LLM API call costs money, and coding agent sessions can involve dozens or hundreds of calls. A single intensive session with Claude Sonnet can easily cost $5-20 in API fees if context management is careless. The techniques in this subchapter can reduce that cost by 60-80% while maintaining the same output quality. This isn't about cutting corners -- it's about not paying for the same tokens multiple times.

## Understanding the Cost Equation

API costs have two components: input tokens and output tokens. Input tokens are everything you send (system prompt, conversation history, tool definitions). Output tokens are what the model generates. For most models, output tokens cost 3-5x more than input tokens:

```rust
#[derive(Debug, Clone)]
struct CostModel {
    model_name: String,
    input_cost_per_mtok: f64,    // USD per million input tokens
    output_cost_per_mtok: f64,   // USD per million output tokens
    cached_input_per_mtok: f64,  // USD per million cached input tokens
}

impl CostModel {
    fn claude_sonnet_4() -> Self {
        Self {
            model_name: "claude-sonnet-4-20250514".into(),
            input_cost_per_mtok: 3.0,
            output_cost_per_mtok: 15.0,
            cached_input_per_mtok: 0.30,
        }
    }

    fn claude_haiku() -> Self {
        Self {
            model_name: "claude-haiku".into(),
            input_cost_per_mtok: 0.25,
            output_cost_per_mtok: 1.25,
            cached_input_per_mtok: 0.025,
        }
    }

    fn calculate_cost(&self, usage: &TokenUsage) -> f64 {
        let fresh_input = usage.input_tokens.saturating_sub(usage.cached_tokens);
        let fresh_cost = (fresh_input as f64 / 1_000_000.0) * self.input_cost_per_mtok;
        let cached_cost = (usage.cached_tokens as f64 / 1_000_000.0)
            * self.cached_input_per_mtok;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0)
            * self.output_cost_per_mtok;
        fresh_cost + cached_cost + output_cost
    }
}

#[derive(Debug, Default, Clone)]
struct TokenUsage {
    input_tokens: u32,
    output_tokens: u32,
    cached_tokens: u32,
}
```

The key insight: input tokens are repeated on every turn. If your conversation has 50,000 tokens of history, every single API call sends those 50,000 tokens again. Over a 30-turn conversation, that's 1.5 million input tokens just from history repetition. This is where caching and compaction have the biggest impact.

::: python Coming from Python
Python's `openai` and `anthropic` libraries return usage information in the response object. In Rust, you parse the same fields from the JSON response. The cost calculation is identical across languages -- the optimization strategies are about reducing the numbers that go into this calculation, not about the calculation itself.
:::

## Prompt Caching

Anthropic's prompt caching is the single most impactful cost optimization. When you mark portions of your input with `cache_control`, the API caches those tokens server-side. Subsequent requests that start with the same tokens get a 90% discount on those cached tokens:

```rust
use serde::Serialize;

#[derive(Serialize)]
struct CacheableRequest {
    model: String,
    max_tokens: u32,
    system: Vec<SystemBlock>,
    messages: Vec<CacheableMessage>,
}

#[derive(Serialize)]
struct SystemBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

#[derive(Serialize)]
struct CacheControl {
    #[serde(rename = "type")]
    control_type: String, // "ephemeral"
}

#[derive(Serialize)]
struct CacheableMessage {
    role: String,
    content: Vec<CacheableContent>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum CacheableContent {
    Text {
        #[serde(rename = "type")]
        block_type: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

struct CacheOptimizer;

impl CacheOptimizer {
    /// Build a request with optimal cache breakpoints
    fn build_cached_request(
        system_prompt: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
        model: &str,
    ) -> CacheableRequest {
        // Strategy: cache the system prompt and the first ~80% of messages
        // The system prompt rarely changes, so it gets the highest cache benefit
        let cache_breakpoint = (messages.len() as f32 * 0.8) as usize;

        let mut api_messages: Vec<CacheableMessage> = Vec::new();

        for (i, msg) in messages.iter().enumerate() {
            let is_cache_boundary = i == cache_breakpoint;
            let text = msg.content.iter()
                .filter_map(|b| match b {
                    ContentBlock::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            let cache = if is_cache_boundary {
                Some(CacheControl {
                    control_type: "ephemeral".into(),
                })
            } else {
                None
            };

            api_messages.push(CacheableMessage {
                role: match msg.role {
                    Role::User | Role::ToolResult => "user".into(),
                    Role::Assistant | Role::ToolCall => "assistant".into(),
                    Role::System => "user".into(), // shouldn't appear here
                },
                content: vec![CacheableContent::Text {
                    block_type: "text".into(),
                    text,
                    cache_control: cache,
                }],
            });
        }

        CacheableRequest {
            model: model.to_string(),
            max_tokens: 4096,
            system: vec![SystemBlock {
                block_type: "text".into(),
                text: system_prompt.to_string(),
                cache_control: Some(CacheControl {
                    control_type: "ephemeral".into(),
                }),
            }],
            messages: api_messages,
        }
    }
}
```

The cache breakpoint placement matters. You want to cache the portion of the conversation that won't change between turns: the system prompt (always cache this) and the older messages (they're fixed). Only the most recent messages and the new user input are "fresh" tokens that pay full price.

::: tip In the Wild
Claude Code makes extensive use of prompt caching. The system prompt, tool definitions, and earlier conversation turns are cached, which means that on a typical multi-turn session, 80-90% of input tokens are cached at the 90% discount rate. This makes the effective cost of long conversations dramatically lower than the nominal per-token pricing suggests. The cache has a 5-minute TTL, which aligns well with the pace of interactive coding -- users rarely pause for more than 5 minutes between turns.
:::

## Model Routing

Not every task needs the most capable (and expensive) model. Model routing sends simple tasks to cheaper models and reserves the expensive model for complex reasoning:

```rust
#[derive(Debug, Clone)]
enum TaskComplexity {
    Simple,    // Yes/no questions, simple lookups
    Moderate,  // Code generation, explanations
    Complex,   // Multi-step reasoning, architecture decisions
}

struct ModelRouter {
    simple_model: CostModel,
    moderate_model: CostModel,
    complex_model: CostModel,
}

impl ModelRouter {
    fn new() -> Self {
        Self {
            simple_model: CostModel::claude_haiku(),
            moderate_model: CostModel::claude_sonnet_4(),
            complex_model: CostModel::claude_sonnet_4(),
        }
    }

    fn route(&self, task: &TaskAnalysis) -> &CostModel {
        match task.complexity {
            TaskComplexity::Simple => &self.simple_model,
            TaskComplexity::Moderate => &self.moderate_model,
            TaskComplexity::Complex => &self.complex_model,
        }
    }

    fn analyze_task(&self, user_message: &str, history_len: usize) -> TaskAnalysis {
        let complexity = if self.is_simple_task(user_message) {
            TaskComplexity::Simple
        } else if self.is_complex_task(user_message, history_len) {
            TaskComplexity::Complex
        } else {
            TaskComplexity::Moderate
        };

        TaskAnalysis {
            complexity,
            estimated_output_tokens: self.estimate_output(user_message),
        }
    }

    fn is_simple_task(&self, message: &str) -> bool {
        let lower = message.to_lowercase();
        let simple_patterns = [
            "yes", "no", "thanks", "looks good", "continue",
            "what is", "show me", "list",
        ];
        simple_patterns.iter().any(|p| lower.starts_with(p))
            || message.len() < 30
    }

    fn is_complex_task(&self, message: &str, history_len: usize) -> bool {
        let lower = message.to_lowercase();
        let complex_patterns = [
            "refactor", "redesign", "architect", "implement",
            "debug", "why does", "explain how",
        ];
        complex_patterns.iter().any(|p| lower.contains(p))
            || history_len > 50
    }

    fn estimate_output(&self, message: &str) -> u32 {
        // Rough heuristic based on request type
        if message.len() < 50 { 500 } else { 2000 }
    }
}

struct TaskAnalysis {
    complexity: TaskComplexity,
    estimated_output_tokens: u32,
}
```

The cost savings from routing can be substantial. Haiku costs 12x less than Sonnet for input tokens and 12x less for output tokens. If 30% of your agent's turns are simple tasks that Haiku can handle, you save 30% * 91% = ~27% on those turns.

## Usage Tracking and Budget Controls

Users need visibility into what they're spending and the ability to set limits:

```rust
use std::sync::{Arc, Mutex};

struct UsageTracker {
    session_usage: Arc<Mutex<SessionUsage>>,
    budget: Option<BudgetConfig>,
}

#[derive(Debug, Default, Clone)]
struct SessionUsage {
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_cached_tokens: u64,
    total_cost_usd: f64,
    calls_made: u32,
    per_call_usage: Vec<CallUsage>,
}

#[derive(Debug, Clone)]
struct CallUsage {
    timestamp: chrono::DateTime<chrono::Utc>,
    model: String,
    input_tokens: u32,
    output_tokens: u32,
    cached_tokens: u32,
    cost_usd: f64,
}

#[derive(Debug, Clone)]
struct BudgetConfig {
    /// Maximum spend per session in USD
    max_session_cost: Option<f64>,
    /// Maximum spend per day in USD
    max_daily_cost: Option<f64>,
    /// Warning threshold (percentage of budget)
    warning_threshold: f64,
}

impl UsageTracker {
    fn new(budget: Option<BudgetConfig>) -> Self {
        Self {
            session_usage: Arc::new(Mutex::new(SessionUsage::default())),
            budget,
        }
    }

    fn record_call(
        &self,
        model: &str,
        usage: &TokenUsage,
        cost_model: &CostModel,
    ) -> BudgetStatus {
        let cost = cost_model.calculate_cost(usage);

        let mut session = self.session_usage.lock().unwrap();
        session.total_input_tokens += usage.input_tokens as u64;
        session.total_output_tokens += usage.output_tokens as u64;
        session.total_cached_tokens += usage.cached_tokens as u64;
        session.total_cost_usd += cost;
        session.calls_made += 1;
        session.per_call_usage.push(CallUsage {
            timestamp: chrono::Utc::now(),
            model: model.to_string(),
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cached_tokens: usage.cached_tokens,
            cost_usd: cost,
        });

        self.check_budget(&session)
    }

    fn check_budget(&self, session: &SessionUsage) -> BudgetStatus {
        if let Some(ref budget) = self.budget {
            if let Some(max) = budget.max_session_cost {
                if session.total_cost_usd >= max {
                    return BudgetStatus::Exceeded {
                        spent: session.total_cost_usd,
                        limit: max,
                    };
                }
                let utilization = session.total_cost_usd / max;
                if utilization >= budget.warning_threshold {
                    return BudgetStatus::Warning {
                        spent: session.total_cost_usd,
                        limit: max,
                        percent: utilization * 100.0,
                    };
                }
            }
        }
        BudgetStatus::Ok
    }

    fn format_usage_report(&self) -> String {
        let session = self.session_usage.lock().unwrap();

        let cache_hit_rate = if session.total_input_tokens > 0 {
            (session.total_cached_tokens as f64 / session.total_input_tokens as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "Session Usage Report\n\
             ====================\n\
             API calls:      {}\n\
             Input tokens:   {} ({} cached, {:.1}% hit rate)\n\
             Output tokens:  {}\n\
             Total cost:     ${:.4}\n\
             Avg cost/call:  ${:.4}",
            session.calls_made,
            session.total_input_tokens,
            session.total_cached_tokens,
            cache_hit_rate,
            session.total_output_tokens,
            session.total_cost_usd,
            if session.calls_made > 0 {
                session.total_cost_usd / session.calls_made as f64
            } else {
                0.0
            }
        )
    }
}

#[derive(Debug)]
enum BudgetStatus {
    Ok,
    Warning { spent: f64, limit: f64, percent: f64 },
    Exceeded { spent: f64, limit: f64 },
}
```

## Comprehensive Cost Optimization Strategy

Combining all the techniques into a coherent strategy:

```rust
struct CostOptimizer {
    cache_optimizer: CacheOptimizer,
    model_router: ModelRouter,
    usage_tracker: UsageTracker,
    compaction_policy: CompactionPolicy,
}

impl CostOptimizer {
    fn optimize_request(
        &self,
        system_prompt: &str,
        messages: &MessageHistory,
        tools: &[ToolDefinition],
        user_message: &str,
    ) -> OptimizedRequest {
        // Step 1: Route to appropriate model
        let task = self.model_router.analyze_task(user_message, messages.len());
        let model = self.model_router.route(&task);

        // Step 2: Check if compaction would save money
        let current_tokens = messages.total_tokens();
        let should_compact = current_tokens > 50_000; // Arbitrary threshold

        // Step 3: Build request with cache breakpoints
        let api_messages: Vec<Message> = messages.iter().cloned().collect();
        let request = CacheOptimizer::build_cached_request(
            system_prompt,
            &api_messages,
            tools,
            &model.model_name,
        );

        // Step 4: Estimate cost before sending
        let estimated_cost = model.calculate_cost(&TokenUsage {
            input_tokens: current_tokens,
            output_tokens: task.estimated_output_tokens,
            cached_tokens: (current_tokens as f32 * 0.8) as u32, // Assume 80% cache hit
        });

        OptimizedRequest {
            request,
            model_name: model.model_name.clone(),
            estimated_cost,
            should_compact_first: should_compact,
        }
    }
}

struct OptimizedRequest {
    request: CacheableRequest,
    model_name: String,
    estimated_cost: f64,
    should_compact_first: bool,
}
```

The optimization flows in order of impact: prompt caching (saves 60-80%), compaction (reduces repeated tokens), and model routing (uses cheaper models for simple tasks). Together, these can reduce a $10 session to $2-3 with no loss in quality.

## Key Takeaways

- Prompt caching is the highest-impact cost optimization: marking stable portions of the conversation (system prompt, older messages) with `cache_control` gives a 90% discount on those tokens.
- Model routing sends simple tasks to cheaper models (Haiku) and reserves expensive models (Sonnet) for complex reasoning, reducing costs by 20-30% for typical sessions.
- Track usage per-call with `TokenUsage` structs and provide users with real-time cost visibility, including cache hit rates and per-call breakdowns.
- Budget controls with warning thresholds prevent runaway spending -- enforce per-session or per-day limits and alert users before they exceed their budget.
- The combined strategy of caching + compaction + routing can reduce session costs by 70-80% compared to naive "send everything, use the best model" approaches.
