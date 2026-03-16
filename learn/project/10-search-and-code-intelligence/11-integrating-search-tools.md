---
title: Integrating Search Tools
description: Register all search tools with the agent's tool system and design schemas that make them easy for the LLM to use effectively.
---

# Integrating Search Tools

> **What you'll learn:**
> - How to design JSON schemas for grep, glob, and semantic search tools that guide LLM usage
> - How to write tool descriptions that help the LLM choose the right search tool for each task
> - How to format search results as tool output that the LLM can parse and act on efficiently

You have built the search engines: grep for content search, glob for file discovery, tree-sitter for structural analysis, language detection for grammar selection, and a ranking system for prioritizing results. Now it is time to connect everything to the agent's tool system so the LLM can actually use these capabilities. This subchapter focuses on the integration layer -- the JSON schemas, tool descriptions, result formatting, and registration code that make search tools available to the agent.

## Tool Design Principles

Before writing code, let's establish the design principles that make search tools effective for LLM consumption:

**1. One tool, one purpose.** Do not combine grep and glob into a single "search" tool. The LLM needs to reason about which search strategy to use, and distinct tools with clear purposes make that reasoning easier.

**2. Descriptive, actionable tool descriptions.** The description is not documentation for humans -- it is guidance for the LLM. Tell the model *when* to use this tool and *what* to expect from the results.

**3. Sensible defaults for every optional parameter.** The LLM should be able to call the tool with just the required parameters and get useful results. Advanced parameters are there for refinement, not as prerequisites.

**4. Structured output with explicit truncation.** Always tell the LLM how many results were returned, how many were truncated, and what the model can do to narrow the search.

::: python Coming from Python
Python tool frameworks like LangChain use similar patterns:
```python
from langchain.tools import tool

@tool
def grep(
    pattern: str,
    path: str = ".",
    include: str = None,
    context_lines: int = 2,
) -> str:
    """Search file contents using regex patterns. Returns matching lines
    with surrounding context. Use this to find function definitions,
    error messages, imports, and any text pattern across the codebase."""
    # implementation...
```
In Rust, you define the schema as a `serde_json::Value` and the execution logic in an `async fn`. The pattern is the same: clear parameter descriptions, sensible defaults, and a tool docstring that guides usage.
:::

## The Complete Search Tool Suite

Here is the full implementation of all three search tools registered with the agent. Each tool follows the `Tool` trait from Chapter 4:

### Grep Tool Registration

```rust
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GrepTool {
    max_results: usize,
}

impl GrepTool {
    pub fn new() -> Self {
        Self { max_results: 50 }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using regex patterns. Returns matching lines \
         with surrounding context. Use this to find where functions are called, \
         locate error messages, find import statements, and search for any \
         text pattern in the codebase. Respects .gitignore rules. For finding \
         files by name, use the glob tool instead."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for. Use \\b for word \
                                    boundaries. Use literal strings for exact matching."
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in. Defaults to the project root."
                },
                "include": {
                    "type": "string",
                    "description": "Glob to filter files, e.g., '*.rs', '*.{ts,tsx}'. \
                                    Only files matching this pattern are searched."
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Lines of context to show before and after each \
                                    match. Default: 2."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "If true, search case-insensitively. Default: false."
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<String, String> {
        let params: GrepToolInput = serde_json::from_value(input)
            .map_err(|e| format!("Invalid input: {e}"))?;

        let matches = grep_search(&params)?;
        Ok(format_grep_results(&matches, self.max_results))
    }
}
```

### Glob Tool Registration

```rust
pub struct GlobTool {
    max_results: usize,
}

impl GlobTool {
    pub fn new() -> Self {
        Self { max_results: 100 }
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files by name pattern. Returns matching file paths sorted by \
         modification time (most recent first). Use this to discover project \
         structure, find test files, locate configuration files, or identify \
         all files of a specific type. For searching file contents, use the \
         grep tool instead."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern. Use ** for recursive matching. \
                                    Examples: '**/*.rs', 'src/**/*.ts', '**/test_*.py'"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in. Defaults to project root."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results. Default: 100."
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<String, String> {
        let mut params: GlobToolInput = serde_json::from_value(input)
            .map_err(|e| format!("Invalid input: {e}"))?;

        // Normalize the pattern (add **/ prefix if needed)
        params.pattern = normalize_glob_pattern(&params.pattern);

        let results = glob_search(&params)?;
        Ok(format_glob_results(&results))
    }
}
```

