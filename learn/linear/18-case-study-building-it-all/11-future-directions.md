---
title: Future Directions
description: Explore the emerging trends and unsolved problems in coding agent development, from multi-agent collaboration to formal verification.
---

# Future Directions

> **What you'll learn:**
> - How multi-agent architectures, agent-to-agent delegation, and parallel agent execution are reshaping what coding agents can accomplish
> - The emerging role of formal verification, proof-carrying code, and type-level guarantees in making agent-generated code trustworthy
> - What advances in model capabilities (longer context, better reasoning, native tool use) mean for the architecture patterns covered in this book

You have built a complete coding agent. You understand the architecture, the wiring, the loop, the tools, the safety model, the provider abstraction, and the extensibility patterns. You have studied three production agents and extracted their lessons. Now let's look forward. The field of coding agents is moving fast, and the patterns you have learned are a foundation, not a ceiling. This subchapter surveys the directions that are emerging, the problems that remain unsolved, and the opportunities for builders who understand the fundamentals.

## Multi-Agent Architectures

The agents you have built so far are single-agent systems: one model, one loop, one conversation. The next frontier is multi-agent systems where specialized agents collaborate on different aspects of a task.

Consider a complex feature request: "Add a user authentication system with OAuth support, database migrations, API endpoints, and frontend components." A single agent handles this by working through each part sequentially. A multi-agent system might delegate:

- **Architecture agent**: designs the overall structure and interfaces
- **Backend agent**: implements API endpoints and database models
- **Frontend agent**: builds the UI components
- **Testing agent**: writes and runs integration tests
- **Review agent**: checks the work of other agents for consistency and correctness

```rust
pub struct AgentOrchestrator {
    agents: HashMap<String, AgentInstance>,
    coordinator: Box<dyn Coordinator>,
}

pub struct AgentInstance {
    pub role: String,
    pub provider: Box<dyn Provider>,
    pub tools: ToolRegistry,
    pub context: Arc<RwLock<ContextManager>>,
}

pub struct DelegatedTask {
    pub id: String,
    pub description: String,
    pub assigned_to: String,
    pub dependencies: Vec<String>,
    pub shared_context: Vec<Message>,
}

impl AgentOrchestrator {
    pub async fn execute_plan(&self, tasks: Vec<DelegatedTask>) -> anyhow::Result<()> {
        // Build a dependency graph
        let graph = DependencyGraph::from_tasks(&tasks);

        // Execute in topological order, parallelizing independent tasks
        for batch in graph.batches() {
            let futures: Vec<_> = batch.iter().map(|task| {
                let agent = &self.agents[&task.assigned_to];
                self.execute_task(agent, task)
            }).collect();

            let results = futures::future::join_all(futures).await;

            for (task, result) in batch.iter().zip(results) {
                result.with_context(|| {
                    format!("Task '{}' assigned to '{}' failed",
                        task.description, task.assigned_to)
                })?;
            }
        }

        Ok(())
    }
}
```

Multi-agent systems introduce new challenges: agents need to share context without overwhelming each other's context windows. They need to resolve conflicts when two agents modify the same file. They need a coordination protocol that ensures coherent results. These are active research problems.

::: tip In the Wild
Claude Code's "sub-agent" pattern hints at multi-agent architecture. When Claude Code encounters a complex task, it can spawn a sub-agent with a focused context to handle a subtask (like researching how an API works) and incorporate the results into its main conversation. This is not full multi-agent orchestration, but it demonstrates the principle of delegation with focused context. OpenCode does not yet implement sub-agents but its architecture — with clean provider and tool abstractions — is well-positioned to support them.
:::

## Longer Context Windows and Their Implications

Context windows have grown from 4K tokens (GPT-3 era) to 200K (Claude 3.5) to 1M+ tokens (current models). This growth has direct architectural implications:

**Context management becomes less critical but more nuanced.** With 1M tokens, a typical coding session rarely hits the limit. But when it does (working on a large codebase with extensive tool output), the compaction problem is harder because there is more history to reason about.

