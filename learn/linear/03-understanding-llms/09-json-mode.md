---
title: JSON Mode
description: Constraining LLM output to valid JSON for structured data extraction and how this complements tool use in agent systems.
---

# JSON Mode

> **What you'll learn:**
> - How JSON mode forces the model to output syntactically valid JSON and when to use it
> - The difference between JSON mode and tool use for obtaining structured output from the model
> - Practical patterns for combining JSON mode with schema validation in agent pipelines

When you build an agent, you often need the model to produce structured data -- not free-form text. You might need a list of files to modify, a structured plan with steps, or a classification of what kind of task the user is asking about. JSON mode and structured output features give you a way to constrain the model's output to valid, parseable JSON, eliminating the need to extract structured data from natural language responses.

## What JSON Mode Does

Without JSON mode, the model generates free-form text. If you ask it to "list the files that need changes as JSON," it might respond with:

```
Here are the files that need changes:

```json
["src/main.rs", "src/config.rs", "tests/integration.rs"]
```

The actual file list is there, but it is wrapped in explanatory text and markdown code fences. Parsing this requires fragile string manipulation -- finding the code fence, extracting the content, handling cases where the model formats it differently.

With JSON mode enabled, the model is constrained to output only valid JSON:

```json
["src/main.rs", "src/config.rs", "tests/integration.rs"]
```

No preamble, no markdown, no explanation -- just valid JSON that you can parse directly with `serde_json::from_str()`.

## Enabling JSON Mode

**Anthropic** does not have a standalone "JSON mode" toggle. Instead, you achieve structured output through the tool use protocol (which we covered in the previous subchapters) or by instructing the model in the system/user prompt to respond with JSON only. Anthropic's recommended approach for structured output is to define a tool whose `input_schema` matches the structure you want, and let the tool use protocol guarantee the output format.

**OpenAI** offers an explicit `response_format` parameter:

```json
{
  "model": "gpt-4o",
  "response_format": {"type": "json_object"},
  "messages": [
    {
      "role": "system",
      "content": "You are a helpful assistant. Respond with a JSON object containing a 'files' array listing files that need modification."
    },
    {
      "role": "user",
      "content": "I need to add logging to my Rust web server"
    }
  ]
}
```

When `response_format` is set to `json_object`, OpenAI guarantees the response is valid JSON. The model is constrained during generation to never produce a token that would break JSON syntax.

**Important:** When using OpenAI's JSON mode, you must mention "JSON" in the system or user message. If you do not, the API returns an error. This is because the model needs context about what JSON structure to produce.

OpenAI also offers a more powerful variant called **Structured Outputs** that guarantees conformance to a specific JSON Schema:

```json
{
  "model": "gpt-4o",
  "response_format": {
    "type": "json_schema",
    "json_schema": {
      "name": "file_changes",
      "strict": true,
      "schema": {
        "type": "object",
        "properties": {
          "files": {
            "type": "array",
            "items": {"type": "string"}
          },
          "reason": {"type": "string"}
        },
        "required": ["files", "reason"],
        "additionalProperties": false
      }
    }
  },
  "messages": [...]
}
```

With `strict: true`, the output is guaranteed to match the schema exactly -- every required field will be present, types will be correct, and no extra fields will appear. This is the strongest guarantee available.

## JSON Mode vs. Tool Use

For agent builders, there is a natural question: if you need structured output, should you use JSON mode or tool use? They serve different purposes:

| Aspect | JSON Mode | Tool Use |
|---|---|---|
| Primary purpose | Get structured data from the model | Let the model take actions |
| Who processes the output | Your code parses and uses it | Your code executes and returns result |
| Conversation flow | Single response, no follow-up needed | Requires tool result message back to model |
| Schema enforcement | OpenAI Structured Outputs: strict; basic JSON mode: syntax only | JSON Schema on input parameters |
| Anthropic support | Via prompting or tool use | Native protocol |

**Use tool use when:** the model is requesting an action -- reading a file, running a command, searching code. The model expects a result back.

**Use JSON mode when:** you need the model's analysis or decision in a structured format, and there is no action to execute. Examples: classifying a user request, generating a structured plan, extracting entities from text.

In practice, most agent operations use tool use because the model needs to interact with the environment. JSON mode is more useful for specific sub-tasks within the agent:

```
Agent receives: "Refactor the authentication module"
Step 1 (JSON mode): Ask model to produce a structured plan
  -> {"steps": ["read auth module", "identify concerns", "extract into separate files", ...]}
