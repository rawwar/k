---
title: Multi Agent Conversations
description: Managing conversations where multiple LLM agents collaborate, including message routing, shared context, role delineation, and coordination protocols.
---

# Multi Agent Conversations

> **What you'll learn:**
> - How to structure message history when multiple agents contribute to a conversation, each with distinct roles and capabilities
> - Message routing strategies that direct sub-tasks to specialized agents while maintaining a coherent shared context
> - Coordination patterns for agent handoff, result aggregation, and conflict resolution in multi-agent workflows

So far, every conversation pattern has involved a single LLM agent. But complex coding tasks sometimes benefit from multiple specialized agents working together: a planning agent that breaks down tasks, a coding agent that implements them, a review agent that checks the code, and a testing agent that verifies correctness. Managing the conversation state for multi-agent systems introduces new challenges around context sharing, role delineation, and coordination.

## Multi-Agent Architectures

There are three dominant patterns for organizing multiple agents, and each has different implications for conversation state:

```rust
/// The three main multi-agent architectures
#[derive(Debug)]
enum MultiAgentArchitecture {
    /// One orchestrator agent delegates to specialists
    Orchestrator {
        coordinator: AgentConfig,
        specialists: Vec<AgentConfig>,
    },
    /// Agents pass work to each other in a pipeline
    Pipeline {
        stages: Vec<AgentConfig>,
    },
    /// Agents collaborate as peers with shared state
    Collaborative {
        agents: Vec<AgentConfig>,
        shared_context: SharedContext,
    },
}

#[derive(Debug, Clone)]
struct AgentConfig {
    id: String,
    name: String,
    model: String,
    system_prompt: String,
    tools: Vec<String>,
    /// Maximum tokens this agent can use from the shared budget
    token_budget: u32,
}

#[derive(Debug)]
struct SharedContext {
    /// Messages visible to all agents
    shared_messages: Vec<Message>,
    /// Per-agent private message histories
    private_histories: std::collections::HashMap<String, Vec<Message>>,
}
```

**Orchestrator pattern**: A central agent receives the user's request, decides which specialist to invoke, and synthesizes their results. The orchestrator sees the full conversation; specialists see only their sub-task.

**Pipeline pattern**: Each agent processes the conversation in sequence. Agent A plans, Agent B codes, Agent C reviews. Each stage sees the output of previous stages plus any shared context.

**Collaborative pattern**: All agents share a conversation and can respond to each other. This requires the most sophisticated state management but produces the most natural multi-agent interaction.

::: python Coming from Python
Python frameworks like LangGraph and CrewAI make multi-agent orchestration straightforward with decorator-based agent definitions. In Rust, you build the same patterns using traits and enums, but with explicit message ownership -- you decide at compile time which agents own which parts of the conversation.
:::

## The Orchestrator Pattern in Detail

The orchestrator pattern is the most practical for coding agents. One agent manages the conversation with the user and delegates specific tasks to specialists:

