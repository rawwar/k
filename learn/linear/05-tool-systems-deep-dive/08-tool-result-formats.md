---
title: Tool Result Formats
description: Designing tool result formats that are informative for the LLM, compact for context efficiency, and consistent across tool types.
---

# Tool Result Formats

> **What you'll learn:**
> - How to structure tool results with status, content, and metadata fields for consistent LLM consumption
> - When to return structured data versus plain text and how to truncate large results without losing meaning
> - How different result formats (text, JSON, error) affect the model's ability to reason about next steps

You have seen how tool inputs flow from the model through validation into execution. Now let's look at the other direction: how tool results flow back to the model. The format of your tool results determines how effectively the model can use the information to make decisions and plan next steps.

## The Anatomy of a Tool Result

In Claude's API, a tool result is sent back as a content block in the conversation. The core structure looks like this:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
  "content": "fn main() {\n    println!(\"Hello, world!\");\n}"
}
```

For errors, you add an `is_error` flag:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
  "is_error": true,
  "content": "File not found: '/project/src/mian.rs'. Did you mean '/project/src/main.rs'?"
}
```

The `is_error` flag is important. It tells the model that the tool call failed and the content is an error message, not the requested output. Without this flag, the model might try to interpret an error message as actual file content.

In Rust, you can represent this with a simple enum:

```rust
#[derive(Debug)]
pub enum ToolResult {
    Success(String),
    Error(String),
}

impl ToolResult {
    pub fn to_api_content(&self) -> (String, bool) {
        match self {
            ToolResult::Success(content) => (content.clone(), false),
            ToolResult::Error(message) => (message.clone(), true),
        }
    }
}
```

## Plain Text vs Structured Results

The most fundamental design decision for tool results is whether to return plain text or structured data. Both have their place.

### Plain Text Results

Plain text is the simplest format and often the best. When the model reads a file, it expects source code -- not JSON-wrapped source code:

```rust
pub fn format_read_result(content: &str, path: &str, total_lines: usize) -> String {
    let line_count = content.lines().count();

    if line_count < total_lines {
        // File is fully returned
        content.to_string()
    } else {
        // File was truncated
        format!(
            "{}\n\n[Truncated: showing {} of {} lines. \
             Use offset and limit parameters for remaining content.]",
            content, line_count, total_lines
        )
    }
}
```

Plain text results work best when:
- The content is inherently textual (source code, logs, documentation)
- The model needs to reason about the content directly (understanding code structure)
- There is no meaningful metadata beyond the content itself

### Structured Results

Structured results use a consistent format with metadata. This works well for tools that return complex information:

```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub total_matches: usize,
    pub files_searched: usize,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct SearchMatch {
    pub file: String,
    pub line: usize,
    pub content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

pub fn format_search_result(result: &SearchResult) -> String {
    let mut output = String::new();

    for m in &result.matches {
        output.push_str(&format!("{}:{}: {}\n", m.file, m.line, m.content.trim()));
    }

    output.push_str(&format!(
        "\n[{} matches in {} files",
        result.total_matches, result.files_searched
    ));

    if result.truncated {
        output.push_str(". Results truncated — refine your pattern for fewer matches");
    }

    output.push_str("]\n");

    output
}
```

Notice that even though the underlying data is structured (the `SearchResult` struct), the formatted output is plain text. This is a common pattern: use structured data internally for processing, but convert to a clean text format before sending to the model.

::: python Coming from Python
In Python, you might be tempted to return `json.dumps(result)` and let the model parse it. This often works but is inefficient -- the model must spend tokens parsing JSON structure instead of reasoning about content. Compare:
```python
# JSON format - the model parses structure
'{"file": "src/main.rs", "line": 42, "content": "fn main() {"}'

# Text format - the model reads naturally
'src/main.rs:42: fn main() {'
```
The text format is shorter (fewer tokens) and easier for the model to scan. Use JSON in results only when the structure itself is what the model needs to reason about.
:::

## Truncation Strategies

Tool results can be large. A file might be 10,000 lines. A search might return 500 matches. A shell command might produce megabytes of output. You need truncation strategies that preserve the most useful information.

### Head Truncation

Return the first N lines and indicate how much was cut:

```rust
pub fn truncate_head(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() <= max_lines {
        return content.to_string();
    }

    let truncated: String = lines[..max_lines].join("\n");
    format!(
        "{}\n\n[Output truncated: showing first {} of {} lines]",
        truncated,
        max_lines,
        lines.len()
    )
}
```

Head truncation works for file reads and sequential output where the beginning is most relevant.

### Tail Truncation

Return the last N lines -- useful for build output and test results where errors appear at the end:

```rust
pub fn truncate_tail(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() <= max_lines {
        return content.to_string();
    }

    let start = lines.len() - max_lines;
    let truncated: String = lines[start..].join("\n");
    format!(
        "[Output truncated: showing last {} of {} lines]\n\n{}",
        max_lines,
        lines.len(),
        truncated
    )
}
```

