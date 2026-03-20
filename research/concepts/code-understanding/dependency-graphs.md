---
title: Dependency Graph Analysis
status: complete
---

# Dependency Graphs

> How coding agents analyze import/require relationships, build call graphs, trace module dependencies, and understand project structure through dependency analysis.

## Overview

Dependency graph analysis reveals the **relationships** between code components — which files import which other files, which functions call which other functions, which packages depend on which other packages. For coding agents, this understanding is critical: changing a function without understanding its callers is dangerous, and adding a feature without understanding the existing module structure leads to architectural violations.

### Types of Dependency Graphs

| Graph Type | Nodes | Edges | Granularity | Use Case |
|---|---|---|---|---|
| **File dependency** | Files | Import/require statements | Coarse | Understand project structure |
| **Module dependency** | Modules/packages | Module imports | Medium | Understand module boundaries |
| **Symbol dependency** | Functions/classes | Function calls, type references | Fine | Understand code relationships |
| **Call graph** | Functions | Function calls | Fine | Trace execution paths |
| **Package dependency** | External packages | package.json, requirements.txt | External | Understand third-party dependencies |
| **Type dependency** | Types/interfaces | Type references, implements, extends | Fine | Understand type hierarchies |

### Agent Adoption of Dependency Analysis

| Agent | Dependency Analysis | Approach |
|---|---|---|
| **Aider** | Deep — graph-based repo map | Tree-sitter tag extraction → PageRank ranking |
| **Droid** | Medium — incremental dependency tracking | AST-based import analysis |
| **Claude Code** | Shallow — implicit via search | Follows imports manually when reading files |
| **ForgeCode** | Medium — entry-point detection | Traces imports from entry points |
| **Junie CLI** | Deep — JetBrains dependency analysis | Full IDE dependency graph |
| **Others** | None to minimal | Rely on LLM to infer relationships |

---

## Import/Require Tracking

### JavaScript/TypeScript Import Analysis

JavaScript has two module systems with different import syntax:

```javascript
// ESM (ECMAScript Modules)
import { createUser } from './services/user';
import * as auth from './auth';
import React, { useState, useEffect } from 'react';
import type { User } from './types';
export { handler } from './handler';
export default class App {}

// CommonJS
const express = require('express');
const { Router } = require('express');
module.exports = { createUser };
module.exports.handler = function() {};
```

**Extracting imports with tree-sitter:**

```scheme
;; Tree-sitter query for JavaScript/TypeScript imports
(import_statement
  source: (string) @import.source) @import.stmt

(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @import.symbol))))

;; CommonJS require
(call_expression
  function: (identifier) @_func
  arguments: (arguments (string) @require.source)
  (#eq? @_func "require"))

;; Dynamic imports
(call_expression
  function: (import)
  arguments: (arguments (string) @dynamic_import.source))
```

### Python Import Analysis

Python's import system is more complex with relative imports, namespace packages, and dynamic imports:

```python
# Standard imports
import os
import json
from pathlib import Path
from typing import Optional, List

# Relative imports
from . import utils
from ..models import User
from .services.auth import authenticate

# Dynamic imports
module = importlib.import_module(f"plugins.{name}")

# Conditional imports
try:
    import ujson as json
except ImportError:
    import json
```

**Building a Python import graph:**

```python
import ast
from pathlib import Path
from collections import defaultdict

class PythonImportAnalyzer:
    def __init__(self, project_root: str):
        self.root = Path(project_root)
        self.graph = defaultdict(set)  # file -> set of imported files

    def analyze(self):
        for py_file in self.root.rglob("*.py"):
            self.analyze_file(py_file)

    def analyze_file(self, filepath: Path):
        try:
            source = filepath.read_text()
            tree = ast.parse(source)
        except (SyntaxError, UnicodeDecodeError):
            return

        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    resolved = self.resolve_import(alias.name, filepath)
                    if resolved:
                        self.graph[str(filepath)].add(str(resolved))

            elif isinstance(node, ast.ImportFrom):
                if node.module:
                    module_path = node.module
                    if node.level > 0:  # Relative import
                        module_path = self.resolve_relative(
                            module_path, filepath, node.level
                        )
                    resolved = self.resolve_import(module_path, filepath)
                    if resolved:
                        self.graph[str(filepath)].add(str(resolved))

    def resolve_import(self, module_name: str, from_file: Path):
        """Resolve a module name to a file path."""
        parts = module_name.split(".")
        # Try as package (directory with __init__.py)
        package_path = self.root / Path(*parts) / "__init__.py"
        if package_path.exists():
            return package_path
        # Try as module (file.py)
        module_path = self.root / Path(*parts[:-1]) / f"{parts[-1]}.py"
        if module_path.exists():
            return module_path
        return None  # External package

    def resolve_relative(self, module: str, from_file: Path, level: int):
        """Resolve a relative import."""
        base = from_file.parent
        for _ in range(level - 1):
            base = base.parent
        if module:
            return str(base / module.replace(".", "/"))
        return str(base)
```

