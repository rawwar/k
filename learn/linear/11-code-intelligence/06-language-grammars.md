---
title: Language Grammars
description: How tree-sitter grammars are defined, the ecosystem of available language grammars, and strategies for supporting multiple programming languages in an agent.
---

# Language Grammars

> **What you'll learn:**
> - How tree-sitter grammars are defined using JavaScript DSL rules that generate C parsers for each language
> - The ecosystem of maintained grammars for popular languages and how to evaluate grammar quality and completeness
> - Strategies for loading and managing multiple language grammars at runtime based on file extensions and content detection

So far, every example has used `tree_sitter_rust::LANGUAGE` — a single grammar for a single language. Real codebases are polyglot. A typical web project has Rust or Go for the backend, TypeScript for the frontend, SQL for queries, TOML or YAML for configuration, Dockerfiles for deployment, and Markdown for documentation. A useful coding agent needs to parse all of them.

This subchapter explains how tree-sitter grammars work, what the ecosystem looks like, and how to build a multi-language parser that selects the right grammar based on the file it encounters.

## How Grammars Are Defined

A tree-sitter grammar is defined in a `grammar.js` file using a JavaScript DSL. This file describes the syntax rules of the language as a set of production rules. Here is a simplified excerpt from a grammar showing how function definitions might be described:

```javascript
// Simplified excerpt from a tree-sitter grammar.js
module.exports = grammar({
  name: 'example',

  rules: {
    // The entry point
    source_file: $ => repeat($._definition),

    _definition: $ => choice(
      $.function_definition,
      $.struct_definition,
    ),

    function_definition: $ => seq(
      optional($.visibility_modifier),
      'fn',
      field('name', $.identifier),
      field('parameters', $.parameter_list),
      optional(seq('->', field('return_type', $._type))),
      field('body', $.block),
    ),

    // ... more rules
  }
});
```

The `field()` function assigns names to children — this is what makes `node.child_by_field_name("name")` work in the Rust API. The `seq()` function defines a sequence of elements, `choice()` defines alternatives, `optional()` marks optional elements, and `repeat()` handles repetition.

When you run `tree-sitter generate`, this JavaScript file is compiled into a C parser. The C code is what actually runs at parse time — the JavaScript is only used during grammar development. This is why tree-sitter parsers are fast: the generated C code is a state machine that processes one byte at a time with no interpretation overhead.

::: python Coming from Python
Python's `ast` module has its grammar hardcoded in the CPython source as a PEG grammar (in `Grammar/python.gram`). You cannot extend it or use it for other languages. Tree-sitter's grammar-per-language approach is more like ANTLR's `.g4` grammar files if you have used those, but with the critical difference that tree-sitter grammars generate incremental parsers with error recovery, while ANTLR generates batch parsers.
:::

## The Grammar Ecosystem

Tree-sitter has a rich ecosystem of community-maintained grammars. As of 2025, there are grammars for over 150 languages. The most commonly used ones for a coding agent:

| Language | Crate | Maturity |
|----------|-------|----------|
| Rust | `tree-sitter-rust` | Excellent — maintained by the tree-sitter team |
| Python | `tree-sitter-python` | Excellent — covers Python 3.12+ syntax |
| TypeScript/TSX | `tree-sitter-typescript` | Excellent — separate parsers for TS and TSX |
| JavaScript/JSX | `tree-sitter-javascript` | Excellent — foundation for the TS grammar |
| Go | `tree-sitter-go` | Excellent — maintained by the tree-sitter team |
| C / C++ | `tree-sitter-c` / `tree-sitter-cpp` | Excellent |
| Java | `tree-sitter-java` | Good |
| Ruby | `tree-sitter-ruby` | Good |
| JSON | `tree-sitter-json` | Excellent — useful for config files |
| TOML | `tree-sitter-toml` | Good — useful for Cargo.toml |
| YAML | `tree-sitter-yaml` | Good |
| Markdown | `tree-sitter-md` | Good — used for documentation parsing |
| Bash | `tree-sitter-bash` | Good |
| SQL | `tree-sitter-sql` | Moderate — SQL dialects vary significantly |

### Evaluating Grammar Quality

Not all grammars are equal. Before relying on a grammar for agent tasks, check:

**Parse accuracy.** Does it correctly parse the language's latest syntax? Test it against files from popular open-source projects in that language. A grammar that fails on modern syntax features (like Rust's `async`/`await` or Python's pattern matching) will produce `ERROR` nodes in the tree.

**Node type coverage.** Does the grammar define specific node types for the constructs you need? A grammar that lumps all declarations into a generic `declaration` node is less useful than one that distinguishes `function_definition`, `class_definition`, and `variable_declaration`.

**Field names.** Does the grammar use `field()` annotations so you can use `child_by_field_name()`? Older grammars sometimes lack field names, forcing you to rely on child indices.

**Maintenance status.** Is the grammar actively maintained? Check the repository for recent commits and open issues. An unmaintained grammar will not keep up with language evolution.

## Building a Multi-Language Parser

A coding agent needs to select the right grammar for each file. The standard approach is mapping file extensions to languages:

```rust
use tree_sitter::{Language, Parser};
use std::collections::HashMap;
use std::path::Path;

struct MultiLanguageParser {
    languages: HashMap<String, Language>,
}

impl MultiLanguageParser {
    fn new() -> Self {
        let mut languages = HashMap::new();

        // Register languages by file extension
        let rust_lang: Language = tree_sitter_rust::LANGUAGE.into();
        languages.insert("rs".to_string(), rust_lang);

        // In a full agent, you would add more languages:
        // languages.insert("py".to_string(), tree_sitter_python::LANGUAGE.into());
        // languages.insert("js".to_string(), tree_sitter_javascript::LANGUAGE.into());
        // languages.insert("ts".to_string(), tree_sitter_typescript::language_typescript());
        // languages.insert("go".to_string(), tree_sitter_go::LANGUAGE.into());
        // languages.insert("json".to_string(), tree_sitter_json::LANGUAGE.into());
        // languages.insert("toml".to_string(), tree_sitter_toml::LANGUAGE.into());

        MultiLanguageParser { languages }
    }

    fn language_for_file(&self, path: &Path) -> Option<&Language> {
        let ext = path.extension()?.to_str()?;
        self.languages.get(ext)
    }

    fn parse_file(&self, path: &Path, source: &str) -> Option<tree_sitter::Tree> {
        let language = self.language_for_file(path)?;
        let mut parser = Parser::new();
        parser.set_language(language).ok()?;
        parser.parse(source, None)
    }
}

fn main() {
    let multi_parser = MultiLanguageParser::new();

    let rust_source = r#"
fn main() {
    println!("Hello from Rust!");
}
"#;

    let path = Path::new("src/main.rs");
    if let Some(tree) = multi_parser.parse_file(path, rust_source) {
        let root = tree.root_node();
        println!("Parsed {} — root has {} children", path.display(), root.named_child_count());
    } else {
        println!("No grammar available for {}", path.display());
    }
}
```

### Handling Ambiguous Extensions

Some extensions are ambiguous. `.h` files could be C or C++. `.jsx` could be JavaScript with JSX or React. The common strategies are:

**Extension priority.** Map each extension to exactly one grammar and make it configurable.

**Content sniffing.** Look at the first few lines for shebang lines (`#!/usr/bin/env python3`), file headers, or language-specific patterns.

**Configuration files.** Check for `Cargo.toml` (Rust), `package.json` (JavaScript/TypeScript), `go.mod` (Go), or `pyproject.toml` (Python) in parent directories to determine the project's primary language.

```rust
use std::path::Path;

fn detect_language(path: &Path, first_line: &str) -> &'static str {
    // Check extension first
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext {
            "rs" => return "rust",
            "py" | "pyi" => return "python",
            "js" | "mjs" | "cjs" => return "javascript",
            "ts" | "mts" | "cts" => return "typescript",
            "tsx" => return "tsx",
            "jsx" => return "jsx",
            "go" => return "go",
            "rb" => return "ruby",
            "java" => return "java",
            "c" => return "c",
            "cpp" | "cc" | "cxx" => return "cpp",
            "h" => {
                // Ambiguous — check content for C++ features
                if first_line.contains("class ") || first_line.contains("namespace ") {
                    return "cpp";
                }
                return "c";
            }
            "json" => return "json",
            "toml" => return "toml",
            "yaml" | "yml" => return "yaml",
            "md" | "markdown" => return "markdown",
            "sh" | "bash" => return "bash",
            "sql" => return "sql",
            _ => {}
        }
    }

    // Check shebang line
    if first_line.starts_with("#!") {
        if first_line.contains("python") {
            return "python";
        }
        if first_line.contains("node") {
            return "javascript";
        }
        if first_line.contains("bash") || first_line.contains("/sh") {
            return "bash";
        }
        if first_line.contains("ruby") {
            return "ruby";
        }
    }

    // Check for known filenames
    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
        match filename {
            "Dockerfile" => return "dockerfile",
            "Makefile" | "makefile" => return "make",
            "Cargo.toml" => return "toml",
            "package.json" | "tsconfig.json" => return "json",
            _ => {}
        }
    }

    "unknown"
}
```

::: wild In the Wild
Editors like Neovim and Zed maintain their own language detection tables that map extensions, filenames, and content patterns to tree-sitter grammars. GitHub uses a library called Linguist that combines file extensions, content heuristics, and even statistical analysis of token frequencies to determine file languages. For a coding agent, extension-based detection with a handful of fallback rules covers the vast majority of files you will encounter.
:::

## Grammar Compilation and Binary Size

Each tree-sitter grammar adds code to your binary. A Rust grammar is around 300KB of compiled C code, and each additional language adds a similar amount. If you bundle 15 grammars, expect your binary to grow by 4-5MB. This is usually acceptable for a CLI tool, but worth knowing.

The grammars are compiled at build time by the grammar crate's `build.rs` script, which invokes a C compiler. This means your build machine needs a C compiler toolchain (`cc` on most systems). The compiled parsers are linked into your Rust binary as static code — there is no runtime dependency on external files.

If binary size is a concern, you have two options:

**Compile only the grammars you need.** Only add grammar crates to `Cargo.toml` for languages your agent will support.

**Load grammars dynamically.** Tree-sitter supports loading compiled grammars from shared libraries (`.so` / `.dylib` / `.dll`) at runtime using `Language::from_ptr()`. This is more complex but lets you ship grammar packs separately from the main binary.

For most coding agents, the static approach is simpler and sufficient. Compile in grammars for the 8-10 most common languages and return graceful fallbacks (plain text handling) for everything else.

## Key Takeaways

- Tree-sitter grammars are defined in JavaScript DSL files and compiled to C parsers — the JavaScript is only used at generation time, not at parse time
- The ecosystem includes grammars for over 150 languages with varying quality — evaluate parse accuracy, node type coverage, field name support, and maintenance status before depending on a grammar
- A multi-language parser maps file extensions to grammars and falls back to content detection for ambiguous cases
- Each grammar adds roughly 300KB to binary size — compile in the languages your agent supports and handle unknown languages gracefully
- Extension-based language detection with shebang and filename fallbacks covers the vast majority of files in real-world codebases
