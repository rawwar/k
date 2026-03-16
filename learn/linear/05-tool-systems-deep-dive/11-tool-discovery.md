---
title: Tool Discovery
description: Dynamic tool registration, conditional tool availability, and how agents can adapt their tool set based on project context.
---

# Tool Discovery

> **What you'll learn:**
> - How dynamic tool registration allows agents to load tools based on the project type and available executables
> - Strategies for conditional tool availability where some tools are only offered when prerequisites are met
> - How tool discovery interacts with context window limits since each tool definition consumes tokens

So far, we have treated the tool set as fixed -- a predetermined list of tools that the agent always has available. But in practice, the tools an agent needs depend on the project it is working on. A Rust project needs `cargo` integration. A Python project needs `pip` and `pytest`. A JavaScript project needs `npm` and `node`. Tool discovery is the mechanism that adapts the agent's capabilities to its environment.

## Static vs Dynamic Tool Sets

The simplest approach is a static tool set: define all your tools at compile time and always send all of them to the model.

```rust
pub fn get_all_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::read_file(),
        ToolDefinition::write_file(),
        ToolDefinition::edit_file(),
        ToolDefinition::list_files(),
        ToolDefinition::search_files(),
        ToolDefinition::shell(),
    ]
}
```

This works fine for a small tool set. But as you add more tools, two problems emerge:

1. **Token cost.** Each tool definition consumes tokens in the context window. The name, description, and full JSON Schema of each tool are sent with every API request. Ten simple tools might consume 2,000-3,000 tokens. Twenty detailed tools might consume 8,000-10,000 tokens. That is a significant portion of your context budget spent on tool definitions alone.

2. **Decision complexity.** The more tools the model sees, the harder it is to choose the right one. A model choosing between 5 tools is more accurate than a model choosing between 30. This is not just theoretical -- research shows that model accuracy on tool selection decreases as the number of available tools increases.

Dynamic tool sets solve both problems by only including tools that are relevant to the current context.

## Detecting the Project Environment

The first step in dynamic discovery is understanding what kind of project the agent is working with:

```rust
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ProjectEnvironment {
    pub root: String,
    pub languages: Vec<Language>,
    pub available_commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Unknown(String),
}

pub fn detect_environment(project_root: &str) -> ProjectEnvironment {
    let root = Path::new(project_root);
    let mut languages = Vec::new();

    // Detect languages by marker files
    if root.join("Cargo.toml").exists() {
        languages.push(Language::Rust);
    }
    if root.join("pyproject.toml").exists() || root.join("setup.py").exists() {
        languages.push(Language::Python);
    }
    if root.join("package.json").exists() {
        let has_ts = root.join("tsconfig.json").exists();
        if has_ts {
            languages.push(Language::TypeScript);
        } else {
            languages.push(Language::JavaScript);
        }
    }
    if root.join("go.mod").exists() {
        languages.push(Language::Go);
    }

    // Detect available commands
    let commands_to_check = [
        "cargo", "rustc", "python3", "pip", "pytest",
        "node", "npm", "npx", "go", "git", "docker",
    ];

    let available_commands: Vec<String> = commands_to_check
        .iter()
        .filter(|cmd| command_exists(cmd))
        .map(|cmd| cmd.to_string())
        .collect();

    ProjectEnvironment {
        root: project_root.to_string(),
        languages,
        available_commands,
    }
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
```

## Conditional Tool Registration

With the environment detected, you can register tools conditionally:

```rust
pub fn discover_tools(env: &ProjectEnvironment) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    // Core tools are always available
    tools.push(ToolDefinition::read_file());
    tools.push(ToolDefinition::write_file());
    tools.push(ToolDefinition::edit_file());
    tools.push(ToolDefinition::list_files());
    tools.push(ToolDefinition::search_files());
    tools.push(ToolDefinition::shell());

    // Rust-specific tools
    if env.languages.contains(&Language::Rust)
        && env.available_commands.contains(&"cargo".to_string())
    {
        tools.push(ToolDefinition::cargo_check());
        tools.push(ToolDefinition::cargo_test());
    }

    // Python-specific tools
    if env.languages.contains(&Language::Python)
        && env.available_commands.contains(&"pytest".to_string())
    {
        tools.push(ToolDefinition::pytest_run());
    }

    // Git tools (available if git repo)
    if std::path::Path::new(&env.root).join(".git").exists() {
        tools.push(ToolDefinition::git_status());
        tools.push(ToolDefinition::git_diff());
    }

    tools
}
```