### Go Import Analysis

Go has a simpler import model — packages are identified by their import path:

```go
import (
    "fmt"
    "net/http"

    "github.com/gorilla/mux"

    "myproject/internal/auth"
    "myproject/internal/models"
    "myproject/pkg/utils"
)
```

**Go import graph with tree-sitter:**

```scheme
;; Tree-sitter query for Go imports
(import_declaration
  (import_spec
    path: (interpreted_string_literal) @import.path))

(import_declaration
  (import_spec_list
    (import_spec
      path: (interpreted_string_literal) @import.path)))
```

### Rust Dependency Analysis

Rust uses both module imports (`use`) and crate dependencies (`Cargo.toml`):

```rust
// Module imports
use std::collections::HashMap;
use crate::models::User;
use super::utils;

// External crate imports
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// Module declarations
mod auth;
mod handlers;
pub mod api;
```

---

## Call Graph Analysis

### What Is a Call Graph?

A call graph maps which functions call which other functions. It's the finest-grained dependency graph and the most useful for understanding code behavior:

```
main()
├── initialize_app()
│   ├── load_config()
│   ├── connect_database()
│   └── setup_routes()
│       ├── auth_routes()
│       │   ├── login_handler()
│       │   │   └── authenticate_user()
│       │   │       ├── find_user_by_email()
│       │   │       └── verify_password()
│       │   └── logout_handler()
│       └── api_routes()
│           ├── get_users_handler()
│           │   └── query_users()
│           └── create_user_handler()
│               ├── validate_input()
│               └── insert_user()
└── start_server()
```

### Building Call Graphs with Tree-sitter

```python
class CallGraphBuilder:
    """Build a function-level call graph using tree-sitter."""

    def __init__(self):
        self.definitions = {}  # func_name -> (file, line)
        self.calls = defaultdict(set)  # caller -> set of callees

    def analyze_file(self, filepath, content, language):
        parser = get_parser(language)
        tree = parser.parse(content.encode())

        # Extract function definitions
        for node in self.walk_functions(tree.root_node, language):
            func_name = self.get_function_name(node, language)
            self.definitions[func_name] = (filepath, node.start_point[0])

            # Extract calls within this function
            for call_node in self.walk_calls(node, language):
                callee_name = self.get_call_name(call_node, language)
                self.calls[func_name].add(callee_name)

    def get_callers(self, function_name):
        """Find all functions that call the given function."""
        return [
            caller for caller, callees in self.calls.items()
            if function_name in callees
        ]

    def get_call_chain(self, function_name, depth=5):
        """Get the full call chain from a function."""
        chain = {}
        visited = set()

        def traverse(func, current_depth):
            if current_depth >= depth or func in visited:
                return
            visited.add(func)
            callees = self.calls.get(func, set())
            chain[func] = list(callees)
            for callee in callees:
                traverse(callee, current_depth + 1)

        traverse(function_name, 0)
        return chain
```

### Static vs. Dynamic Call Graphs

| Aspect | Static Call Graph | Dynamic Call Graph |
|---|---|---|
| **Construction** | Parse source code | Run code with instrumentation |
| **Completeness** | Over-approximation (includes unreachable paths) | Under-approximation (only observed paths) |
| **Dynamic dispatch** | Cannot resolve (includes all possibilities) | Resolves to actual target |
| **Performance** | Fast (no execution needed) | Slow (requires running code) |
| **Agent suitability** | High (no execution needed) | Low (requires test execution) |

Coding agents use static call graphs because they don't require code execution, which agents may not be able to perform safely.

---

## Module Dependency Trees

### Understanding Module Structure

Real projects organize code into modules with explicit boundaries:

```
src/
├── api/                    # API layer — depends on services
│   ├── routes/
│   │   ├── auth.ts
│   │   ├── users.ts
│   │   └── posts.ts
│   └── middleware/
│       ├── auth.ts
│       └── logging.ts
├── services/               # Business logic — depends on models
│   ├── auth.service.ts
│   ├── user.service.ts
│   └── post.service.ts
├── models/                 # Data models — depends on database
│   ├── user.model.ts
│   └── post.model.ts
├── database/               # Database layer — no internal deps
│   ├── connection.ts
│   └── migrations/
└── utils/                  # Shared utilities — no internal deps
    ├── logger.ts
    └── validators.ts
```