### Semantic Search Tool Registration

```rust
pub struct SemanticSearchTool {
    index: std::sync::Arc<std::sync::RwLock<SymbolIndex>>,
}

impl SemanticSearchTool {
    pub fn new(index: std::sync::Arc<std::sync::RwLock<SymbolIndex>>) -> Self {
        Self { index }
    }
}

#[derive(Debug, serde::Deserialize)]
struct SemanticSearchInput {
    /// The symbol name to search for
    name: String,
    /// Optional kind filter: "function", "struct", "enum", "trait"
    #[serde(default)]
    kind: Option<String>,
}

#[async_trait]
impl Tool for SemanticSearchTool {
    fn name(&self) -> &str {
        "symbol_search"
    }

    fn description(&self) -> &str {
        "Search for code symbols (functions, structs, enums, traits) by name. \
         Returns definition locations with signatures. Use this when you need \
         to find where a type or function is defined, what methods a struct has, \
         or what a function's signature looks like. More precise than grep for \
         finding definitions."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Symbol name to search for (exact or prefix match)"
                },
                "kind": {
                    "type": "string",
                    "enum": ["function", "struct", "enum", "trait"],
                    "description": "Filter by symbol kind. Omit to search all kinds."
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<String, String> {
        let params: SemanticSearchInput = serde_json::from_value(input)
            .map_err(|e| format!("Invalid input: {e}"))?;

        let index = self.index.read()
            .map_err(|_| "Failed to acquire index lock".to_string())?;

        let kind_filter = params.kind.as_ref().map(|k| match k.as_str() {
            "function" => SymbolKind::Function,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "trait" => SymbolKind::Trait,
            _ => SymbolKind::Function, // Default fallback
        });

        // Try exact match first, then prefix match
        let mut results = index.lookup(&params.name);

        if results.is_empty() {
            results = index.search_prefix(&params.name);
        }

        // Apply kind filter if specified
        if let Some(ref kind) = kind_filter {
            results.retain(|s| &s.kind == kind);
        }

        if results.is_empty() {
            return Ok(format!(
                "No symbols found matching '{}'. Try using grep for a text search.",
                params.name
            ));
        }

        Ok(format_symbol_lookup_results(&results))
    }
}

fn format_symbol_lookup_results(results: &[&SymbolDefinition]) -> String {
    let mut output = format!("Found {} symbol(s):\n\n", results.len());

    for sym in results {
        let parent_info = sym
            .parent
            .as_ref()
            .map(|p| format!(" (in impl {p})"))
            .unwrap_or_default();

        output.push_str(&format!(
            "{:?}: {} at {}:{}{}\n  {}\n\n",
            sym.kind,
            sym.name,
            sym.file.display(),
            sym.line,
            parent_info,
            sym.signature,
        ));
    }

    output
}
```

## Registering All Tools

In your agent's initialization code, register all search tools together:

```rust
use std::sync::{Arc, RwLock};

pub fn register_search_tools(
    registry: &mut ToolRegistry,
    project_root: &std::path::Path,
) {
    // Register grep tool
    registry.register(Box::new(GrepTool::new()));

    // Register glob tool
    registry.register(Box::new(GlobTool::new()));

    // Build the symbol index and register semantic search
    let index = Arc::new(RwLock::new(SymbolIndex::new()));

    // Index the project in the background
    let index_clone = Arc::clone(&index);
    let root = project_root.to_path_buf();
    std::thread::spawn(move || {
        match index_project(&root) {
            Ok(built_index) => {
                if let Ok(mut idx) = index_clone.write() {
                    *idx = built_index;
                }
            }
            Err(e) => eprintln!("Warning: failed to build symbol index: {e}"),
        }
    });

    registry.register(Box::new(SemanticSearchTool::new(index)));
}
```

Notice the background indexing: the symbol index is built on a separate thread so the agent is responsive immediately. The `Arc<RwLock<SymbolIndex>>` allows the search tool to read the index while it is being built -- early queries will return fewer results, but the agent does not block waiting for indexing to complete.

## Guiding LLM Tool Selection

