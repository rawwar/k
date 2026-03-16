---
title: "Chapter 1: Hello, Rust CLI"
description: Set up your Rust development environment and build your first interactive command-line application — the skeleton of a coding agent.
---

# Hello, Rust CLI

This chapter takes you from zero to a working Rust command-line application. You install the Rust toolchain, learn how Cargo manages projects, and write your first binary. Along the way you pick up the foundational language features you need for every chapter that follows: variables, types, functions, modules, and basic error handling.

By the end of the chapter you have a small but functional REPL (Read-Eval-Print Loop) that reads user input, processes commands, and prints results. This REPL becomes the skeleton of the coding agent you build throughout the rest of the book. Every concept introduced here is immediately applied to that project so nothing feels abstract.

The pace assumes you already know how to program in Python (or a similar language). We do not teach general programming concepts; instead we focus on what makes Rust different and why those differences matter for building reliable CLI tools and coding agents.

## Learning Objectives

- Install Rust and configure a productive development environment with rust-analyzer
- Create, build, and run projects with Cargo — Rust's all-in-one build tool and package manager
- Understand Rust's ownership model at an introductory level through variables and types
- Organize code with functions and modules for a clean, scalable project layout
- Handle errors with `Result` and `Option` instead of exceptions
- Parse command-line arguments using the `clap` crate
- Build an interactive REPL that reads from stdin and writes to stdout

## Subchapters

1. [Why Rust](/project/01-hello-rust-cli/01-why-rust) — understand why Rust is the right choice for building coding agents
2. [Installing Rust](/project/01-hello-rust-cli/02-installing-rust) — get your toolchain and editor set up
3. [Cargo Basics](/project/01-hello-rust-cli/03-cargo-basics) — learn the build tool you will use every day
4. [First Binary](/project/01-hello-rust-cli/04-first-binary) — write, compile, and run your first Rust program
5. [Project Structure](/project/01-hello-rust-cli/05-project-structure) — organize your coding-agent project for growth
6. [Variables and Types](/project/01-hello-rust-cli/06-variables-and-types) — master immutability, strings, and ownership basics
7. [Functions and Modules](/project/01-hello-rust-cli/07-functions-and-modules) — define functions and split code into modules
8. [Error Handling Basics](/project/01-hello-rust-cli/08-error-handling-basics) — use `Result` and `Option` instead of exceptions
9. [CLI Argument Parsing](/project/01-hello-rust-cli/09-cli-argument-parsing) — parse arguments and flags with `clap`
10. [Reading User Input](/project/01-hello-rust-cli/10-reading-user-input) — read lines from stdin and handle edge cases
11. [Building a Simple REPL](/project/01-hello-rust-cli/11-building-a-simple-repl) — combine everything into a working interactive loop
12. [Summary and Exercises](/project/01-hello-rust-cli/12-summary-and-exercises) — review what you learned and practice with exercises

## Prerequisites

- Solid programming experience in Python (or another dynamic language)
- A computer running macOS, Linux, or Windows with internet access
- Comfort using a terminal or command prompt
- A code editor you enjoy — VS Code, Zed, Neovim, or any editor that supports rust-analyzer
