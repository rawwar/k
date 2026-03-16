---
title: Language Detection
description: Detect programming languages from file extensions, shebangs, and content heuristics to select the correct parser and search behavior.
---

# Language Detection

> **What you'll learn:**
> - How to map file extensions to programming languages with a comprehensive lookup table
> - How to use shebang lines and content heuristics as fallback detection methods
> - How to select the correct tree-sitter grammar based on detected language

Every tree-sitter feature you have built so far -- AST navigation, semantic search, code-aware search -- depends on knowing which language a file is written in. You need the correct grammar to parse the file. A Rust grammar cannot parse Python, and a Python grammar produces garbage when fed TypeScript. This subchapter builds a language detection system that maps files to their languages using file extensions, shebang lines, and content analysis.

## Extension-Based Detection

The simplest and most reliable method is matching file extensions. This covers the vast majority of cases in real codebases:

```rust
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    C,
    Cpp,
    Java,
    Ruby,
    Shell,
    Markdown,
    Toml,
    Yaml,
    Json,
    Html,
    Css,
    Unknown,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "rs" => Language::Rust,
            "py" | "pyi" | "pyw" => Language::Python,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "mts" | "cts" => Language::TypeScript,
            "tsx" => Language::TypeScript,
            "jsx" => Language::JavaScript,
            "go" => Language::Go,
            "c" | "h" => Language::C,
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" => Language::Cpp,
            "java" => Language::Java,
            "rb" | "rake" | "gemspec" => Language::Ruby,
            "sh" | "bash" | "zsh" | "fish" => Language::Shell,
            "md" | "markdown" => Language::Markdown,
            "toml" => Language::Toml,
            "yml" | "yaml" => Language::Yaml,
            "json" | "jsonc" => Language::Json,
            "html" | "htm" => Language::Html,
            "css" | "scss" | "sass" => Language::Css,
            _ => Language::Unknown,
        }
    }
}
```

::: python Coming from Python
Python developers might reach for the `mimetypes` module or the `python-magic` library:
```python
import mimetypes

mime_type, _ = mimetypes.guess_type("main.rs")
# Returns None for .rs files -- mimetypes is oriented toward web content

# A manual mapping is more reliable for programming languages:
LANG_MAP = {
    ".py": "python",
    ".rs": "rust",
    ".js": "javascript",
    ".ts": "typescript",
}

def detect_language(path: str) -> str:
    from pathlib import Path
    ext = Path(path).suffix
    return LANG_MAP.get(ext, "unknown")
```
Both approaches use manual mappings because no standard library has a comprehensive programming language database. The Rust version uses an enum for type safety -- the compiler ensures you handle every language variant in match expressions.
:::

## Filename-Based Detection

Some files have no extension but are well-known by name. `Makefile`, `Dockerfile`, `Rakefile`, and similar files need special handling:

```rust
impl Language {
    /// Detect language from the filename (not just extension)
    pub fn from_filename(path: &Path) -> Self {
        // First try by filename (for extensionless files)
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        match filename {
            "Makefile" | "makefile" | "GNUmakefile" => return Language::Shell,
            "Dockerfile" => return Language::Shell,
            "Rakefile" | "Gemfile" => return Language::Ruby,
            "Cargo.toml" => return Language::Toml,
            "package.json" | "tsconfig.json" => return Language::Json,
            ".bashrc" | ".zshrc" | ".profile" | ".bash_profile" => {
                return Language::Shell;
            }
            _ => {}
        }

        // Fall back to extension-based detection
        Self::from_extension(path)
    }
}
```

## Shebang-Based Detection

When a file has no recognizable extension and is not a known filename, the shebang line (`#!` at the start of the file) can identify the interpreter:

```rust
use std::fs::File;
use std::io::{BufRead, BufReader};

impl Language {
    /// Detect language from the shebang line
    pub fn from_shebang(path: &Path) -> Option<Self> {
        let file = File::open(path).ok()?;
        let reader = BufReader::new(file);
        let first_line = reader.lines().next()?.ok()?;

        if !first_line.starts_with("#!") {
            return None;
        }

        let shebang = first_line.to_lowercase();

        if shebang.contains("python") {
            Some(Language::Python)
        } else if shebang.contains("node") {
            Some(Language::JavaScript)
        } else if shebang.contains("ruby") {
            Some(Language::Ruby)
        } else if shebang.contains("bash") || shebang.contains("/sh") {
            Some(Language::Shell)
        } else if shebang.contains("perl") {
            // We don't have a Perl variant, but this shows the pattern
            None
        } else {
            None
        }
    }
}
```

