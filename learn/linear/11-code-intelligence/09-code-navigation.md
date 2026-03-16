---
title: Code Navigation
description: Building code navigation features — go-to-definition, find-references, and outline views — using tree-sitter queries and cross-file symbol indexing.
---

# Code Navigation

> **What you'll learn:**
> - How to implement go-to-definition by matching identifiers to their declaration sites using tree-sitter scope analysis
> - Building a find-references feature that locates all usages of a symbol across multiple files in a project
> - Creating file and project outline views that list functions, classes, and modules with their hierarchical structure

Code navigation is the set of features that let you jump around a codebase by meaning rather than by text. "Go to definition" takes you from a function call to the function's declaration. "Find references" shows every place a symbol is used. "Outline view" gives you a table of contents for a file — every function, struct, and impl block with its line number.

These features are what make IDEs powerful, and they are equally valuable for coding agents. When an agent sees a function call it does not recognize, it needs to find the definition to understand what the function does. When an agent refactors a function signature, it needs to find all callers to update them. Tree-sitter gives you the building blocks for implementing these features without a full language server.

## File Outline: The Foundation

The simplest navigation feature is the file outline — a structured list of every top-level symbol in a file. This is the foundation for everything else:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug)]
struct OutlineEntry {
    kind: String,
    name: String,
    line: usize,
    children: Vec<OutlineEntry>,
}

fn file_outline(source: &str) -> Vec<OutlineEntry> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // Query for top-level definitions
    let query = Query::new(&language, r#"
        (function_item
            name: (identifier) @func_name) @function

        (struct_item
            name: (type_identifier) @struct_name) @struct_def

        (enum_item
            name: (type_identifier) @enum_name) @enum_def

        (trait_item
            name: (type_identifier) @trait_name) @trait_def

        (impl_item
            type: (type_identifier) @impl_type) @impl_block

        (const_item
            name: (identifier) @const_name) @const_def

        (static_item
            name: (identifier) @static_name) @static_def

        (type_item
            name: (type_identifier) @type_name) @type_def
    "#).expect("Invalid query");

    let mut cursor = QueryCursor::new();
    let mut entries = Vec::new();

    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        let mut kind = String::new();
        let mut name = String::new();
        let mut line = 0;
        let mut impl_node = None;

        for capture in m.captures {
            let capture_name = &query.capture_names()[capture.index as usize];
            let text = &source[capture.node.start_byte()..capture.node.end_byte()];

            match capture_name.as_str() {
                "func_name" => { kind = "fn".into(); name = text.into(); line = capture.node.start_position().row + 1; }
                "struct_name" => { kind = "struct".into(); name = text.into(); line = capture.node.start_position().row + 1; }
                "enum_name" => { kind = "enum".into(); name = text.into(); line = capture.node.start_position().row + 1; }
                "trait_name" => { kind = "trait".into(); name = text.into(); line = capture.node.start_position().row + 1; }
                "impl_type" => { kind = "impl".into(); name = text.into(); line = capture.node.start_position().row + 1; }
                "const_name" => { kind = "const".into(); name = text.into(); line = capture.node.start_position().row + 1; }
                "static_name" => { kind = "static".into(); name = text.into(); line = capture.node.start_position().row + 1; }
                "type_name" => { kind = "type".into(); name = text.into(); line = capture.node.start_position().row + 1; }
                "impl_block" => { impl_node = Some(capture.node); }
                _ => {}
            }
        }

        if !name.is_empty() {
            let mut entry = OutlineEntry { kind, name, line, children: Vec::new() };

            // For impl blocks, extract methods as children
            if let Some(node) = impl_node {
                let method_query = Query::new(&language, r#"
                    (function_item
                        name: (identifier) @method_name)
                "#).unwrap();

                let method_idx = method_query.capture_index_for_name("method_name").unwrap();
                let mut method_cursor = QueryCursor::new();

                for method_match in method_cursor.matches(&method_query, node, source.as_bytes()) {
                    for c in method_match.captures {
                        if c.index == method_idx {
                            let method_name = &source[c.node.start_byte()..c.node.end_byte()];
                            entry.children.push(OutlineEntry {
                                kind: "fn".into(),
                                name: method_name.into(),
                                line: c.node.start_position().row + 1,
                                children: Vec::new(),
                            });
                        }
                    }
                }
            }

            entries.push(entry);
        }
    }

    entries
}

fn print_outline(entries: &[OutlineEntry], indent: usize) {
    for entry in entries {
        println!(
            "{:indent$}L{:>4}  {} {}",
            "", entry.line, entry.kind, entry.name,
            indent = indent
        );
        print_outline(&entry.children, indent + 4);
    }
}

