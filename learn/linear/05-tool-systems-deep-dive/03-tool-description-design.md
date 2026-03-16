---
title: Tool Description Design
description: Crafting tool descriptions that guide language models to select the right tool and provide correct parameters every time.
---

# Tool Description Design

> **What you'll learn:**
> - How the tool description directly influences model behavior and why vague descriptions cause misuse
> - Techniques for writing descriptions that include usage examples, edge cases, and when-not-to-use guidance
> - How to test and iterate on tool descriptions by analyzing model invocation patterns

You have seen how JSON Schema defines the *structure* of tool inputs. But structure alone does not tell the model *when* to use a tool, *why* it should choose one tool over another, or *how* to fill in parameters correctly. That is the job of the tool description -- the natural-language text that accompanies every tool definition.

Tool descriptions are the single most underrated aspect of agent design. A mediocre description leads to subtly wrong tool usage that is hard to debug. A good description leads to an agent that uses tools as if it were an experienced developer.

## Why Descriptions Matter So Much

When a model decides which tool to call, it reads the tool name and description, examines the parameter schemas, and makes a choice. The description is the primary signal the model uses for tool selection. Consider two descriptions for the same tool:

**Vague description:**
```json
{
  "name": "search",
  "description": "Search for something."
}
```

**Precise description:**
```json
{
  "name": "search_files",
  "description": "Search for a regex pattern across all files in the project directory. Returns matching lines with file paths and line numbers. Use this for finding function definitions, variable usages, import statements, or any text pattern. Do NOT use this for searching file names — use list_files with a glob pattern instead."
}
```

The vague description tells the model almost nothing. The precise description tells it what the tool does, what it returns, when to use it, and when *not* to use it. That last part -- the "do NOT use" guidance -- prevents a common misuse pattern where the model tries to use a content search tool to find files by name.

## Anatomy of a Good Tool Description

A well-crafted tool description has up to five components. Not every tool needs all five, but the first three are always present.

### 1. What It Does

Start with a concise statement of the tool's function. This should be one sentence that answers "what happens when I call this tool?"

```
Read the contents of a file at the given path and return the text.
```

### 2. What It Returns

Tell the model what to expect in the result. This shapes how the model plans to use the output.

```
Returns the file content as a string. If the file is longer than 2000 lines,
returns only the first 2000 lines with a truncation notice.
```

### 3. When to Use It

Describe the use cases where this tool is the right choice. Be specific:

```
Use this tool when you need to examine the contents of a known file.
Good for reading source code, configuration files, and documentation.
```

### 4. When NOT to Use It

This is the component most people skip, and it is arguably the most valuable. Tell the model what this tool is *not* for:

```
Do NOT use this to check if a file exists — use list_files instead.
Do NOT use this for binary files (images, compiled artifacts) — the content will be garbled.
```

### 5. Edge Cases and Constraints

Document any limits, special behaviors, or common pitfalls:

```
The path must be absolute. Relative paths will be rejected.
Maximum file size is 10MB. Larger files return an error.
Symbolic links are followed.
```

## A Complete Example

Let's put it all together for a `write_file` tool:

```json
{
  "name": "write_file",
  "description": "Create or overwrite a file at the given path with the provided content. Creates parent directories if they do not exist. Returns the number of bytes written on success. Use this for creating new files or completely replacing the content of existing files. For making targeted edits to existing files (changing specific lines or sections), use the edit_file tool instead — it is safer because it verifies the old content before replacing. WARNING: This tool overwrites the entire file. Any content not included in the 'content' parameter will be lost.",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Absolute path to the file to write. Parent directories are created automatically."
      },
      "content": {
        "type": "string",
        "description": "The complete file content to write. Must include all content for the file — this tool does not append, it replaces."
      }
    },
    "required": ["path", "content"]
  }
}
```

Notice how the description guides the model at every decision point:
- "Create or overwrite" tells it the tool's effect
- "Returns the number of bytes written" tells it what to expect back
- "For making targeted edits... use edit_file instead" prevents it from using write_file when edit_file is better
- "WARNING: This tool overwrites the entire file" reinforces the danger of data loss

::: python Coming from Python
If you have written docstrings for Python functions, you already understand the value of clear documentation. But there is a crucial difference: Python docstrings are read by human developers who can infer intent, read source code, and experiment interactively. Tool descriptions are read by a language model that has no other way to understand the tool's behavior. You need to be far more explicit than you would be in a Python docstring -- state every assumption, every constraint, and every edge case.
:::