**File reading patterns change.** With small context windows, agents must be selective about which files to read. With large windows, an agent can load an entire project directory and reason about it holistically. This shifts the tool design from "read one file at a time" toward "read everything relevant" strategies.

**System prompts become more powerful.** A 200-token context budget for system prompts versus a 10,000-token budget changes what you can include. Larger budgets let you embed coding standards, architectural guidelines, and project-specific conventions directly in the system prompt rather than relying on the model to discover them.

```rust
// Future: system prompt that includes project-specific context
fn build_rich_system_prompt(project: &ProjectContext) -> String {
    let mut prompt = String::new();

    // Base instructions
    prompt.push_str(include_str!("prompts/system_base.md"));

    // Project architecture (discovered from README, doc comments, etc.)
    if let Some(architecture) = &project.architecture_summary {
        prompt.push_str("\n\n## Project Architecture\n");
        prompt.push_str(architecture);
    }

    // Coding conventions (from linter config, .editorconfig, etc.)
    if let Some(conventions) = &project.coding_conventions {
        prompt.push_str("\n\n## Coding Conventions\n");
        prompt.push_str(conventions);
    }

    // Recent git history (for context on what's been changing)
    if let Some(recent_changes) = &project.recent_changes {
        prompt.push_str("\n\n## Recent Changes\n");
        prompt.push_str(recent_changes);
    }

    prompt
}
```

The lesson is that your agent should detect and adapt to the available context window. When running with a 1M-token model, be aggressive about loading context. When running with a 32K-token model, be selective.

## Formal Verification of Agent-Generated Code

One of the most important open problems in coding agents is *trust*. When the agent writes code, how do you know it is correct? Currently, the answer is "run the tests" — but the agent also wrote the tests. This creates a circularity problem.

Formal verification offers a path forward. Instead of (or in addition to) testing, the agent could:

- Generate code with type-level invariants that the compiler checks
- Produce proofs alongside code that a verifier can validate
- Use property-based testing to explore edge cases the agent might not have considered
- Leverage Rust's type system to encode constraints as types

```rust
// Future: agent generates code with machine-checkable invariants
// Using Rust's type system to encode constraints

/// A non-empty vector — the type system guarantees at least one element
pub struct NonEmpty<T> {
    first: T,
    rest: Vec<T>,
}

impl<T> NonEmpty<T> {
    pub fn new(first: T) -> Self {
        Self { first, rest: Vec::new() }
    }

    pub fn first(&self) -> &T {
        &self.first // Can never fail — the type guarantees it
    }

    pub fn len(&self) -> usize {
        1 + self.rest.len() // Always at least 1
    }
}
```

This is an area where Rust has a particular advantage. Its type system is expressive enough to encode many invariants at compile time, reducing the surface area where the agent can produce incorrect code.

::: python Coming from Python
Python's gradual typing (with `mypy` and type hints) is a step in this direction but is far less powerful than Rust's type system. In Python, `list[int]` does not prevent you from creating an empty list. In Rust, a `NonEmpty<i32>` structurally cannot be empty. As models improve and agents target strongly-typed languages, the type system becomes a collaborator in ensuring correctness — the agent proposes code, and the compiler either accepts or rejects it with precise feedback.
:::

## Native Tool Use in Models

Current models interact with tools through a text-based protocol: they emit structured JSON describing which tool to call, the host executes it, and the result comes back as text. This works but adds overhead — the model must generate JSON (which sometimes it gets wrong), and the host must parse it.

Future models may have native tool use built into the architecture. Instead of generating text that represents a tool call, the model directly produces a tool invocation as a structured output. This would eliminate parsing errors, reduce latency, and potentially allow the model to make better decisions about when and how to use tools.

For your agent architecture, this means the tool dispatch layer should be ready for different invocation protocols. The trait-based abstraction you built is well-positioned for this — the `Tool` trait does not care how the invocation arrives, only that it gets a name and parameters.

## Autonomous Background Agents

Current coding agents are interactive — the user provides a prompt and watches the agent work. A growing pattern is background agents that work autonomously on larger tasks:

