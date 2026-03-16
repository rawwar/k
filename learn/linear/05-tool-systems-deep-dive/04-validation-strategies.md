---
title: Validation Strategies
description: Validating tool inputs before execution to prevent crashes, security issues, and wasted compute from malformed parameters.
---

# Validation Strategies

> **What you'll learn:**
> - Why validation is critical when inputs come from a probabilistic model that can produce invalid data
> - How to implement layered validation: schema validation, type checking, semantic validation, and permission checks
> - Strategies for returning helpful validation errors that help the model self-correct on retry

When a human developer calls an API, they can read the documentation, notice their mistake, and fix the input before hitting "send." A language model does not have that luxury. It generates tool parameters in a single pass, and those parameters might be subtly wrong -- a path that is relative instead of absolute, a line number that is zero instead of one-based, or a regex pattern with unescaped special characters. Validation is your defense against these errors.

Validation is not just about preventing crashes. It is about catching mistakes early, returning helpful error messages, and giving the model a chance to self-correct on the next loop iteration. A well-validated tool that returns a clear error message is far more useful than an unvalidated tool that produces confusing garbage output or silently does the wrong thing.

## The Four Layers of Validation

Think of validation as four concentric layers, from broadest to most specific. Each layer catches a different class of error.

### Layer 1: Schema Validation

Schema validation checks whether the input conforms to the JSON Schema you defined for the tool. This is the coarsest layer -- it catches missing required fields, wrong types, values outside enum ranges, and violated constraints like `minimum` or `maxLength`.

Most LLM APIs perform some schema validation on the model's output before returning the tool call to you. However, you should not rely on this exclusively. Always validate the schema yourself:

```rust
use jsonschema::JSONSchema;
use serde_json::Value;

pub fn validate_schema(input: &Value, schema: &Value) -> Result<(), Vec<String>> {
    let compiled = JSONSchema::compile(schema)
        .map_err(|e| vec![format!("Invalid schema: {}", e)])?;

    let result = compiled.validate(input);
    match result {
        Ok(()) => Ok(()),
        Err(errors) => {
            let messages: Vec<String> = errors
                .map(|e| format!("{} at {}", e, e.instance_path))
                .collect();
            Err(messages)
        }
    }
}
```

Schema validation catches errors like:
- Missing the `path` parameter when it is required
- Passing a number where a string was expected
- Passing `"verbose"` when the enum only allows `["rust", "python", "all"]`

### Layer 2: Type Deserialization

