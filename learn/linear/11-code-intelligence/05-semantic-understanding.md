---
title: Semantic Understanding
description: Moving from syntactic parsing to semantic understanding — resolving references, understanding scope chains, and extracting type information from syntax trees.
---

# Semantic Understanding

> **What you'll learn:**
> - The gap between syntactic structure (what tree-sitter provides) and semantic meaning (what the code actually does at runtime)
> - How to build basic scope analysis by walking the tree to track variable declarations and their visibility ranges
> - Techniques for extracting type hints, function signatures, and interface definitions that inform agent decision-making

Tree-sitter gives you the grammar of code — it tells you that a node is a `function_item`, that it has parameters, that its body contains `let_declaration` nodes. But grammar alone does not tell you what the code *means*. When you see `let x = foo();`, tree-sitter tells you this is a let declaration with a call expression on the right side. It does not tell you what type `x` has, what function `foo` resolves to, or whether this code even compiles.

The gap between syntax and semantics is real, and a coding agent operates in that gap constantly. This subchapter explores what you can extract from syntax trees that approaches semantic understanding — and where you hit the limits and need richer tools.

## Syntax vs Semantics

Consider this Rust code:

```rust
fn process(items: Vec<String>) -> usize {
    let count = items.len();
    let filtered: Vec<&String> = items.iter().filter(|s| !s.is_empty()).collect();
    filtered.len()
}
```

Tree-sitter tells you the **syntactic** facts:
- There is a function named `process` with one parameter
- The parameter is named `items` with type annotation `Vec<String>`
- The function body contains two `let` declarations and a return expression
- The return type annotation is `usize`

But tree-sitter cannot tell you the **semantic** facts:
- `items.len()` resolves to `Vec::len`, which returns `usize`
- `count` has type `usize` (inferred by the compiler, not written in source)
- `.filter()` returns an `Iterator`, and `.collect()` infers its target type from the annotation on `filtered`
- `filtered.len()` matches the return type `usize`, so this code is type-correct

Full semantic analysis requires a type checker — for Rust, that means `rust-analyzer` or the Rust compiler itself. But there is a useful middle ground: extracting the semantic information that *is* written in the source code, even if it is not complete.

## Extracting Type Annotations

Many languages include explicit type annotations that tree-sitter can extract. These annotations are not full type resolution, but they provide valuable signal:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug)]
struct TypedBinding {
    name: String,
    type_annotation: Option<String>,
    line: usize,
}

fn extract_typed_bindings(source: &str) -> Vec<TypedBinding> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // Query for let declarations with optional type annotations
    let query = Query::new(&language, r#"
        (let_declaration
            pattern: (identifier) @var_name
            type: (_) @var_type)

        (let_declaration
            pattern: (identifier) @untyped_name)
    "#).expect("Invalid query");

    let name_idx = query.capture_index_for_name("var_name").unwrap();
    let type_idx = query.capture_index_for_name("var_type").unwrap();
    let untyped_idx = query.capture_index_for_name("untyped_name").unwrap();

    let mut bindings = Vec::new();
    let mut cursor = QueryCursor::new();

    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        let mut name = String::new();
        let mut type_ann = None;
        let mut line = 0;

        for capture in m.captures {
            let text = &source[capture.node.start_byte()..capture.node.end_byte()];
            if capture.index == name_idx || capture.index == untyped_idx {
                name = text.to_string();
                line = capture.node.start_position().row + 1;
            }
            if capture.index == type_idx {
                type_ann = Some(text.to_string());
            }
        }

        if !name.is_empty() {
            bindings.push(TypedBinding {
                name,
                type_annotation: type_ann,
                line,
            });
        }
    }

    // Deduplicate: a binding with type annotation matches both patterns
    bindings.dedup_by(|a, b| a.name == b.name && a.line == b.line);
    bindings
}