The tool descriptions are your primary lever for guiding which tool the LLM selects. Here is how the three tools complement each other:

| Task | Best Tool | Why |
|------|-----------|-----|
| "Find where `parse_config` is called" | grep | Text pattern search across all files |
| "Find all Rust files in the project" | glob | File name pattern matching |
| "Where is the `Config` struct defined?" | symbol_search | Structural definition lookup |
| "Find all TODO comments" | grep (with include filter) | Text pattern in specific context |
| "What methods does `Database` have?" | symbol_search (kind: function) | Structural method listing |
| "Find all test files" | glob (`**/*_test.rs`) | File name pattern |

The descriptions include phrases like "For finding files by name, use the glob tool instead" -- these cross-references help the LLM choose correctly when the task is ambiguous.

::: wild In the Wild
Claude Code exposes grep, glob, and a file read tool as its core search primitives. The tool descriptions are carefully worded to help the model differentiate between them. For example, the grep description mentions "searching file contents" while the glob description mentions "finding files by name pattern." This seemingly small difference in wording significantly affects how often the model picks the right tool. Some agents also include example invocations in the description to further guide the LLM.
:::

## Result Format Consistency

All three tools should produce output in a consistent format so the LLM can parse results uniformly. Establish conventions:

```rust
/// Standard result header format
pub fn result_header(tool_name: &str, count: usize, total: usize) -> String {
    if count == total {
        format!("[{tool_name}] Found {count} result(s):\n")
    } else {
        format!("[{tool_name}] Showing {count} of {total} result(s):\n")
    }
}

/// Standard truncation footer
pub fn truncation_footer(omitted: usize) -> String {
    if omitted == 0 {
        String::new()
    } else {
        format!(
            "\n[{omitted} more result(s) not shown. \
             Narrow your search with a more specific pattern or add file filters.]\n"
        )
    }
}
```

Consistent headers and footers across all tools mean the LLM learns one parsing pattern instead of three. The truncation footer with actionable advice (narrow your search, add filters) prompts the LLM to iterate rather than give up.

## Testing the Integration

Test the tools end-to-end with realistic scenarios:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_project(dir: &std::path::Path) {
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("src/main.rs"),
            r#"use crate::config::Config;

fn main() {
    let config = Config::new("app");
    println!("Starting {}", config.name());
}
"#,
        ).unwrap();

        fs::write(
            dir.join("src/config.rs"),
            r#"pub struct Config {
    name: String,
}

impl Config {
    pub fn new(name: &str) -> Self {
        Config { name: name.to_string() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
"#,
        ).unwrap();

        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    }

    #[tokio::test]
    async fn test_grep_finds_usage() {
        let dir = TempDir::new().unwrap();
        create_test_project(dir.path());

        let tool = GrepTool::new();
        let result = tool
            .execute(json!({
                "pattern": "Config",
                "path": dir.path().to_string_lossy(),
                "include": "*.rs"
            }))
            .await
            .unwrap();

        assert!(result.contains("Config"));
        assert!(result.contains("main.rs"));
        assert!(result.contains("config.rs"));
    }

    #[tokio::test]
    async fn test_glob_finds_rust_files() {
        let dir = TempDir::new().unwrap();
        create_test_project(dir.path());

        let tool = GlobTool::new();
        let result = tool
            .execute(json!({
                "pattern": "**/*.rs",
                "path": dir.path().to_string_lossy()
            }))
            .await
            .unwrap();

        assert!(result.contains("main.rs"));
        assert!(result.contains("config.rs"));
        assert!(!result.contains("Cargo.toml"));
    }
}
```

## Key Takeaways

- Design search tools with one clear purpose each: grep for content search, glob for file discovery, symbol_search for structural lookups. This separation helps the LLM reason about which tool to use.
- Tool descriptions should include cross-references ("for file names, use glob instead") to guide the LLM toward the right tool when the task is ambiguous.
- Consistent result formatting across all search tools (standard headers, truncation footers, file:line format) reduces the LLM's parsing burden and improves reliability.
- Background indexing with `Arc<RwLock<SymbolIndex>>` keeps the agent responsive while the symbol index is being built, letting early queries use text search as a fallback.
- The truncation footer is not just metadata -- it is an actionable hint that prompts the LLM to refine its search rather than accepting incomplete results.
