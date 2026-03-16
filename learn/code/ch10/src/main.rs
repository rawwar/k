// Chapter 10: Search and Code Intelligence — Code snapshot
//
// Builds on the Tool trait from Chapter 4 and the context management ideas
// from Chapter 9. Implements GrepTool (file content search via regex) and
// GlobTool (file discovery via glob patterns). Tree-sitter integration for
// AST-level code intelligence is covered in the learning content but not
// fully implemented here — see subchapters 05–09 for those details.

use globset::{Glob, GlobMatcher};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Tool trait (from Chapter 4)
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
// Grep Tool — search file contents with regex
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
struct GrepToolInput {
    /// The regex pattern to search for.
    pattern: String,

    /// Directory to search in (defaults to current working directory).
    #[serde(default)]
    path: Option<String>,

    /// Glob pattern to filter which files are searched (e.g. "*.rs").
    #[serde(default)]
    include: Option<String>,

    /// Number of context lines before and after each match.
    #[serde(default = "default_context_lines")]
    context_lines: usize,

    /// If true, search case-insensitively.
    #[serde(default)]
    case_insensitive: bool,
}

fn default_context_lines() -> usize {
    2
}

/// A single grep match with optional context lines.
#[derive(Debug)]
struct GrepMatch {
    path: PathBuf,
    line_number: usize,
    line_content: String,
    context_before: Vec<String>,
    context_after: Vec<String>,
}

/// Walk directories, read each file, and return lines matching the regex.
fn grep_search(input: &GrepToolInput) -> Result<Vec<GrepMatch>, String> {
    let regex = RegexBuilder::new(&input.pattern)
        .case_insensitive(input.case_insensitive)
        .build()
        .map_err(|e| format!("Invalid regex pattern: {e}"))?;

    let search_path = input.path.as_deref().unwrap_or(".");

    // Optionally compile an include-glob for file name filtering.
    let include_matcher: Option<GlobMatcher> = match &input.include {
        Some(pattern) => {
            let glob = Glob::new(pattern)
                .map_err(|e| format!("Invalid include glob '{pattern}': {e}"))?;
            Some(glob.compile_matcher())
        }
        None => None,
    };

    let mut matches = Vec::new();

    for entry in WalkDir::new(search_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        // Apply include filter against the file name.
        if let Some(ref matcher) = include_matcher {
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if !matcher.is_match(file_name) {
                continue;
            }
        }

        // Skip binary files (check first 512 bytes for null bytes).
        if is_binary(path) {
            continue;
        }

        // Read file contents; silently skip files we cannot read.
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                matches.push(GrepMatch {
                    path: path.to_path_buf(),
                    line_number: i + 1,
                    line_content: line.to_string(),
                    context_before: context_before(&lines, i, input.context_lines),
                    context_after: context_after(&lines, i, input.context_lines),
                });
            }
        }
    }

    Ok(matches)
}

fn context_before(lines: &[&str], index: usize, count: usize) -> Vec<String> {
    let start = index.saturating_sub(count);
    lines[start..index].iter().map(|l| l.to_string()).collect()
}

fn context_after(lines: &[&str], index: usize, count: usize) -> Vec<String> {
    let end = (index + 1 + count).min(lines.len());
    lines[index + 1..end]
        .iter()
        .map(|l| l.to_string())
        .collect()
}

/// Heuristic: a file is binary if its first 512 bytes contain a null byte.
fn is_binary(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return true;
    };
    let mut buf = [0u8; 512];
    let bytes_read = match file.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return true,
    };
    buf[..bytes_read].contains(&0)
}

