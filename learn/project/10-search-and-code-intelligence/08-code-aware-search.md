---
title: Code Aware Search
description: Combine text search with AST awareness to search within specific code contexts like function bodies, comments, or string literals.
---

# Code Aware Search

> **What you'll learn:**
> - How to restrict text search to specific AST node types like function bodies or doc comments
> - How to implement scope-aware search that understands nesting and containment relationships
> - How to present search results with structural context like the enclosing function or class name

Semantic search finds definitions by name. Grep finds text patterns anywhere. Code-aware search sits between these two: it finds text patterns *within specific code contexts*. Want to search only inside function bodies, ignoring comments? Search only in doc comments? Find all string literals that contain a URL? Code-aware search combines the flexibility of regex with the structural understanding of tree-sitter to answer these questions precisely.

## Why Context-Restricted Search Matters

Consider this scenario: the LLM wants to find all TODO comments in a codebase. A naive grep for `TODO` also matches variable names like `todo_items`, function names like `process_todo`, and string contents like `"TODO: implement"`. A code-aware search restricts the match to comment nodes only, returning exactly what the agent needs.

Another example: the LLM is looking for hardcoded API URLs. A grep for `https://` returns every URL in the codebase, including documentation, tests, and commented-out code. A code-aware search restricted to string literal nodes gives just the URLs that are actually used in the running code.

## Mapping Text Positions to AST Nodes

The key technique is position mapping: given a text match at a specific byte offset, find the tree-sitter node that contains that position and check whether it matches the desired context:

```rust
use tree_sitter::{Node, Point, Tree};

/// Find the deepest (most specific) AST node containing the given byte offset.
pub fn node_at_position(tree: &Tree, byte_offset: usize) -> Option<Node> {
    let root = tree.root_node();
    find_deepest_node(root, byte_offset)
}

fn find_deepest_node(node: Node, byte_offset: usize) -> Option<Node> {
    // Check if this node contains the offset
    if byte_offset < node.start_byte() || byte_offset >= node.end_byte() {
        return None;
    }

    // Try to find a more specific child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(deeper) = find_deepest_node(child, byte_offset) {
            return Some(deeper);
        }
    }

    // No child contains it -- this node is the deepest
    Some(node)
}

/// Check whether a node is inside a node of the specified kind
pub fn is_inside_node_kind(node: Node, kind: &str) -> bool {
    let mut current = node;
    loop {
        if current.kind() == kind {
            return true;
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return false,
        }
    }
}
```

## Building the Code-Aware Search

Here is the core function that combines regex matching with AST context checking:

```rust
use regex::Regex;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Parser, Tree};

#[derive(Debug, Clone)]
pub enum SearchContext {
    /// Search everywhere (no restriction)
    Any,
    /// Search only inside function/method bodies
    FunctionBody,
    /// Search only inside comments (line comments and block comments)
    Comments,
    /// Search only inside string literals
    StringLiterals,
    /// Search only inside doc comments
    DocComments,
    /// Search only in top-level items (not nested inside functions)
    TopLevel,
}

#[derive(Debug)]
pub struct CodeAwareMatch {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub matched_text: String,
    pub line_content: String,
    pub node_kind: String,
    pub enclosing_context: Vec<String>,
}

pub fn code_aware_search(
    source: &str,
    file_path: &Path,
    pattern: &Regex,
    context: &SearchContext,
    language: Language,
) -> Result<Vec<CodeAwareMatch>, String> {
    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|e| e.to_string())?;

    let tree = parser.parse(source, None)
        .ok_or("Failed to parse file")?;

    let mut matches = Vec::new();

    for regex_match in pattern.find_iter(source) {
        let byte_offset = regex_match.start();

        // Find the AST node at this position
        let Some(node) = node_at_position(&tree, byte_offset) else {
            continue;
        };

        // Check if the match is in the requested context
        if !matches_context(&node, context) {
            continue;
        }

        // Calculate line number from byte offset
        let line = source[..byte_offset].matches('\n').count() + 1;
        let line_start = source[..byte_offset].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let column = byte_offset - line_start;

        // Get the full line content
        let line_end = source[byte_offset..]
            .find('\n')
            .map(|p| byte_offset + p)
            .unwrap_or(source.len());
        let line_content = source[line_start..line_end].to_string();

        // Get enclosing context
        let enclosing = get_enclosing_context(source, node);

        matches.push(CodeAwareMatch {
            path: file_path.to_path_buf(),
            line,
            column,
            matched_text: regex_match.as_str().to_string(),
            line_content,
            node_kind: node.kind().to_string(),
            enclosing_context: enclosing,
        });
    }

    Ok(matches)
}

fn matches_context(node: &Node, context: &SearchContext) -> bool {
    match context {
        SearchContext::Any => true,
        SearchContext::FunctionBody => {
            is_inside_node_kind(*node, "block")
                && has_ancestor_kind(*node, "function_item")
        }
        SearchContext::Comments => {
            let kind = node.kind();
            kind == "line_comment" || kind == "block_comment"
        }
        SearchContext::StringLiterals => {
            let kind = node.kind();
            kind == "string_literal"
                || kind == "raw_string_literal"
                || kind == "string_content"
        }
        SearchContext::DocComments => {
            // Doc comments in tree-sitter Rust have specific node types
            node.kind() == "line_comment"
                && node.parent().map(|p| p.kind()) == Some("attribute_item")
                || is_doc_comment_text(node)
        }
        SearchContext::TopLevel => {
            // Not inside any function body
            !has_ancestor_kind(*node, "function_item")
                || is_inside_node_kind(*node, "source_file")
        }
    }
}

fn has_ancestor_kind(node: Node, kind: &str) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == kind {
            return true;
        }
        current = parent;
    }
    false
}

fn is_doc_comment_text(node: &Node) -> bool {
    if node.kind() != "line_comment" {
        return false;
    }
    // Check if the comment starts with /// or //!
    // We would need the source text for this, so this is approximate
    true // Simplified -- in practice, check the text content
}

fn get_enclosing_context(source: &str, node: Node) -> Vec<String> {
    let mut context = Vec::new();
    let mut current = node;

    while let Some(parent) = current.parent() {
        match parent.kind() {
            "function_item" | "function_definition" => {
                if let Some(name) = parent.child_by_field_name("name") {
                    context.push(format!("fn {}", &source[name.byte_range()]));
                }
            }
            "impl_item" => {
                if let Some(ty) = parent.child_by_field_name("type") {
                    context.push(format!("impl {}", &source[ty.byte_range()]));
                }
            }
            "struct_item" => {
                if let Some(name) = parent.child_by_field_name("name") {
                    context.push(format!("struct {}", &source[name.byte_range()]));
                }
            }
            _ => {}
        }
        current = parent;
    }

    context.reverse();
    context
}
```

::: python Coming from Python
Python's `ast` module lets you do similar context-restricted analysis:
```python
import ast

class StringFinder(ast.NodeVisitor):
    """Find all string literals containing a pattern."""
    def __init__(self, pattern):
        self.pattern = pattern
        self.matches = []

    def visit_Constant(self, node):
        if isinstance(node.value, str) and self.pattern in node.value:
            self.matches.append((node.lineno, node.value))
```
The tree-sitter approach generalizes across languages. The same `code_aware_search` function works for any language by swapping the grammar and adjusting node type names. The structural concept (comments, strings, function bodies) exists in every language, just with different node type names.
:::

## Practical Search Patterns

Here are the code-aware searches that a coding agent uses most frequently:

### Find TODO/FIXME in Comments Only

```rust
fn find_todos_in_comments(
    source: &str,
    file_path: &Path,
    language: Language,
) -> Result<Vec<CodeAwareMatch>, String> {
    let pattern = Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX)\b:?\s*(.+)")
        .map_err(|e| format!("Regex error: {e}"))?;

    code_aware_search(source, file_path, &pattern, &SearchContext::Comments, language)
}
```

### Find Hardcoded Strings (Not in Comments or Tests)

