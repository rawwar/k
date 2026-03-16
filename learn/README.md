# Build a CLI Coding Agent — Learning Platform

A comprehensive guide for Python developers learning to build AI-powered coding agents in Rust.

## Quick Start

```bash
cd learn
bash start.sh
```

This starts a VitePress dev server at `http://localhost:5173`.

### Prerequisites

- **Node.js 18+** — for the VitePress documentation site
- **Rust and Cargo** — for compiling the code snapshots (`https://rustup.rs/`)

## Directory Structure

```
learn/
  project/           # Project-based track (15 chapters, build-first)
  linear/            # Linear tutorial track (18 chapters, concept-first)
  code/              # Compilable Rust code snapshots (ch01–ch15)
  research/          # Research notes on production agents and concepts
  .vitepress/        # VitePress configuration and theme
  index.md           # Site home page
  start.sh           # Dev server launcher
  AUTHORING-GUIDE.md # Guide for writing and maintaining content
```

## Learning Tracks

**Project Track** — Build a fully functional CLI coding agent chapter by chapter. Each chapter adds a new capability. Best for learning by doing.

**Linear Track** — Concept-first approach. Each chapter explores a key topic in depth before implementation. Best for understanding the *why* behind design decisions.

Both tracks cover the same material from different angles and cross-reference each other.

## Code Snapshots

Each chapter has a corresponding Rust project in `learn/code/chNN/`. These are cumulative — each builds on the previous. Verify any chapter compiles with:

```bash
cd learn/code/ch06
cargo check
```

## Contributing

See [AUTHORING-GUIDE.md](./AUTHORING-GUIDE.md) for content standards, file conventions, and quality checklists.
