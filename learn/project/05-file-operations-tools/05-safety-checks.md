---
title: Safety Checks
description: Implement guardrails that prevent the agent from accessing sensitive files, escaping sandboxes, or causing damage.
---

# Safety Checks

> **What you'll learn:**
> - How to implement an allowlist/blocklist system that restricts which paths the agent can read and write
> - How to detect and prevent directory traversal attacks using `..`, symlinks, and other path tricks
> - How to add confirmation prompts for destructive operations like overwriting existing files or writing outside the project

You have a path resolver that prevents the agent from escaping the base directory. But safety goes beyond directory containment. The agent should not overwrite your `.git/config`, it should not write to `node_modules`, and it probably should not be able to read `.env` files full of API keys. This subchapter builds a layered safety system that checks every file operation against a set of configurable rules.

## Why Safety Checks Are Not Optional

When you give a language model the ability to write files, you are giving it the power to do real damage. The model might:

- Overwrite a critical configuration file that breaks the project
- Write to a hidden directory like `.git` and corrupt the repository
- Read a `.env` file and then include its contents in a subsequent API request (leaking secrets)
- Create thousands of files in a loop, filling up disk space

None of these are malicious intent -- they are mistakes a model can make when trying to be helpful. Safety checks are the guardrails that turn catastrophic mistakes into harmless error messages.

## The SafetyChecker Struct

Create `src/tools/safety.rs`:

```rust
use std::path::Path;

pub struct SafetyChecker {
    /// File patterns that the agent cannot read or write
    blocked_patterns: Vec<String>,
    /// File patterns the agent can read but not write
    read_only_patterns: Vec<String>,
    /// Maximum file size the agent can write (in bytes)
    max_write_size: usize,
}

impl SafetyChecker {
    pub fn new() -> Self {
        Self {
            blocked_patterns: vec![
                ".env".to_string(),
                ".env.*".to_string(),
                "*.pem".to_string(),
                "*.key".to_string(),
                "**/credentials*".to_string(),
                "**/.git/objects/**".to_string(),
                "**/.git/refs/**".to_string(),
            ],
            read_only_patterns: vec![
                ".gitignore".to_string(),
                "Cargo.lock".to_string(),
                "**/node_modules/**".to_string(),
            ],
            max_write_size: 1024 * 1024, // 1 MB default
        }
    }

    pub fn with_blocked(mut self, patterns: Vec<String>) -> Self {
        self.blocked_patterns.extend(patterns);
        self
    }

    pub fn with_read_only(mut self, patterns: Vec<String>) -> Self {
        self.read_only_patterns.extend(patterns);
        self
    }

    pub fn with_max_write_size(mut self, bytes: usize) -> Self {
        self.max_write_size = bytes;
        self
    }
}
```

The builder pattern lets you configure the checker at startup. The defaults are reasonable for most projects -- block secret files, protect `.git` internals, and limit write size.

## Checking Read Access

The read check is straightforward: match the path against blocked patterns.

```rust
impl SafetyChecker {
    /// Check if a path is safe to read.
    pub fn check_read(&self, path: &Path) -> Result<(), String> {
        let path_str = path.display().to_string();

        for pattern in &self.blocked_patterns {
            if matches_pattern(&path_str, pattern) {
                return Err(format!(
                    "Access denied: '{}' matches blocked pattern '{}'. \
                     This file cannot be read for security reasons.",
                    path.display(),
                    pattern
                ));
            }
        }

        Ok(())
    }
}
```

## Checking Write Access

The write check is stricter: it checks both blocked and read-only patterns, plus the file size limit.

