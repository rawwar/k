---
title: AST Navigation
description: Navigate tree-sitter syntax trees using cursors, queries, and pattern matching to find specific code constructs.
---

# AST Navigation

> **What you'll learn:**
> - How to traverse a tree-sitter syntax tree using `TreeCursor` for depth-first exploration
> - How to write tree-sitter query patterns to find specific node types like function definitions
> - How to extract source text, line numbers, and parent context from matched AST nodes

Now that you can parse code into a syntax tree, the next step is navigating that tree to find specific constructs. You need to answer questions like "what functions are defined in this file?", "what does this struct look like?", and "where is this function called?" This subchapter covers two complementary approaches: manual traversal with `TreeCursor` and declarative pattern matching with tree-sitter queries.

## Manual Traversal with TreeCursor

The simplest way to explore a tree is to walk it depth-first using a `TreeCursor`. This gives you complete control over which nodes to visit and which to skip:

```rust
use tree_sitter::Parser;

fn main() {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into()).unwrap();

    let source = r#"
use std::collections::HashMap;

struct Config {
    name: String,
    values: HashMap<String, i32>,
}

impl Config {
    fn new(name: &str) -> Self {
        Config {
            name: name.to_string(),
            values: HashMap::new(),
        }
    }

    fn get(&self, key: &str) -> Option<&i32> {
        self.values.get(key)
    }
}

fn main() {
    let config = Config::new("app");
}
"#;

    let tree = parser.parse(source, None).unwrap();
    let mut cursor = tree.root_node().walk();

    // Walk top-level nodes
    let mut reached_root = false;
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            let start = node.start_position();
            let kind = node.kind();

            // Extract the first line of this node's text
            let text = &source[node.byte_range()];
            let first_line = text.lines().next().unwrap_or("");

            println!(
                "Line {}: {} -> {}",
                start.row + 1,
                kind,
                first_line
            );

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
```

Output:
```
Line 2: use_declaration -> use std::collections::HashMap;
Line 4: struct_item -> struct Config {
Line 9: impl_item -> impl Config {
Line 22: function_item -> fn main() {
```

The cursor API has these core navigation methods:

| Method | Moves to |
|--------|----------|
| `goto_first_child()` | First child of current node |
| `goto_next_sibling()` | Next sibling at the same level |
| `goto_parent()` | Parent of current node |
| `node()` | Returns the current `Node` |

By combining these, you can implement any traversal pattern. Here is a recursive function finder that descends into `impl` blocks:

```rust
use tree_sitter::Node;

pub struct FunctionInfo {
    pub name: String,
    pub line: usize,
    pub parent: Option<String>, // e.g., "Config" if inside impl Config
    pub signature: String,
}

pub fn find_functions(source: &str, root: Node) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    collect_functions(source, root, None, &mut functions);
    functions
}

fn collect_functions(
    source: &str,
    node: Node,
    parent_name: Option<&str>,
    results: &mut Vec<FunctionInfo>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = source[name_node.byte_range()].to_string();
                    let sig_end = child
                        .child_by_field_name("body")
                        .map(|b| b.start_byte())
                        .unwrap_or(child.end_byte());
                    let signature = source[child.start_byte()..sig_end]
                        .trim()
                        .to_string();

                    results.push(FunctionInfo {
                        name,
                        line: child.start_position().row + 1,
                        parent: parent_name.map(|s| s.to_string()),
                        signature,
                    });
                }
            }
            "impl_item" => {
                // Extract the type name for this impl block
                let impl_name = child
                    .child_by_field_name("type")
                    .map(|t| source[t.byte_range()].to_string());

                collect_functions(
                    source,
                    child,
                    impl_name.as_deref(),
                    results,
                );
            }
            _ => {
                // Recurse into other nodes (e.g., mod blocks)
                collect_functions(source, child, parent_name, results);
            }
        }
    }
}
```

::: tip Coming from Python
In Python's `ast` module, you would use `ast.walk()` or a `NodeVisitor` subclass:
```python
import ast

class FunctionFinder(ast.NodeVisitor):
    def __init__(self):
        self.functions = []

    def visit_FunctionDef(self, node):
        self.functions.append((node.name, node.lineno))
        self.generic_visit(node)  # Visit children too
```
Tree-sitter's cursor-based traversal is the Rust equivalent. The key difference is that tree-sitter works for *any* language, while Python's `ast` is Python-only. Also, tree-sitter's node types are strings (`"function_item"`) rather than Python class types (`ast.FunctionDef`), so you match on string values.
:::

## Tree-sitter Queries

Manual traversal works but gets verbose quickly. Tree-sitter queries provide a declarative pattern matching language inspired by S-expressions. You write a pattern that describes the tree structure you want to find, and tree-sitter finds all matches:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

fn main() {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into()).unwrap();

    let source = r#"
pub fn process(data: &[u8]) -> Result<(), Error> {
    validate(data)?;
    transform(data)
}

fn validate(data: &[u8]) -> Result<(), Error> {
    if data.is_empty() {
        return Err(Error::Empty);
    }
    Ok(())
}