## Parameter Descriptions Are Tool Descriptions Too

The tool-level description is not the only place you communicate with the model. Every parameter's `description` field is a mini-instruction. Compare:

**Weak parameter description:**
```json
{
  "pattern": {
    "type": "string",
    "description": "The search pattern."
  }
}
```

**Strong parameter description:**
```json
{
  "pattern": {
    "type": "string",
    "description": "A regex pattern to search for. Uses Rust regex syntax (similar to Python's re module). Special characters like . * + ? must be escaped with \\ for literal matching. Example: 'fn\\s+main' matches function definitions named 'main'."
  }
}
```

The strong description tells the model what kind of pattern to use, what syntax it follows, how to escape special characters, and gives a concrete example. This dramatically reduces the chance of the model constructing a broken regex.

## The "Tool Disambiguation" Problem

When your agent has many tools, the model must choose between them. Disambiguation becomes critical when tools have overlapping capabilities. For example:

- `read_file` -- reads a whole file
- `search_files` -- searches content across files
- `list_files` -- lists files matching a glob pattern

A model might confuse these in several ways:
- Using `search_files` when it just wants to list files (no content search needed)
- Using `read_file` on multiple files one at a time when `search_files` would find the relevant lines directly
- Using `list_files` when it actually needs `search_files` with a pattern

The fix is to add explicit disambiguation to each tool's description:

```
read_file: "...Use this when you know the exact file path and want its full content.
            If you need to find which files contain a pattern, use search_files instead.
            If you need to find files by name, use list_files instead."

search_files: "...Use this to find lines matching a pattern across many files.
               If you already know which file to read, use read_file instead.
               This searches file content, not file names — use list_files for that."

list_files: "...Use this to find files by name or extension.
             This does not search file content — use search_files for that."
```

Each description explicitly points to the right alternative for common misuse scenarios.

::: wild In the Wild
Claude Code's tool descriptions are extensive, often running to several paragraphs. The `Edit` tool description, for example, includes specific guidance about when to use edit versus write, how to handle indentation, and what makes a good `old_string` for matching. OpenCode's tool descriptions are shorter but include explicit notes about what each tool does NOT do. Both approaches reflect the same insight: the description is your primary lever for controlling how the model uses your tools.
:::

## Testing and Iterating on Descriptions

Tool descriptions are not something you write once and forget. They need testing and iteration, just like any other interface. Here is a practical process:

**Step 1: Write the initial description** following the five-component structure above.

**Step 2: Run the agent on realistic tasks** and log every tool call. Pay attention to:
- Does the model choose the right tool for each task?
- Does it fill in parameters correctly?
- Does it misuse the tool in predictable ways?

**Step 3: Identify patterns in misuse.** If the model keeps using `write_file` when it should use `edit_file`, your disambiguation is not strong enough. If it keeps passing relative paths when you need absolute paths, your parameter description is not explicit enough.

**Step 4: Update the description** to address the specific misuse patterns you observed.

**Step 5: Repeat.** You will go through this cycle several times for each tool.

The Rust code for storing descriptions should keep them in a central, easy-to-edit location:

```rust
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    pub fn read_file() -> Self {
        Self {
            name: "read_file",
            description: "Read the contents of a file at the given absolute path \
                and return the text content. Returns up to 2000 lines; \
                use the offset and limit parameters for longer files. \
                Use this when you know which file to read. \
                For finding files by name, use list_files. \
                For finding content across files, use search_files.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to read."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (1-based). Defaults to 1.",
                        "minimum": 1
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to return. Defaults to 2000.",
                        "minimum": 1,
                        "maximum": 10000
                    }
                },
                "required": ["path"]
            }),
        }
    }
}
```

Keeping descriptions as string constants in a dedicated struct makes it easy to find and update them when you discover a misuse pattern.

## Key Takeaways

- Tool descriptions are the primary signal models use for tool selection -- vague descriptions directly cause incorrect tool usage
- A complete description covers five areas: what the tool does, what it returns, when to use it, when NOT to use it, and edge cases or constraints
- Parameter-level descriptions matter just as much as the tool-level description -- include syntax hints, examples, and constraints for each parameter
- Explicit disambiguation between similar tools ("use X instead of Y for this case") prevents the most common misuse patterns
- Treat descriptions as a living interface: log tool calls, identify misuse patterns, update descriptions, and repeat