### Smart Truncation

For some results, neither head nor tail is ideal. Compilation output, for example, has the most useful information scattered throughout -- errors near the end, but relevant context (file names, line numbers) near each error. A smarter approach extracts the error lines with context:

```rust
pub fn truncate_smart(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() <= max_lines {
        return content.to_string();
    }

    // Find lines that look like errors or warnings
    let important_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.contains("error") || line.contains("warning")
                || line.contains("Error") || line.contains("FAILED")
        })
        .map(|(i, _)| i)
        .collect();

    let mut selected = std::collections::BTreeSet::new();

    // Include 2 lines of context around each important line
    for &idx in &important_indices {
        let start = idx.saturating_sub(2);
        let end = (idx + 3).min(lines.len());
        for i in start..end {
            selected.insert(i);
        }
        if selected.len() >= max_lines {
            break;
        }
    }

    // If we have room, add the first and last few lines
    if selected.len() < max_lines {
        for i in 0..5.min(lines.len()) {
            selected.insert(i);
        }
        for i in lines.len().saturating_sub(5)..lines.len() {
            selected.insert(i);
        }
    }

    let mut result = String::new();
    let mut last_idx: Option<usize> = None;

    for &idx in &selected {
        if let Some(last) = last_idx {
            if idx > last + 1 {
                result.push_str("\n  [...]\n");
            }
        }
        result.push_str(lines[idx]);
        result.push('\n');
        last_idx = Some(idx);
    }

    format!(
        "{}\n[Smart truncation: showing {} of {} lines, \
         focused on errors and warnings]",
        result.trim_end(),
        selected.len(),
        lines.len()
    )
}
```

::: wild In the Wild
Claude Code uses line-limited truncation on file reads, typically capping at around 2000 lines and telling the model to use offset/limit parameters for more. For shell command output, it uses a combination of head and tail truncation, keeping both the first and last portions of long output. OpenCode applies similar limits, and both agents include a truncation notice in the output so the model knows information was omitted.
:::

## Consistent Result Formatting Across Tools

The model learns to expect a certain style of result from your tools. If `read_file` returns raw text but `search_files` returns JSON and `shell` returns a mix, the model has to constantly switch mental models. Consistency helps.

Here is a formatter that applies a consistent style across all tools:

```rust
pub struct ToolResultFormatter {
    pub max_output_lines: usize,
}

impl ToolResultFormatter {
    pub fn format(&self, tool_name: &str, result: Result<String, String>) -> ToolResult {
        match result {
            Ok(output) => {
                let truncated = self.maybe_truncate(tool_name, &output);
                ToolResult::Success(truncated)
            }
            Err(error) => ToolResult::Error(error),
        }
    }

    fn maybe_truncate(&self, tool_name: &str, output: &str) -> String {
        let line_count = output.lines().count();

        if line_count <= self.max_output_lines {
            return output.to_string();
        }

        // Choose truncation strategy based on tool type
        match tool_name {
            "read_file" => truncate_head(output, self.max_output_lines),
            "shell" => truncate_tail(output, self.max_output_lines),
            "search_files" => truncate_head(output, self.max_output_lines),
            _ => truncate_head(output, self.max_output_lines),
        }
    }
}
```

## Context Window Awareness

Every character in a tool result consumes tokens from the context window. Large results crowd out conversation history, tool definitions, and the model's reasoning space. This makes context-aware formatting important:

```rust
pub fn estimate_tokens(text: &str) -> usize {
    // Rough approximation: ~4 characters per token for English/code
    text.len() / 4
}

pub fn format_with_budget(output: &str, token_budget: usize) -> String {
    let estimated = estimate_tokens(output);

    if estimated <= token_budget {
        return output.to_string();
    }

    // Calculate how many characters we can afford
    let char_budget = token_budget * 4;
    let truncated = &output[..char_budget.min(output.len())];

    // Find the last complete line within budget
    let last_newline = truncated.rfind('\n').unwrap_or(truncated.len());
    let clean_truncated = &truncated[..last_newline];

    format!(
        "{}\n\n[Truncated to ~{} tokens. Full output was ~{} tokens.]",
        clean_truncated, token_budget, estimated
    )
}
```

This is a simplified example -- real token counting is more complex. But the principle is sound: be aware of how much context your tool results consume, and truncate proactively when results are large.

## Key Takeaways

- Use the `is_error` flag on tool results to distinguish errors from successful output -- this helps the model interpret the content correctly
- Prefer plain text over JSON for tool results unless the structure itself is what the model needs to reason about
- Implement truncation strategies appropriate to each tool: head truncation for file reads, tail for build output, smart truncation for compiler errors
- Keep result formatting consistent across tools so the model can develop reliable expectations
- Be context-window aware -- large tool results crowd out conversation history and reduce the model's effective reasoning space