fn main() {
    let source = r#"
fn example() {
    let name: String = "Alice".to_string();
    let age = 30;
    let scores: Vec<i32> = vec![90, 85, 92];
    let average = scores.iter().sum::<i32>() / scores.len() as i32;
}
"#;

    for binding in extract_typed_bindings(source) {
        println!(
            "Line {}: {} — type: {}",
            binding.line,
            binding.name,
            binding.type_annotation.as_deref().unwrap_or("(inferred)")
        );
    }
}
```

This is already useful for an agent. When asked "what type is `scores`?", the agent can look at the type annotation without needing a full type checker. For variables without annotations (like `age` and `average`), the agent knows it needs to either infer the type from context or defer to an LSP query.

## Building Scope Analysis

Scope analysis answers the question: "Given this position in the code, which variables are visible?" This requires understanding which declarations are in scope at any given point. While full scope analysis requires tracking closures, lifetimes, and module visibility, a basic version that tracks local variables within function bodies is straightforward:

```rust
use tree_sitter::{Parser, Node};
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct ScopeEntry {
    name: String,
    declared_at_line: usize,
    declared_at_byte: usize,
    scope_end_byte: usize,
}

fn analyze_scope(source: &str, target_byte: usize) -> Vec<ScopeEntry> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    let tree = parser.parse(source, None).unwrap();

    let mut visible = Vec::new();
    collect_scope_entries(tree.root_node(), source, target_byte, &mut visible);
    visible
}

fn collect_scope_entries(
    node: Node,
    source: &str,
    target_byte: usize,
    entries: &mut Vec<ScopeEntry>,
) {
    // Only descend into nodes that contain our target position
    if target_byte < node.start_byte() || target_byte > node.end_byte() {
        return;
    }

    // Check if this node is a let declaration before our target position
    if node.kind() == "let_declaration" && node.start_byte() < target_byte {
        if let Some(pattern) = node.child_by_field_name("pattern") {
            if pattern.kind() == "identifier" {
                let name = &source[pattern.start_byte()..pattern.end_byte()];
                // The scope extends to the end of the parent block
                let scope_end = node.parent()
                    .map(|p| p.end_byte())
                    .unwrap_or(node.end_byte());
                entries.push(ScopeEntry {
                    name: name.to_string(),
                    declared_at_line: pattern.start_position().row + 1,
                    declared_at_byte: pattern.start_byte(),
                    scope_end_byte: scope_end,
                });
            }
        }
    }

    // Check function parameters
    if node.kind() == "parameters" {
        for i in 0..node.named_child_count() {
            if let Some(param) = node.named_child(i) {
                if let Some(pattern) = param.child_by_field_name("pattern") {
                    let name = &source[pattern.start_byte()..pattern.end_byte()];
                    let scope_end = node.parent()
                        .and_then(|func| func.child_by_field_name("body"))
                        .map(|body| body.end_byte())
                        .unwrap_or(node.end_byte());
                    entries.push(ScopeEntry {
                        name: name.to_string(),
                        declared_at_line: pattern.start_position().row + 1,
                        declared_at_byte: pattern.start_byte(),
                        scope_end_byte: scope_end,
                    });
                }
            }
        }
    }

    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_scope_entries(child, source, target_byte, entries);
        }
    }
}