fn main() {
    let source = r#"
use std::collections::HashMap;

const MAX_RETRIES: u32 = 3;

pub struct Server {
    addr: String,
    port: u16,
    connections: HashMap<String, Connection>,
}

impl Server {
    pub fn new(addr: String, port: u16) -> Self {
        Server { addr, port, connections: HashMap::new() }
    }

    pub fn start(&mut self) -> Result<(), Error> {
        todo!()
    }

    fn handle_connection(&mut self, conn: Connection) {
        todo!()
    }
}

pub trait Handler {
    fn handle(&self, request: &Request) -> Response;
}

pub enum Status {
    Active,
    Idle,
    Shutdown,
}

fn validate_config(config: &Config) -> bool {
    config.port > 0
}
"#;

    let outline = file_outline(source);
    println!("File outline:");
    print_outline(&outline, 0);
}
```

The outline gives the agent a quick structural overview of any file. Instead of reading 500 lines of source code, the agent can scan a 20-line outline and decide which parts are relevant to the task.

## Go-to-Definition Within a File

Go-to-definition is more complex. Given a symbol name and a position, you need to find where that symbol was defined. Within a single file, this means finding the declaration that is in scope at the reference point:

```rust
use tree_sitter::{Parser, Query, QueryCursor, Node};

#[derive(Debug)]
struct Definition {
    name: String,
    kind: String,
    line: usize,
    column: usize,
}