Common shebang patterns:

| Shebang | Language |
|---------|----------|
| `#!/usr/bin/env python3` | Python |
| `#!/usr/bin/python` | Python |
| `#!/usr/bin/env node` | JavaScript |
| `#!/bin/bash` | Shell |
| `#!/usr/bin/env ruby` | Ruby |
| `#!/bin/sh` | Shell |

## The Combined Detection Pipeline

Put all three methods together into a single detection function that tries each method in order of reliability:

```rust
impl Language {
    /// Detect language using all available methods.
    /// Order: filename -> extension -> shebang -> unknown
    pub fn detect(path: &Path) -> Self {
        // 1. Try filename-based detection (handles Makefile, Dockerfile, etc.)
        let by_filename = Self::from_filename(path);
        if by_filename != Language::Unknown {
            return by_filename;
        }

        // 2. Try extension-based detection
        let by_extension = Self::from_extension(path);
        if by_extension != Language::Unknown {
            return by_extension;
        }

        // 3. Try shebang detection (reads the file)
        if let Some(by_shebang) = Self::from_shebang(path) {
            return by_shebang;
        }

        Language::Unknown
    }

    /// Get the human-readable language name
    pub fn name(&self) -> &str {
        match self {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Go => "Go",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Java => "Java",
            Language::Ruby => "Ruby",
            Language::Shell => "Shell",
            Language::Markdown => "Markdown",
            Language::Toml => "TOML",
            Language::Yaml => "YAML",
            Language::Json => "JSON",
            Language::Html => "HTML",
            Language::Css => "CSS",
            Language::Unknown => "Unknown",
        }
    }
}
```

## Mapping Languages to Tree-sitter Grammars

Not every detected language has a tree-sitter grammar available. The grammar selection function bridges the gap between language detection and parsing:

```rust
use tree_sitter::Language as TsLanguage;

/// Get the tree-sitter language grammar for a detected language.
/// Returns None if no grammar is available.
pub fn get_tree_sitter_language(lang: &Language) -> Option<TsLanguage> {
    match lang {
        Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
        Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
        Language::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
        Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        // Add more as you add grammar crates to Cargo.toml:
        // Language::Go => Some(tree_sitter_go::LANGUAGE.into()),
        // Language::C => Some(tree_sitter_c::LANGUAGE.into()),
        _ => None,
    }
}

/// Parse a file with automatic language detection.
pub fn parse_file(path: &Path) -> Result<(tree_sitter::Tree, Language), String> {
    let lang = Language::detect(path);

    let ts_lang = get_tree_sitter_language(&lang)
        .ok_or_else(|| {
            format!("No tree-sitter grammar for {} ({})", lang.name(), path.display())
        })?;

    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang)
        .map_err(|e| format!("Failed to set language: {e}"))?;

    let tree = parser.parse(&source, None)
        .ok_or_else(|| format!("Failed to parse {}", path.display()))?;

    Ok((tree, lang))
}
```

## Language-Specific Node Types

Different tree-sitter grammars use different node type names for equivalent constructs. A function definition is `function_item` in Rust, `function_definition` in Python, and `function_declaration` in JavaScript. You need a mapping layer:

