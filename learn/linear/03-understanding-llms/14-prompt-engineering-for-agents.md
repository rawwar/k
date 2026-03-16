---
title: Prompt Engineering for Agents
description: Specialized prompt engineering techniques that improve tool use accuracy, multi-step reasoning, and reliable code generation in agents.
---

# Prompt Engineering for Agents

> **What you'll learn:**
> - How agent system prompts differ from chatbot prompts in structure, specificity, and behavioral guidance
> - Techniques for improving tool selection accuracy including explicit tool descriptions and usage examples
> - Patterns for encouraging step-by-step reasoning and reducing common failure modes like premature completion

Prompt engineering for agents is fundamentally different from prompt engineering for chatbots. With a chatbot, you are optimizing for a single good response. With an agent, you are optimizing for a sequence of good decisions -- which tool to call, what arguments to pass, how to interpret results, when to retry, and when to declare the task complete. A slight miscalibration in the prompt can cause the agent to skip verification steps, choose the wrong tool, or give up too early. The techniques in this subchapter are specific to agentic use cases.

## The Agent Prompt Structure

A production agent system prompt is organized into distinct sections, each serving a specific purpose. Here is the architecture:

```
[Identity] - Who the agent is and what it can do
[Tool Usage Rules] - When and how to use each tool
[Workflow Patterns] - Multi-step procedures for common tasks
[Output Guidelines] - How to format responses and communicate
[Safety Constraints] - What operations to avoid or confirm
[Environment Context] - Dynamic information about the current session
```

Let's look at each section with concrete examples.

### Identity

The identity section establishes the agent's role and capabilities in specific terms:

```
You are a coding assistant that helps developers with software projects.
You can read and write files, execute shell commands, and search codebases.
You operate on the user's local filesystem and can run build tools, tests,
and version control commands.
```

Specific is better than generic. "You are a coding assistant that can read files, write files, and run commands" works better than "You are a helpful AI." The specificity primes the model to think in terms of file operations and shell commands rather than generic conversation.

### Tool Usage Rules

This is the highest-impact section. Each rule addresses a specific failure mode:

```
## Tool Usage Rules

- ALWAYS read a file before modifying it. Never assume you know the contents of a file.
- When editing a file, write the COMPLETE new file contents. Do not use partial edits
  or placeholders like "... rest of file unchanged ...".
- After modifying code, ALWAYS run the appropriate build/check command (e.g., cargo check)
  to verify the change compiles.
- If a build fails, read the error message carefully, fix the issue, and try again.
  Do not report failure to the user until you have attempted at least 2 fixes.
- Use grep/ripgrep to find relevant code rather than reading files one by one.
- Prefer targeted file reads (specific files) over broad directory listings.
```

Each rule follows the pattern: **action + reason**. "ALWAYS read a file before modifying it" (action) because "never assume you know the contents" (reason). The reason helps the model understand the intent behind the rule, making it more likely to apply the rule in novel situations.

### Workflow Patterns

Workflow patterns provide templates for multi-step operations:

```
## Workflow Patterns

When fixing a bug:
1. Read the relevant source files to understand the code
2. Identify the root cause of the bug
3. Make the minimal change to fix the issue
4. Run the build to verify the fix compiles
5. Run tests to verify the fix works
6. Summarize what you changed and why

When adding a new feature:
1. Understand the existing code structure
2. Plan the changes needed
3. Implement the changes incrementally
4. Run the build after each significant change
5. Write or update tests
6. Run the full test suite
7. Summarize what was added
```

These workflows reduce the model's decision space at each step. Instead of choosing from all possible actions, it follows a predefined sequence that encodes your engineering judgment about the best approach.

::: python Coming from Python
Python agent frameworks like LangChain use "chains" and "agents" with predefined step sequences. In your Rust agent, the system prompt serves a similar function -- the workflow patterns in the prompt are like soft-coded chains that guide the model without rigidly constraining it. The model can deviate from the pattern when the situation demands it, but the pattern establishes a strong default behavior.
:::

## Technique 1: Explicit Tool Selection Guidance

The model chooses tools based on the task and the tool descriptions. You can improve accuracy by providing explicit guidance about when to use each tool:

**Weak (model must infer from description):**
```
read_file: Reads a file
shell: Runs a command
```

**Strong (model gets explicit selection criteria):**
```
read_file: Read the contents of a single file. Use this when you need to examine
source code, configuration, or any text file. Prefer this over using shell commands
like 'cat' because it handles encoding correctly and provides better error messages.

shell: Execute any shell command. Use this for:
- Build commands: cargo check, cargo build, cargo test
- Search: grep -r, ripgrep (rg), find
- Version control: git status, git diff, git log
- File operations: ls, mkdir, cp (but prefer read_file/write_file for content)
Do NOT use for: reading file contents (use read_file instead), writing files
(use write_file instead).
```

The "Do NOT use for" guidance is particularly effective. It creates negative constraints that prevent the model from using a tool in suboptimal ways.

## Technique 2: Think-Before-Acting Instructions

Encouraging the model to reason before acting improves the quality of tool calls:

```
Before making any changes, briefly state:
1. What you understand about the problem
2. What approach you plan to take
3. Which files you expect to modify

Then proceed with the implementation.
```

This technique works because it forces the model to generate reasoning tokens before generating action tokens. The reasoning tokens condition the subsequent generation, leading to better decisions. It is not magic -- it is exploiting the autoregressive nature of the model.

However, be careful not to make the model too verbose. "Briefly state" is key -- without it, the model might generate paragraphs of analysis before every minor action.

## Technique 3: Error Recovery Patterns

Models tend to give up after a single failure. Explicit error recovery instructions prevent premature abandonment:

```
When you encounter an error:
1. Read the error message carefully and completely
2. Identify the root cause (not just the symptom)
3. Fix the specific issue
4. Verify the fix
5. If the same error persists after 2 attempts, try a different approach
6. Only report failure to the user if you have exhausted your approaches
```

Without this guidance, the model often responds with "I encountered an error: [error message]. Would you like me to try a different approach?" This puts the decision back on the user and breaks the autonomous behavior that makes agents valuable.

## Technique 4: Output Formatting Control

Control how the model presents its work to the user:

```
## Communication

- When you make changes, show a brief summary of what changed and why.
  Do not repeat the entire file content.
- For multi-step operations, provide a brief status after each step.
  Example: "Read src/main.rs (245 lines). Found the issue on line 87."
- If you need to make multiple changes, list them all at the end rather
  than explaining each one in detail during execution.
- Use code blocks with language tags when showing code snippets.
```

Without formatting guidance, the model tends toward excessive verbosity -- explaining every decision, showing full file contents after every edit, and providing long justifications for simple changes. This is especially problematic for agents that make 10-20 tool calls per task.

## Technique 5: Preventing Common Failure Modes

Here are specific prompt patterns that address the most common agent failure modes:

**Premature completion** -- the model declares "done" before verifying:
```
Never say a task is complete until you have verified it. For code changes,
this means running the build. For bug fixes, this means running the test.
Do not assume your changes are correct -- verify them.
```

**Hallucinated file contents** -- the model guesses instead of reading:
```
NEVER assume the contents of a file. Always read it first, even if you
think you know what it contains. File contents may have changed since
you last read them.
```

**Overly broad changes** -- the model rewrites more than necessary:
```
Make the minimum change necessary to accomplish the task. Do not refactor,
reorganize, or "improve" code beyond what was requested unless the user
specifically asks for it.
```

**Missing error handling** -- the model ignores errors in tool results:
```
Always check the exit code of shell commands. A non-zero exit code indicates
failure. Read stderr output when a command fails -- it usually contains
the most useful diagnostic information.
```

## Technique 6: Few-Shot Examples in System Prompts

For complex tool use patterns, including a concrete example in the system prompt dramatically improves accuracy:

```
## Example Interaction

User: "Fix the compilation error in src/main.rs"

Good approach:
1. shell("cargo check 2>&1") -> reads the error message
2. read_file("src/main.rs") -> examines the source
3. write_file("src/main.rs", <fixed content>) -> applies the fix
4. shell("cargo check") -> verifies the fix

Bad approach:
1. write_file("src/main.rs", <guessed fix>) -> modifies without reading first
```

The "Good approach / Bad approach" format is especially effective because it explicitly shows the model what not to do. This contrastive example is worth more than several paragraphs of instruction.

::: wild In the Wild
Claude Code's system prompt includes detailed tool usage guidelines, workflow patterns, and explicit instructions about error handling and verification. It is the product of iterative refinement -- each rule in the prompt corresponds to a specific failure mode observed during testing. The system prompt is one of the most carefully maintained components of the entire agent, and changes to it go through rigorous testing across diverse coding tasks.
:::

## Iterative Prompt Development

Prompt engineering is empirical. Here is the process that works:

1. **Start with a minimal prompt** -- identity, basic tool descriptions, one or two rules
2. **Run a diverse set of tasks** -- file editing, debugging, test writing, multi-file refactoring
3. **Document failures** -- what went wrong, what the model should have done instead
4. **Add targeted rules** -- one rule per failure mode, as specific as possible
5. **Test regressions** -- verify new rules do not break previously working tasks
6. **Prune dead rules** -- if a rule does not measurably affect behavior, remove it

Keep a log of your prompt changes and the failures that motivated them. This log becomes invaluable documentation for understanding why each rule exists and whether it can be removed when model behavior improves with newer versions.

## Key Takeaways

- Agent prompt engineering optimizes for sequences of good decisions (tool selection, argument quality, error recovery) rather than single good responses
- The highest-impact prompt sections are tool usage rules and workflow patterns -- each rule should address a specific, observed failure mode with both action and reason
- Explicit "Do NOT use for" guidance in tool descriptions and "Good approach / Bad approach" examples in system prompts are among the most effective techniques
- Prevent common failures (premature completion, hallucinated file contents, overly broad changes) with targeted instructions that make the undesired behavior explicitly forbidden
- Prompt development is iterative: start minimal, observe failures, add targeted rules, test regressions, and prune what does not help -- maintain a log of changes and their motivations