Step 2-N (tool use): Execute each step with tool calls
```

::: python Coming from Python
In Python, libraries like Pydantic and `instructor` make it easy to get structured output from LLMs by defining a Pydantic model and having the library handle prompting and validation. In Rust, you define a struct with `#[derive(Deserialize)]` and use `serde_json` to parse the model's output. The Rust approach requires you to handle validation errors explicitly, but the strong type system catches schema mismatches at compile time that Python would only catch at runtime.
:::

## Parsing JSON Responses in Rust

When the model returns JSON (whether through JSON mode or through a prompt instruction), you parse it into a typed struct:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct FilePlan {
    files: Vec<String>,
    reason: String,
}

fn parse_model_response(json_str: &str) -> Result<FilePlan, serde_json::Error> {
    serde_json::from_str(json_str)
}
```

Even with JSON mode guaranteeing valid JSON syntax, the model might produce valid JSON that does not match your expected schema -- for example, using `"file_list"` instead of `"files"` as the key name. Robust parsing includes:

1. **Deserialization with serde**: catches structural mismatches
2. **Default values for optional fields**: `#[serde(default)]` provides fallbacks
3. **Flexible field names**: `#[serde(alias = "file_list")]` accepts common variations
4. **Retry on failure**: if parsing fails, send the error back to the model and ask it to fix the format

```rust
#[derive(Deserialize)]
struct FilePlan {
    #[serde(alias = "file_list", alias = "files_to_change")]
    files: Vec<String>,

    #[serde(default = "default_reason")]
    reason: String,
}

fn default_reason() -> String {
    "No reason provided".to_string()
}
```

## When to Avoid JSON Mode

JSON mode is not always the right choice:

**Do not use JSON mode for conversational responses.** If the user asks "what does this code do?" they expect a natural language explanation, not a JSON object.

**Do not use JSON mode when tool use is more appropriate.** If the model needs to take an action and receive a result, tool use is the correct protocol. JSON mode is a one-way output format.

**Do not use JSON mode for very complex nested structures.** The more complex the schema, the more likely the model is to make mistakes. If you need deeply nested output, consider breaking it into multiple simpler calls.

**Be cautious with JSON mode for code generation.** Code often contains characters that are problematic inside JSON strings -- backslashes, quotes, newlines. Having the model generate code wrapped in a JSON string leads to escaping issues. Tool use with a `content` parameter is a cleaner approach for code.

## Practical Agent Pattern: Structured Planning

One effective use of JSON mode in agents is structured planning. Before the model starts executing tools, ask it to produce a plan:

```json
{
  "model": "gpt-4o",
  "response_format": {
    "type": "json_schema",
    "json_schema": {
      "name": "task_plan",
      "strict": true,
      "schema": {
        "type": "object",
        "properties": {
          "analysis": {"type": "string"},
          "steps": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "action": {"type": "string"},
                "tool": {"type": "string"},
                "reasoning": {"type": "string"}
              },
              "required": ["action", "tool", "reasoning"],
              "additionalProperties": false
            }
          }
        },
        "required": ["analysis", "steps"],
        "additionalProperties": false
      }
    }
  },
  "messages": [
    {"role": "system", "content": "Analyze the user's request and produce a step-by-step plan."},
    {"role": "user", "content": "Add error handling to all the database functions in src/db.rs"}
  ]
}
```

The structured plan gives you several advantages: you can present the plan to the user for approval before execution, you can track progress through the steps, and you can retry individual steps if they fail.

::: wild In the Wild
Some production agents use a two-phase approach: a "planning" phase that uses structured output to create a step-by-step plan, followed by an "execution" phase that uses tool calls to implement each step. This is not universal -- Claude Code operates more fluidly, letting the model plan and execute interleaved. But the structured planning approach can improve reliability for complex, multi-file changes where you want to ensure the model considers the full scope before diving in.
:::

## Key Takeaways

- JSON mode constrains the model to output valid JSON, eliminating the need to parse structured data from free-form text
- OpenAI offers explicit `response_format` with optional Structured Outputs for strict schema conformance; Anthropic achieves structured output primarily through the tool use protocol
- Use JSON mode for structured analysis and planning; use tool use for actions that require execution and results -- most agent operations use tool use
- Parse JSON responses into typed Rust structs with `serde`, using `alias` and `default` attributes for robustness against minor model output variations
- Structured planning (JSON mode for the plan, tool use for execution) is a powerful pattern for complex agent tasks, though it adds latency from the extra API call
