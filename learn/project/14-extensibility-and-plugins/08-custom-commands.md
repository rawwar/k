---
title: Custom Commands
description: Building a custom slash command system that lets plugins register new user-facing commands with argument parsing, help text, and tab completion.
---

# Custom Commands

> **What you'll learn:**
> - How to implement a command registry that maps slash commands to plugin handlers
> - Techniques for argument parsing and validation in custom command implementations
> - How to generate help text and tab-completion suggestions from command metadata

Your agent already handles tool calls from the LLM, but users also need direct control. Slash commands -- like `/help`, `/status`, or `/clear` -- give users a way to interact with the agent without going through the LLM. In this section, you will build a command system that lets plugins register their own slash commands, complete with argument parsing, help text, and tab completion.

## The Command Trait

Every slash command implements a trait that defines its behavior and metadata:

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata describing a slash command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    /// The command name without the leading slash (e.g., "status").
    pub name: String,
    /// Short description shown in help listings.
    pub summary: String,
    /// Detailed usage information.
    pub usage: String,
    /// Expected arguments.
    pub arguments: Vec<CommandArgument>,
    /// Which plugin registered this command.
    pub owner: String,
    /// Whether this command is hidden from help listings.
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default_value: Option<String>,
}

/// The result of executing a command.
#[derive(Debug)]
pub enum CommandResult {
    /// Display this text to the user.
    Output(String),
    /// The command modified agent state but has no output.
    Silent,
    /// The command wants to inject a user message into the conversation.
    InjectMessage(String),
    /// The command failed.
    Error(String),
}

/// The trait that all slash commands implement.
#[async_trait]
pub trait SlashCommand: Send + Sync {
    /// Return metadata about this command.
    fn metadata(&self) -> &CommandMetadata;

    /// Execute the command with the given arguments.
    async fn execute(
        &self,
        args: &[String],
        context: &CommandContext,
    ) -> CommandResult;

    /// Provide tab-completion suggestions for the given partial input.
    fn completions(&self, partial: &str, arg_index: usize) -> Vec<String> {
        // Default: no completions
        Vec::new()
    }
}

/// Context available to commands during execution.
pub struct CommandContext {
    pub session_id: String,
    pub working_directory: String,
    pub active_skills: Vec<String>,
    pub tool_count: usize,
    pub conversation_turn_count: usize,
}
```

::: tip Coming from Python
In Python CLI frameworks like Click or Typer, commands are decorated functions:
```python
import click

@click.command()
@click.argument("name")
@click.option("--verbose", "-v", is_flag=True)
def greet(name: str, verbose: bool):
    """Greet someone by name."""
    if verbose:
        click.echo(f"Hello there, {name}! Nice to meet you.")
    else:
        click.echo(f"Hello, {name}!")
