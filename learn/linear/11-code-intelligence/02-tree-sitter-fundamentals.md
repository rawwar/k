---
title: Tree Sitter Fundamentals
description: The design and architecture of tree-sitter — its incremental GLR parsing algorithm, error recovery, and why it became the standard for editor-grade syntax parsing.
---

# Tree Sitter Fundamentals

> **What you'll learn:**
> - How tree-sitter's incremental parsing algorithm reuses previous parse results to achieve sub-millisecond re-parsing on edits
> - The error recovery mechanism that produces useful partial trees even when source code contains syntax errors
> - Why tree-sitter's design choices — C runtime, generated parsers, concrete syntax trees — make it ideal for editor and agent use cases

Tree-sitter is a parser generator tool and an incremental parsing library. It was created by Max Brunsfeld at GitHub in 2017 to solve a specific problem: editors need to parse source code fast enough that syntax highlighting and code folding update instantly as the user types, across every programming language, even when the code is incomplete or contains errors. Traditional parser generators like yacc or ANTLR were designed for compilers that process complete, valid source files in batch mode. Tree-sitter was designed for the real world — where code is edited character by character, is often broken mid-keystroke, and spans dozens of languages within a single editor session.

This design makes tree-sitter equally well-suited for coding agents. An agent edits code incrementally, needs to understand partially-written files, and must work across whatever languages exist in the project. Let's understand how tree-sitter achieves this.

## The Incremental Parsing Algorithm

Most parsers work in batch mode: they consume the entire input and produce a parse tree. If you change one character, you parse the entire file again. For a 10,000-line file, this means re-processing tens of thousands of tokens on every keystroke.

Tree-sitter takes a fundamentally different approach. It keeps the previous parse tree in memory and, when the source code changes, identifies which parts of the tree are affected by the edit. Only those subtrees are re-parsed — the rest of the tree is reused. This is called **incremental parsing**.

The algorithm works in three steps:

1. **Edit notification.** You tell tree-sitter what changed: "bytes 142 through 145 were replaced with bytes 142 through 150." This is a byte-range edit, not a line-based diff.

2. **Tree invalidation.** Tree-sitter walks the old tree and marks nodes whose byte ranges overlap with the edited region. These nodes and their ancestors need re-parsing.

3. **Incremental re-parse.** Tree-sitter re-parses only the invalidated regions, reusing all unaffected subtrees from the old tree. The result is a new tree that reflects the edit.

In practice, editing a single line in a 10,000-line file re-parses only a handful of nodes. The re-parse takes microseconds, not milliseconds.

```rust
use tree_sitter::Parser;

fn demonstrate_incremental_parse() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");

    // Initial parse
    let source = b"fn main() { let x = 1; }";
    let mut tree = parser.parse(source, None).unwrap();

    // Edit: change "1" to "42"
    // The "1" is at byte offset 20, length 1
    // We're replacing it with "42", length 2
    let new_source = b"fn main() { let x = 42; }";

    // Tell tree-sitter about the edit
    tree.edit(&tree_sitter::InputEdit {
        start_byte: 20,
        old_end_byte: 21,   // old "1" was 1 byte
        new_end_byte: 22,   // new "42" is 2 bytes
        start_position: tree_sitter::Point { row: 0, column: 20 },
        old_end_position: tree_sitter::Point { row: 0, column: 21 },
        new_end_position: tree_sitter::Point { row: 0, column: 22 },
    });

    // Incremental re-parse: pass the old tree
    let new_tree = parser.parse(new_source, Some(&tree)).unwrap();

    // Only the affected subtree was re-parsed
    let root = new_tree.root_node();
    println!("Root: {}", root.to_sexp());
}
```

The key API detail is the second argument to `parser.parse()`. When you pass `Some(&old_tree)`, tree-sitter performs an incremental parse. When you pass `None`, it performs a full parse. For an agent that makes successive edits to a file, passing the old tree each time keeps parsing effectively instantaneous.

::: python Coming from Python
Python's `ast.parse()` always performs a full parse — there is no way to pass a previous tree for incremental re-parsing. For small files this does not matter, but for large files (thousands of lines) or rapid successive edits, tree-sitter's incremental approach is dramatically faster. The tree-sitter Python bindings (`py-tree-sitter`) expose the same incremental API:
```python
import tree_sitter_python as tspython
from tree_sitter import Language, Parser

parser = Parser(Language(tspython.language()))
tree = parser.parse(b"def foo(): pass")
# Edit and re-parse with old tree — same concept as in Rust
new_tree = parser.parse(b"def foo(): return 1", old_tree=tree)
```
:::

## The GLR Parsing Strategy

Under the hood, tree-sitter uses a **Generalized LR (GLR)** parsing algorithm. Standard LR parsers maintain a single parse state — when they encounter ambiguity (multiple possible parse interpretations), they fail. GLR parsers maintain multiple parse states simultaneously, exploring all valid interpretations in parallel and pruning invalid ones as more input is consumed.

This matters for real-world code because many languages have genuine syntactic ambiguities. In C++, `A * B` could be a multiplication expression or a pointer declaration depending on whether `A` is a type. In Rust, `<` could start a generic parameter list or be a less-than operator. GLR parsing handles these cases gracefully by tracking both possibilities until the ambiguity resolves.

You do not need to understand GLR parsing in detail to use tree-sitter — the algorithm is hidden behind the parser interface. But it explains why tree-sitter can handle complex grammars that would choke a simpler parser.

