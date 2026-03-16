---
title: Skill System Design
description: Design a skill system that packages tools, prompts, and workflows into reusable higher-level capabilities that can be loaded on demand.
---

# Skill System Design

> **What you'll learn:**
> - How skills differ from raw tools -- skills combine a system prompt fragment, one or more tools, and execution logic into a cohesive capability
> - How to design a skill manifest format that declares dependencies, configuration options, and activation triggers
> - Techniques for implementing skill loading, activation (slash commands, auto-detection), and context injection into the agent's conversation

Tools are the atoms of agent capability. A `read_file` tool reads a file. A `shell` tool runs a command. But most real tasks require orchestrating multiple tools with domain-specific knowledge. "Review this pull request" requires reading files, understanding diffs, knowing code review conventions, and producing structured feedback. Packaging this orchestration into a reusable unit is what skills are for.

A skill is a higher-level abstraction that bundles together a system prompt fragment (domain knowledge), a set of tools (capabilities), and optionally a workflow (a sequence of steps). When activated, a skill enriches the agent's context with specialized knowledge and ensures the right tools are available.

## Skills vs. Tools vs. Plugins

These three concepts operate at different levels:

| Concept | Level | Example | Provides |
|---------|-------|---------|----------|
| Tool | Atomic | `read_file`, `shell` | A single action the LLM can invoke |
| Skill | Composed | "Code Review", "Database Migration" | Prompt + tools + workflow |
| Plugin | Infrastructure | "PostgreSQL Plugin" | Tools + resources + config + hooks |

A plugin is a packaging mechanism -- it can provide tools, hook handlers, event listeners, and configuration. A skill is a user-facing capability -- it provides specialized context and behavior for a specific task type. Plugins are loaded at startup; skills are activated on demand.

## The Skill Manifest

Every skill declares what it provides and what it requires through a manifest. This manifest can be defined in code or loaded from a configuration file:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Unique identifier, e.g., "code-review"
    pub name: String,
    /// Human-readable display name
    pub display_name: String,
    /// What this skill does (shown in help and slash command listings)
    pub description: String,
    /// Version of this skill
    pub version: String,
    /// Tools this skill requires to be available
    pub required_tools: Vec<String>,
    /// Additional tools this skill provides
    pub provided_tools: Vec<String>,
    /// How the skill is activated
    pub activation: SkillActivation,
    /// Configuration options the user can set
    pub config_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillActivation {
    /// Activated by a slash command, e.g., "/review"
    SlashCommand { command: String, aliases: Vec<String> },
    /// Activated automatically when certain file patterns are detected
    AutoDetect { file_patterns: Vec<String> },
    /// Always active
    AlwaysOn,
    /// Activated manually through configuration
    Manual,
}
```

## The Skill Trait

The core skill trait defines how a skill integrates with the agent:

```rust
#[async_trait::async_trait]
pub trait Skill: Send + Sync {
    /// Return the skill's manifest.
    fn manifest(&self) -> &SkillManifest;

    /// Called when the skill is activated.
    /// Returns a system prompt fragment to inject into the conversation.
    fn system_prompt(&self) -> &str;

    /// Optional: provide additional tools specific to this skill.
    fn tools(&self) -> Vec<Box<dyn Tool>> {
        Vec::new()
    }

    /// Optional: transform user input before it reaches the LLM.
    /// For example, a code review skill might prepend "Review the following:"
    async fn transform_input(
        &self,
        input: &str,
        _context: &SkillContext,
    ) -> Result<String> {
        Ok(input.to_string())
    }

    /// Optional: post-process the LLM's response.
    /// For example, a skill might format the output as a checklist.
    async fn transform_output(
        &self,
        output: &str,
        _context: &SkillContext,
    ) -> Result<String> {
        Ok(output.to_string())
    }

    /// Called when the skill is deactivated.
    async fn deactivate(&self) -> Result<()> {
        Ok(())
    }
}

/// Context available to skills during execution.
pub struct SkillContext {
    pub project_root: std::path::PathBuf,
    pub active_file: Option<String>,
    pub config: serde_json::Value,
}
```

## Example: A Code Review Skill

Let's implement a concrete skill that demonstrates the pattern:

```rust
pub struct CodeReviewSkill {
    manifest: SkillManifest,
}

