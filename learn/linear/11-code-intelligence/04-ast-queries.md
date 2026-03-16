---
title: AST Queries
description: Writing tree-sitter S-expression queries to pattern-match against syntax trees, extracting functions, classes, imports, and other structural elements.
---

# AST Queries

> **What you'll learn:**
> - The tree-sitter query language syntax: S-expression patterns, captures (@name), predicates (#eq?, #match?), and quantifiers
> - Writing queries to extract common code patterns: function definitions, class declarations, import statements, and variable bindings
> - Executing queries efficiently against large files and combining multiple query patterns for complex extractions

In the previous subchapter, you navigated syntax trees manually — checking `node.kind()`, calling `child_by_field_name()`, walking with cursors. That works, but it is verbose and brittle. If you want to find all async functions that return a `Result`, you need a nested chain of conditionals that is hard to read and easy to get wrong.

Tree-sitter includes a query language that solves this. You write a pattern in S-expression syntax, and tree-sitter finds all nodes in the tree that match it. Queries are declarative — you describe what you are looking for, not how to walk the tree. They are also compiled to an efficient bytecode representation, making them fast enough to run on every keystroke in an editor.

## S-Expression Pattern Syntax

Tree-sitter queries use S-expressions (the parenthesized notation familiar from Lisp) to describe tree patterns. The simplest query matches a node by type:

```scheme
(function_item)
```

This matches every `function_item` node in the tree. To capture the match for later extraction, add a `@capture_name`:

```scheme
(function_item) @function
```

To match children, nest them inside the parent:

```scheme
(function_item
    name: (identifier) @func_name)
```

This matches `function_item` nodes that have a child in the `name` field which is an `identifier` node. The identifier text is captured as `@func_name`. The `name:` prefix is the field name from the grammar — it means "the child that plays the `name` role," not just any child that happens to be an identifier.

Let's see this in action with real Rust code:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

fn find_function_names(source: &str) -> Vec<String> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // Compile the query
    let query = Query::new(&language, r#"
        (function_item
            name: (identifier) @func_name)
    "#).expect("Invalid query");

    // Find the index of our capture
    let capture_idx = query.capture_index_for_name("func_name").unwrap();

    // Execute the query
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut names = Vec::new();
    for m in matches {
        for capture in m.captures {
            if capture.index == capture_idx {
                let name = &source[capture.node.start_byte()..capture.node.end_byte()];
                names.push(name.to_string());
            }
        }
    }

    names
}

fn main() {
    let source = r#"
fn connect(addr: &str) -> Result<TcpStream> {
    TcpStream::connect(addr)
}

pub fn start_server() {
    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
}

fn validate(config: &Config) -> bool {
    config.is_valid()
}
"#;

    let names = find_function_names(source);
    println!("Functions: {:?}", names);
    // Output: Functions: ["connect", "start_server", "validate"]
}
```

The workflow is: compile a `Query` from a pattern string, create a `QueryCursor`, and call `matches()` or `captures()` against a tree node. The `QueryCursor` is reusable across multiple queries and trees.

## Predicates: Filtering Matches

Raw node-type matching is often too broad. Predicates let you add constraints to captures. The two most common predicates are `#eq?` (exact text match) and `#match?` (regex match):

```rust
use tree_sitter::{Parser, Query, QueryCursor};

fn find_specific_functions(source: &str) {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // Find functions whose name starts with "test_"
    let query = Query::new(&language, r#"
        (function_item
            name: (identifier) @func_name
            (#match? @func_name "^test_"))
    "#).expect("Invalid query");

    let capture_idx = query.capture_index_for_name("func_name").unwrap();

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    for m in matches {
        for capture in m.captures {
            if capture.index == capture_idx {
                let name = &source[capture.node.start_byte()..capture.node.end_byte()];
                let line = capture.node.start_position().row + 1;
                println!("Test function: {} (line {})", name, line);
            }
        }
    }
}

fn main() {
    let source = r#"
fn test_connection() {
    assert!(connect("localhost").is_ok());
}

fn helper_setup() {
    // Not a test function
}

fn test_validation() {
    assert!(validate(&config));
}

fn teardown() {
    // Not a test function
}
"#;

    find_specific_functions(source);
    // Output:
    // Test function: test_connection (line 2)
    // Test function: test_validation (line 10)
}
```

The `#match?` predicate takes a capture name and a regex pattern. Only matches where the capture's text matches the regex are returned. The `#eq?` predicate checks for exact equality:

```scheme
; Find functions named exactly "main"
(function_item
    name: (identifier) @func_name
    (#eq? @func_name "main"))

; Compare two captures against each other
; Find variables assigned to a function call with the same name
(let_declaration
    pattern: (identifier) @var_name
    value: (call_expression
        function: (identifier) @call_name)
    (#eq? @var_name @call_name))
```

::: python Coming from Python
Python's `ast` module does not have a query language — you walk the tree manually with `ast.walk()` or `ast.NodeVisitor`. Tree-sitter's query language is more like XPath for XML or CSS selectors for HTML: you declare the pattern, and the engine finds all matches. If you have used `cssselect` or `lxml.xpath()` in Python, the mental model is similar, just applied to syntax trees instead of document trees.
:::

## Multi-Pattern Queries

A single query string can contain multiple patterns. Each pattern is independent — tree-sitter finds matches for all of them in a single pass:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

fn extract_code_structure(source: &str) {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // Three patterns in one query
    let query = Query::new(&language, r#"
        (function_item
            name: (identifier) @func_name) @function

        (struct_item
            name: (type_identifier) @struct_name) @struct_def

        (impl_item
            type: (type_identifier) @impl_type) @impl_block
    "#).expect("Invalid query");

    let func_idx = query.capture_index_for_name("func_name").unwrap();
    let struct_idx = query.capture_index_for_name("struct_name").unwrap();
    let impl_idx = query.capture_index_for_name("impl_type").unwrap();

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    for m in matches {
        for capture in m.captures {
            let text = &source[capture.node.start_byte()..capture.node.end_byte()];
            let line = capture.node.start_position().row + 1;

            if capture.index == func_idx {
                println!("Function: {} (line {})", text, line);
            } else if capture.index == struct_idx {
                println!("Struct: {} (line {})", text, line);
            } else if capture.index == impl_idx {
                println!("Impl for: {} (line {})", text, line);
            }
        }
    }
}

fn main() {
    let source = r#"
struct Server {
    addr: String,
    port: u16,
}

impl Server {
    fn new(addr: String, port: u16) -> Self {
        Server { addr, port }
    }

    fn start(&self) -> Result<(), Error> {
        todo!()
    }
}

fn main() {
    let server = Server::new("0.0.0.0".to_string(), 8080);
    server.start().unwrap();
}
"#;

    extract_code_structure(source);
}
```

Multi-pattern queries are how you build file outline features: one query extracts functions, structs, enums, impl blocks, trait definitions, and use statements, all in one pass over the tree.

## Captures vs Matches

The `QueryCursor` offers two iteration modes: `matches()` and `captures()`. They differ in how they group results:

- **`matches()`** returns one `QueryMatch` per complete pattern match. Each match contains all the captures from that pattern instance. Use this when you need captures that belong together (e.g., a function name and its return type from the same function).

- **`captures()`** returns captures individually, one at a time, in order of their position in the source. Use this when you just want a flat list of all matched items regardless of which pattern they came from.

```rust
use tree_sitter::{Parser, Query, QueryCursor};

fn matches_vs_captures(source: &str) {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let query = Query::new(&language, r#"
        (function_item
            name: (identifier) @name
            parameters: (parameters) @params)
    "#).unwrap();

    let name_idx = query.capture_index_for_name("name").unwrap();
    let params_idx = query.capture_index_for_name("params").unwrap();

    // Using matches() — grouped by pattern match
    let mut cursor = QueryCursor::new();
    println!("=== Using matches() ===");
    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        let mut name = "";
        let mut params = "";
        for capture in m.captures {
            let text = &source[capture.node.start_byte()..capture.node.end_byte()];
            if capture.index == name_idx {
                name = text;
            } else if capture.index == params_idx {
                params = text;
            }
        }
        println!("Function {} with params {}", name, params);
    }

    // Using captures() — flat sequence
    let mut cursor = QueryCursor::new();
    println!("\n=== Using captures() ===");
    for (m, capture_idx) in cursor.captures(&query, tree.root_node(), source.as_bytes()) {
        let capture = m.captures[capture_idx];
        let text = &source[capture.node.start_byte()..capture.node.end_byte()];
        println!("Capture @{}: {}", if capture.index == name_idx { "name" } else { "params" }, text);
    }
}

fn main() {
    let source = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
fn greet(name: &str) { println!("{}", name); }
"#;
    matches_vs_captures(source);
}
```

For most agent tasks, `matches()` is the right choice because you need related captures together — the function name paired with its parameters, the struct name paired with its fields.

## Scoping Queries to Subtrees

You can run a query against any node, not just the root. This is useful when you want to search within a specific function or block:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

fn find_variables_in_function(source: &str, function_name: &str) {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // First, find the target function
    let func_query = Query::new(&language, r#"
        (function_item
            name: (identifier) @name
            (#eq? @name "TARGET_FUNC")) @func
    "#.replace("TARGET_FUNC", function_name).as_str()).unwrap();

    let func_idx = func_query.capture_index_for_name("func").unwrap();
    let mut cursor = QueryCursor::new();
    let func_matches: Vec<_> = cursor
        .matches(&func_query, tree.root_node(), source.as_bytes())
        .collect();

    if let Some(func_match) = func_matches.first() {
        let func_node = func_match.captures.iter()
            .find(|c| c.index == func_idx)
            .unwrap()
            .node;

        // Now query for let bindings WITHIN this function only
        let let_query = Query::new(&language, r#"
            (let_declaration
                pattern: (identifier) @var_name)
        "#).unwrap();

        let var_idx = let_query.capture_index_for_name("var_name").unwrap();
        let mut inner_cursor = QueryCursor::new();

        println!("Variables in {}:", function_name);
        for m in inner_cursor.matches(&let_query, func_node, source.as_bytes()) {
            for capture in m.captures {
                if capture.index == var_idx {
                    let name = &source[capture.node.start_byte()..capture.node.end_byte()];
                    println!("  let {}", name);
                }
            }
        }
    }
}

fn main() {
    let source = r#"
fn setup() {
    let config = load_config();
    let logger = Logger::new();
}

fn run(config: Config) {
    let server = Server::new(config);
    let handle = server.start();
    let result = handle.join();
}
"#;

    find_variables_in_function(source, "run");
    // Output:
    // Variables in run:
    //   let server
    //   let handle
    //   let result
}
```

::: wild In the Wild
Production coding agents use scoped queries heavily. When a user asks "what variables are used in this function?", the agent first locates the function node in the tree, then runs a query scoped to that function. This avoids returning variables from other functions in the same file — a common source of confusion when using flat text search. GitHub's code navigation uses this same technique to scope symbol resolution within function boundaries.
:::

## Key Takeaways

- Tree-sitter queries use S-expression syntax to declaratively match tree patterns — `(function_item name: (identifier) @name)` matches function definitions and captures their names
- Predicates like `#eq?` and `#match?` add text-level constraints to structural matches, combining the precision of structural search with the flexibility of text matching
- Multi-pattern queries extract multiple kinds of code elements (functions, structs, impls) in a single pass over the tree
- `matches()` returns grouped captures per pattern match while `captures()` returns a flat sequence — use `matches()` when you need related captures together
- Queries can be scoped to any subtree node, enabling precise extraction like "all variables within this specific function"
