---
title: Parsing Source Code
description: Using tree-sitter's Rust bindings to parse source files, navigate the resulting tree with cursors, and access node types, text ranges, and parent-child relationships.
---

# Parsing Source Code

> **What you'll learn:**
> - How to initialize a tree-sitter parser with a language grammar and parse source code into a Tree with a root Node
> - Navigating the syntax tree using TreeCursor for efficient depth-first traversal and direct child access
> - Extracting information from nodes: kind (type name), text content via byte ranges, start/end positions, and named vs anonymous nodes

Now that you understand tree-sitter's architecture, let's get your hands on the Rust API. This subchapter walks through the complete workflow: setting up a parser, feeding it source code, and navigating the resulting tree to extract structural information. By the end, you will have a working function that can parse any Rust file and list every function definition with its line number and parameter count.

## Setting Up the Parser

Tree-sitter's Rust bindings live in the `tree-sitter` crate. Language grammars are separate crates — `tree-sitter-rust` for Rust, `tree-sitter-python` for Python, and so on. Add them to your `Cargo.toml`:

```toml
[dependencies]
tree-sitter = "0.24"
tree-sitter-rust = "0.23"
```

Creating a parser and parsing source code takes three lines:

```rust
use tree_sitter::Parser;

fn parse_rust_source(source: &str) -> tree_sitter::Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");

    parser.parse(source, None).expect("Failed to parse")
}

fn main() {
    let source = r#"
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;

    let tree = parse_rust_source(source);
    let root = tree.root_node();

    println!("Root node kind: {}", root.kind());
    println!("Number of children: {}", root.named_child_count());
    println!("Source range: {} - {} bytes", root.start_byte(), root.end_byte());
}
```

The `parse` method returns an `Option<Tree>` — it returns `None` only if cancellation was requested or a timeout was set and exceeded. For normal usage, it always succeeds. The returned `Tree` owns its data and is `Send` but not `Sync`, meaning you can move it between threads but cannot share it across threads without a mutex.

## Understanding the Node API

Every node in the tree is a lightweight `Node` struct that borrows from the `Tree`. Nodes are `Copy` — they are just a pointer and an index, not a heap allocation. The key methods you will use constantly:

```rust
use tree_sitter::Parser;

fn explore_nodes(source: &str) {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");
    let tree = parser.parse(source, None).unwrap();
    let root = tree.root_node();

    // Walk the top-level children
    for i in 0..root.named_child_count() {
        let node = root.named_child(i).unwrap();

        // kind() returns the node type as a string
        println!("Kind: {}", node.kind());

        // is_named() distinguishes named nodes from anonymous (punctuation) nodes
        println!("Named: {}", node.is_named());

        // Byte range in the source
        println!("Bytes: {}..{}", node.start_byte(), node.end_byte());

        // Line and column position (0-indexed)
        let start = node.start_position();
        let end = node.end_position();
        println!(
            "Position: line {}:{} to line {}:{}",
            start.row, start.column, end.row, end.column
        );

        // Extract the text using the byte range
        let text = &source.as_bytes()[node.start_byte()..node.end_byte()];
        println!("Text: {}", std::str::from_utf8(text).unwrap());

        // Access named children by field name
        if node.kind() == "function_item" {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = &source.as_bytes()[name_node.start_byte()..name_node.end_byte()];
                println!("Function name: {}", std::str::from_utf8(name).unwrap());
            }
        }

        println!("---");
    }
}

fn main() {
    let source = r#"
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

struct Config {
    verbose: bool,
    timeout: u64,
}
"#;
    explore_nodes(source);
}
```

There are two ways to access children: by index (`node.child(i)` for all children, `node.named_child(i)` for named children only) and by field name (`node.child_by_field_name("name")`). Field names are defined in the grammar and give semantic meaning to the parent-child relationship. For a `function_item` node, the grammar defines fields like `name`, `parameters`, `return_type`, and `body`. Using field names makes your code self-documenting and resilient to grammar changes that might reorder children.

::: python Coming from Python
In Python's `ast` module, you access node attributes directly: `node.name`, `node.args`, `node.body`. Tree-sitter uses a different pattern — `node.child_by_field_name("name")` — because tree-sitter nodes are generic across all languages. There are no language-specific node classes. The trade-off is less type safety but universal applicability: the same code pattern works whether you are parsing Rust, Python, or TypeScript.
:::

## Navigating with TreeCursor

Calling `node.child(i)` in a loop works for shallow exploration, but for deep tree traversal, `TreeCursor` is significantly more efficient. A cursor maintains its position in the tree and exposes methods to move to children, siblings, and parents without allocating new node objects:

```rust
use tree_sitter::Parser;

fn walk_with_cursor(source: &str) {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");
    let tree = parser.parse(source, None).unwrap();

    let mut cursor = tree.walk();
    let mut depth = 0;

    // Depth-first traversal using the cursor
    loop {
        let node = cursor.node();

        // Only print named nodes to avoid punctuation clutter
        if node.is_named() {
            let indent = "  ".repeat(depth);
            let text_preview: String = source[node.start_byte()..node.end_byte()]
                .chars()
                .take(40)
                .collect();
            println!(
                "{}{}  [{}:{}] \"{}\"",
                indent,
                node.kind(),
                node.start_position().row,
                node.start_position().column,
                text_preview
            );
        }

        // Try to go deeper
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }

        // Try to go to next sibling
        if cursor.goto_next_sibling() {
            continue;
        }

        // Go up until we find a sibling or reach the root
        loop {
            if !cursor.goto_parent() {
                return; // Back at root, traversal complete
            }
            depth -= 1;
            if cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn main() {
    let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
    walk_with_cursor(source);
}
```