/// Format grep results for LLM consumption, capping at `max_results`.
fn format_grep_results(matches: &[GrepMatch], max_results: usize) -> String {
    if matches.is_empty() {
        return "No matches found.".to_string();
    }

    let display_count = matches.len().min(max_results);
    let total = matches.len();

    let mut output = if display_count == total {
        format!("[grep] Found {total} result(s):\n\n")
    } else {
        format!("[grep] Showing {display_count} of {total} result(s):\n\n")
    };

    for m in matches.iter().take(display_count) {
        // Context before
        for (offset, ctx) in m.context_before.iter().enumerate() {
            let num = m.line_number - m.context_before.len() + offset;
            output.push_str(&format!("  {num}: {ctx}\n"));
        }
        // Matching line (marked with >)
        output.push_str(&format!(
            "> {}:{}: {}\n",
            m.path.display(),
            m.line_number,
            m.line_content
        ));
        // Context after
        for (offset, ctx) in m.context_after.iter().enumerate() {
            let num = m.line_number + 1 + offset;
            output.push_str(&format!("  {num}: {ctx}\n"));
        }
        output.push_str("---\n");
    }

    if total > display_count {
        output.push_str(&format!(
            "\n[{} more result(s) not shown. \
             Narrow your search with a more specific pattern or add file filters.]\n",
            total - display_count
        ));
    }

    output
}

// --- GrepTool implements Tool ------------------------------------------------

struct GrepTool {
    max_results: usize,
}

impl GrepTool {
    fn new() -> Self {
        Self { max_results: 50 }
    }
}

impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using regex patterns. Returns matching lines \
         with surrounding context. Use this to find function definitions, \
         error messages, imports, and any text pattern across the codebase. \
         For finding files by name, use the glob tool instead."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (defaults to cwd)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob to filter files, e.g. '*.rs'"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Lines of context around matches (default 2)"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive search (default false)"
                }
            }
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        let params: GrepToolInput = serde_json::from_value(input.clone())
            .map_err(|e| format!("Invalid input: {e}"))?;
        let matches = grep_search(&params)?;
        Ok(format_grep_results(&matches, self.max_results))
    }
}

// ---------------------------------------------------------------------------
// Glob Tool — find files by name pattern
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
struct GlobToolInput {
    /// Glob pattern to match files against (e.g. "**/*.rs").
    pattern: String,

    /// Directory to search in (defaults to current working directory).
    #[serde(default)]
    path: Option<String>,

    /// Maximum number of results to return.
    #[serde(default = "default_glob_limit")]
    limit: usize,
}

fn default_glob_limit() -> usize {
    100
}

#[derive(Debug)]
struct GlobResult {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: u64,
}

/// Walk the directory tree and return paths matching the glob pattern.
fn glob_search(input: &GlobToolInput) -> Result<Vec<GlobResult>, String> {
    let glob = Glob::new(&input.pattern)
        .map_err(|e| format!("Invalid glob pattern '{}': {e}", input.pattern))?;
    let matcher = glob.compile_matcher();

    let search_path = input.path.as_deref().unwrap_or(".");

    let mut results: Vec<GlobResult> = Vec::new();

    for entry in WalkDir::new(search_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Match against the path relative to the search root.
        let relative = path.strip_prefix(search_path).unwrap_or(path);
        if relative.as_os_str().is_empty() {
            continue;
        }

        if matcher.is_match(relative) {
            let metadata = entry.metadata().ok();
            results.push(GlobResult {
                path: path.to_path_buf(),
                modified: metadata.as_ref().and_then(|m| m.modified().ok()),
                size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            });
        }

        // Collect extra results so sorting has a larger pool, then truncate.
        if results.len() >= input.limit * 2 {
            break;
        }
    }

    // Sort by modification time — most recently modified first.
    results.sort_by(|a, b| {
        b.modified
            .unwrap_or(UNIX_EPOCH)
            .cmp(&a.modified.unwrap_or(UNIX_EPOCH))
    });

    results.truncate(input.limit);
    Ok(results)
}