- The user files a GitHub issue describing a feature
- A background agent picks up the issue
- The agent analyzes the codebase, creates a plan, implements the change, runs tests
- The agent opens a pull request for human review
- The human reviews and merges (or requests changes, which the agent addresses)

This shifts the agent from a pair-programming tool to an autonomous contributor. The architectural requirements change: the agent needs to operate without a terminal, manage its own sessions, integrate with issue trackers and CI systems, and handle much longer execution times (minutes to hours).

```rust
pub struct BackgroundAgent {
    agent: Agent,
    issue_tracker: Box<dyn IssueTracker>,
    ci_system: Box<dyn CiSystem>,
    git: GitClient,
}

impl BackgroundAgent {
    pub async fn process_issue(&self, issue: &Issue) -> anyhow::Result<PullRequest> {
        // Create a branch
        let branch = self.git.create_branch(&format!("agent/{}", issue.id))?;

        // Run the agent in non-interactive mode
        let result = self.agent.run_once(&format!(
            "Implement the following feature request:\n\n{}\n\n{}",
            issue.title, issue.description
        )).await?;

        // Run tests
        let test_result = self.ci_system.run_tests(&branch).await?;
        if !test_result.passed {
            // Ask the agent to fix the failures
            self.agent.run_once(&format!(
                "The tests failed:\n{}\nPlease fix the issues.",
                test_result.output
            )).await?;
        }

        // Open a PR
        let pr = self.git.create_pull_request(
            &branch,
            &format!("Implement: {}", issue.title),
            &result.summary,
        )?;

        Ok(pr)
    }
}
```

::: tip In the Wild
This pattern is already emerging in production. GitHub's Copilot agent can pick up issues and create pull requests autonomously. Anthropic's Claude Code supports a headless mode for use in CI pipelines. The trajectory is clear: coding agents are moving from interactive tools to autonomous team members that participate in the standard software development workflow.
:::

## Improved Safety Through Sandboxing

Current safety models rely on rule-based filters and user approval. Future agents will likely operate in more sophisticated sandboxes:

- **Filesystem sandboxing** using container technologies or `seccomp` filters to prevent access outside the project directory at the OS level, not just the application level
- **Network sandboxing** to prevent the agent from making unexpected network calls
- **Resource limits** (CPU, memory, time) enforced by the runtime to prevent runaway operations
- **Capability-based security** where each tool call carries a proof of authorization

These mechanisms complement rather than replace the permission model you have built. Defense in depth means that even if the application-level safety check has a bug, the OS-level sandbox catches the violation.

## What to Build Next

With the foundation you have built, here are concrete projects that push into these future directions:

1. **Sub-agent delegation**: Add the ability for your agent to spawn focused sub-conversations for subtasks, merging the results back into the main context.

2. **Background mode**: Add a non-interactive mode that processes tasks from a queue (file, API, or message queue) and produces results as files or API calls.

3. **Project-aware system prompts**: Build an automatic system that reads project structure, README files, CI configuration, and coding conventions, then injects this context into the system prompt.

4. **Multi-model routing**: Implement a router that sends simple tasks (like file reads) to a fast, cheap model and complex tasks (like architectural decisions) to a frontier model.

5. **Self-verification**: After the agent makes a change, have it automatically run the project's test suite and linter, feeding any failures back into the loop before presenting the final result.

## Key Takeaways

- Multi-agent architectures with delegation and parallel execution are the next major frontier, enabling complex tasks that exceed the capabilities of a single agent's context and expertise.
- Growing context windows change the agent's strategy from selective file reading to holistic project understanding, but your architecture should adapt dynamically to the available context budget.
- Formal verification and strong type systems are becoming complementary to testing as trust mechanisms for agent-generated code — Rust's type system is particularly well-suited to this role.
- Background agents that process issues, create PRs, and iterate on feedback autonomously are shifting coding agents from interactive pair-programming tools to asynchronous team members.
- The architectural patterns you have learned — trait-based abstraction, component wiring, error boundaries, and safety layers — are the foundation on which all these future capabilities will be built.