**Expected dependency flow:**
```
api → services → models → database
  ↘      ↘         ↘
   middleware  utils   utils
```

**Circular dependencies indicate architectural problems:**
```
api → services → models → api  ❌ (circular!)
```

### Detecting Circular Dependencies

```python
def find_circular_dependencies(import_graph):
    """Find all circular dependency chains in the import graph."""
    cycles = []

    def dfs(node, path, visited):
        if node in path:
            cycle_start = path.index(node)
            cycles.append(path[cycle_start:] + [node])
            return
        if node in visited:
            return

        visited.add(node)
        path.append(node)

        for neighbor in import_graph.get(node, []):
            dfs(neighbor, path[:], visited)

    visited = set()
    for node in import_graph:
        dfs(node, [], visited)

    return cycles
```

---

## Package Dependency Analysis

### package.json Analysis (Node.js)

```python
import json

def analyze_package_json(filepath):
    with open(filepath) as f:
        pkg = json.load(f)

    return {
        "name": pkg.get("name"),
        "dependencies": pkg.get("dependencies", {}),
        "devDependencies": pkg.get("devDependencies", {}),
        "peerDependencies": pkg.get("peerDependencies", {}),
        "scripts": pkg.get("scripts", {}),
        "main": pkg.get("main"),
        "module": pkg.get("module"),
        "types": pkg.get("types"),
        "workspaces": pkg.get("workspaces", []),
    }

def categorize_dependencies(deps):
    """Categorize dependencies by purpose."""
    categories = {
        "framework": [],      # react, express, fastify
        "database": [],       # prisma, mongoose, typeorm
        "testing": [],        # jest, vitest, mocha
        "build": [],          # typescript, webpack, vite
        "utility": [],        # lodash, date-fns, zod
        "auth": [],           # passport, jsonwebtoken
    }

    framework_patterns = ["react", "vue", "angular", "express", "fastify", "next", "nuxt"]
    db_patterns = ["prisma", "mongoose", "typeorm", "sequelize", "knex", "drizzle"]
    test_patterns = ["jest", "vitest", "mocha", "chai", "cypress", "playwright"]

    for name, version in deps.items():
        if any(p in name for p in framework_patterns):
            categories["framework"].append((name, version))
        elif any(p in name for p in db_patterns):
            categories["database"].append((name, version))
        elif any(p in name for p in test_patterns):
            categories["testing"].append((name, version))
        else:
            categories["utility"].append((name, version))

    return categories
```

### requirements.txt / pyproject.toml Analysis (Python)

```python
def analyze_python_deps(project_root):
    """Analyze Python project dependencies."""
    deps = {"production": [], "development": [], "build": []}

    # Check pyproject.toml
    pyproject = Path(project_root) / "pyproject.toml"
    if pyproject.exists():
        import tomllib
        with open(pyproject, "rb") as f:
            config = tomllib.load(f)
        project = config.get("project", {})
        deps["production"] = project.get("dependencies", [])
        optional = project.get("optional-dependencies", {})
        deps["development"] = optional.get("dev", [])

    # Check requirements.txt
    requirements = Path(project_root) / "requirements.txt"
    if requirements.exists():
        for line in requirements.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                deps["production"].append(line)

    return deps
```

### Cargo.toml Analysis (Rust)

```python
def analyze_cargo_toml(filepath):
    """Analyze Rust project dependencies from Cargo.toml."""
    import tomllib
    with open(filepath, "rb") as f:
        cargo = tomllib.load(f)

    return {
        "name": cargo.get("package", {}).get("name"),
        "dependencies": cargo.get("dependencies", {}),
        "dev_dependencies": cargo.get("dev-dependencies", {}),
        "build_dependencies": cargo.get("build-dependencies", {}),
        "workspace_members": cargo.get("workspace", {}).get("members", []),
        "features": cargo.get("features", {}),
    }
```

### go.mod Analysis (Go)

```python
def analyze_go_mod(filepath):
    """Analyze Go project dependencies from go.mod."""
    content = Path(filepath).read_text()
    module_name = None
    go_version = None
    requires = []
    replaces = []

    for line in content.splitlines():
        line = line.strip()
        if line.startswith("module "):
            module_name = line.split()[1]
        elif line.startswith("go "):
            go_version = line.split()[1]
        elif line.startswith("require "):
            parts = line.replace("require ", "").strip("()").split()
            if len(parts) >= 2:
                requires.append({"module": parts[0], "version": parts[1]})

    return {
        "module": module_name,
        "go_version": go_version,
        "requires": requires,
        "replaces": replaces,
    }
```