impl CodeReviewSkill {
    pub fn new() -> Self {
        Self {
            manifest: SkillManifest {
                name: "code-review".to_string(),
                display_name: "Code Review".to_string(),
                description: "Review code changes with structured feedback \
                              covering correctness, style, and security".to_string(),
                version: "1.0.0".to_string(),
                required_tools: vec![
                    "read_file".to_string(),
                    "shell".to_string(),
                ],
                provided_tools: vec![
                    "get_diff".to_string(),
                    "post_review_comment".to_string(),
                ],
                activation: SkillActivation::SlashCommand {
                    command: "review".to_string(),
                    aliases: vec!["cr".to_string()],
                },
                config_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "severity_threshold": {
                            "type": "string",
                            "enum": ["info", "warning", "error"],
                            "default": "warning"
                        },
                        "check_security": {
                            "type": "boolean",
                            "default": true
                        }
                    }
                })),
            },
        }
    }
}

#[async_trait::async_trait]
impl Skill for CodeReviewSkill {
    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn system_prompt(&self) -> &str {
        r#"You are performing a code review. Follow these guidelines:

1. **Correctness**: Check for bugs, edge cases, and logic errors.
2. **Style**: Flag inconsistencies with the project's existing patterns.
3. **Security**: Look for injection vulnerabilities, hardcoded secrets,
   and unsafe operations.
4. **Performance**: Note obvious performance issues but don't micro-optimize.

Structure your review as:
- A summary (2-3 sentences)
- A list of findings, each with severity (info/warning/error) and a suggestion
- An overall verdict (approve, request changes, or comment)

Use the get_diff tool to see the changes, and read_file to view full file
context when needed."#
    }

    fn tools(&self) -> Vec<Box<dyn Tool>> {
        vec![
            Box::new(GetDiffTool::new()),
            Box::new(PostReviewCommentTool::new()),
        ]
    }

    async fn transform_input(
        &self,
        input: &str,
        context: &SkillContext,
    ) -> Result<String> {
        // If the user just said "/review", expand it to a full instruction
        if input.trim().is_empty() {
            Ok("Review the current changes in this repository. \
                Use get_diff to see what has changed.".to_string())
        } else {
            Ok(input.to_string())
        }
    }
}
```

::: python Coming from Python
Python agent frameworks often implement skills as decorated functions or classes:
```python
@skill(name="code-review", command="/review")
class CodeReviewSkill:
    system_prompt = "You are performing a code review..."
    required_tools = ["read_file", "shell"]

    def transform_input(self, input: str) -> str:
        if not input.strip():
            return "Review the current changes in this repository."
        return input