fn main() {
    let source = r#"fn process(items: Vec<String>, limit: usize) {
    let count = items.len();
    let filtered = items.into_iter().take(limit);
    for item in filtered {
        let trimmed = item.trim();
        println!("{}", trimmed);
    }
    let result = count;
}"#;

    // What's visible at the println line? (approximately byte 170)
    let byte_position = source.find("println!").unwrap();
    let visible = analyze_scope(source, byte_position);

    println!("Variables visible at println position:");
    for entry in &visible {
        println!("  {} (declared at line {})", entry.name, entry.declared_at_line);
    }
}
```

::: python Coming from Python
In Python, scope analysis is simpler because Python has function-level scoping (not block-level like Rust). A variable declared anywhere in a Python function is visible throughout the function. Rust's block scoping means a `let` inside an `if` block is not visible outside it. This makes Rust scope analysis more granular and more useful — the agent can determine precisely which variables are accessible at a given edit location.
:::

## Extracting Function Signatures

Function signatures are a goldmine for agent decision-making. When the agent needs to call a function, it needs to know the parameter names, types, and return type. Tree-sitter extracts this information reliably:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug)]
struct FunctionSignature {
    name: String,
    is_public: bool,
    is_async: bool,
    parameters: Vec<(String, String)>, // (name, type)
    return_type: Option<String>,
    line: usize,
}

fn extract_signatures(source: &str) -> Vec<FunctionSignature> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let query = Query::new(&language, r#"
        (function_item) @func
    "#).unwrap();

    let func_idx = query.capture_index_for_name("func").unwrap();
    let mut cursor = QueryCursor::new();
    let mut signatures = Vec::new();

    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        for capture in m.captures {
            if capture.index != func_idx {
                continue;
            }
            let node = capture.node;

            let name = node.child_by_field_name("name")
                .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                .unwrap_or_default();

            let is_public = node.child_by_field_name("visibility_modifier").is_some();

            // Check for async keyword by looking at the node's text
            let func_text = &source[node.start_byte()..node.end_byte()];
            let is_async = func_text.starts_with("async ");

            let return_type = node.child_by_field_name("return_type")
                .map(|n| source[n.start_byte()..n.end_byte()].to_string());

            // Extract parameters
            let mut parameters = Vec::new();
            if let Some(params_node) = node.child_by_field_name("parameters") {
                for i in 0..params_node.named_child_count() {
                    if let Some(param) = params_node.named_child(i) {
                        let param_name = param.child_by_field_name("pattern")
                            .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                            .unwrap_or_default();
                        let param_type = param.child_by_field_name("type")
                            .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                            .unwrap_or_else(|| "?".to_string());
                        if !param_name.is_empty() {
                            parameters.push((param_name, param_type));
                        }
                    }
                }
            }

            signatures.push(FunctionSignature {
                name,
                is_public,
                is_async,
                parameters,
                return_type,
                line: node.start_position().row + 1,
            });
        }
    }

    signatures
}

fn main() {
    let source = r#"
pub async fn fetch_data(url: &str, timeout: Duration) -> Result<Response, Error> {
    let client = Client::new();
    client.get(url).timeout(timeout).send().await
}

fn parse_response(body: &[u8]) -> Vec<Record> {
    serde_json::from_slice(body).unwrap_or_default()
}
"#;

    for sig in extract_signatures(source) {
        let params: Vec<String> = sig.parameters.iter()
            .map(|(name, ty)| format!("{}: {}", name, ty))
            .collect();
        println!(
            "{}{}{}fn {}({}) -> {}",
            if sig.is_public { "pub " } else { "" },
            if sig.is_async { "async " } else { "" },
            "",
            sig.name,
            params.join(", "),
            sig.return_type.as_deref().unwrap_or("()")
        );
    }
}
```

This extracted signature information is exactly what an agent needs when generating code that calls these functions. Instead of reading the entire function body (which costs context window tokens), the agent gets a compact summary of the API.

::: wild In the Wild
Claude Code and similar agents often include function signatures in their context when making edits. Rather than pasting entire files into the prompt, they extract the signatures of relevant functions and include those as compact references. Tree-sitter makes this extraction fast and reliable across languages — the same query patterns work for Rust `fn`, Python `def`, and TypeScript `function` declarations with only minor grammar-specific adjustments.
:::

## The Limits of Syntactic Semantics

Tree-sitter-based analysis hits clear walls:

**Type inference.** When a variable has no type annotation, you cannot determine its type from syntax alone. Rust relies heavily on type inference, so many variables have no written type.

**Trait resolution.** Calling `x.len()` requires knowing the type of `x` to determine which `len()` method is called. This requires full type analysis.

**Macro expansion.** Rust macros generate code at compile time. Tree-sitter sees the macro invocation (`vec![1, 2, 3]`), not the generated code. The macro body's structure is opaque.

**Cross-file resolution.** `use crate::config::Config` tells you the import path, but resolving it to the actual struct definition requires understanding the module tree and file layout.

For these capabilities, you need either a Language Server Protocol (LSP) server (covered in a later subchapter) or the language's own compiler toolchain. The pragmatic approach for an agent is to extract what tree-sitter can provide quickly and cheaply, then fall back to LSP queries for the deeper semantic questions.

## Key Takeaways

- Tree-sitter provides syntactic structure but not semantic meaning — it cannot resolve types, traits, or cross-file references without additional tooling
- Explicit type annotations, function signatures, and parameter lists can be extracted reliably from syntax trees, providing valuable signal for agent decision-making
- Basic scope analysis — tracking which variables are visible at a given position — can be built by walking the tree and tracking declaration positions relative to block boundaries
- Function signature extraction gives agents compact API summaries without the token cost of reading entire function bodies
- The practical approach is to extract what syntax provides cheaply, then use LSP or compiler queries for deeper semantic analysis like type inference and trait resolution