struct Config {
    path: String,
}
"#;

    let tree = parser.parse(source, None).unwrap();

    // Query: find all function definitions and capture their names
    let query = Query::new(
        &language.into(),
        "(function_item name: (identifier) @func_name) @func_def",
    ).unwrap();

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    for m in matches {
        for capture in m.captures {
            let node = capture.node;
            let capture_name = &query.capture_names()[capture.index as usize];
            let text = &source[node.byte_range()];

            if capture_name == "func_name" {
                println!(
                    "Function '{}' at line {}",
                    text,
                    node.start_position().row + 1
                );
            }
        }
    }
}
```

Output:
```
Function 'process' at line 2
Function 'validate' at line 7
```

### Query Syntax

Tree-sitter queries use S-expression patterns with captures (prefixed with `@`):

```scheme
; Match any function definition
(function_item name: (identifier) @name)

; Match public functions only
(function_item
  (visibility_modifier) @vis
  name: (identifier) @name)

; Match struct definitions with their fields
(struct_item
  name: (type_identifier) @struct_name
  body: (field_declaration_list
    (field_declaration
      name: (field_identifier) @field_name
      type: (_) @field_type)))

; Match impl blocks and the functions inside them
(impl_item
  type: (type_identifier) @impl_type
  body: (declaration_list
    (function_item
      name: (identifier) @method_name)))
```

The `(_)` wildcard matches any node type. Named fields (like `name:` and `body:`) match specific child fields of the parent node.

## Building a Reusable Query Engine

Let's wrap tree-sitter queries in a reusable struct that your search tools can share:

```rust
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

pub struct QueryEngine {
    parser: Parser,
    language: Language,
}

#[derive(Debug)]
pub struct QueryMatch {
    pub capture_name: String,
    pub text: String,
    pub line: usize,
    pub column: usize,
    pub byte_range: std::ops::Range<usize>,
}

impl QueryEngine {
    pub fn new(language: Language) -> Result<Self, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set language: {e}"))?;

        Ok(Self { parser, language })
    }

    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, None)
    }

    pub fn query(
        &self,
        source: &str,
        tree: &Tree,
        query_str: &str,
    ) -> Result<Vec<QueryMatch>, String> {
        let query = Query::new(&self.language, query_str)
            .map_err(|e| format!("Invalid query: {e}"))?;

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        let mut results = Vec::new();
        for m in matches {
            for capture in m.captures {
                let node = capture.node;
                let name = query.capture_names()[capture.index as usize].clone();
                let text = source[node.byte_range()].to_string();

                results.push(QueryMatch {
                    capture_name: name,
                    text,
                    line: node.start_position().row + 1,
                    column: node.start_position().column,
                    byte_range: node.byte_range(),
                });
            }
        }

        Ok(results)
    }
}
```

Usage:

```rust
fn main() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut engine = QueryEngine::new(language).unwrap();

    let source = r#"
pub struct AppState {
    config: Config,
    db: Database,
}

impl AppState {
    pub fn new(config: Config, db: Database) -> Self {
        Self { config, db }
    }
}
"#;

    let tree = engine.parse(source).unwrap();

    // Find all struct fields
    let field_query = r#"
        (field_declaration
          name: (field_identifier) @field_name
          type: (_) @field_type)
    "#;

    let fields = engine.query(source, &tree, field_query).unwrap();
    for field in fields {
        println!("{}: {} at line {}", field.capture_name, field.text, field.line);
    }
}
```

Output:
```
field_name: config at line 3
field_type: Config at line 3
field_name: db at line 4
field_type: Database at line 4
```

## Extracting Parent Context

When presenting search results to the LLM, knowing *where* a match occurs is as important as the match itself. A function named `new` could be in any struct -- the enclosing `impl` block tells you which one:

```rust
use tree_sitter::Node;

pub fn get_parent_context(source: &str, node: Node) -> Vec<String> {
    let mut context = Vec::new();
    let mut current = node;

    while let Some(parent) = current.parent() {
        match parent.kind() {
            "impl_item" => {
                if let Some(type_node) = parent.child_by_field_name("type") {
                    let type_name = &source[type_node.byte_range()];
                    context.push(format!("impl {type_name}"));
                }
            }
            "function_item" => {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    let fn_name = &source[name_node.byte_range()];
                    context.push(format!("fn {fn_name}"));
                }
            }
            "struct_item" | "enum_item" => {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    let name = &source[name_node.byte_range()];
                    context.push(format!("{} {name}", parent.kind().replace("_item", "")));
                }
            }
            "mod_item" => {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    let mod_name = &source[name_node.byte_range()];
                    context.push(format!("mod {mod_name}"));
                }
            }
            _ => {}
        }
        current = parent;
    }

    context.reverse(); // Root-first order
    context
}
```

This produces context chains like `["mod utils", "impl Config", "fn new"]` that the LLM can use to understand exactly where a search result lives in the code hierarchy.

::: info In the Wild
Many coding agents use tree-sitter for building "outline" views of files -- a compact representation that shows the structure without the implementation details. Claude Code, for instance, can parse a file into its structural components (functions, classes, methods) to present a high-level overview when the full file would exceed the context window. This is more reliable than regex-based extraction because tree-sitter handles edge cases like nested structures and multiline signatures correctly.
:::

## Key Takeaways

- `TreeCursor` provides low-level, imperative tree traversal -- use it when you need full control over which nodes to visit and in what order.
- Tree-sitter queries provide high-level, declarative pattern matching -- use them when you need to find all instances of a specific code construct across a file.
- Named fields (`child_by_field_name`) are the preferred way to access specific parts of a node, like a function's name, parameters, or body.
- Parent context extraction (walking up the tree from a match to find enclosing constructs) is essential for presenting search results that the LLM can act on effectively.
- The `QueryEngine` struct wraps tree-sitter's parser and query system into a reusable component that all your semantic search tools can share.