/// Normalise an LLM-generated glob: if the pattern has no path separator
/// and doesn't start with `**`, prepend `**/` so it matches recursively.
fn normalize_glob_pattern(pattern: &str) -> String {
    let mut p = pattern.to_string();
    if !p.contains('/') && !p.starts_with("**") {
        p = format!("**/{p}");
    }
    if p.starts_with("./") {
        p = p[2..].to_string();
    }
    p
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Format glob results for LLM consumption.
fn format_glob_results(results: &[GlobResult], limit: usize) -> String {
    if results.is_empty() {
        return "No files matched the pattern.".to_string();
    }

    let display_count = results.len().min(limit);
    let total = results.len();

    let mut output = if display_count == total {
        format!("[glob] Found {total} file(s):\n\n")
    } else {
        format!("[glob] Showing {display_count} of {total} file(s):\n\n")
    };

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for r in results.iter().take(display_count) {
        let size = format_file_size(r.size);
        let age = r
            .modified
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| {
                let hours_ago = now_secs.saturating_sub(d.as_secs()) / 3600;
                if hours_ago < 1 {
                    "just now".to_string()
                } else if hours_ago < 24 {
                    format!("{hours_ago}h ago")
                } else {
                    format!("{}d ago", hours_ago / 24)
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        output.push_str(&format!("  {} ({size}, {age})\n", r.path.display()));
    }

    if total > display_count {
        output.push_str(&format!(
            "\n[{} more file(s) not shown. Use a more specific pattern or reduce the search scope.]\n",
            total - display_count
        ));
    }

    output
}

// --- GlobTool implements Tool ------------------------------------------------

struct GlobTool {
    max_results: usize,
}

impl GlobTool {
    fn new() -> Self {
        Self { max_results: 100 }
    }
}

impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files by name pattern using glob syntax. Supports ** for \
         recursive matching, {a,b} for alternatives, and ? for single \
         characters. Use this to discover project structure, find test \
         files, locate configs, or identify all files of a specific type. \
         For searching file contents, use the grep tool instead."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g. '**/*.rs', 'src/**/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (defaults to cwd)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default 100)"
                }
            }
        })
    }

    fn execute(&self, input: &Value) -> Result<String, String> {
        let mut params: GlobToolInput = serde_json::from_value(input.clone())
            .map_err(|e| format!("Invalid input: {e}"))?;

        // Normalise the pattern so bare "*.rs" becomes "**/*.rs".
        params.pattern = normalize_glob_pattern(&params.pattern);

        let results = glob_search(&params)?;
        Ok(format_glob_results(&results, self.max_results))
    }
}

// ---------------------------------------------------------------------------
// Simple tool registry (mirrors Chapter 4 approach)
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

    fn execute(&self, tool_name: &str, input: &Value) -> Result<String, String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == tool_name)
            .ok_or_else(|| format!("Unknown tool: {tool_name}"))?;
        tool.execute(input)
    }

    /// Convert all registered tools to the API format for tool definitions.
    fn to_api_tools(&self) -> Vec<Value> {
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
// Main — demo the search tools
// ---------------------------------------------------------------------------

fn main() {
    println!("Chapter 10: Search and Code Intelligence\n");

    // --- Register tools (as discussed in subchapter 11) ---
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(GrepTool::new()));
    registry.register(Box::new(GlobTool::new()));

    println!("Registered tools:");
    for def in registry.to_api_tools() {
        println!("  - {}", def["name"]);
    }
    println!();

    // --- Demo: grep for "fn " in the current project ---
    println!("=== Grep: searching for 'fn ' in src/ ===\n");
    let grep_input = json!({
        "pattern": "fn ",
        "path": "src",
        "include": "*.rs",
        "context_lines": 1
    });
    match registry.execute("grep", &grep_input) {
        Ok(output) => println!("{output}"),
        Err(e) => println!("Error: {e}"),
    }

    // --- Demo: glob for Rust source files ---
    println!("=== Glob: finding *.rs files ===\n");
    let glob_input = json!({
        "pattern": "**/*.rs",
        "path": "."
    });
    match registry.execute("glob", &glob_input) {
        Ok(output) => println!("{output}"),
        Err(e) => println!("Error: {e}"),
    }

    // --- Demo: glob with normalisation (bare pattern without **/) ---
    println!("=== Glob: normalised bare pattern '*.toml' ===\n");
    let glob_bare = json!({ "pattern": "*.toml" });
    match registry.execute("glob", &glob_bare) {
        Ok(output) => println!("{output}"),
        Err(e) => println!("Error: {e}"),
    }

    // NOTE: Tree-sitter based semantic search (symbol_search tool) is covered
    // in learning subchapters 05–09. A production agent would also register a
    // SemanticSearchTool here, backed by a SymbolIndex built with tree-sitter
    // grammars. The index is typically populated on a background thread using
    // Arc<RwLock<SymbolIndex>> so the agent remains responsive while indexing.
}