```rust
struct OrchestratorAgent {
    /// The main conversational agent
    coordinator: Box<dyn LlmClient>,
    coordinator_config: AgentConfig,
    /// Specialist agents indexed by name
    specialists: HashMap<String, SpecialistAgent>,
    /// The unified conversation history seen by the user
    conversation: MessageHistory,
}

struct SpecialistAgent {
    client: Box<dyn LlmClient>,
    config: AgentConfig,
    /// This specialist's private conversation with the coordinator
    private_history: Vec<Message>,
}

impl OrchestratorAgent {
    async fn handle_user_message(
        &mut self,
        user_message: String,
        tokenizer: &dyn Tokenizer,
    ) -> Result<String, AgentError> {
        // Add user message to main conversation
        self.conversation.push(Message::user(user_message.clone()));

        // Ask coordinator to decide what to do
        let coordinator_response = self.call_coordinator(tokenizer).await?;

        // Check if the coordinator wants to delegate
        match self.parse_delegation(&coordinator_response) {
            Some(delegation) => {
                // Route to specialist
                let specialist_result = self.delegate_to_specialist(
                    &delegation.specialist_name,
                    &delegation.task,
                    tokenizer,
                ).await?;

                // Feed specialist result back to coordinator
                let final_response = self.synthesize_result(
                    &specialist_result,
                    tokenizer,
                ).await?;

                self.conversation.push(Message::assistant(final_response.clone()));
                Ok(final_response)
            }
            None => {
                // Coordinator handles directly
                self.conversation.push(
                    Message::assistant(coordinator_response.clone())
                );
                Ok(coordinator_response)
            }
        }
    }

    async fn delegate_to_specialist(
        &mut self,
        specialist_name: &str,
        task: &str,
        tokenizer: &dyn Tokenizer,
    ) -> Result<String, AgentError> {
        let specialist = self.specialists.get_mut(specialist_name)
            .ok_or_else(|| AgentError::SpecialistNotFound(specialist_name.to_string()))?;

        // Build context for the specialist: shared context + task description
        let context = self.build_specialist_context(task);

        specialist.private_history.push(Message::user(context));

        let response = specialist.client.complete(&CompletionRequest {
            model: specialist.config.model.clone(),
            messages: specialist.private_history.iter()
                .map(|m| m.to_api_message())
                .collect(),
            max_tokens: 4096,
            temperature: 0.0,
        }).await?;

        let result = response.extract_text();
        specialist.private_history.push(Message::assistant(result.clone()));

        Ok(result)
    }

    fn build_specialist_context(&self, task: &str) -> String {
        // Give the specialist relevant context from the main conversation
        let recent_messages: Vec<String> = self.conversation.last_n(5)
            .map(|msg| {
                let role = format!("{:?}", msg.role);
                let text = msg.content.iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("[{}]: {}", role, text)
            })
            .collect();

        format!(
            "Context from the main conversation:\n{}\n\nYour task:\n{}",
            recent_messages.join("\n"),
            task
        )
    }

    async fn call_coordinator(
        &self,
        tokenizer: &dyn Tokenizer,
    ) -> Result<String, AgentError> {
        let messages = self.conversation.to_api_messages();
        let response = self.coordinator.complete(&CompletionRequest {
            model: self.coordinator_config.model.clone(),
            messages,
            max_tokens: 4096,
            temperature: 0.2,
        }).await?;
        Ok(response.extract_text())
    }

    fn parse_delegation(&self, response: &str) -> Option<Delegation> {
        // Parse structured delegation from coordinator's response
        // e.g., "[DELEGATE:code_review] Review the auth middleware changes"
        if let Some(start) = response.find("[DELEGATE:") {
            let rest = &response[start + 10..];
            if let Some(end) = rest.find(']') {
                let specialist_name = rest[..end].to_string();
                let task = rest[end + 1..].trim().to_string();
                return Some(Delegation { specialist_name, task });
            }
        }
        None
    }

    async fn synthesize_result(
        &mut self,
        specialist_result: &str,
        tokenizer: &dyn Tokenizer,
    ) -> Result<String, AgentError> {
        // Add specialist result as context for coordinator
        let synthesis_prompt = format!(
            "The specialist agent returned the following result. \
             Synthesize this into a response for the user:\n\n{}",
            specialist_result
        );
        self.conversation.push(Message::user(synthesis_prompt));
        self.call_coordinator(tokenizer).await
    }
}

struct Delegation {
    specialist_name: String,
    task: String,
}
```

The user sees a single coherent conversation. Behind the scenes, the coordinator routes sub-tasks to specialists, each with their own private history. The specialist results are synthesized by the coordinator before being shown to the user.

## Token Budget Management for Multi-Agent

Multiple agents multiply the token consumption problem. Each agent has its own context window, and the shared context is duplicated across them:

