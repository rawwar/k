---
title: Symbol Resolution
description: Resolving symbols across files and modules — import path following, module resolution algorithms, and building a cross-file symbol index.
---

# Symbol Resolution

> **What you'll learn:**
> - How import/require/use statements create cross-file symbol dependencies and how to follow these paths programmatically
> - Module resolution algorithms for different languages: Node.js require resolution, Python import system, Rust module tree
> - Building and maintaining a cross-file symbol index that maps symbol names to their definition locations and types

The code navigation features from the previous subchapter work within a single file or across files by name matching. But real code is organized into modules, and symbols flow between files through import statements. When your agent sees `use crate::config::DatabaseConfig` in a Rust file, it needs to follow that path to find the actual struct definition. When it sees `from utils.parser import parse_json` in Python, it needs to resolve `utils.parser` to a file path and `parse_json` to a symbol within that file.

Symbol resolution is the bridge between per-file tree-sitter analysis and cross-file understanding. It is more complex than single-file navigation because each language has its own module system with its own rules.

## Extracting Import Statements

The first step in symbol resolution is parsing import statements. Each language has its own syntax, but tree-sitter handles them all:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug)]
struct ImportInfo {
    path: String,
    symbols: Vec<String>,
    line: usize,
    is_wildcard: bool,
}

fn extract_rust_imports(source: &str) -> Vec<ImportInfo> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // Match use declarations
    let query = Query::new(&language, r#"
        (use_declaration
            argument: (use_as_clause
                path: (scoped_identifier) @path
                alias: (identifier) @alias))

        (use_declaration
            argument: (scoped_identifier) @simple_path)

        (use_declaration
            argument: (use_wildcard
                (scoped_identifier) @wildcard_path))

        (use_declaration
            argument: (scoped_use_list
                path: (scoped_identifier) @list_path
                list: (use_list) @list))
    "#).expect("Invalid query");

    let mut imports = Vec::new();
    let mut cursor = QueryCursor::new();

    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        for capture in m.captures {
            let text = &source[capture.node.start_byte()..capture.node.end_byte()];
            let capture_name = &query.capture_names()[capture.index as usize];
            let line = capture.node.start_position().row + 1;

            match capture_name.as_str() {
                "simple_path" => {
                    // use std::collections::HashMap;
                    let parts: Vec<&str> = text.rsplitn(2, "::").collect();
                    let symbol = parts[0].to_string();
                    let path = if parts.len() > 1 { parts[1].to_string() } else { text.to_string() };
                    imports.push(ImportInfo {
                        path,
                        symbols: vec![symbol],
                        line,
                        is_wildcard: false,
                    });
                }
                "wildcard_path" => {
                    // use std::io::*;
                    imports.push(ImportInfo {
                        path: text.to_string(),
                        symbols: vec![],
                        line,
                        is_wildcard: true,
                    });
                }
                _ => {}
            }
        }
    }

    imports
}

fn main() {
    let source = r#"
use std::collections::HashMap;
use std::io::{self, Read, Write};
use crate::config::DatabaseConfig;
use super::utils::parse_json;
"#;

    let imports = extract_rust_imports(source);
    for imp in &imports {
        println!("Line {}: use {}", imp.line, imp.path);
        if imp.is_wildcard {
            println!("  (wildcard import)");
        } else {
            println!("  symbols: {:?}", imp.symbols);
        }
    }
}
```

::: python Coming from Python
Python imports have different syntax but the same concept:
```python
import ast

source = """
from pathlib import Path
from utils.parser import parse_json, validate
import os
from . import helpers
"""

tree = ast.parse(source)
for node in ast.walk(tree):
    if isinstance(node, ast.ImportFrom):
        module = node.module or ""
        names = [alias.name for alias in node.names]
        print(f"from {module} import {names}")
    elif isinstance(node, ast.Import):
        names = [alias.name for alias in node.names]
        print(f"import {names}")
```
Tree-sitter gives you the same extraction for Python, JavaScript, Go, or any language — the query patterns differ but the approach is identical.
:::

## Rust Module Resolution

Rust has a particularly well-defined module system. Resolving `use crate::config::DatabaseConfig` follows these rules:

1. `crate` refers to the current crate's root module (usually `src/lib.rs` or `src/main.rs`).
2. Each `::` segment maps to either a `mod` declaration within a file or a directory/file in the filesystem.
3. `config` could be `src/config.rs` or `src/config/mod.rs`.
4. `DatabaseConfig` is a symbol (struct, enum, function, etc.) defined within that module.

```rust
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct ResolvedImport {
    file_path: PathBuf,
    symbol_name: String,
}