```
Rust's trait-based approach gives you the same structure with stronger guarantees: the compiler verifies that every skill implements the required methods and that tool types match expected interfaces.
:::

## The Skill Registry

The skill registry manages available skills and handles activation:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
    active_skill: RwLock<Option<String>>,
    slash_commands: HashMap<String, String>, // command -> skill name
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            active_skill: RwLock::new(None),
            slash_commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        let manifest = skill.manifest();

        // Register slash command activation
        if let SkillActivation::SlashCommand { ref command, ref aliases } =
            manifest.activation
        {
            self.slash_commands
                .insert(command.clone(), manifest.name.clone());
            for alias in aliases {
                self.slash_commands
                    .insert(alias.clone(), manifest.name.clone());
            }
        }

        self.skills.insert(manifest.name.clone(), skill);
    }

    /// Try to activate a skill based on user input.
    /// Returns the skill's system prompt and any additional tools if activated.
    pub async fn try_activate(
        &self,
        input: &str,
    ) -> Option<SkillActivationResult> {
        // Check slash commands
        if input.starts_with('/') {
            let command = input.split_whitespace().next()?;
            let command = command.trim_start_matches('/');

            if let Some(skill_name) = self.slash_commands.get(command) {
                if let Some(skill) = self.skills.get(skill_name) {
                    let mut active = self.active_skill.write().await;
                    *active = Some(skill_name.clone());

                    return Some(SkillActivationResult {
                        skill_name: skill_name.clone(),
                        system_prompt: skill.system_prompt().to_string(),
                        additional_tools: skill.tools(),
                    });
                }
            }
        }

        // Check auto-detect skills
        for (name, skill) in &self.skills {
            if let SkillActivation::AutoDetect { ref file_patterns } =
                skill.manifest().activation
            {
                // Check if the current context matches any file patterns
                for pattern in file_patterns {
                    if input.contains(pattern) {
                        let mut active = self.active_skill.write().await;
                        *active = Some(name.clone());

                        return Some(SkillActivationResult {
                            skill_name: name.clone(),
                            system_prompt: skill.system_prompt().to_string(),
                            additional_tools: skill.tools(),
                        });
                    }
                }
            }
        }

        None
    }

    /// Deactivate the current skill.
    pub async fn deactivate_current(&self) -> Result<()> {
        let mut active = self.active_skill.write().await;
        if let Some(skill_name) = active.take() {
            if let Some(skill) = self.skills.get(&skill_name) {
                skill.deactivate().await?;
            }
        }
        Ok(())
    }

    /// List all available skills for help output.
    pub fn list_skills(&self) -> Vec<SkillInfo> {
        self.skills.values().map(|skill| {
            let manifest = skill.manifest();
            SkillInfo {
                name: manifest.name.clone(),
                display_name: manifest.display_name.clone(),
                description: manifest.description.clone(),
                activation: format!("{:?}", manifest.activation),
            }
        }).collect()
    }
}

pub struct SkillActivationResult {
    pub skill_name: String,
    pub system_prompt: String,
    pub additional_tools: Vec<Box<dyn Tool>>,
}

pub struct SkillInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub activation: String,
}
```

::: wild In the Wild
Claude Code implements a skill-like system through its slash commands. When you type `/review`, Claude Code activates a code review mode that loads a specialized prompt and makes certain tools more prominent. The "skill" concept packages domain expertise (the review prompt), tool requirements (file reading, diff viewing), and an activation trigger (the slash command) into a single, discoverable unit. This makes the agent feel more like a specialized assistant that has different "modes" of operation rather than a generic tool caller.
:::

## Integrating Skills into the Agentic Loop

The agentic loop checks for skill activation before sending messages to the LLM:

```rust
pub async fn process_user_input(
    input: &str,
    skill_registry: &SkillRegistry,
    tool_registry: &mut ToolRegistry,
    system_prompt: &mut String,
) -> String {
    // Check if this input activates a skill
    if let Some(activation) = skill_registry.try_activate(input).await {
        // Inject the skill's system prompt
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&activation.system_prompt);

        // Register the skill's additional tools
        for tool in activation.additional_tools {
            tool_registry.register(tool);
        }

        // Strip the slash command from the input
        let user_input = input
            .split_whitespace()
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ");

        // Let the skill transform the remaining input
        if let Some(skill) = skill_registry.skills.get(&activation.skill_name) {
            let context = SkillContext {
                project_root: std::env::current_dir().unwrap_or_default(),
                active_file: None,
                config: serde_json::Value::Null,
            };
            return skill.transform_input(&user_input, &context)
                .await
                .unwrap_or(user_input);
        }

        return user_input;
    }

    input.to_string()
}
```

## Key Takeaways

- **Skills** are higher-level than tools: they bundle a system prompt fragment, tool requirements, and optional input/output transformations into a reusable capability for specific task types.
- A **skill manifest** declares activation triggers (slash commands, auto-detection, always-on), tool dependencies, configuration options, and versioning information.
- The **skill registry** manages discovery, activation, and deactivation, mapping slash commands and file patterns to the appropriate skill.
- Skills integrate into the agentic loop by **injecting context** (system prompt additions) and **registering tools** when activated, enriching the LLM's capabilities for the duration of the task.
- The skill pattern works well for **domain-specific modes** like code review, database migration, debugging, and documentation generation -- tasks that benefit from specialized prompts and tool sets.