## Error Recovery

Real code is often broken. A developer is mid-edit, a bracket is missing, a semicolon is absent. A compiler parser would reject this input and produce an error message. Tree-sitter takes a different approach: it produces the **best possible tree** for invalid input.

When tree-sitter encounters a syntax error, it inserts an `ERROR` node in the tree and continues parsing the rest of the file. The tree around the error is still valid and queryable:

```rust
use tree_sitter::Parser;

fn parse_broken_code() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");

    // This code has a missing closing brace
    let broken_source = b"fn main() { let x = 1; fn helper() { return 2; }";

    let tree = parser.parse(broken_source, None).unwrap();
    let root = tree.root_node();

    // The tree still exists and is partially correct
    println!("Has errors: {}", root.has_error());
    println!("S-expression: {}", root.to_sexp());

    // We can still query the parts that parsed correctly
    for i in 0..root.named_child_count() {
        let child = root.named_child(i).unwrap();
        println!("Child {}: {} (error: {})", i, child.kind(), child.has_error());
    }
}
```

Error recovery is what makes tree-sitter practical for agents. When an agent is making edits to a file, there will be intermediate states where the code does not compile. With tree-sitter, the agent can still parse the file, understand its structure, and continue making targeted edits — even with syntax errors present.

## Concrete Syntax Trees vs Abstract Syntax Trees

Tree-sitter produces **concrete syntax trees (CSTs)**, not abstract syntax trees (ASTs). The difference is important:

- An **AST** omits syntactic details that do not affect meaning — parentheses, semicolons, commas, braces. Python's `ast.parse()` produces an AST. You cannot reconstruct the original source from an AST.

- A **CST** preserves every token in the source, including punctuation and whitespace. You can reconstruct the original source exactly from the CST. Tree-sitter produces CSTs.

This means tree-sitter trees contain nodes for commas, semicolons, brackets, and other punctuation. These are called **anonymous nodes** (as opposed to **named nodes** like function definitions and identifiers). Most of the time you work with named nodes and ignore anonymous ones, but the full CST is there when you need it — for example, when formatting code or computing precise byte offsets for edits.

```rust
use tree_sitter::Parser;

fn show_cst_detail() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");

    let source = b"let x: i32 = 5;";
    let tree = parser.parse(source, None).unwrap();
    let root = tree.root_node();

    // Walk all children including anonymous nodes (punctuation)
    fn print_tree(node: tree_sitter::Node, source: &[u8], indent: usize) {
        let text = &source[node.start_byte()..node.end_byte()];
        let text_str = std::str::from_utf8(text).unwrap_or("<binary>");
        let named = if node.is_named() { "named" } else { "anon" };
        println!(
            "{:indent$}{} [{}] \"{}\"",
            "",
            node.kind(),
            named,
            text_str,
            indent = indent
        );
        for i in 0..node.child_count() {
            print_tree(node.child(i).unwrap(), source, indent + 2);
        }
    }

    print_tree(root, source, 0);
}
```

Running this on `let x: i32 = 5;` reveals every token as a node — the `let` keyword, the `:` separator, the `=` operator, the `;` terminator. This level of detail is what enables tree-sitter to support precise code transformations.

::: wild In the Wild
Neovim, Helix, Zed, and GitHub's code navigation all use tree-sitter for syntax-aware features. GitHub uses tree-sitter to power the "jump to definition" feature in the code browser and to generate the symbol outlines you see in the file header. When a coding agent like Claude Code reads a file and needs to understand its structure, tree-sitter provides the same quality of parsing that powers your editor's syntax highlighting.
:::

## Why Tree-Sitter for Agents

Several design choices make tree-sitter specifically valuable for coding agents:

**Language-agnostic API.** The same Rust API works for parsing Python, JavaScript, Go, Rust, C++, and over 100 other languages. You write your code-navigation logic once, and it works across the entire polyglot codebase.

**Speed.** Initial parsing of a 10,000-line file takes a few milliseconds. Incremental re-parsing after an edit takes microseconds. This is fast enough that an agent can parse every file it touches without worrying about latency.

**Robustness.** Error recovery means the agent always gets a tree, even for broken code. This is essential during multi-step editing where intermediate states may not compile.

**Query language.** Tree-sitter includes a pattern-matching query language (which we cover in the AST Queries subchapter) that lets you express structural patterns concisely. This is how you ask questions like "find all async functions" or "find all struct definitions with a `pub` visibility modifier."

**Embeddable.** The tree-sitter runtime is a small C library with minimal dependencies. The Rust bindings wrap it cleanly, and the generated parsers are self-contained. You can embed tree-sitter in your agent binary without pulling in a language runtime or a compiler toolchain.

## Key Takeaways

- Tree-sitter's incremental parsing reuses unaffected subtrees when source code changes, enabling sub-millisecond re-parsing after edits — ideal for agents that make iterative code changes
- The GLR parsing algorithm handles syntactic ambiguities in real-world languages by tracking multiple parse states simultaneously
- Error recovery produces partial trees for broken code, letting agents understand file structure even during mid-edit states
- Tree-sitter produces concrete syntax trees (CSTs) that preserve every token including punctuation, enabling precise byte-offset calculations for code edits
- The combination of speed, robustness, language-agnostic API, and an embedded query language makes tree-sitter the standard foundation for code intelligence in editors and agents alike