fn resolve_rust_import(
    crate_root: &Path,
    import_path: &str,
    symbol: &str,
) -> Option<ResolvedImport> {
    // Split the path into segments
    let segments: Vec<&str> = import_path.split("::").collect();

    if segments.is_empty() {
        return None;
    }

    // Determine the starting directory
    let start_dir = match segments[0] {
        "crate" => crate_root.join("src"),
        "self" => {
            // Relative to current module — needs current file context
            crate_root.join("src")
        }
        "super" => {
            // Parent module — needs current file context
            crate_root.join("src")
        }
        _ => {
            // External crate — we cannot resolve without dependency info
            return None;
        }
    };

    // Walk the remaining segments to find the file
    let mut current_path = start_dir;
    for (i, segment) in segments[1..].iter().enumerate() {
        let is_last_path_segment = i == segments.len() - 2;

        if is_last_path_segment {
            // This might be the file containing the symbol
            // Check for segment.rs
            let file_path = current_path.join(format!("{}.rs", segment));
            if file_path.exists() {
                return Some(ResolvedImport {
                    file_path,
                    symbol_name: symbol.to_string(),
                });
            }

            // Check for segment/mod.rs
            let mod_path = current_path.join(segment).join("mod.rs");
            if mod_path.exists() {
                return Some(ResolvedImport {
                    file_path: mod_path,
                    symbol_name: symbol.to_string(),
                });
            }
        }

        // Descend into the directory
        current_path = current_path.join(segment);
    }

    None
}

fn main() {
    let crate_root = Path::new(".");

    if let Some(resolved) = resolve_rust_import(
        crate_root,
        "crate::config",
        "DatabaseConfig",
    ) {
        println!("Resolved to: {} -> {}", resolved.file_path.display(), resolved.symbol_name);
    } else {
        println!("Could not resolve import");
    }
}
```

## Cross-Language Resolution Patterns

Each language has its own resolution algorithm, but they share common patterns:

**Rust (`use` statements):**
- `crate::module::Symbol` maps to `src/module.rs` or `src/module/mod.rs`
- `super::sibling::Symbol` goes up one module level
- External crates are resolved through `Cargo.toml` dependencies

**Python (`import` / `from ... import`):**
- `from package.module import Symbol` maps to `package/module.py`
- Relative imports (`.module`) are relative to the current package
- `sys.path` determines the search order for top-level packages

**JavaScript/TypeScript (`import` / `require`):**
- `import { Symbol } from './module'` resolves relative paths, adding `.js`/`.ts`/`.tsx`
- `import { Symbol } from 'package'` looks in `node_modules/`
- TypeScript path aliases in `tsconfig.json` add remapping rules

**Go (`import`):**
- `import "github.com/user/repo/pkg"` maps to `$GOPATH` or module cache
- Internal packages use relative paths from the module root

A practical agent does not need to implement every language's resolution algorithm perfectly. A useful heuristic approach handles 90% of cases:

```rust
use std::path::{Path, PathBuf};

