// Chapter 5: File Operations Tools — Code snapshot
//
// Implements ReadFile, WriteFile, and ListDirectory tools that satisfy the
// Tool trait from Chapter 4. Each tool resolves paths against an allowed
// base directory and handles I/O errors with descriptive messages.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Tool trait (carried forward from Chapter 4)
// ---------------------------------------------------------------------------

/// The core trait that all tools must implement.
trait Tool {
    /// The unique name of this tool (used in API tool definitions).
    fn name(&self) -> &str;

    /// A human-readable description for the LLM.
    fn description(&self) -> &str;

    /// The JSON Schema for this tool's input parameters.
    fn input_schema(&self) -> Value;

    /// Execute the tool with the given input and return the result.
    fn execute(&self, input: &Value) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// Path validation helpers
// ---------------------------------------------------------------------------

/// Resolve a requested path against a base directory and verify it stays
/// within the allowed boundary. Handles both relative and absolute inputs,
/// canonicalizing to eliminate `..` and symlink escapes.
fn resolve_and_validate_path(base_dir: &Path, requested: &str) -> Result<PathBuf, String> {
    let requested_path = Path::new(requested);

    // If absolute, use directly; otherwise join with base.
    let joined = if requested_path.is_absolute() {
        PathBuf::from(requested)
    } else {
        base_dir.join(requested)
    };

    // Canonicalize the base so we have a stable anchor for comparison.
    let canonical_base = base_dir
        .canonicalize()
        .map_err(|e| format!("Cannot resolve base directory '{}': {}", base_dir.display(), e))?;

    // Canonicalize the target. For files that don't yet exist (write targets)
    // we canonicalize the nearest existing ancestor and re-append the rest.
    let canonical = if joined.exists() {
        joined
            .canonicalize()
            .map_err(|e| format!("Cannot resolve path '{}': {}", joined.display(), e))?
    } else {
        // Walk up until we find an ancestor that exists.
        let mut ancestor = joined.clone();
        let mut trailing: Vec<std::ffi::OsString> = Vec::new();
        loop {
            if ancestor.exists() {
                break;
            }
            if let Some(name) = ancestor.file_name() {
                trailing.push(name.to_os_string());
            }
            if !ancestor.pop() {
                return Err(format!("No existing ancestor for path '{}'", requested));
            }
        }
        let mut canonical_ancestor = ancestor.canonicalize().map_err(|e| {
            format!("Cannot resolve ancestor '{}': {}", ancestor.display(), e)
        })?;
        for component in trailing.into_iter().rev() {
            canonical_ancestor.push(component);
        }
        canonical_ancestor
    };

    // Boundary check — the resolved path must live under the base directory.
    if !canonical.starts_with(&canonical_base) {
        return Err(format!(
            "Path '{}' resolves to '{}' which is outside the allowed directory '{}'",
            requested,
            canonical.display(),
            canonical_base.display(),
        ));
    }

    Ok(canonical)
}

// ---------------------------------------------------------------------------
// ReadFile tool
// ---------------------------------------------------------------------------

/// Reads a file from disk and returns its contents with line numbers.
/// Supports optional `offset` (1-based) and `limit` for reading ranges.
struct ReadFileTool {
    base_dir: PathBuf,
}

impl ReadFileTool {
    fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path. Returns the file contents \
         with line numbers. Optionally specify offset and limit to read a specific \
         range of lines."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read, relative to the project root"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-based). Defaults to 1."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read. Defaults to reading the entire file."
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        // Extract and validate the path.
        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: path".to_string())?;

        let path = resolve_and_validate_path(&self.base_dir, path_str)?;

        // Read the file.
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read file '{}': {}", path.display(), e))?;

        // Parse optional offset (1-based) and limit.
        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(1);

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // Add line numbers and apply range.
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start = (offset.saturating_sub(1)).min(total_lines);
        let end = match limit {
            Some(lim) => (start + lim).min(total_lines),
            None => total_lines,
        };

