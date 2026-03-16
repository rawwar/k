---
title: Semantic Search
description: Go beyond text matching with semantic search that understands code structure to find definitions, references, and related constructs.
---

# Semantic Search

> **What you'll learn:**
> - How to use tree-sitter queries to find function definitions, struct declarations, and impl blocks
> - How to build a symbol index that maps names to their definition locations across a codebase
> - How to resolve references by connecting usage sites to their corresponding definitions

Grep finds text patterns. Semantic search finds *code constructs*. When the LLM asks "where is the `Config` struct defined?" or "what methods does `Database` have?", a text search might return dozens of results -- every line that mentions the word "Config." A semantic search returns precisely the struct definition, its fields, and its impl blocks. This precision is what separates a competent coding agent from a frustrating one.

## From Text Search to Semantic Search

Consider searching for "Config" in a typical Rust project. A grep search returns:

```
src/config.rs:5:  pub struct Config {
src/config.rs:12: impl Config {
src/config.rs:13:     pub fn new() -> Config {
src/main.rs:3:    use crate::config::Config;
src/main.rs:8:    let config = Config::new();
src/server.rs:1:  use crate::config::Config;
src/server.rs:15: fn start(config: &Config) {
src/server.rs:22: // TODO: add Config validation
```

That is eight results. A semantic search for "Config definition" returns exactly one:

```
src/config.rs:5: struct Config { name: String, port: u16 }
```

And a semantic search for "Config methods" returns:

```
src/config.rs:13: fn new() -> Config
src/config.rs:18: fn validate(&self) -> Result<(), Error>
```

This targeted precision saves context window tokens and gives the LLM exactly the information it needs to make edits.

## Building a Symbol Index

A symbol index maps identifiers (function names, type names, field names) to their definition locations. Building it requires parsing every file in the project with tree-sitter and extracting definition nodes:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SymbolDefinition {
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: usize,
    pub signature: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    TypeAlias,
    Module,
}

pub struct SymbolIndex {
    /// Maps symbol names to their definitions (a name can have multiple definitions)
    symbols: HashMap<String, Vec<SymbolDefinition>>,
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }

    pub fn add(&mut self, symbol: SymbolDefinition) {
        self.symbols
            .entry(symbol.name.clone())
            .or_default()
            .push(symbol);
    }

    /// Find all definitions for a given name
    pub fn lookup(&self, name: &str) -> Vec<&SymbolDefinition> {
        self.symbols
            .get(name)
            .map(|defs| defs.iter().collect())
            .unwrap_or_default()
    }

    /// Find definitions matching a prefix (for fuzzy/autocomplete search)
    pub fn search_prefix(&self, prefix: &str) -> Vec<&SymbolDefinition> {
        self.symbols
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .flat_map(|(_, defs)| defs.iter())
            .collect()
    }

    /// Find all symbols of a specific kind
    pub fn find_by_kind(&self, kind: &SymbolKind) -> Vec<&SymbolDefinition> {
        self.symbols
            .values()
            .flatten()
            .filter(|s| &s.kind == kind)
            .collect()
    }

    pub fn total_symbols(&self) -> usize {
        self.symbols.values().map(|v| v.len()).sum()
    }
}
```

## Extracting Symbols with Tree-sitter

Now let's write the code that populates the symbol index by parsing Rust files:

```rust
use tree_sitter::{Parser, Query, QueryCursor, Language};
use std::path::Path;

const RUST_SYMBOLS_QUERY: &str = r#"
    (function_item
        name: (identifier) @func_name) @func_def

    (struct_item
        name: (type_identifier) @struct_name) @struct_def

    (enum_item
        name: (type_identifier) @enum_name) @enum_def

    (trait_item
        name: (type_identifier) @trait_name) @trait_def

    (impl_item
        type: (type_identifier) @impl_type) @impl_def

    (const_item
        name: (identifier) @const_name) @const_def

    (type_item
        name: (type_identifier) @type_alias_name) @type_alias_def
"#;

pub fn index_rust_file(
    source: &str,
    file_path: &Path,
    index: &mut SymbolIndex,
) -> Result<(), String> {
    let language: Language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|e| e.to_string())?;

    let tree = parser.parse(source, None)
        .ok_or("Failed to parse file")?;

    let query = Query::new(&language, RUST_SYMBOLS_QUERY)
        .map_err(|e| format!("Query error: {e}"))?;

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    for m in matches {
        let mut name = None;
        let mut kind = None;
        let mut def_node = None;

        for capture in m.captures {
            let capture_name = &query.capture_names()[capture.index as usize];
            let text = &source[capture.node.byte_range()];

            match capture_name.as_str() {
                "func_name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Function);
                }
                "struct_name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Struct);
                }
                "enum_name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Enum);
                }
                "trait_name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Trait);
                }
                "impl_type" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Impl);
                }
                "const_name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Const);
                }
                "type_alias_name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::TypeAlias);
                }
                _ if capture_name.ends_with("_def") => {
                    def_node = Some(capture.node);
                }
                _ => {}
            }
        }

        if let (Some(name_str), Some(kind_val), Some(node)) = (name, kind, def_node) {
            // Extract signature (first line of the definition)
            let sig_text = &source[node.byte_range()];
            let signature = sig_text
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();

            // Find parent context (e.g., the impl block this function belongs to)
            let parent = find_parent_type(source, node);

            index.add(SymbolDefinition {
                name: name_str,
                kind: kind_val,
                file: file_path.to_path_buf(),
                line: node.start_position().row + 1,
                signature,
                parent,
            });
        }
    }

    Ok(())
}