fn heuristic_resolve(
    project_root: &Path,
    import_path: &str,
    language: &str,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // Convert module path separators to file path separators
    let file_stem = match language {
        "rust" => import_path.replace("::", "/"),
        "python" => import_path.replace(".", "/"),
        "javascript" | "typescript" => {
            // JS imports often include the extension or are relative
            import_path.trim_start_matches("./").to_string()
        }
        "go" => {
            // Go imports are full paths, take the last segment
            import_path.rsplit('/').next().unwrap_or(import_path).to_string()
        }
        _ => import_path.replace("::", "/").replace(".", "/"),
    };

    // Generate candidate file paths
    let src_dir = project_root.join("src");
    let lib_dir = project_root.join("lib");

    let extensions = match language {
        "rust" => vec!["rs"],
        "python" => vec!["py"],
        "javascript" => vec!["js", "jsx", "mjs"],
        "typescript" => vec!["ts", "tsx"],
        "go" => vec!["go"],
        _ => vec!["rs", "py", "js", "ts"],
    };

    for ext in &extensions {
        // Direct file match: src/module.ext
        candidates.push(src_dir.join(format!("{}.{}", file_stem, ext)));
        candidates.push(lib_dir.join(format!("{}.{}", file_stem, ext)));
        candidates.push(project_root.join(format!("{}.{}", file_stem, ext)));

        // Directory with index/mod file: src/module/mod.ext or src/module/index.ext
        let index_name = match language {
            "rust" => "mod",
            _ => "index",
        };
        candidates.push(src_dir.join(&file_stem).join(format!("{}.{}", index_name, ext)));
    }

    // Filter to existing files
    candidates.into_iter().filter(|p| p.exists()).collect()
}
```

::: tip In the Wild
Production language servers like rust-analyzer and TypeScript's tsserver implement full module resolution including path remapping, conditional exports, and workspace configurations. A coding agent does not need this level of completeness. Claude Code relies on the LLM's understanding of import paths combined with file search to navigate between modules. When more precision is needed, the agent can invoke a language server for definitive resolution. The heuristic approach handles the common cases quickly and saves the expensive LSP call for ambiguous situations.
:::

## Building a Cross-File Symbol Index

A symbol index maps symbol names to their definition locations across the entire project. Building one combines the import extraction, module resolution, and per-file outline techniques from previous subchapters:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct SymbolDef {
    file: PathBuf,
    line: usize,
    kind: String,
    is_exported: bool,
}

struct SymbolIndex {
    /// Maps fully qualified name -> definition
    definitions: HashMap<String, Vec<SymbolDef>>,
    /// Maps short name -> list of fully qualified names
    short_names: HashMap<String, Vec<String>>,
}

impl SymbolIndex {
    fn new() -> Self {
        SymbolIndex {
            definitions: HashMap::new(),
            short_names: HashMap::new(),
        }
    }

    fn add_symbol(
        &mut self,
        short_name: &str,
        qualified_name: &str,
        def: SymbolDef,
    ) {
        self.definitions
            .entry(qualified_name.to_string())
            .or_default()
            .push(def);

        self.short_names
            .entry(short_name.to_string())
            .or_default()
            .push(qualified_name.to_string());
    }

    /// Look up by short name — may return multiple candidates
    fn lookup(&self, name: &str) -> Vec<&SymbolDef> {
        let mut results = Vec::new();

        // Try exact qualified name first
        if let Some(defs) = self.definitions.get(name) {
            results.extend(defs.iter());
            return results;
        }

        // Fall back to short name lookup
        if let Some(qualified_names) = self.short_names.get(name) {
            for qn in qualified_names {
                if let Some(defs) = self.definitions.get(qn) {
                    results.extend(defs.iter());
                }
            }
        }

        results
    }

    fn stats(&self) -> (usize, usize) {
        (self.definitions.len(), self.short_names.len())
    }
}

fn main() {
    let mut index = SymbolIndex::new();

    // Simulate indexing results
    index.add_symbol(
        "DatabaseConfig",
        "crate::config::DatabaseConfig",
        SymbolDef {
            file: PathBuf::from("src/config.rs"),
            line: 15,
            kind: "struct".into(),
            is_exported: true,
        },
    );

    index.add_symbol(
        "connect",
        "crate::db::connect",
        SymbolDef {
            file: PathBuf::from("src/db.rs"),
            line: 42,
            kind: "fn".into(),
            is_exported: true,
        },
    );

    index.add_symbol(
        "connect",
        "crate::network::connect",
        SymbolDef {
            file: PathBuf::from("src/network.rs"),
            line: 10,
            kind: "fn".into(),
            is_exported: true,
        },
    );

    // Lookup by short name — returns all matches
    let results = index.lookup("connect");
    println!("Definitions of 'connect':");
    for def in results {
        println!("  {}:{} ({}, exported: {})", def.file.display(), def.line, def.kind, def.is_exported);
    }

    // Lookup by qualified name — returns exact match
    let results = index.lookup("crate::config::DatabaseConfig");
    println!("\nDefinition of 'crate::config::DatabaseConfig':");
    for def in results {
        println!("  {}:{} ({})", def.file.display(), def.line, def.kind);
    }

    let (defs, shorts) = index.stats();
    println!("\nIndex stats: {} qualified names, {} short names", defs, shorts);
}
```

The index supports both short name lookups ("connect" returns both `crate::db::connect` and `crate::network::connect`) and qualified name lookups (exact match). An agent can use the import context to disambiguate: if the current file has `use crate::db::connect`, then `connect` resolves to the database version.

## Key Takeaways

- Import statements create cross-file symbol dependencies — tree-sitter can extract import paths and imported symbols from any language using language-specific queries
- Each language has its own module resolution algorithm, but they all map import paths to file system paths through a combination of directory structure and configuration files
- A heuristic resolution approach that maps import path segments to directories and files handles 90% of common cases without implementing full language-specific resolution
- A cross-file symbol index maps symbol names to definition locations, supporting both qualified lookups (exact) and short name lookups (may return multiple candidates)
- The practical agent approach is heuristic resolution for speed, with fallback to LSP for cases where multiple candidates exist or the heuristic fails