```
Rust does not have decorators, but the pattern is the same: metadata (name, arguments, help text) is declared alongside the handler function. The `SlashCommand` trait replaces the decorator, and the `CommandMetadata` struct replaces the automatic introspection that Click performs. The explicit struct is more verbose but gives you compile-time guarantees that all metadata is present.
:::

## The Command Registry

The command registry stores all registered commands and dispatches user input:

```rust
pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn SlashCommand>>,
    aliases: HashMap<String, String>, // alias -> command name
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Unknown command: /{0}. Type /help for available commands.")]
    NotFound(String),
    #[error("Wrong number of arguments for /{cmd}: expected {expected}, got {got}")]
    WrongArgCount {
        cmd: String,
        expected: usize,
        got: usize,
    },
    #[error("Command /{0} is already registered by plugin '{1}'")]
    Conflict(String, String),
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    /// Register a new slash command.
    pub fn register(
        &mut self,
        command: Box<dyn SlashCommand>,
    ) -> Result<(), CommandError> {
        let name = command.metadata().name.clone();

        if let Some(existing) = self.commands.get(&name) {
            return Err(CommandError::Conflict(
                name,
                existing.metadata().owner.clone(),
            ));
        }

        self.commands.insert(name, command);
        Ok(())
    }

    /// Register an alias for an existing command.
    pub fn register_alias(
        &mut self,
        alias: &str,
        command_name: &str,
    ) -> Result<(), CommandError> {
        if !self.commands.contains_key(command_name) {
            return Err(CommandError::NotFound(command_name.to_string()));
        }
        self.aliases
            .insert(alias.to_string(), command_name.to_string());
        Ok(())
    }

    /// Remove all commands registered by a specific plugin.
    pub fn deregister_all_by_owner(&mut self, owner: &str) {
        self.commands
            .retain(|_, cmd| cmd.metadata().owner != owner);
        // Also remove aliases that point to removed commands
        let valid_commands: Vec<String> =
            self.commands.keys().cloned().collect();
        self.aliases
            .retain(|_, target| valid_commands.contains(target));
    }

    /// Parse and dispatch a slash command.
    pub async fn dispatch(
        &self,
        input: &str,
        context: &CommandContext,
    ) -> Result<CommandResult, CommandError> {
        let input = input.trim();

        // Split into command name and arguments
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd_name = parts[0].strip_prefix('/').unwrap_or(parts[0]);
        let arg_string = parts.get(1).unwrap_or(&"");

        // Resolve aliases
        let resolved_name = self
            .aliases
            .get(cmd_name)
            .map(|s| s.as_str())
            .unwrap_or(cmd_name);

        let command = self
            .commands
            .get(resolved_name)
            .ok_or_else(|| CommandError::NotFound(cmd_name.to_string()))?;

        // Parse arguments (respecting quoted strings)
        let args = parse_args(arg_string);

        // Validate required argument count
        let required_count = command
            .metadata()
            .arguments
            .iter()
            .filter(|a| a.required)
            .count();

        if args.len() < required_count {
            return Err(CommandError::WrongArgCount {
                cmd: cmd_name.to_string(),
                expected: required_count,
                got: args.len(),
            });
        }

        Ok(command.execute(&args, context).await)
    }

    /// Get tab-completion suggestions for partial input.
    pub fn complete(&self, partial: &str) -> Vec<String> {
        let partial = partial.strip_prefix('/').unwrap_or(partial);

        if !partial.contains(' ') {
            // Completing the command name
            let mut suggestions: Vec<String> = self
                .commands
                .keys()
                .filter(|name| name.starts_with(partial))
                .map(|name| format!("/{}", name))
                .collect();

            // Also check aliases
            for (alias, _) in &self.aliases {
                if alias.starts_with(partial) {
                    suggestions.push(format!("/{}", alias));
                }
            }

            suggestions.sort();
            suggestions
        } else {
            // Completing arguments for a specific command
            let parts: Vec<&str> = partial.splitn(2, ' ').collect();
            let cmd_name = parts[0];
            let arg_partial = parts.get(1).unwrap_or(&"");

            let resolved = self
                .aliases
                .get(cmd_name)
                .map(|s| s.as_str())
                .unwrap_or(cmd_name);

            if let Some(command) = self.commands.get(resolved) {
                let args = parse_args(arg_partial);
                command.completions(
                    args.last().map(|s| s.as_str()).unwrap_or(""),
                    args.len().saturating_sub(1),
                )
            } else {
                Vec::new()
            }
        }
    }

    /// Generate help text listing all commands.
    pub fn help_text(&self) -> String {
        let mut lines = vec!["Available commands:".to_string(), String::new()];

        let mut commands: Vec<&CommandMetadata> = self
            .commands
            .values()
            .map(|c| c.metadata())
            .filter(|m| !m.hidden)
            .collect();

        commands.sort_by_key(|m| &m.name);

        for meta in commands {
            lines.push(format!("  /{:<16} {}", meta.name, meta.summary));
        }

        lines.push(String::new());
        lines.push("Type /help <command> for detailed usage.".to_string());
        lines.join("\n")
    }
}

/// Parse an argument string, respecting quoted strings.
fn parse_args(input: &str) -> Vec<String> {
    let input = input.trim();
    if input.is_empty() {
        return Vec::new();
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';

    for ch in input.chars() {
        if in_quotes {
            if ch == quote_char {
                in_quotes = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quotes = true;
            quote_char = ch;
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                args.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}
```

## Built-in Commands

Let's implement the core commands every agent needs:

```rust
/// The /help command.
pub struct HelpCommand {
    metadata: CommandMetadata,
}

impl HelpCommand {
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata {
                name: "help".to_string(),
                summary: "Show available commands".to_string(),
                usage: "/help [command]".to_string(),
                arguments: vec![CommandArgument {
                    name: "command".to_string(),
                    description: "Command to get detailed help for".to_string(),
                    required: false,
                    default_value: None,
                }],
                owner: "core".to_string(),
                hidden: false,
            },
        }
    }
}

#[async_trait]
impl SlashCommand for HelpCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        args: &[String],
        _context: &CommandContext,
    ) -> CommandResult {
        // When a specific command is requested, the registry handles
        // providing detailed help. Here we return a generic message.
        if args.is_empty() {
            CommandResult::Output(
                "Use /help to see all commands.\n\
                 Type your message to chat with the agent.\n\
                 Use /skills to manage active skills."
                    .to_string(),
            )
        } else {
            CommandResult::Output(format!(
                "Detailed help for /{} is not yet available.",
                args[0]
            ))
        }
    }
}

/// The /status command shows agent state.
pub struct StatusCommand {
    metadata: CommandMetadata,
}

impl StatusCommand {
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata {
                name: "status".to_string(),
                summary: "Show agent status and active configuration".to_string(),
                usage: "/status".to_string(),
                arguments: vec![],
                owner: "core".to_string(),
                hidden: false,
            },
        }
    }
}