Now a Rust project gets `cargo_check` and `cargo_test` tools, a Python project gets `pytest_run`, and a project without git does not get git tools cluttering the tool list.

::: python Coming from Python
In Python, conditional registration might look like a plugin system:
```python
tools = [read_file, write_file, edit_file]  # Core tools

if Path("Cargo.toml").exists() and shutil.which("cargo"):
    tools.extend([cargo_check, cargo_test])

if Path("pyproject.toml").exists() and shutil.which("pytest"):
    tools.append(pytest_run)
```
Rust's approach is structurally similar. The main difference is type safety: each tool definition in Rust is a typed struct, and the compiler ensures you do not accidentally reference a tool that was not registered.
:::

## The Token Budget Problem

Every tool definition consumes tokens. Let's quantify this:

```rust
pub fn estimate_tool_tokens(tools: &[ToolDefinition]) -> usize {
    let mut total = 0;
    for tool in tools {
        // Rough estimate: tool name + description + schema
        let name_tokens = tool.name.len() / 4;
        let desc_tokens = tool.description.len() / 4;
        let schema_str = serde_json::to_string(&tool.input_schema)
            .unwrap_or_default();
        let schema_tokens = schema_str.len() / 4;

        total += name_tokens + desc_tokens + schema_tokens;
        // Add overhead for JSON structure
        total += 20;
    }
    total
}
```

For a typical tool with a 200-character description and a schema with 5 parameters, you might use 150-250 tokens. With 20 tools, that is 3,000-5,000 tokens dedicated to tool definitions before the conversation even starts.

This creates a design tension: more tools give the model more capabilities, but fewer tools give it more context for reasoning. The solution is to be selective about which tools you include and to keep descriptions and schemas as concise as possible.

## Dynamic Tool Sets Per Turn

An advanced pattern is to adjust the tool set on a per-turn basis. Early in a conversation, you might include discovery tools (search, list files). Once the model is actively editing, you might swap in editing-focused tools and remove discovery tools:

```rust
pub fn tools_for_phase(
    phase: &ConversationPhase,
    env: &ProjectEnvironment,
) -> Vec<ToolDefinition> {
    match phase {
        ConversationPhase::Exploring => {
            // Heavy on search and discovery
            vec![
                ToolDefinition::read_file(),
                ToolDefinition::list_files(),
                ToolDefinition::search_files(),
                ToolDefinition::shell(),
            ]
        }
        ConversationPhase::Implementing => {
            // Heavy on editing and verification
            vec![
                ToolDefinition::read_file(),
                ToolDefinition::write_file(),
                ToolDefinition::edit_file(),
                ToolDefinition::search_files(),
                ToolDefinition::shell(),
            ]
        }
        ConversationPhase::Debugging => {
            // All tools available
            discover_tools(env)
        }
    }
}

pub enum ConversationPhase {
    Exploring,
    Implementing,
    Debugging,
}
```

This is a more sophisticated approach that requires phase detection logic. In practice, most agents send the full relevant tool set on every turn and let the model sort it out. Phase-based tool selection is an optimization you add later when you observe token budget pressure.

::: wild In the Wild
Claude Code includes all available tools in every API request, relying on the model's judgment to pick the right one. The tool set is determined at startup based on the project environment. OpenCode also registers all tools at startup, though it has a smaller overall tool set. Neither agent dynamically adjusts tools per turn in the public implementations, though this is an area of active research in the agent community.
:::

## Plugin Architectures

If you want users to extend your agent with custom tools, you need a plugin architecture. The simplest approach is a trait that plugins implement:

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn execute(&self, input: serde_json::Value) -> Result<String, ToolError>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|t| ToolDefinition {
                name: t.name(),
                description: t.description(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    pub fn execute(&self, name: &str, input: serde_json::Value) -> Result<String, ToolError> {
        let tool = self.tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| ToolError::ToolFailure {
                message: format!("Unknown tool: '{}'", name),
                suggestion: Some("Check available tools.".to_string()),
            })?;

        tool.execute(input)
    }
}
```

This trait-based approach lets you add tools at runtime, load them from configuration files, or even let users write custom tools in a scripting language.

## Key Takeaways

- Dynamic tool discovery adapts the agent's capabilities to the project type and available executables, reducing irrelevant tools
- Each tool definition consumes tokens in the context window -- be selective about which tools you include and keep definitions concise
- Detect the project environment at startup (marker files, available commands) and register tools conditionally
- A trait-based Tool interface enables plugin architectures where users can extend the agent with custom tools
- Start with a static tool set for simplicity, then add dynamic discovery when you observe token budget pressure or irrelevant tool selection