fn find_parent_type(source: &str, node: tree_sitter::Node) -> Option<String> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "impl_item" {
            if let Some(type_node) = parent.child_by_field_name("type") {
                return Some(source[type_node.byte_range()].to_string());
            }
        }
        current = parent;
    }
    None
}
```

::: tip Coming from Python
Python developers often use `jedi` or `rope` for semantic code analysis, or the built-in `ast` module for simpler cases:
```python
import ast

class SymbolCollector(ast.NodeVisitor):
    def __init__(self):
        self.symbols = {}

    def visit_FunctionDef(self, node):
        self.symbols[node.name] = ("function", node.lineno)
        self.generic_visit(node)

    def visit_ClassDef(self, node):
        self.symbols[node.name] = ("class", node.lineno)
        self.generic_visit(node)
```
The tree-sitter approach scales across languages: the same indexing infrastructure works for Rust, Python, TypeScript, and any language with a tree-sitter grammar. The query language is different for each grammar, but the indexing logic stays the same.
:::

## Indexing a Full Project

To index an entire project, combine the file filter from the previous subchapter with the symbol extractor:

```rust
use std::path::Path;

pub fn index_project(root: &Path) -> Result<SymbolIndex, String> {
    let mut index = SymbolIndex::new();

    let filter = FileFilter::new(root).include("**/*.rs".to_string());

    for file_path in filter.walk() {
        let source = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Err(e) = index_rust_file(&source, &file_path, &mut index) {
            eprintln!("Warning: failed to index {}: {e}", file_path.display());
        }
    }

    println!(
        "Indexed {} symbols from project at {}",
        index.total_symbols(),
        root.display()
    );

    Ok(index)
}
```

For a medium-sized Rust project (100 files, 10,000 lines), indexing takes under a second. For larger projects, you could cache the index and use tree-sitter's incremental parsing to update only changed files.

## Querying the Symbol Index

With the index built, you can now answer semantic queries:

```rust
pub fn format_symbol_lookup(index: &SymbolIndex, name: &str) -> String {
    let definitions = index.lookup(name);

    if definitions.is_empty() {
        return format!("No definitions found for '{name}'.");
    }

    let mut output = format!("Found {} definition(s) for '{name}':\n\n", definitions.len());

    for def in definitions {
        let parent_str = def
            .parent
            .as_ref()
            .map(|p| format!(" (in impl {p})"))
            .unwrap_or_default();

        output.push_str(&format!(
            "  {:?} at {}:{}{}\n  Signature: {}\n\n",
            def.kind,
            def.file.display(),
            def.line,
            parent_str,
            def.signature
        ));
    }

    output
}

pub fn format_kind_search(index: &SymbolIndex, kind: &SymbolKind) -> String {
    let symbols = index.find_by_kind(kind);

    if symbols.is_empty() {
        return format!("No {kind:?} definitions found.");
    }

    let mut output = format!("Found {} {:?} definition(s):\n\n", symbols.len(), kind);

    for sym in symbols {
        output.push_str(&format!(
            "  {} at {}:{} - {}\n",
            sym.name,
            sym.file.display(),
            sym.line,
            sym.signature
        ));
    }

    output
}
```

::: info In the Wild
Production coding agents typically maintain a symbol index that is updated incrementally as files change. Claude Code achieves something similar by combining grep results with structural understanding. When you ask it to find a function definition, it uses targeted grep patterns plus context to distinguish definitions from call sites. A full symbol index is more precise but requires upfront parsing time -- the right trade-off depends on the project size and how often the agent needs structural queries.
:::

## Handling Name Collisions

Real codebases have many symbols with the same name. There might be a `new` function in every struct's impl block, a `Config` type in multiple modules, and helper functions with common names like `parse` or `validate`. The symbol index handles this by storing multiple definitions per name, but the search tool should disambiguate:

```rust
pub fn disambiguate_symbol(
    index: &SymbolIndex,
    name: &str,
    kind_hint: Option<&SymbolKind>,
    file_hint: Option<&Path>,
) -> Vec<&SymbolDefinition> {
    let mut results = index.lookup(name);

    // Filter by kind if a hint is provided
    if let Some(kind) = kind_hint {
        results.retain(|s| &s.kind == kind);
    }

    // Boost results in the hinted file
    if let Some(file) = file_hint {
        results.sort_by(|a, b| {
            let a_match = a.file == file;
            let b_match = b.file == file;
            b_match.cmp(&a_match)
        });
    }

    results
}
```

## Key Takeaways

- Semantic search finds code *constructs* (definitions, declarations, impl blocks) rather than text patterns, giving the LLM precise answers to structural questions.
- A symbol index maps names to definition locations across the entire project, enabling instant lookups that would otherwise require grepping every file.
- Tree-sitter queries with multiple captures (`@func_name`, `@func_def`) let you extract both the name and the full context of each definition in a single pass.
- Name collisions are common in real codebases -- disambiguate with kind hints (function vs. struct), file hints (prefer the file the user is currently editing), and parent context (which impl block).
- Index the project once on startup and update incrementally as files change to keep semantic search fast without re-parsing the entire codebase.
