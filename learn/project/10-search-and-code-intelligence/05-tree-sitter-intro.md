---
title: Tree Sitter Intro
description: Introduction to tree-sitter as an incremental parsing framework and how it enables language-aware code intelligence features.
---

# Tree Sitter Intro

> **What you'll learn:**
> - How tree-sitter generates concrete syntax trees that preserve all source text information
> - How incremental parsing allows re-parsing only changed portions of a file
> - How to set up tree-sitter in Rust with language grammars for common programming languages

Up to this point, all your search tools treat code as plain text. The grep tool finds lines matching a regex, and the glob tool finds files matching a name pattern. But code is not plain text -- it has structure. A function definition, a struct declaration, an import statement: these are *syntactic constructs* that regex can only approximate. Tree-sitter gives your agent the ability to understand code *as code*, opening the door to semantic search, intelligent navigation, and structure-aware analysis.

## What Is Tree-sitter?

Tree-sitter is an incremental parsing framework created by Max Brunsfeld at GitHub. It takes source code as input and produces a concrete syntax tree (CST) -- a tree data structure that represents every token and construct in the source. Unlike abstract syntax trees (ASTs) that discard whitespace, comments, and formatting, tree-sitter's CST preserves *everything*. Every byte of the original source can be recovered from the tree.

Here is what tree-sitter does with a simple Rust function:

```rust
fn greet(name: &str) {
    println!("Hello, {name}!");
}
```

The tree-sitter parse tree for this code looks like:

```
(source_file
  (function_item
    name: (identifier)           // "greet"
    parameters: (parameters
      (parameter
        pattern: (identifier)    // "name"
        type: (reference_type
          (primitive_type))))    // "&str"
    body: (block
      (expression_statement
        (macro_invocation
          macro: (identifier)    // "println"
          (token_tree ...))))))
```

Every node has a type (`function_item`, `identifier`, `parameters`), a start and end position in the source, and children that represent nested constructs. This is the foundation for everything in the next several subchapters.

## Why Not Just Use Regex?

Regex works well for simple pattern matching, but it breaks down for structural queries. Consider these challenges:

**Finding function definitions.** A regex like `fn\s+\w+` works for simple cases, but it also matches `fn` inside comments, strings, and doc examples. Tree-sitter knows the difference because comments and strings are distinct node types in the syntax tree.

**Matching balanced delimiters.** Regex fundamentally cannot match balanced braces -- `{...{...}...}` requires counting. Tree-sitter represents the entire block as a tree node with the correct nesting.

**Cross-language support.** A regex for Python function definitions (`def\s+\w+`) is completely different from Rust (`fn\s+\w+`) or JavaScript (`function\s+\w+`). Tree-sitter uses grammar files for each language, so you write *one* query pattern that works across languages: "find all function definitions."