        let numbered_lines: Vec<String> = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let line_num = start + i + 1;
                format!("{:>4}\t{}", line_num, line)
            })
            .collect();

        let mut result = numbered_lines.join("\n");

        // Prepend a header when reading a sub-range.
        if offset > 1 || limit.is_some() {
            let header = format!(
                "[Showing lines {}-{} of {} total]\n",
                start + 1,
                end,
                total_lines,
            );
            result = header + &result;
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// WriteFile tool
// ---------------------------------------------------------------------------

/// Writes complete content to a file, creating parent directories as needed.
struct WriteFileTool {
    base_dir: PathBuf,
}

impl WriteFileTool {
    fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file at the given path. Creates the file if it does not \
         exist. Creates parent directories as needed. Overwrites the file if it \
         already exists."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to write to, relative to the project root"
                },
                "content": {
                    "type": "string",
                    "description": "The complete content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: path".to_string())?;

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: content".to_string())?;

        let path = resolve_and_validate_path(&self.base_dir, path_str)?;

        // Create parent directories if they don't exist.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!("Failed to create directory '{}': {}", parent.display(), e)
            })?;
        }

        let existed = path.exists();

        // Normalize line endings to Unix-style and ensure trailing newline.
        let normalized = content.replace("\r\n", "\n");
        let final_content = if normalized.is_empty() || normalized.ends_with('\n') {
            normalized
        } else {
            format!("{}\n", normalized)
        };

        fs::write(&path, &final_content)
            .map_err(|e| format!("Failed to write file '{}': {}", path.display(), e))?;

        let line_count = final_content.lines().count();
        let byte_count = final_content.len();
        let status = if existed { "Updated" } else { "Created" };

        Ok(format!(
            "{} {} ({} lines, {} bytes)",
            status,
            path.display(),
            line_count,
            byte_count,
        ))
    }
}

// ---------------------------------------------------------------------------
// ListDirectory tool
// ---------------------------------------------------------------------------

/// Lists files in a directory, optionally matching a glob pattern.
struct ListDirectoryTool {
    base_dir: PathBuf,
}

impl ListDirectoryTool {
    fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}

/// Directories to skip during listing.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".cache",
];

/// Returns true if any component of the path is an ignored directory.
fn is_in_ignored_dir(path: &Path) -> bool {
    path.components().any(|component| {
        if let std::path::Component::Normal(name) = component {
            IGNORED_DIRS.iter().any(|ignored| name.to_string_lossy() == *ignored)
        } else {
            false
        }
    })
}

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List files in a directory. Provide a glob pattern like '**/*.rs' to find \
         all Rust files or 'src/*.toml' for TOML files under src/. Results are sorted \
         by modification time (most recent first). Defaults to listing all files in \
         the project root if no pattern is given."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files. Supports *, **, ?, and [abc]. Defaults to '*' (top-level files)."
                }
            },
            "required": []
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        let pattern_str = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("*");

        let canonical_base = self
            .base_dir
            .canonicalize()
            .map_err(|e| format!("Cannot resolve base directory '{}': {}", self.base_dir.display(), e))?;

        let full_pattern = canonical_base.join(pattern_str);
        let pattern_string = full_pattern.to_string_lossy().to_string();

        let entries = glob::glob(&pattern_string)
            .map_err(|e| format!("Invalid glob pattern '{}': {}", pattern_str, e))?;

        let max_results: usize = 100;
        let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for entry in entries {
            match entry {
                Ok(path) => {
                    if path.is_dir() {
                        continue;
                    }
                    if is_in_ignored_dir(&path) {
                        continue;
                    }
                    // Safety: ensure the matched path is within the base dir.
                    if let Ok(canonical) = path.canonicalize() {
                        if !canonical.starts_with(&canonical_base) {
                            continue;
                        }
                    }
                    let modified = path
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);

                    matches.push((path, modified));

                    if matches.len() > max_results * 2 {
                        break;
                    }
                }
                Err(_) => continue,
            }
        }

        // Sort by modification time, most recent first.
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        matches.truncate(max_results);

        if matches.is_empty() {
            return Ok(format!("No files found matching pattern '{}'", pattern_str));
        }

        let result_lines: Vec<String> = matches
            .iter()
            .map(|(path, _)| {
                path.strip_prefix(&canonical_base)
                    .unwrap_or(path)
                    .display()
                    .to_string()
            })
            .collect();

        let mut output = format!(
            "Found {} file(s) matching '{}':\n",
            result_lines.len(),
            pattern_str,
        );
        output.push_str(&result_lines.join("\n"));

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Tool registry (lightweight version from Chapter 4)
// ---------------------------------------------------------------------------

struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    fn new() -> Self {
        Self { tools: Vec::new() }
    }

    fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Look up a tool by name and execute it with the given input.
    fn dispatch(&self, name: &str, input: &Value) -> Result<String, String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| format!("Unknown tool: {}", name))?;
        tool.execute(input)
    }

    /// Return tool definitions in the format expected by an LLM API.
    fn tool_definitions(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// main — demonstrate the tools
// ---------------------------------------------------------------------------

fn main() {
    println!("Chapter 5: File Operations Tools\n");

    // Use a temporary directory as the sandbox so the demo is self-contained.
    let base_dir = std::env::temp_dir().join("ch05_demo");
    fs::create_dir_all(&base_dir).expect("failed to create demo directory");

    // --- Register tools ------------------------------------------------
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ReadFileTool::new(base_dir.clone())));
    registry.register(Box::new(WriteFileTool::new(base_dir.clone())));
    registry.register(Box::new(ListDirectoryTool::new(base_dir.clone())));

    // Print the tool definitions the way an API call would see them.
    println!("Registered tools:");
    for def in registry.tool_definitions() {
        println!("  - {}", def["name"]);
    }
    println!();

    // --- WriteFile: create a new file ----------------------------------
    let write_result = registry.dispatch(
        "write_file",
        &json!({
            "path": "src/greeting.rs",
            "content": "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n"
        }),
    );
    println!("write_file => {}", format_result(&write_result));

    // --- ReadFile: read it back ----------------------------------------
    let read_result = registry.dispatch("read_file", &json!({ "path": "src/greeting.rs" }));
    println!("read_file  =>\n{}", format_result(&read_result));

    // --- ReadFile: read with offset and limit --------------------------
    let range_result = registry.dispatch(
        "read_file",
        &json!({ "path": "src/greeting.rs", "offset": 2, "limit": 1 }),
    );
    println!("read_file (range) =>\n{}", format_result(&range_result));

    // --- WriteFile: create a second file so listing is interesting ------
    let _ = registry.dispatch(
        "write_file",
        &json!({ "path": "README.md", "content": "# Chapter 5 Demo\n" }),
    );

    // --- ListDirectory: find all files ---------------------------------
    let list_result = registry.dispatch("list_directory", &json!({ "pattern": "**/*" }));
    println!("list_directory =>\n{}", format_result(&list_result));

    // --- Error handling: read a file that does not exist ----------------
    let err_result = registry.dispatch("read_file", &json!({ "path": "nonexistent.txt" }));
    println!("read_file (missing) => {}", format_result(&err_result));

    // --- Error handling: path traversal attempt ------------------------
    let escape_result = registry.dispatch("read_file", &json!({ "path": "../../etc/passwd" }));
    println!("read_file (escape)  => {}", format_result(&escape_result));

    // Clean up the demo directory.
    let _ = fs::remove_dir_all(&base_dir);
}

/// Format a Result for display.
fn format_result(r: &Result<String, String>) -> String {
    match r {
        Ok(s) => s.clone(),
        Err(e) => format!("[ERROR] {}", e),
    }
}