#[async_trait]
impl SlashCommand for StatusCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        _args: &[String],
        context: &CommandContext,
    ) -> CommandResult {
        let output = format!(
            "Agent Status:\n\
             \n\
             Session:          {}\n\
             Working Directory: {}\n\
             Active Skills:    {}\n\
             Available Tools:  {}\n\
             Conversation Turns: {}",
            context.session_id,
            context.working_directory,
            if context.active_skills.is_empty() {
                "none".to_string()
            } else {
                context.active_skills.join(", ")
            },
            context.tool_count,
            context.conversation_turn_count,
        );
        CommandResult::Output(output)
    }
}

/// The /skills command manages skill activation.
pub struct SkillsCommand {
    metadata: CommandMetadata,
}

impl SkillsCommand {
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata {
                name: "skills".to_string(),
                summary: "List, activate, or deactivate skills".to_string(),
                usage: "/skills [activate|deactivate] [skill-name]".to_string(),
                arguments: vec![
                    CommandArgument {
                        name: "action".to_string(),
                        description: "Action: activate, deactivate, or omit to list".to_string(),
                        required: false,
                        default_value: None,
                    },
                    CommandArgument {
                        name: "skill".to_string(),
                        description: "Name of the skill".to_string(),
                        required: false,
                        default_value: None,
                    },
                ],
                owner: "core".to_string(),
                hidden: false,
            },
        }
    }
}

#[async_trait]
impl SlashCommand for SkillsCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        args: &[String],
        _context: &CommandContext,
    ) -> CommandResult {
        match args.first().map(|s| s.as_str()) {
            None | Some("list") => {
                // In practice, this would query the SkillLoader
                CommandResult::Output("Use /skills activate <name> or /skills deactivate <name>".to_string())
            }
            Some("activate") => {
                if let Some(name) = args.get(1) {
                    // This would call skill_loader.activate(name)
                    CommandResult::Output(format!("Skill '{}' activation requested.", name))
                } else {
                    CommandResult::Error("Usage: /skills activate <skill-name>".to_string())
                }
            }
            Some("deactivate") => {
                if let Some(name) = args.get(1) {
                    CommandResult::Output(format!("Skill '{}' deactivation requested.", name))
                } else {
                    CommandResult::Error("Usage: /skills deactivate <skill-name>".to_string())
                }
            }
            Some(other) => {
                CommandResult::Error(format!("Unknown action: '{}'. Use activate, deactivate, or list.", other))
            }
        }
    }

    fn completions(&self, partial: &str, arg_index: usize) -> Vec<String> {
        match arg_index {
            0 => {
                // Complete the action
                vec!["activate", "deactivate", "list"]
                    .into_iter()
                    .filter(|a| a.starts_with(partial))
                    .map(String::from)
                    .collect()
            }
            1 => {
                // Complete the skill name -- would query SkillLoader
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}
```

## Integrating Commands into the Input Loop

Wire the command registry into your agent's input handler:

```rust
impl Agent {
    pub async fn handle_input(&self, input: &str) -> Result<String> {
        let trimmed = input.trim();

        // Check if this is a slash command
        if trimmed.starts_with('/') {
            let context = CommandContext {
                session_id: self.session_id.clone(),
                working_directory: self.working_dir.display().to_string(),
                active_skills: self.skill_loader.list_skills()
                    .into_iter()
                    .filter(|s| s.active)
                    .map(|s| s.name)
                    .collect(),
                tool_count: self.tool_registry.read().await.list_definitions().len(),
                conversation_turn_count: self.conversation.len(),
            };

            let registry = self.command_registry.read().await;
            match registry.dispatch(trimmed, &context).await {
                Ok(CommandResult::Output(text)) => return Ok(text),
                Ok(CommandResult::Silent) => return Ok(String::new()),
                Ok(CommandResult::InjectMessage(msg)) => {
                    // Treat this as a regular message to the LLM
                    return self.handle_user_message(&msg).await;
                }
                Ok(CommandResult::Error(err)) => return Ok(format!("Error: {}", err)),
                Err(e) => return Ok(format!("{}", e)),
            }
        }

        // Regular message -- send to the LLM
        self.handle_user_message(trimmed).await
    }
}
```

::: info In the Wild
Claude Code provides built-in slash commands like `/help`, `/clear`, `/compact`, `/status`, and `/init`. The `/compact` command is particularly interesting -- it summarizes the current conversation to reduce context size, a feature that the LLM itself cannot trigger. Custom slash commands from plugins would follow the same pattern: direct user actions that bypass the LLM conversation loop.
:::

## Key Takeaways

- Slash commands provide a direct user-to-agent interaction path that bypasses the LLM, essential for control operations like session management, skill toggling, and status inspection
- The `SlashCommand` trait encapsulates both behavior (the `execute` method) and metadata (name, arguments, help text), enabling auto-generated help and tab completion
- Argument parsing respects quoted strings and validates required arguments before dispatching, giving users clear error messages for incorrect usage
- The `CommandResult` enum supports multiple response types -- text output, silent state changes, and message injection back into the conversation loop
- Tab completion using command and argument metadata makes the agent feel responsive and discoverable, especially for commands with subcommands like `/skills activate`