```rust
impl SafetyChecker {
    /// Check if a path is safe to write with the given content.
    pub fn check_write(&self, path: &Path, content: &str) -> Result<(), String> {
        let path_str = path.display().to_string();

        // Check blocked patterns
        for pattern in &self.blocked_patterns {
            if matches_pattern(&path_str, pattern) {
                return Err(format!(
                    "Access denied: '{}' matches blocked pattern '{}'. \
                     This file cannot be written for security reasons.",
                    path.display(),
                    pattern
                ));
            }
        }

        // Check read-only patterns
        for pattern in &self.read_only_patterns {
            if matches_pattern(&path_str, pattern) {
                return Err(format!(
                    "Access denied: '{}' matches read-only pattern '{}'. \
                     This file can be read but not modified.",
                    path.display(),
                    pattern
                ));
            }
        }

        // Check file size limit
        if content.len() > self.max_write_size {
            return Err(format!(
                "Content too large: {} bytes exceeds the maximum write size of {} bytes.",
                content.len(),
                self.max_write_size
            ));
        }

        Ok(())
    }
}
```

## Pattern Matching

The pattern matching function supports basic glob-style syntax. For a production agent you would use the `glob` crate, but for our safety checker, a simple implementation covers the common cases:

```rust
/// Match a path string against a simple glob pattern.
/// Supports `*` (any sequence within a component) and `**` (any path components).
fn matches_pattern(path: &str, pattern: &str) -> bool {
    // Normalize separators
    let path = path.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");

    if pattern.contains("**/") {
        // "**/" means "any number of directories"
        let suffix = pattern.trim_start_matches("**/");
        return path.ends_with(suffix)
            || path.contains(&format!("/{}", suffix));
    }

    if pattern.starts_with("*.") {
        // "*.ext" matches any file with that extension
        let ext = &pattern[1..]; // includes the dot
        return path.ends_with(ext);
    }

    if pattern.ends_with("/**") {
        // "dir/**" matches anything under that directory
        let prefix = &pattern[..pattern.len() - 3];
        return path.contains(prefix);
    }

    // Exact filename match (anywhere in path)
    let filename = path.rsplit('/').next().unwrap_or(&path);
    filename == pattern || path.ends_with(&format!("/{}", pattern))
}
```

This is intentionally simple. It handles the patterns we defined in our defaults: `*.pem`, `**/.git/objects/**`, `.env`, and `.env.*`. For a more robust solution, you would use the `glob` crate's `Pattern::matches` function.

::: tip Coming from Python
In Python, you might use `fnmatch` for glob-style matching:
```python
import fnmatch

def matches_pattern(path: str, pattern: str) -> bool:
    return fnmatch.fnmatch(path, pattern) or fnmatch.fnmatch(
        path.split("/")[-1], pattern
    )
```
Rust does not have a built-in glob matcher in `std`, which is why we wrote a simple one. The `glob` crate provides `Pattern::matches_path` for production use, which we will use when we implement the GlobSearch tool later in this chapter.
:::

## Integrating Safety into File Tools

Now wire the safety checker into each file tool. The cleanest approach is to pass a shared reference to the checker:

```rust
use std::sync::Arc;
use crate::tools::safety::SafetyChecker;

pub struct ReadFileTool {
    pub base_dir: PathBuf,
    pub safety: Arc<SafetyChecker>,
}

impl ReadFileTool {
    pub fn new(base_dir: PathBuf, safety: Arc<SafetyChecker>) -> Self {
        Self { base_dir, safety }
    }
}
```

Then add the check at the beginning of each `execute` method:

```rust
fn execute(&self, input: &Value) -> Result<String, String> {
    let path_str = input.get("path").and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: path".to_string())?;

    let path = resolve_path(&self.base_dir, path_str)?;

    // Safety check before any file operation
    self.safety.check_read(&path)?;

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file '{}': {}", path.display(), e))?;

    // ... rest of the method
}
```

For the write and edit tools, use `check_write`:

```rust
// In WriteFileTool::execute, after path resolution:
self.safety.check_write(&path, content)?;

// In EditFileTool::execute, after computing new_content:
self.safety.check_write(&path, &new_content)?;
```

## Handling Sensitive File Detection

Beyond pattern matching, you can add content-based detection for sensitive files. If the agent reads a file that looks like it contains secrets, you can either block the read or redact the sensitive values:

```rust
impl SafetyChecker {
    /// Check if content appears to contain secrets.
    /// Returns a warning message if suspicious patterns are found.
    pub fn check_content_safety(&self, content: &str) -> Option<String> {
        let suspicious_patterns = [
            ("API_KEY", "API key"),
            ("SECRET_KEY", "secret key"),
            ("PRIVATE_KEY", "private key"),
            ("password", "password"),
            ("Bearer ", "bearer token"),
            ("-----BEGIN", "PEM certificate/key"),
        ];

        let mut warnings = Vec::new();
        for (pattern, description) in &suspicious_patterns {
            if content.contains(pattern) {
                warnings.push(*description);
            }
        }

        if warnings.is_empty() {
            None
        } else {
            Some(format!(
                "Warning: file appears to contain sensitive data ({}). \
                 Be careful not to include this content in responses.",
                warnings.join(", ")
            ))
        }
    }
}
```

You can prepend this warning to the read tool's output so the model sees it immediately:

```rust
// In ReadFileTool::execute, after reading content:
let mut result = String::new();
if let Some(warning) = self.safety.check_content_safety(&content) {
    result.push_str(&format!("[{}]\n", warning));
}
result.push_str(&numbered_content);
```

::: tip In the Wild
Claude Code implements a sophisticated permission system where dangerous operations require user approval. Reading files inside the project is generally allowed, but writing to certain paths (like `.env` or files outside the project) triggers a confirmation prompt. The user sees exactly what the agent wants to write and can approve or reject. OpenCode takes a similar approach, blocking writes to sensitive paths entirely rather than prompting. Our implementation is closer to OpenCode's approach -- we block rather than prompt -- but you could extend it with user confirmation by adding an `is_interactive` flag.
:::

## Testing Safety Checks

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn checker() -> SafetyChecker {
        SafetyChecker::new()
    }

    #[test]
    fn test_block_env_file() {
        let c = checker();
        let path = PathBuf::from("/project/.env");
        assert!(c.check_read(&path).is_err());
        assert!(c.check_write(&path, "SECRET=123").is_err());
    }

    #[test]
    fn test_block_pem_file() {
        let c = checker();
        let path = PathBuf::from("/project/certs/server.pem");
        assert!(c.check_read(&path).is_err());
    }

    #[test]
    fn test_allow_normal_source_file() {
        let c = checker();
        let path = PathBuf::from("/project/src/main.rs");
        assert!(c.check_read(&path).is_ok());
        assert!(c.check_write(&path, "fn main() {}").is_ok());
    }

    #[test]
    fn test_read_only_file() {
        let c = checker();
        let path = PathBuf::from("/project/Cargo.lock");
        assert!(c.check_read(&path).is_ok()); // can read
        assert!(c.check_write(&path, "modified").is_err()); // cannot write
    }

    #[test]
    fn test_write_size_limit() {
        let c = SafetyChecker::new().with_max_write_size(100);
        let path = PathBuf::from("/project/big.txt");
        let content = "x".repeat(200);
        assert!(c.check_write(&path, &content).is_err());
    }

    #[test]
    fn test_content_safety_detection() {
        let c = checker();
        let content = "DATABASE_URL=postgres://user:password@localhost/db";
        assert!(c.check_content_safety(content).is_some());
    }

    #[test]
    fn test_normal_content_passes() {
        let c = checker();
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        assert!(c.check_content_safety(content).is_none());
    }
}
```

These tests verify each layer of the safety system independently: blocked files, read-only files, size limits, and content detection. Each test is focused on a single behavior, making failures easy to diagnose.

## Key Takeaways

- Safety checks are a separate layer from path resolution: the path resolver ensures the path is within bounds, the safety checker ensures the operation is allowed for that specific path.
- A blocked pattern list prevents reading or writing sensitive files (`.env`, `*.pem`, credentials), while a read-only pattern list allows reading but prevents modification of generated files like `Cargo.lock`.
- Write size limits prevent the model from accidentally creating enormous files that fill up disk space.
- Content-based detection can warn about files that appear to contain secrets, adding a second layer of protection beyond filename matching.
- Error messages from safety checks should explain both what was blocked and why, so the model (or user) understands the restriction and can work around it legitimately.