```rust
struct MultiAgentBudget {
    /// Total token budget across all agents
    total_budget: u64,
    /// Per-agent budgets
    agent_budgets: HashMap<String, AgentBudget>,
    /// Tokens consumed so far
    total_consumed: u64,
}

#[derive(Debug)]
struct AgentBudget {
    allocated: u32,
    consumed: u32,
    calls_made: u32,
}

impl MultiAgentBudget {
    fn new(total_budget: u64, agents: &[AgentConfig]) -> Self {
        let per_agent = (total_budget as u32) / agents.len() as u32;
        let agent_budgets = agents.iter()
            .map(|a| (a.id.clone(), AgentBudget {
                allocated: a.token_budget.min(per_agent),
                consumed: 0,
                calls_made: 0,
            }))
            .collect();

        Self {
            total_budget,
            agent_budgets,
            total_consumed: 0,
        }
    }

    fn can_call(&self, agent_id: &str) -> bool {
        if let Some(budget) = self.agent_budgets.get(agent_id) {
            budget.consumed < budget.allocated
                && self.total_consumed < self.total_budget
        } else {
            false
        }
    }

    fn record_usage(&mut self, agent_id: &str, input_tokens: u32, output_tokens: u32) {
        let total = input_tokens + output_tokens;
        if let Some(budget) = self.agent_budgets.get_mut(agent_id) {
            budget.consumed += total;
            budget.calls_made += 1;
        }
        self.total_consumed += total as u64;
    }

    fn report(&self) -> String {
        let mut lines = vec![format!(
            "Total: {}/{} tokens ({:.1}%)",
            self.total_consumed,
            self.total_budget,
            (self.total_consumed as f64 / self.total_budget as f64) * 100.0
        )];

        for (id, budget) in &self.agent_budgets {
            lines.push(format!(
                "  {}: {}/{} tokens, {} calls",
                id, budget.consumed, budget.allocated, budget.calls_made
            ));
        }

        lines.join("\n")
    }
}
```

::: wild In the Wild
Claude Code uses a single-agent architecture with tool-based dispatch rather than multi-agent coordination. When it needs to perform specialized tasks (like searching code, reading files, or running commands), it uses tools rather than delegating to separate agents. This keeps the context management simple -- one conversation, one context window. However, Claude Code's sub-agent pattern uses inner LLM calls for specific tasks (like generating commit messages or summarizing changes), which is architecturally similar to the orchestrator pattern. The sub-agent calls receive focused context rather than the full conversation.
:::

## Message Attribution and Transparency

When multiple agents contribute to a conversation, users need to understand who said what. This is especially important for trust and debugging:

```rust
#[derive(Debug, Clone)]
struct AttributedMessage {
    message: Message,
    /// Which agent generated this message
    source_agent: String,
    /// Whether this message is visible to the user
    user_visible: bool,
    /// Whether this message is shared across agents
    shared: bool,
}

impl AttributedMessage {
    fn format_for_display(&self) -> String {
        let agent_prefix = if self.source_agent == "coordinator" {
            String::new()
        } else {
            format!("[{}] ", self.source_agent)
        };

        let text: String = self.message.content.iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!("{}{}", agent_prefix, text)
    }
}
```

The design choice of `user_visible` versus internal messages is important. Specialist-to-coordinator messages are internal coordination -- the user doesn't need to see "The code review specialist found 3 issues." They see the coordinator's synthesized response: "I found 3 issues in the code: ..."

## When to Use Multi-Agent Patterns

Multi-agent architectures add significant complexity. Use them when:

- **Tasks decompose naturally**: A task has distinct phases (plan, implement, review, test) that benefit from specialized prompts and tools.
- **Token budget is a constraint**: Specialists with focused context windows can handle sub-tasks more efficiently than one agent trying to fit everything in a single context.
- **Quality requirements vary**: Use an expensive model for critical reasoning and a cheaper model for boilerplate generation.
- **Parallel execution matters**: Independent sub-tasks can run on separate agents simultaneously.

For most coding agents, start with a single agent and add specialist delegation incrementally. The orchestrator pattern is the safest entry point because it keeps the user-facing conversation simple while enabling specialization behind the scenes.

## Key Takeaways

- Three main multi-agent architectures exist: orchestrator (central coordinator with specialists), pipeline (sequential processing stages), and collaborative (peer agents with shared state).
- The orchestrator pattern is most practical for coding agents -- one agent manages the user conversation and delegates specific tasks to specialists with focused context.
- Each agent needs its own token budget and context management, and the shared context is duplicated across agents, multiplying token consumption.
- Message attribution tracks which agent generated each message, distinguishing between user-visible responses and internal coordination messages.
- Start with a single agent and add specialist delegation incrementally -- multi-agent complexity is only justified when tasks decompose naturally into specialized phases.