::: python Coming from Python
Python's `ast` module provides AST parsing for Python code:
```python
import ast

source = """
def greet(name: str):
    print(f"Hello, {name}!")
"""

tree = ast.parse(source)
for node in ast.walk(tree):
    if isinstance(node, ast.FunctionDef):
        print(f"Function: {node.name} at line {node.lineno}")
```
Tree-sitter serves a similar purpose but works for *any* language with a grammar file, not just Python. It also preserves all source text (Python's `ast` module discards comments and whitespace), and it supports incremental re-parsing when code changes.
:::

## Setting Up Tree-sitter in Rust

Add the tree-sitter crate and at least one language grammar to your `Cargo.toml`:

```toml
[dependencies]
tree-sitter = "0.24"
tree-sitter-rust = "0.23"
tree-sitter-python = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-typescript = "0.23"
```

Each `tree-sitter-<language>` crate provides a compiled grammar for that language. The version numbers may vary -- check crates.io for the latest compatible versions.

Now let's parse some code:

```rust
use tree_sitter::{Parser, Language};

fn main() {
    // Create a parser and set the language
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser
        .set_language(&language.into())
        .expect("Failed to set language");

    // Parse some Rust source code
    let source = r#"
use std::io;

fn main() {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    println!("You said: {input}");
}

fn helper(x: i32) -> i32 {
    x * 2
}
"#;

    let tree = parser.parse(source, None)
        .expect("Failed to parse");

    let root = tree.root_node();
    println!("Root node: {} ({} children)", root.kind(), root.child_count());

    // Walk the top-level children
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let start = child.start_position();
        println!(
            "  {} at line {}:{} - \"{}\"",
            child.kind(),
            start.row + 1,
            start.column,
            &source[child.byte_range()].lines().next().unwrap_or("")
        );
    }
}
```

Output:
```
Root node: source_file (3 children)
  use_declaration at line 2:0 - "use std::io;"
  function_item at line 4:0 - "fn main() {"
  function_item at line 10:0 - "fn helper(x: i32) -> i32 {"
```

Notice how tree-sitter identifies the high-level constructs: a `use_declaration` and two `function_item` nodes. Each node knows its exact position in the source, making it easy to extract the original text.

## Understanding the Parse Tree

Every tree-sitter tree has these properties:

**Nodes have types.** Each node has a `kind()` that describes what syntactic construct it represents. For Rust, these include `function_item`, `struct_item`, `enum_item`, `impl_item`, `let_declaration`, and many more.

**Nodes have positions.** `start_position()` and `end_position()` return `(row, column)` coordinates. `start_byte()` and `end_byte()` return byte offsets. You can use either to slice back into the original source text.

**Nodes have named children.** In addition to positional children, many nodes have *named* fields. A `function_item` has a `name` field (the function name), a `parameters` field, and a `body` field:

```rust
use tree_sitter::Parser;

fn main() {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into()).unwrap();

    let source = "fn process(data: &[u8]) -> Result<(), Error> { Ok(()) }";
    let tree = parser.parse(source, None).unwrap();
    let root = tree.root_node();

    // Get the first function_item
    let func = root.child(0).unwrap();
    assert_eq!(func.kind(), "function_item");

    // Access named fields
    if let Some(name_node) = func.child_by_field_name("name") {
        println!("Function name: {}", &source[name_node.byte_range()]);
    }

    if let Some(params_node) = func.child_by_field_name("parameters") {
        println!("Parameters: {}", &source[params_node.byte_range()]);
    }

    if let Some(return_node) = func.child_by_field_name("return_type") {
        println!("Return type: {}", &source[return_node.byte_range()]);
    }
}
```

Output:
```
Function name: process
Parameters: (data: &[u8])
Return type: Result<(), Error>
```

## Incremental Parsing

Tree-sitter's signature feature is incremental parsing. When the source code changes (say, the user edits a function), you do not need to re-parse the entire file. You tell tree-sitter what changed, and it re-parses only the affected portions:

```rust
use tree_sitter::{InputEdit, Parser, Point};

fn main() {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into()).unwrap();

    // Initial parse
    let mut source = String::from("fn main() { println!(\"hello\"); }");
    let mut tree = parser.parse(&source, None).unwrap();

    // Simulate an edit: change "hello" to "world"
    let edit_start = source.find("hello").unwrap();
    let edit_end = edit_start + "hello".len();
    let new_text = "world";

    source.replace_range(edit_start..edit_end, new_text);

    // Tell tree-sitter about the edit
    tree.edit(&InputEdit {
        start_byte: edit_start,
        old_end_byte: edit_end,
        new_end_byte: edit_start + new_text.len(),
        start_position: Point::new(0, edit_start),
        old_end_position: Point::new(0, edit_end),
        new_end_position: Point::new(0, edit_start + new_text.len()),
    });

    // Re-parse incrementally (pass the old tree)
    let new_tree = parser.parse(&source, Some(&tree)).unwrap();

    // Verify the new tree reflects the change
    let root = new_tree.root_node();
    let func = root.child(0).unwrap();
    let body_text = &source[func.byte_range()];
    assert!(body_text.contains("world"));
    println!("Updated: {body_text}");
}
```

For a coding agent, incremental parsing matters when the agent makes multiple edits to the same file in a conversation. Rather than re-parsing the entire file after each edit, you update the tree incrementally, keeping the parse results warm.

## Error Recovery

Real-world code is often syntactically invalid -- the user might be in the middle of editing, or the agent might have made a partial change. Tree-sitter handles this gracefully through error recovery:

```rust
use tree_sitter::Parser;

fn main() {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into()).unwrap();

    // Parse code with a syntax error (missing closing brace)
    let source = "fn main() { println!(\"hello\"); ";
    let tree = parser.parse(source, None).unwrap();
    let root = tree.root_node();

    println!("Has errors: {}", root.has_error());
    println!("Tree: {}", root.to_sexp());

    // Despite the error, tree-sitter still identifies the function
    let func = root.child(0).unwrap();
    if let Some(name) = func.child_by_field_name("name") {
        println!("Found function: {}", &source[name.byte_range()]);
    }
}
```

The tree still has a `function_item` node with the correct name, even though the code has a syntax error. Error nodes are marked in the tree with `ERROR` or `MISSING` node types, but the surrounding valid structure is preserved. This robustness is essential for a coding agent that frequently works with in-progress code.

::: wild In the Wild
GitHub uses tree-sitter for syntax highlighting across all its supported languages. The incremental parsing and error recovery features are why your code gets highlighted correctly on GitHub even when you are viewing a partial diff or a file with syntax errors. Coding agents like Cursor and Continue use tree-sitter for similar purposes: understanding code structure to provide better completions and edits.
:::

## Key Takeaways

- Tree-sitter produces concrete syntax trees that preserve every byte of the original source, enabling precise extraction of code constructs with their exact positions.
- Unlike regex-based analysis, tree-sitter understands code structure: it distinguishes function definitions from function calls, comments from code, and string contents from identifiers.
- Incremental parsing lets you re-parse only the changed portions of a file, which matters when the agent makes multiple edits to the same file in a conversation.
- Tree-sitter's error recovery means it produces useful parse trees even for syntactically invalid code, which is common during active development.
- Each language requires its own grammar crate (`tree-sitter-rust`, `tree-sitter-python`, etc.) -- you will add language detection logic in a later subchapter to select the right grammar automatically.