```rust
fn find_hardcoded_strings(
    source: &str,
    file_path: &Path,
    pattern: &str,
    language: Language,
) -> Result<Vec<CodeAwareMatch>, String> {
    let regex = Regex::new(pattern)
        .map_err(|e| format!("Regex error: {e}"))?;

    code_aware_search(source, file_path, &regex, &SearchContext::StringLiterals, language)
}
```

### Find Function Calls Within a Specific Function

```rust
fn find_calls_in_function(
    source: &str,
    file_path: &Path,
    call_pattern: &str,
    language: Language,
) -> Result<Vec<CodeAwareMatch>, String> {
    let regex = Regex::new(call_pattern)
        .map_err(|e| format!("Regex error: {e}"))?;

    code_aware_search(source, file_path, &regex, &SearchContext::FunctionBody, language)
}
```

## Formatting Code-Aware Results

The output format should emphasize the structural context, since that is the differentiating value of code-aware search:

```rust
pub fn format_code_aware_results(matches: &[CodeAwareMatch]) -> String {
    if matches.is_empty() {
        return "No matches found in the specified context.".to_string();
    }

    let mut output = format!("Found {} context-aware match(es):\n\n", matches.len());

    for m in matches {
        // Show the structural context path
        let context_path = if m.enclosing_context.is_empty() {
            "top level".to_string()
        } else {
            m.enclosing_context.join(" > ")
        };

        output.push_str(&format!(
            "{}:{} [{}] in {}\n",
            m.path.display(),
            m.line,
            m.node_kind,
            context_path,
        ));
        output.push_str(&format!("  > {}\n", m.line_content.trim()));
        output.push_str("---\n");
    }

    output
}
```

Example output:
```
Found 3 context-aware match(es):

src/server.rs:15 [string_literal] in impl Server > fn connect
  > let url = "https://api.example.com/v1";
---
src/client.rs:42 [string_literal] in impl Client > fn base_url
  > "https://api.example.com"
---
src/config.rs:8 [string_literal] in fn default_config
  > default_endpoint: "https://api.example.com/v2".to_string(),
---
```

The structural context (`impl Server > fn connect`) tells the LLM exactly where each match lives without requiring a separate file read.

## Scope-Aware Search

A more advanced capability is scope awareness: understanding that a variable defined in one function is not the same as a variable with the same name in another function. Here is a simplified scope resolver:

```rust
pub fn find_in_scope(
    source: &str,
    tree: &Tree,
    target_name: &str,
    scope_function: &str,
) -> Vec<(usize, String)> {
    let root = tree.root_node();
    let mut results = Vec::new();

    // Find the function node
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "function_item" {
            if let Some(name) = child.child_by_field_name("name") {
                let fn_name = &source[name.byte_range()];
                if fn_name == scope_function {
                    // Search within this function's body
                    if let Some(body) = child.child_by_field_name("body") {
                        let body_text = &source[body.byte_range()];
                        let body_start_line = body.start_position().row + 1;

                        for (i, line) in body_text.lines().enumerate() {
                            if line.contains(target_name) {
                                results.push((
                                    body_start_line + i,
                                    line.trim().to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    results
}
```

::: wild In the Wild
Claude Code uses a combination of grep and structural understanding to answer questions about code. When you ask "where is this variable used?", it combines text search with context analysis to filter out false positives in comments and strings. The code-aware search approach in this chapter achieves similar results by using tree-sitter to understand the syntactic context of each match, providing the precision that makes agent-suggested edits reliable.
:::

## Key Takeaways

- Code-aware search combines regex matching with AST node type checking, letting you restrict searches to specific syntactic contexts like function bodies, comments, or string literals.
- Position mapping (finding the deepest AST node at a byte offset) is the bridge between text-based regex results and tree-based structural analysis.
- Enclosing context (the chain of parent constructs like `impl Server > fn connect`) provides the LLM with structural information that eliminates the need for follow-up file reads.
- Common code-aware searches include TODO finding in comments only, hardcoded string detection, and scoped variable usage analysis.
- The same code-aware search infrastructure works across languages by swapping tree-sitter grammars and adjusting node type names for each language.