---

## How Agents Use Dependency Information

### Understanding Project Architecture

Dependency graphs reveal the architectural layers of a project:

```python
def analyze_architecture(import_graph, file_paths):
    """Identify architectural layers from dependency direction."""
    layers = defaultdict(set)

    for filepath in file_paths:
        directory = Path(filepath).parent.name
        layers[directory].add(filepath)

    # Determine layer ordering based on dependency direction
    layer_deps = defaultdict(set)
    for src, targets in import_graph.items():
        src_layer = Path(src).parent.name
        for target in targets:
            target_layer = Path(target).parent.name
            if src_layer != target_layer:
                layer_deps[src_layer].add(target_layer)

    # Topological sort to find layer ordering
    # Higher layers depend on lower layers
    return topological_sort(layer_deps)
```

### Impact Analysis: "What Breaks If I Change This?"

The most valuable use of dependency graphs for agents:

```python
def analyze_change_impact(function_name, call_graph, import_graph):
    """Determine the impact of changing a function."""
    direct_callers = call_graph.get_callers(function_name)
    affected_files = set()

    # Find all files that directly or transitively depend on this function
    queue = list(direct_callers)
    visited = set()

    while queue:
        caller = queue.pop(0)
        if caller in visited:
            continue
        visited.add(caller)
        affected_files.add(get_file(caller))

        # Add callers of callers
        indirect_callers = call_graph.get_callers(caller)
        queue.extend(indirect_callers)

    return {
        "function": function_name,
        "direct_callers": len(direct_callers),
        "total_affected_functions": len(visited),
        "affected_files": list(affected_files),
        "risk": "high" if len(affected_files) > 10 else
                "medium" if len(affected_files) > 3 else "low"
    }
```

### Suggesting Related Files

When an agent edits a file, dependency analysis can suggest other files that may need updates:

```python
def suggest_related_files(edited_file, import_graph, git_history):
    """Suggest files that might need changes when editing a file."""
    suggestions = []

    # Files that import the edited file (may need API changes)
    importers = [f for f, deps in import_graph.items() if edited_file in deps]
    for f in importers:
        suggestions.append({"file": f, "reason": "imports the edited file"})

    # Files imported by the edited file (may need to understand the API)
    imported = import_graph.get(edited_file, [])
    for f in imported:
        suggestions.append({"file": f, "reason": "imported by the edited file"})

    # Files frequently co-modified (from git history)
    co_modified = get_co_modified_files(edited_file, git_history)
    for f, count in co_modified[:5]:
        suggestions.append({"file": f, "reason": f"co-modified {count} times"})

    return suggestions
```

---

## Visualization

### Dependency Graph Visualization Formats

For debugging and understanding, dependency graphs can be visualized:

```python
def export_to_dot(import_graph, output_path):
    """Export dependency graph to Graphviz DOT format."""
    lines = ["digraph dependencies {"]
    lines.append('  rankdir=TB;')
    lines.append('  node [shape=box, fontsize=10];')

    for src, targets in import_graph.items():
        src_name = Path(src).stem
        for target in targets:
            target_name = Path(target).stem
            lines.append(f'  "{src_name}" -> "{target_name}";')

    lines.append("}")

    with open(output_path, "w") as f:
        f.write("\n".join(lines))

# Then render with: dot -Tpng deps.dot -o deps.png
```

### Mermaid Diagram Output

Mermaid diagrams can be embedded in markdown for documentation:

```python
def export_to_mermaid(import_graph):
    """Export dependency graph as Mermaid diagram."""
    lines = ["graph TD"]

    for src, targets in import_graph.items():
        src_id = Path(src).stem.replace("-", "_")
        for target in targets:
            target_id = Path(target).stem.replace("-", "_")
            lines.append(f"    {src_id} --> {target_id}")

    return "\n".join(lines)
```

---

## Key Takeaways

1. **Dependency graphs are essential for safe editing.** Changing code without understanding its dependents is the most common source of agent-introduced bugs.

2. **Import analysis is straightforward.** Tree-sitter can extract imports from any language, making file-level dependency graphs cheap to build.

3. **Call graphs provide the deepest understanding** but are harder to build accurately, especially for dynamic languages.

4. **Package dependency analysis helps agents understand the technology stack,** guiding decisions about which patterns, APIs, and conventions to follow.

5. **Impact analysis is the highest-value application.** Before making a change, knowing "what else might break" lets agents proactively check and update affected code.

6. **Aider's repo map is essentially a dependency graph with ranking** — it builds a graph from tag references and uses PageRank to prioritize, which is a form of dependency-weighted importance.