The cursor-based traversal is the standard pattern for walking tree-sitter trees. The loop structure — try child, try sibling, go up — is a common idiom you will see in every tree-sitter codebase. The cursor reuses a single allocation for its internal state, making it faster than repeatedly calling `node.child()` which creates new `Node` structs on each call.

The cursor also provides `cursor.field_name()`, which tells you the field name of the current node relative to its parent. This is useful when you need to know not just what a node is, but what role it plays:

```rust
use tree_sitter::Parser;

fn show_field_names(source: &str) {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");
    let tree = parser.parse(source, None).unwrap();

    // Navigate to the first function
    let root = tree.root_node();
    let func = root.named_child(0).unwrap();

    let mut cursor = func.walk();
    cursor.goto_first_child();

    loop {
        let node = cursor.node();
        let field = cursor.field_name().unwrap_or("(none)");
        let text = &source[node.start_byte()..node.end_byte()];
        println!("Field: {:15} Kind: {:20} Text: {}", field, node.kind(), text);

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

fn main() {
    let source = "fn greet(name: &str) -> String { format!(\"Hello, {}!\", name) }";
    show_field_names(source);
}
```

This prints each direct child of the function with its field name, revealing the grammar's structure: the `fn` keyword has no field name (it is anonymous punctuation), but `name`, `parameters`, `return_type`, and `body` are all named fields.

## A Practical Example: Listing Functions

Let's put it all together. Here is a complete function that parses a Rust source file and returns information about every function defined in it:

```rust
use tree_sitter::Parser;

#[derive(Debug)]
struct FunctionInfo {
    name: String,
    line: usize,
    parameter_count: usize,
    return_type: Option<String>,
    is_public: bool,
}

fn extract_functions(source: &str) -> Vec<FunctionInfo> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");
    let tree = parser.parse(source, None).unwrap();

    let mut functions = Vec::new();
    let mut cursor = tree.walk();

    // Walk only top-level named children
    if !cursor.goto_first_child() {
        return functions;
    }

    loop {
        let node = cursor.node();

        if node.kind() == "function_item" {
            let name = node
                .child_by_field_name("name")
                .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                .unwrap_or_default();

            let line = node.start_position().row + 1; // Convert 0-indexed to 1-indexed

            let parameter_count = node
                .child_by_field_name("parameters")
                .map(|params| params.named_child_count())
                .unwrap_or(0);

            let return_type = node
                .child_by_field_name("return_type")
                .map(|rt| source[rt.start_byte()..rt.end_byte()].to_string());

            // Check for visibility modifier
            let is_public = node
                .child_by_field_name("visibility_modifier")
                .is_some();

            functions.push(FunctionInfo {
                name,
                line,
                parameter_count,
                return_type,
                is_public,
            });
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }

    functions
}

fn main() {
    let source = r#"
pub fn connect(addr: &str, timeout: u64) -> Result<TcpStream, Error> {
    TcpStream::connect_timeout(addr, Duration::from_secs(timeout))
}

fn validate_config(config: &Config) -> bool {
    config.timeout > 0 && !config.host.is_empty()
}

pub fn start_server(config: Config, handler: Handler, logger: Logger) -> Result<(), Error> {
    let listener = TcpListener::bind(&config.addr)?;
    Ok(())
}
"#;

    let functions = extract_functions(source);
    for func in &functions {
        println!(
            "Line {}: {}fn {}({} params) -> {}",
            func.line,
            if func.is_public { "pub " } else { "" },
            func.name,
            func.parameter_count,
            func.return_type.as_deref().unwrap_or("()")
        );
    }
}
```

This gives you structured information about every function — name, location, arity, return type, visibility — extracted directly from the syntax tree. No regex required, no false positives from comments or strings, and it works correctly even if the file contains other nodes like structs, enums, or impl blocks.

::: wild In the Wild
Claude Code's file reading tools return source code as plain text that the LLM must parse mentally. Agents that integrate tree-sitter can pre-process files before sending them to the LLM, providing structured summaries like "this file contains 3 public functions, 2 structs, and 1 impl block" along with the raw source. This helps the LLM understand the file organization without spending tokens on mental parsing — a pattern used by several open-source coding agents to improve accuracy on large files.
:::

## Key Takeaways

- Initialize a parser with `Parser::new()`, set the language with `set_language()`, and call `parse()` with source bytes to get a `Tree` — pass an old tree as the second argument for incremental re-parsing
- Nodes expose `kind()` for the type name, `start_byte()`/`end_byte()` for extracting text, `start_position()` for line/column, and `child_by_field_name()` for semantically named children
- `TreeCursor` provides efficient depth-first traversal using `goto_first_child()`, `goto_next_sibling()`, and `goto_parent()` — prefer cursors over indexed child access for deep traversals
- Named nodes represent meaningful grammar elements (functions, identifiers, types) while anonymous nodes represent punctuation and keywords — use `is_named()` to filter
- The combination of field names and node kinds lets you write precise extraction logic that reads naturally: `node.child_by_field_name("return_type")` is self-documenting