After schema validation, you deserialize the JSON into your Rust input struct. This is where serde does its work, converting the raw JSON into typed Rust values:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ReadFileInput {
    pub path: String,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

pub fn parse_input(input: &serde_json::Value) -> Result<ReadFileInput, String> {
    serde_json::from_value(input.clone())
        .map_err(|e| format!("Failed to parse input: {}", e))
}
```

This layer catches type mismatches that the schema layer might miss -- for example, a string that should be a number but was quoted as `"42"` in some edge cases. It also applies serde's deserialization rules, which can handle things like default values via `#[serde(default)]`.

::: python Coming from Python
In Python, you might use Pydantic to validate and parse inputs in a single step:
```python
from pydantic import BaseModel, validator

class ReadFileInput(BaseModel):
    path: str
    offset: int | None = None
    limit: int | None = None

    @validator("path")
    def path_must_be_absolute(cls, v):
        if not v.startswith("/"):
            raise ValueError("Path must be absolute")
        return v
```
Rust separates these concerns: serde handles parsing, and you write validation logic separately. The result is more explicit but equally effective.
:::

### Layer 3: Semantic Validation

Semantic validation checks whether the values *make sense* for the operation, beyond just their types. This is the layer where you catch the most interesting errors:

```rust
use std::path::Path;

pub fn validate_read_file(input: &ReadFileInput) -> Result<(), String> {
    // Check that the path is absolute
    if !input.path.starts_with('/') {
        return Err(format!(
            "Path must be absolute (start with /), got: '{}'. \
             Hint: use the project root from your context.",
            input.path
        ));
    }

    // Check that the path does not traverse outside the project
    let canonical = Path::new(&input.path);
    if input.path.contains("..") {
        return Err(
            "Path must not contain '..' components. \
             Use absolute paths instead of relative traversal.".to_string()
        );
    }

    // Check that the file exists
    if !canonical.exists() {
        return Err(format!(
            "File not found: '{}'. \
             Use the list_files tool to verify the path exists.",
            input.path
        ));
    }

    // Check that it is a file, not a directory
    if canonical.is_dir() {
        return Err(format!(
            "'{}' is a directory, not a file. \
             Use list_files to browse directories.",
            input.path
        ));
    }

    // Check offset and limit if provided
    if let Some(offset) = input.offset {
        if offset == 0 {
            return Err(
                "Offset is 1-based. Use offset=1 for the first line.".to_string()
            );
        }
    }

    Ok(())
}
```

Notice the pattern in the error messages. Each one:
1. States what is wrong
2. Shows the invalid value the model passed
3. Suggests what to do instead

This structure is deliberate. The model receives these error messages as tool results and uses them to construct a corrected tool call. The more helpful the error, the more likely the model self-corrects on the first retry.

### Layer 4: Permission Checks

The final validation layer checks whether the operation is *allowed*, not just whether it is *valid*. This layer applies primarily to mutating tools:

```rust
pub enum Permission {
    Allowed,
    RequiresConfirmation(String),
    Denied(String),
}

pub fn check_write_permission(path: &str, project_root: &str) -> Permission {
    // Deny writes outside the project directory
    if !path.starts_with(project_root) {
        return Permission::Denied(format!(
            "Cannot write to '{}': outside project directory '{}'.",
            path, project_root
        ));
    }

    // Flag certain files for user confirmation
    let sensitive_patterns = ["Cargo.toml", "package.json", ".env", ".gitignore"];
    for pattern in &sensitive_patterns {
        if path.ends_with(pattern) {
            return Permission::RequiresConfirmation(format!(
                "Writing to '{}' — this is a project configuration file. Confirm?",
                path
            ));
        }
    }

    Permission::Allowed
}
```

Permission checks are distinct from semantic validation because they involve policy decisions rather than correctness checks. We will cover the security aspects in detail in the Security Considerations subchapter.

## Composing the Layers

In practice, you run all four layers in sequence, stopping at the first failure:

```rust
pub enum ValidationResult {
    Valid(ReadFileInput),
    Invalid(String),
    NeedsPermission(ReadFileInput, String),
}

pub fn validate_tool_call(
    raw_input: &serde_json::Value,
    schema: &serde_json::Value,
    project_root: &str,
) -> ValidationResult {
    // Layer 1: Schema validation
    if let Err(errors) = validate_schema(raw_input, schema) {
        return ValidationResult::Invalid(
            format!("Schema validation failed: {}", errors.join("; "))
        );
    }

    // Layer 2: Deserialization
    let input = match parse_input(raw_input) {
        Ok(input) => input,
        Err(e) => return ValidationResult::Invalid(e),
    };

    // Layer 3: Semantic validation
    if let Err(e) = validate_read_file(&input) {
        return ValidationResult::Invalid(e);
    }

    // Layer 4: Permission check (for mutating tools)
    // read_file is read-only, so no permission check needed here

    ValidationResult::Valid(input)
}
```

::: wild In the Wild
Claude Code performs validation at multiple levels. Path-based tools validate that paths are absolute, exist, and fall within the project directory. Shell commands are checked against a deny list of dangerous patterns before execution. Importantly, validation errors are returned as tool results with `is_error: true`, which signals to the model that the tool call failed and it should adjust its approach. OpenCode takes a similar layered approach, with schema validation handled by the framework and semantic validation handled by each individual tool implementation.
:::

## Writing Error Messages That Help Models Self-Correct

The quality of your validation error messages directly affects how well the agent recovers from mistakes. Here are principles for writing errors that models can act on:

**Be specific about what went wrong:**
```
Bad:  "Invalid input"
Good: "Path must be absolute (start with /), got: 'src/main.rs'"
```

**Suggest the fix:**
```
Bad:  "File not found: 'src/main.rs'"
Good: "File not found: 'src/main.rs'. Use list_files to verify the path, or try the absolute path '/home/user/project/src/main.rs'."
```

**Redirect to the right tool:**
```
Bad:  "Cannot search directories"
Good: "This tool reads individual files. To search across files, use search_files with a pattern."
```

**Include the invalid value:**
```
Bad:  "Line number out of range"
Good: "Line number 0 is out of range. Line numbers are 1-based — use 1 for the first line."
```

These patterns work because the model treats the error message as context for its next turn. A specific, actionable error message gives the model exactly what it needs to construct a corrected tool call.

## Fail-Fast vs Collect-All-Errors

There are two philosophies for validation error reporting:

**Fail-fast** returns the first error found. This is simpler to implement and gives the model a single clear issue to fix. It works well when errors tend to be independent.

**Collect-all-errors** returns every error at once. This is more complex but can save loop iterations when the model has made multiple mistakes. Instead of fixing one error per loop iteration, the model can fix all of them at once.

For most coding agent tools, fail-fast is the better choice. The model rarely makes more than one validation error per call, and the simpler code is easier to maintain. However, for complex tools with many parameters, collect-all-errors can be worthwhile.

## Key Takeaways

- Validation is essential because LLM-generated inputs are probabilistic -- the model can and will produce subtly invalid parameters
- Implement four layers: schema validation, type deserialization, semantic validation, and permission checks, running them in sequence
- Error messages should be specific, include the invalid value, suggest the fix, and redirect to the right tool when appropriate
- The quality of your validation errors directly determines how well the model self-corrects -- treat error messages as UX for the LLM
- Prefer fail-fast validation for most tools, but consider collect-all-errors for complex multi-parameter tools