fn find_definition_in_file(source: &str, symbol: &str) -> Vec<Definition> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // Search for all definition-like nodes with the given name
    let query = Query::new(&language, &format!(r#"
        (function_item
            name: (identifier) @name
            (#eq? @name "{}")) @def

        (struct_item
            name: (type_identifier) @name
            (#eq? @name "{}")) @def

        (enum_item
            name: (type_identifier) @name
            (#eq? @name "{}")) @def

        (const_item
            name: (identifier) @name
            (#eq? @name "{}")) @def

        (let_declaration
            pattern: (identifier) @name
            (#eq? @name "{}")) @def

        (parameter
            pattern: (identifier) @name
            (#eq? @name "{}")) @def
    "#, symbol, symbol, symbol, symbol, symbol, symbol)).expect("Invalid query");

    let name_idx = query.capture_index_for_name("name").unwrap();
    let def_idx = query.capture_index_for_name("def").unwrap();

    let mut cursor = QueryCursor::new();
    let mut definitions = Vec::new();

    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        let mut name = String::new();
        let mut kind = String::new();
        let mut line = 0;
        let mut column = 0;

        for capture in m.captures {
            if capture.index == name_idx {
                name = source[capture.node.start_byte()..capture.node.end_byte()].to_string();
                line = capture.node.start_position().row + 1;
                column = capture.node.start_position().column;
            }
            if capture.index == def_idx {
                kind = capture.node.kind().to_string();
            }
        }

        if !name.is_empty() {
            definitions.push(Definition { name, kind, line, column });
        }
    }

    definitions
}

fn main() {
    let source = r#"
const BUFFER_SIZE: usize = 4096;

struct Config {
    host: String,
    port: u16,
}

fn process(config: &Config) {
    let buffer = vec![0u8; BUFFER_SIZE];
    let host = &config.host;
    println!("Connecting to {}", host);
}
"#;

    // Find where "config" is defined
    let defs = find_definition_in_file(source, "config");
    println!("Definitions of 'config':");
    for d in &defs {
        println!("  {} at line {}:{} ({})", d.name, d.line, d.column, d.kind);
    }

    // Find where "BUFFER_SIZE" is defined
    let defs = find_definition_in_file(source, "BUFFER_SIZE");
    println!("\nDefinitions of 'BUFFER_SIZE':");
    for d in &defs {
        println!("  {} at line {}:{} ({})", d.name, d.line, d.column, d.kind);
    }
}
```

::: python Coming from Python
Python's `ast` module provides similar capability through `ast.walk()`, but scoping in Python is simpler (function-level, not block-level). The `jedi` library provides full go-to-definition for Python by analyzing imports, class hierarchies, and dynamic dispatch. In the Rust world, `rust-analyzer` provides the same level of precision. The tree-sitter approach shown here is a useful middle ground — less precise than a full language server but much faster to implement and works across languages.
:::

## Find References Across Files

Finding all usages of a symbol across a project combines file discovery with per-file analysis. The strategy is: use ripgrep to find files containing the symbol text, then use tree-sitter to verify each match is an actual identifier reference (not a comment or string):

```rust
use std::process::Command;
use tree_sitter::{Parser, Query, QueryCursor};
use std::path::Path;

#[derive(Debug)]
struct Reference {
    file: String,
    line: usize,
    column: usize,
    context: String,
}

fn find_references(root: &str, symbol: &str) -> Vec<Reference> {
    // Step 1: Use ripgrep to find candidate files
    let output = Command::new("rg")
        .args([
            "--files-with-matches",
            "--glob", "*.rs",
            symbol,
            root,
        ])
        .output()
        .expect("Failed to run rg");

    let files = String::from_utf8_lossy(&output.stdout);
    let mut references = Vec::new();

    // Step 2: Parse each file and find identifier nodes matching the symbol
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();

    let query = Query::new(&language, r#"
        (identifier) @ident
        (type_identifier) @type_ident
    "#).unwrap();

    let ident_idx = query.capture_index_for_name("ident").unwrap();
    let type_ident_idx = query.capture_index_for_name("type_ident").unwrap();

    for file_path in files.lines() {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let tree = match parser.parse(&source, None) {
            Some(t) => t,
            None => continue,
        };

        let mut cursor = QueryCursor::new();
        for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
            for capture in m.captures {
                if capture.index != ident_idx && capture.index != type_ident_idx {
                    continue;
                }

                let text = &source[capture.node.start_byte()..capture.node.end_byte()];
                if text != symbol {
                    continue;
                }

                // Extract the full line for context
                let line_start = source[..capture.node.start_byte()]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let line_end = source[capture.node.end_byte()..]
                    .find('\n')
                    .map(|i| capture.node.end_byte() + i)
                    .unwrap_or(source.len());
                let context = source[line_start..line_end].trim().to_string();

                references.push(Reference {
                    file: file_path.to_string(),
                    line: capture.node.start_position().row + 1,
                    column: capture.node.start_position().column,
                    context,
                });
            }
        }
    }

    references
}

fn main() {
    let refs = find_references("src/", "Config");
    println!("Found {} references to 'Config':", refs.len());
    for r in &refs {
        println!("  {}:{}:{}", r.file, r.line, r.column);
        println!("    {}", r.context);
    }
}
```

This approach has a crucial advantage over pure text search: by using tree-sitter to find `identifier` and `type_identifier` nodes, you automatically exclude matches in comments and string literals. The identifier `Config` in `// Config is used for settings` is a comment node child, not an identifier node, so it is filtered out.

::: tip In the Wild
GitHub's code navigation uses tree-sitter for cross-file reference finding. When you click "Find all references" in the GitHub UI, it uses tree-sitter queries to find identifier nodes matching the symbol name across the repository. This is a heuristic — it can produce false positives when two unrelated symbols share the same name in different modules — but it works remarkably well for most codebases and is far more accurate than plain text search.
:::

## Building a Navigation Index

For repeated navigation queries, parsing every file from scratch on each query is wasteful. A navigation index pre-processes the project once and stores symbol locations for fast lookup:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct SymbolLocation {
    file: PathBuf,
    line: usize,
    kind: SymbolKind,
}

#[derive(Debug, Clone)]
enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Const,
    TypeAlias,
    Method { impl_type: String },
}

struct NavigationIndex {
    /// Maps symbol name -> list of definition locations
    definitions: HashMap<String, Vec<SymbolLocation>>,
    /// Maps symbol name -> list of reference locations
    references: HashMap<String, Vec<SymbolLocation>>,
}

impl NavigationIndex {
    fn new() -> Self {
        NavigationIndex {
            definitions: HashMap::new(),
            references: HashMap::new(),
        }
    }

    fn go_to_definition(&self, symbol: &str) -> &[SymbolLocation] {
        self.definitions.get(symbol).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn find_references(&self, symbol: &str) -> &[SymbolLocation] {
        self.references.get(symbol).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn summary(&self) {
        println!(
            "Index: {} unique definitions, {} unique referenced symbols",
            self.definitions.len(),
            self.references.len()
        );
    }
}
```

The index can be built once when the agent starts working on a project and incrementally updated when files change. This is the same approach that IDE extensions use — build the index on project open, then update it on file save.

## Key Takeaways

- File outlines built with multi-pattern tree-sitter queries provide agents with a compact structural summary of any source file, listing functions, structs, enums, traits, and impl blocks with their line numbers
- Go-to-definition within a file uses tree-sitter queries to match symbol names against declaration-like nodes (function definitions, struct definitions, let bindings, parameters)
- Cross-file reference finding combines ripgrep for fast candidate discovery with tree-sitter for structural validation — filtering out matches in comments and strings
- A navigation index pre-computes symbol locations for fast repeated queries, following the same pattern used by IDE extensions
- These tree-sitter-based navigation features are heuristic (they cannot resolve overloaded names or trait methods) but cover the common cases accurately and work across languages