```rust
/// Get the node type names for function definitions in a given language
pub fn function_node_types(lang: &Language) -> &[&str] {
    match lang {
        Language::Rust => &["function_item"],
        Language::Python => &["function_definition"],
        Language::JavaScript | Language::TypeScript => &[
            "function_declaration",
            "method_definition",
            "arrow_function",
        ],
        Language::Go => &["function_declaration", "method_declaration"],
        Language::Java => &["method_declaration", "constructor_declaration"],
        Language::C | Language::Cpp => &["function_definition"],
        Language::Ruby => &["method", "singleton_method"],
        _ => &[],
    }
}

/// Get the node type names for class/struct definitions
pub fn type_definition_node_types(lang: &Language) -> &[&str] {
    match lang {
        Language::Rust => &["struct_item", "enum_item", "trait_item"],
        Language::Python => &["class_definition"],
        Language::JavaScript | Language::TypeScript => &["class_declaration"],
        Language::Go => &["type_declaration"],
        Language::Java => &["class_declaration", "interface_declaration"],
        Language::C | Language::Cpp => &["struct_specifier", "class_specifier"],
        Language::Ruby => &["class", "module"],
        _ => &[],
    }
}

/// Get the node type names for comment nodes
pub fn comment_node_types(lang: &Language) -> &[&str] {
    match lang {
        Language::Rust => &["line_comment", "block_comment"],
        Language::Python => &["comment"],
        Language::JavaScript | Language::TypeScript => &["comment"],
        Language::Go => &["comment"],
        Language::Java => &["line_comment", "block_comment"],
        Language::C | Language::Cpp => &["comment"],
        Language::Ruby => &["comment"],
        _ => &[],
    }
}
```

This mapping lets your code-aware search work across languages without hardcoding node types:

```rust
pub fn find_functions_any_language(
    source: &str,
    path: &Path,
) -> Result<Vec<String>, String> {
    let lang = Language::detect(path);
    let ts_lang = get_tree_sitter_language(&lang)
        .ok_or("No grammar available")?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).map_err(|e| e.to_string())?;

    let tree = parser.parse(source, None).ok_or("Parse failed")?;

    let func_types = function_node_types(&lang);
    let mut functions = Vec::new();

    collect_nodes_by_kind(source, tree.root_node(), func_types, &mut functions);
    Ok(functions)
}

fn collect_nodes_by_kind(
    source: &str,
    node: tree_sitter::Node,
    kinds: &[&str],
    results: &mut Vec<String>,
) {
    if kinds.contains(&node.kind()) {
        if let Some(name_node) = node.child_by_field_name("name") {
            results.push(source[name_node.byte_range()].to_string());
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes_by_kind(source, child, kinds, results);
    }
}
```

::: wild In the Wild
Production coding agents support dozens of languages. Claude Code supports every language that tree-sitter has a grammar for, which covers virtually all mainstream programming languages. The language detection is typically done once per file, and the result is cached. Some agents also use content-based heuristics for ambiguous cases -- for example, a `.h` file could be C or C++, and examining whether it contains `class` or `template` keywords helps disambiguate.
:::

## Testing Language Detection

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_extension_detection() {
        assert_eq!(Language::detect(Path::new("main.rs")), Language::Rust);
        assert_eq!(Language::detect(Path::new("app.py")), Language::Python);
        assert_eq!(Language::detect(Path::new("index.ts")), Language::TypeScript);
        assert_eq!(Language::detect(Path::new("script.js")), Language::JavaScript);
        assert_eq!(Language::detect(Path::new("main.go")), Language::Go);
    }

    #[test]
    fn test_filename_detection() {
        assert_eq!(Language::detect(Path::new("Makefile")), Language::Shell);
        assert_eq!(Language::detect(Path::new("Cargo.toml")), Language::Toml);
        assert_eq!(Language::detect(Path::new(".bashrc")), Language::Shell);
    }

    #[test]
    fn test_unknown_extension() {
        assert_eq!(Language::detect(Path::new("data.xyz")), Language::Unknown);
    }

    #[test]
    fn test_grammar_availability() {
        assert!(get_tree_sitter_language(&Language::Rust).is_some());
        assert!(get_tree_sitter_language(&Language::Python).is_some());
        assert!(get_tree_sitter_language(&Language::Unknown).is_none());
    }
}
```

## Key Takeaways

- File extension mapping is the primary language detection method and covers over 95% of files in typical codebases -- always try it first.
- Shebang detection (`#!/usr/bin/env python3`) handles extensionless scripts and should be tried as a fallback when extension matching fails.
- A language-to-grammar mapping function bridges your detection logic and tree-sitter, returning `None` for languages without available grammars so callers can gracefully fall back to text-only search.
- Node type names differ across grammars (Rust: `function_item`, Python: `function_definition`, JavaScript: `function_declaration`) -- abstract this with lookup functions so your search tools work across languages.
- Design the `Language` enum to be extensible: adding a new language means adding a variant, an extension mapping, and optionally a tree-sitter grammar binding.
