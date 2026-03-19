# Function Calling / Tool Use Across LLM Providers

## Introduction: Why Function Calling Is the Foundation of AI Agents

Function calling (also known as "tool use") is the mechanism by which a large language model
can request execution of external code, APIs, or system operations during a conversation. Rather
than simply generating text, the model outputs a structured request—specifying a function name
and arguments—that the host application intercepts, executes, and feeds back as context for the
model to continue reasoning.

This single capability transforms LLMs from sophisticated text generators into **agents** that
can interact with the real world. Without function calling, an LLM can only describe what it
*would* do. With function calling, it can actually do it: read files, query databases, call APIs,
execute shell commands, search the web, and manipulate data structures.

Every major coding agent—GitHub Copilot, Cursor, Windsurf, Aider, Continue—is built on this
primitive. The agent loop is deceptively simple:

```
1. User provides a task
2. System prompt defines available tools
3. Model generates a tool call (structured JSON)
4. Host executes the tool and captures output
5. Tool result is appended to conversation
6. Model reasons about the result
7. Repeat from step 3, or return a final text response
```

The sophistication is in the orchestration: which tools to expose, how to describe them, how to
handle errors, when to force tool use, and how to manage multi-step chains. But the foundation
is always the same structured function calling protocol.

---

## History: From Prompt Engineering to Native Function Calling

### The Pre-Function-Calling Era (2020–2023)

Before native function calling, developers resorted to prompt engineering to get models to
output structured actions. Common patterns included:

- **ReAct (Reasoning + Acting)**: Prompting models to output `Thought:`, `Action:`, `Action Input:`,
  and `Observation:` blocks, then parsing the text with regex to extract the action and input.
- **JSON-in-markdown**: Instructing models to output ```json blocks containing tool invocations,
  then parsing the markdown code fence.
- **Custom XML tags**: Using `<tool_call>` or `<function>` tags that the application would parse.

These approaches were fragile. Models would frequently:
- Forget the required format mid-conversation
- Hallucinate tool names that didn't exist
- Output malformed JSON that couldn't be parsed
- Mix tool calls with natural language in unpredictable ways

### OpenAI Introduces Native Function Calling (June 2023)

OpenAI's June 2023 release of `gpt-3.5-turbo-0613` and `gpt-4-0613` introduced the first
native function calling API. This was a paradigm shift: instead of hoping the model would
output parseable text, the API guaranteed a structured `function_call` field in the response
when the model decided to use a tool.

The original API used a `functions` array in the request and returned a `function_call` object
in the assistant message. This was later deprecated in favor of the `tools` array with
`type: "function"`.

### Other Providers Follow (2023–2024)

- **Anthropic** added tool use to Claude in April 2024 (generally available)
- **Google** added function calling to Gemini models in late 2023
- **Open-source models** like Llama 3, Mistral, and others added tool calling capabilities
  through special token formats and fine-tuning

Today, function calling is a standard capability across all major LLM providers, though the
wire formats differ significantly.

---

## OpenAI Function Calling

### Original `function_call` (Deprecated)

The original API (June 2023) used these fields:

```json
{
  "model": "gpt-4-0613",
  "messages": [...],
  "functions": [
    {
      "name": "get_weather",
      "description": "Get the current weather for a location",
      "parameters": {
        "type": "object",
        "properties": {
          "location": { "type": "string", "description": "City and state" },
          "unit": { "type": "string", "enum": ["celsius", "fahrenheit"] }
        },
        "required": ["location"]
      }
    }
  ],
  "function_call": "auto"
}
```

The response included:

```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "content": null,
      "function_call": {
        "name": "get_weather",
        "arguments": "{\"location\": \"San Francisco, CA\", \"unit\": \"celsius\"}"
      }
    },
    "finish_reason": "function_call"
  }]
}
```

This format was deprecated in favor of the `tools` array starting with the November 2023
model releases.

### Modern Tools Array with `type: "function"`

The current API uses a `tools` array where each tool has a `type` (currently always `"function"`)
and a `function` object containing the name, description, and parameters:

```json
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "You are a helpful assistant." },
    { "role": "user", "content": "What's the weather in Tokyo?" }
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get the current weather for a location. Call this whenever the user asks about weather conditions.",
        "parameters": {
          "type": "object",
          "properties": {
            "location": {
              "type": "string",
              "description": "The city and country, e.g., 'Tokyo, Japan'"
            },
            "unit": {
              "type": "string",
              "enum": ["celsius", "fahrenheit"],
              "description": "Temperature unit preference"
            }
          },
          "required": ["location"],
          "additionalProperties": false
        }
      }
    }
  ],
  "tool_choice": "auto"
}
```

### Tool Definition Schema

Each tool definition follows this structure:

```typescript
interface Tool {
  type: "function";
  function: {
    name: string;           // Must match [a-zA-Z0-9_-]{1,64}
    description?: string;   // Natural language description of what the function does
    parameters?: object;    // JSON Schema object describing the function's parameters
    strict?: boolean;       // If true, model guarantees output matches the schema exactly
  };
}
```

The `parameters` field accepts a JSON Schema object. OpenAI supports a subset of JSON Schema:
- `type`, `properties`, `required`, `enum`, `const`
- `anyOf` (for union types)
- `$ref` and `$defs` (for recursive schemas)
- `additionalProperties` (must be `false` when `strict: true`)
- Nested `object`, `array`, `string`, `number`, `integer`, `boolean`, `null`

### `tool_choice` Options

The `tool_choice` parameter controls whether and how the model uses tools:

| Value | Behavior |
|-------|----------|
| `"auto"` | Model decides whether to call a tool or respond with text (default) |
| `"required"` | Model must call at least one tool (but chooses which) |
| `"none"` | Model must not call any tools, respond with text only |
| `{ "type": "function", "function": { "name": "get_weather" } }` | Model must call the specified function |

When `tool_choice` is `"required"` or a specific function, the `finish_reason` will be `"stop"`
rather than `"tool_calls"`.

### Parallel Function Calls

OpenAI models can return **multiple tool calls** in a single response. This is called parallel
function calling and is enabled by default. The model may decide that answering a query requires
calling multiple tools simultaneously:

```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [
        {
          "id": "call_abc123",
          "type": "function",
          "function": {
            "name": "get_weather",
            "arguments": "{\"location\": \"Tokyo\"}"
          }
        },
        {
          "id": "call_def456",
          "type": "function",
          "function": {
            "name": "get_weather",
            "arguments": "{\"location\": \"New York\"}"
          }
        }
      ]
    },
    "finish_reason": "tool_calls"
  }]
}
```

You can disable parallel tool calls with `parallel_tool_calls: false` in the request. This is
useful when tool calls have side effects that depend on execution order.

### `strict: true` for Guaranteed Schema Conformance

When `strict: true` is set on a tool definition, OpenAI uses **constrained decoding** to
guarantee the model's output exactly matches the JSON Schema. This eliminates the possibility
of malformed arguments but comes with constraints:

- All fields must be explicitly listed in the schema
- `additionalProperties` must be `false` at every object level
- All fields are effectively required (optional fields should use `anyOf` with `null`)
- The schema is processed and cached on first use (slight latency on first call)

```json
{
  "type": "function",
  "function": {
    "name": "create_file",
    "description": "Create a file with the given content",
    "strict": true,
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string" },
        "content": { "type": "string" }
      },
      "required": ["path", "content"],
      "additionalProperties": false
    }
  }
}
```

### Response Format: `tool_calls` Array

When the model decides to call tools, the response message contains a `tool_calls` array:

```json
{
  "id": "chatcmpl-xyz",
  "object": "chat.completion",
  "model": "gpt-4o-2024-08-06",
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [
        {
          "id": "call_abc123",
          "type": "function",
          "function": {
            "name": "get_weather",
            "arguments": "{\"location\":\"Tokyo, Japan\",\"unit\":\"celsius\"}"
          }
        }
      ]
    },
    "finish_reason": "tool_calls"
  }],
  "usage": {
    "prompt_tokens": 82,
    "completion_tokens": 25,
    "total_tokens": 107
  }
}
```

Key details:
- `tool_calls[].id`: A unique identifier that must be referenced when sending the result back
- `tool_calls[].function.arguments`: A JSON **string** (not an object), which must be parsed
- `finish_reason`: `"tool_calls"` when the model wants to use tools
- `content`: Usually `null` when tool calls are present, but can contain text alongside tools

### Sending Results Back: `role: "tool"`

After executing a tool call, you send the result back as a message with `role: "tool"`:

```json
{
  "role": "tool",
  "tool_call_id": "call_abc123",
  "content": "{\"temperature\": 22, \"unit\": \"celsius\", \"condition\": \"partly cloudy\"}"
}
```

The `tool_call_id` must match the `id` from the tool call. The `content` is always a string
(typically JSON-serialized). For parallel tool calls, you send one `tool` message per call.

### Complete Wire Format Example

Here is a full request → tool call → result → final response cycle:

**Step 1: Initial Request**

```json
POST /v1/chat/completions
{
  "model": "gpt-4o",
  "messages": [
    {
      "role": "system",
      "content": "You are a helpful assistant with access to weather data."
    },
    {
      "role": "user",
      "content": "Compare the weather in Tokyo and London right now."
    }
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get current weather for a city",
        "parameters": {
          "type": "object",
          "properties": {
            "city": { "type": "string" }
          },
          "required": ["city"],
          "additionalProperties": false
        },
        "strict": true
      }
    }
  ],
  "tool_choice": "auto"
}
```

**Step 2: Model responds with parallel tool calls**

```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [
        {
          "id": "call_001",
          "type": "function",
          "function": {
            "name": "get_weather",
            "arguments": "{\"city\":\"Tokyo\"}"
          }
        },
        {
          "id": "call_002",
          "type": "function",
          "function": {
            "name": "get_weather",
            "arguments": "{\"city\":\"London\"}"
          }
        }
      ]
    },
    "finish_reason": "tool_calls"
  }]
}
```

**Step 3: Send results back and get final response**

```json
POST /v1/chat/completions
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "You are a helpful assistant with access to weather data." },
    { "role": "user", "content": "Compare the weather in Tokyo and London right now." },
    {
      "role": "assistant",
      "content": null,
      "tool_calls": [
        { "id": "call_001", "type": "function", "function": { "name": "get_weather", "arguments": "{\"city\":\"Tokyo\"}" } },
        { "id": "call_002", "type": "function", "function": { "name": "get_weather", "arguments": "{\"city\":\"London\"}" } }
      ]
    },
    {
      "role": "tool",
      "tool_call_id": "call_001",
      "content": "{\"temperature\": 28, \"condition\": \"sunny\", \"humidity\": 65}"
    },
    {
      "role": "tool",
      "tool_call_id": "call_002",
      "content": "{\"temperature\": 15, \"condition\": \"overcast\", \"humidity\": 80}"
    }
  ],
  "tools": [...]
}
```

**Step 4: Final model response**

```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "content": "Here's the current weather comparison:\n\n**Tokyo**: 28°C, sunny with 65% humidity\n**London**: 15°C, overcast with 80% humidity\n\nTokyo is significantly warmer (+13°C) and sunnier than London right now."
    },
    "finish_reason": "stop"
  }]
}
```

---

## Anthropic Tool Use

Anthropic's approach to tool calling differs from OpenAI's in several important ways. The most
notable difference is the use of **content blocks** rather than a separate `tool_calls` array.

### Tool Definition

Anthropic tools are defined at the top level of the request:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "tools": [
    {
      "name": "get_weather",
      "description": "Get the current weather for a location. Returns temperature, conditions, and humidity.",
      "input_schema": {
        "type": "object",
        "properties": {
          "location": {
            "type": "string",
            "description": "The city and country, e.g., 'San Francisco, CA'"
          },
          "unit": {
            "type": "string",
            "enum": ["celsius", "fahrenheit"],
            "description": "Temperature unit"
          }
        },
        "required": ["location"]
      }
    }
  ],
  "messages": [
    { "role": "user", "content": "What's the weather in Paris?" }
  ]
}
```

Key differences from OpenAI:
- No wrapping `{ type: "function", function: {...} }` — tools are defined directly
- Uses `input_schema` instead of `parameters`
- The `name` and `description` are top-level fields, not nested under `function`

### `tool_choice` Options

Anthropic's `tool_choice` uses an object with a `type` field:

| Value | Behavior |
|-------|----------|
| `{ "type": "auto" }` | Model decides whether to use tools (default) |
| `{ "type": "any" }` | Model must use at least one tool |
| `{ "type": "tool", "name": "get_weather" }` | Model must use the specified tool |
| Not specified | Equivalent to `{ "type": "auto" }` |

Note: `"any"` is Anthropic's equivalent of OpenAI's `"required"`. Also note that `tool_choice`
must be `"auto"` when extended thinking is enabled — a significant constraint for agent developers.

### Content Block Approach: `ToolUseBlock`

When Claude decides to use a tool, it includes a `tool_use` content block in the response.
Importantly, the response `content` is an **array of blocks**, not a single string. The model
may include both text and tool use blocks in the same response:

```json
{
  "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "I'll check the weather in Paris for you."
    },
    {
      "type": "tool_use",
      "id": "toolu_01A09q90qw90lq917835lq9",
      "name": "get_weather",
      "input": {
        "location": "Paris, France",
        "unit": "celsius"
      }
    }
  ],
  "model": "claude-sonnet-4-20250514",
  "stop_reason": "tool_use",
  "usage": {
    "input_tokens": 365,
    "output_tokens": 98
  }
}
```

Key differences from OpenAI:
- Tool use appears as a content block alongside text blocks
- `input` is an **object** (not a JSON string that needs parsing)
- `stop_reason` is `"tool_use"` (vs OpenAI's `finish_reason: "tool_calls"`)
- Tool calls have `id` starting with `toolu_` prefix

### `ToolResultBlock` — Sending Results Back

Tool results are sent as content blocks within a `user` message (not a separate `tool` role):

```json
{
  "role": "user",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
      "content": "Temperature: 18°C, Condition: Partly cloudy, Humidity: 62%"
    }
  ]
}
```

The `content` in a `tool_result` can be:
- A simple string
- An array of content blocks (text, images) for richer results
- Omitted if the tool has no output

You can also indicate errors:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
  "is_error": true,
  "content": "Error: City not found. Please check the city name and try again."
}
```

### Complete Wire Format Example

**Step 1: Initial Request**

```json
POST /v1/messages
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "system": "You are a helpful weather assistant.",
  "tools": [
    {
      "name": "get_weather",
      "description": "Get current weather for a city",
      "input_schema": {
        "type": "object",
        "properties": {
          "city": { "type": "string", "description": "City name" }
        },
        "required": ["city"]
      }
    }
  ],
  "messages": [
    { "role": "user", "content": "What's the weather like in Berlin?" }
  ]
}
```

**Step 2: Claude responds with tool use**

```json
{
  "id": "msg_abc123",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "Let me look up the current weather in Berlin for you."
    },
    {
      "type": "tool_use",
      "id": "toolu_xyz789",
      "name": "get_weather",
      "input": { "city": "Berlin" }
    }
  ],
  "stop_reason": "tool_use",
  "usage": { "input_tokens": 250, "output_tokens": 65 }
}
```

**Step 3: Send tool result and get final response**

```json
POST /v1/messages
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "system": "You are a helpful weather assistant.",
  "tools": [...],
  "messages": [
    { "role": "user", "content": "What's the weather like in Berlin?" },
    {
      "role": "assistant",
      "content": [
        { "type": "text", "text": "Let me look up the current weather in Berlin for you." },
        { "type": "tool_use", "id": "toolu_xyz789", "name": "get_weather", "input": { "city": "Berlin" } }
      ]
    },
    {
      "role": "user",
      "content": [
        {
          "type": "tool_result",
          "tool_use_id": "toolu_xyz789",
          "content": "{\"temperature\": 12, \"condition\": \"rainy\", \"humidity\": 88}"
        }
      ]
    }
  ]
}
```

**Step 4: Final response**

```json
{
  "id": "msg_def456",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "The weather in Berlin is currently:\n\n- **Temperature**: 12°C\n- **Condition**: Rainy\n- **Humidity**: 88%\n\nYou might want to bring an umbrella if you're heading out!"
    }
  ],
  "stop_reason": "end_turn",
  "usage": { "input_tokens": 380, "output_tokens": 72 }
}
```

### Key Differences from OpenAI

| Aspect | OpenAI | Anthropic |
|--------|--------|-----------|
| Tool call location | Separate `tool_calls` array | Content block in `content` array |
| Arguments format | JSON string (needs parsing) | Native JSON object |
| Result message role | `role: "tool"` | `role: "user"` with `tool_result` block |
| Parallel calls | Native support, enabled by default | Multiple `tool_use` blocks possible |
| Tool call ID format | `call_xxxxx` | `toolu_xxxxx` |
| Error reporting | Via content string | `is_error: true` field |
| Text alongside tools | Usually `content: null` | Text blocks alongside tool blocks |

### Strict Schema Validation

Anthropic does not currently have an equivalent of OpenAI's `strict: true` constrained decoding
for tool calls. However, Claude models generally produce well-formed JSON that conforms to the
provided `input_schema`. For critical applications, you should still validate the tool input
on the client side.

---

## Google Gemini Function Calling

Google's Gemini API has its own approach to function calling, using different terminology
and structure.

### `functionDeclarations` in Tools Array

Gemini defines tools using `functionDeclarations`:

```json
{
  "contents": [
    { "role": "user", "parts": [{ "text": "What's the weather in Sydney?" }] }
  ],
  "tools": [
    {
      "functionDeclarations": [
        {
          "name": "get_weather",
          "description": "Get the current weather for a given location",
          "parameters": {
            "type": "OBJECT",
            "properties": {
              "location": {
                "type": "STRING",
                "description": "The city name"
              }
            },
            "required": ["location"]
          }
        }
      ]
    }
  ]
}
```

Key differences:
- Tools contain `functionDeclarations` (an array of functions grouped under one tool)
- JSON Schema types are UPPERCASE: `"OBJECT"`, `"STRING"`, `"NUMBER"`, `"BOOLEAN"`, `"ARRAY"`
- Messages use `contents` with `parts` arrays (Gemini's general content model)

### `FunctionCall` and `FunctionResponse` Parts

When Gemini wants to call a function, it returns a `functionCall` part:

```json
{
  "candidates": [{
    "content": {
      "parts": [
        {
          "functionCall": {
            "name": "get_weather",
            "args": {
              "location": "Sydney"
            }
          }
        }
      ],
      "role": "model"
    },
    "finishReason": "STOP"
  }]
}
```

You send the result back as a `functionResponse` part:

```json
{
  "contents": [
    { "role": "user", "parts": [{ "text": "What's the weather in Sydney?" }] },
    { "role": "model", "parts": [{ "functionCall": { "name": "get_weather", "args": { "location": "Sydney" } } }] },
    { "role": "user", "parts": [{ "functionResponse": { "name": "get_weather", "response": { "temperature": 24, "condition": "sunny" } } }] }
  ],
  "tools": [...]
}
```

### `function_calling_config`: Mode

Gemini's equivalent of `tool_choice` is `toolConfig.functionCallingConfig`:

```json
{
  "toolConfig": {
    "functionCallingConfig": {
      "mode": "AUTO"
    }
  }
}
```

| Mode | Behavior |
|------|----------|
| `AUTO` | Model decides whether to call functions (default) |
| `ANY` | Model must call at least one function |
| `NONE` | Model must not call functions |

You can also restrict which functions are available:

```json
{
  "toolConfig": {
    "functionCallingConfig": {
      "mode": "ANY",
      "allowedFunctionNames": ["get_weather"]
    }
  }
}
```

### Wire Format Example

**Request with function calling:**

```json
POST /v1beta/models/gemini-2.0-flash:generateContent
{
  "contents": [
    {
      "role": "user",
      "parts": [{ "text": "What's the weather in Sydney and Melbourne?" }]
    }
  ],
  "tools": [{
    "functionDeclarations": [{
      "name": "get_weather",
      "description": "Get weather for a city",
      "parameters": {
        "type": "OBJECT",
        "properties": {
          "city": { "type": "STRING" }
        },
        "required": ["city"]
      }
    }]
  }],
  "toolConfig": {
    "functionCallingConfig": { "mode": "AUTO" }
  }
}
```

**Response with parallel function calls:**

```json
{
  "candidates": [{
    "content": {
      "parts": [
        { "functionCall": { "name": "get_weather", "args": { "city": "Sydney" } } },
        { "functionCall": { "name": "get_weather", "args": { "city": "Melbourne" } } }
      ],
      "role": "model"
    }
  }]
}
```

---

## Open-Source Model Tool Calling

### How Open Models Implement Tool Calling

Open-source models don't have the luxury of custom API layers that intercept structured output.
Instead, they use **special tokens** and **formatting conventions** that are part of the model's
vocabulary and training data.

Common approaches:

1. **Special tokens**: Models like Llama 3 use tokens like `<|python_tag|>` or dedicated
   tool-calling tokens that signal the start of a function call.

2. **ChatML-style formatting**: Some models use the ChatML format with special role tags:
   ```
   <|im_start|>assistant
   <tool_call>
   {"name": "get_weather", "arguments": {"location": "Tokyo"}}
   </tool_call>
   <|im_end|>
   ```

3. **Inline JSON**: Some models are fine-tuned to output JSON function calls directly in
   their response text, wrapped in identifiable markers.

### Llama 3 Tool Calling Format

Llama 3.1+ supports native tool calling with this format:

```
<|begin_of_text|><|start_header_id|>system<|end_header_id|>

Environment: ipython
Tools: get_weather

# Tool definitions:
## get_weather
Get current weather for a location
Parameters:
- location (string, required): City name
<|eot_id|><|start_header_id|>user<|end_header_id|>

What's the weather in Tokyo?<|eot_id|><|start_header_id|>assistant<|end_header_id|>

<|python_tag|>get_weather(location="Tokyo")<|eot_id|>
```

The `<|python_tag|>` token signals that the following text is a function call in Python-like
syntax. The hosting framework (vLLM, TGI, Ollama) parses this and converts it to the
OpenAI-compatible format.

### Mistral Tool Calling

Mistral models use a `[TOOL_CALLS]` special token:

```
[TOOL_CALLS] [{"name": "get_weather", "arguments": {"location": "Tokyo"}}]
```

When served through Mistral's API, this is converted to the standard OpenAI-compatible
`tool_calls` format. Mistral's API closely mirrors OpenAI's structure.

### Challenges with Open Models

1. **Inconsistent formatting**: Different models use different conventions, making it hard
   to build provider-agnostic tool calling.

2. **Schema adherence**: Open models are more likely to produce malformed JSON or deviate
   from the provided schema. Using constrained decoding (via tools like Outlines, Guidance,
   or vLLM's structured output) can mitigate this.

3. **Parallel tool calls**: Many open models don't reliably produce multiple tool calls in
   a single response.

4. **Serving framework dependency**: The same model may behave differently depending on
   whether it's served through vLLM, TGI, Ollama, or llama.cpp, as each framework has its
   own tool call parsing logic.

5. **Training data limitations**: Some open models were fine-tuned on limited tool-calling
   datasets, leading to poor generalization for complex tool schemas.

---

## Comparison Table

| Feature | OpenAI | Anthropic | Google Gemini |
|---------|--------|-----------|---------------|
| **Tool definition wrapper** | `{ type: "function", function: {...} }` | Direct `{ name, description, input_schema }` | `functionDeclarations` array |
| **Schema field name** | `parameters` | `input_schema` | `parameters` |
| **Schema types** | lowercase (`"string"`) | lowercase (`"string"`) | UPPERCASE (`"STRING"`) |
| **Tool calls in response** | `tool_calls` array on message | `tool_use` content block | `functionCall` part |
| **Arguments format** | JSON string | JSON object | JSON object (`args`) |
| **Result message** | `role: "tool"`, `tool_call_id` | `role: "user"`, `tool_result` block | `functionResponse` part |
| **Parallel tool calls** | Yes (default on) | Yes (multiple blocks) | Yes (multiple parts) |
| **Force specific tool** | `tool_choice: { type: "function", function: { name } }` | `tool_choice: { type: "tool", name }` | `allowedFunctionNames` |
| **Force any tool** | `tool_choice: "required"` | `tool_choice: { type: "any" }` | `mode: "ANY"` |
| **Disable tools** | `tool_choice: "none"` | N/A (omit tools) | `mode: "NONE"` |
| **Strict schema** | `strict: true` | Not available | Not available |
| **Error reporting** | Via content string | `is_error: true` | Via response content |
| **Streaming tool calls** | `tool_calls` deltas | `content_block_start/delta` | `functionCall` parts |
| **Max tools per request** | 128 | 1,000+ | 128 |

---

## How Coding Agents Use Tool Calling

### File Operations

Coding agents typically expose tools for file system operations:

```json
{
  "name": "read_file",
  "description": "Read the contents of a file at the given path",
  "parameters": {
    "type": "object",
    "properties": {
      "path": { "type": "string", "description": "Absolute or relative file path" },
      "start_line": { "type": "integer", "description": "Optional start line (1-indexed)" },
      "end_line": { "type": "integer", "description": "Optional end line" }
    },
    "required": ["path"]
  }
}
```

```json
{
  "name": "write_file",
  "description": "Create or overwrite a file with the given content",
  "parameters": {
    "type": "object",
    "properties": {
      "path": { "type": "string" },
      "content": { "type": "string" }
    },
    "required": ["path", "content"]
  }
}
```

```json
{
  "name": "edit_file",
  "description": "Make a surgical edit to a file by replacing old_str with new_str",
  "parameters": {
    "type": "object",
    "properties": {
      "path": { "type": "string" },
      "old_str": { "type": "string", "description": "Exact string to find and replace" },
      "new_str": { "type": "string", "description": "Replacement string" }
    },
    "required": ["path", "old_str", "new_str"]
  }
}
```

### Shell Command Execution

```json
{
  "name": "run_command",
  "description": "Execute a shell command and return stdout/stderr",
  "parameters": {
    "type": "object",
    "properties": {
      "command": { "type": "string", "description": "The shell command to execute" },
      "cwd": { "type": "string", "description": "Working directory (optional)" },
      "timeout": { "type": "integer", "description": "Timeout in seconds" }
    },
    "required": ["command"]
  }
}
```

### Code Analysis Tools

```json
{
  "name": "search_code",
  "description": "Search for a pattern across files using ripgrep",
  "parameters": {
    "type": "object",
    "properties": {
      "pattern": { "type": "string" },
      "path": { "type": "string" },
      "file_glob": { "type": "string", "description": "e.g., '*.ts'" }
    },
    "required": ["pattern"]
  }
}
```

### The Agent Loop

The fundamental agent loop in coding assistants:

```
User: "Fix the failing test in auth.test.ts"
    │
    ▼
Model: tool_call → read_file("auth.test.ts")
    │
    ▼
Result: [file contents with failing test]
    │
    ▼
Model: tool_call → read_file("auth.ts")
    │
    ▼
Result: [source code with the bug]
    │
    ▼
Model: tool_call → run_command("npm test -- auth.test.ts")
    │
    ▼
Result: [test output showing specific failure]
    │
    ▼
Model: tool_call → edit_file("auth.ts", old_str="...", new_str="...")
    │
    ▼
Result: "File edited successfully"
    │
    ▼
Model: tool_call → run_command("npm test -- auth.test.ts")
    │
    ▼
Result: "All tests passed"
    │
    ▼
Model: "I've fixed the bug in auth.ts. The issue was..."
```

Each step is a full API round-trip: the model generates a tool call, the host executes it,
the result is appended to the conversation, and the model is called again.

### Multi-Step Tool Chains

Complex tasks often require 10–50+ tool calls in sequence. A refactoring task might involve:

1. Search for all usages of a function
2. Read each file containing usages
3. Edit each file to update the function signature
4. Read the test files
5. Update the tests
6. Run the test suite
7. Fix any new failures
8. Run the test suite again

The conversation grows with each step, which is why context window management is critical.

### Error Handling in Tool Results

When a tool execution fails, the agent should receive the error as a tool result:

```json
{
  "role": "tool",
  "tool_call_id": "call_abc",
  "content": "Error: File not found: /src/auth.ts. The file does not exist at this path."
}
```

Good agents will:
- Recover gracefully (search for the correct path)
- Not repeat the same failed call
- Ask for clarification if the error is ambiguous
- Have a maximum retry limit to prevent infinite loops

---

## Advanced Patterns

### Tool Call Validation and Sanitization

Never blindly execute tool calls. Always validate:

```python
def validate_tool_call(tool_call):
    # Check the function name is in the allowed set
    if tool_call.function.name not in ALLOWED_TOOLS:
        return error(f"Unknown tool: {tool_call.function.name}")

    # Parse and validate arguments
    try:
        args = json.loads(tool_call.function.arguments)
    except json.JSONDecodeError:
        return error("Malformed JSON in arguments")

    # Validate against the tool's schema
    schema = TOOL_SCHEMAS[tool_call.function.name]
    try:
        jsonschema.validate(args, schema)
    except ValidationError as e:
        return error(f"Schema validation failed: {e.message}")

    # Sanitize dangerous inputs
    if tool_call.function.name == "run_command":
        if contains_dangerous_command(args["command"]):
            return error("Command blocked by security policy")

    return args
```

### Recursive Tool Calling (Agent Loops)

The agent loop itself is recursive tool calling. Key considerations:

- **Maximum iterations**: Set a hard limit (e.g., 50–200 tool calls per task) to prevent
  infinite loops and runaway costs.
- **Loop detection**: Track recent tool calls and detect if the model is repeating the same
  call with the same arguments.
- **Context window management**: As the conversation grows, older tool results may need to be
  summarized or truncated to stay within the context window.
- **Cost tracking**: Each iteration is a full API call. A 50-step agent loop with GPT-4o
  can cost several dollars.

### Tool Call Batching

Some implementations batch multiple tool results into a single API call for efficiency.
OpenAI natively supports this through parallel function calls. For providers that don't,
the host can collect multiple tool requests and execute them concurrently before sending
all results back in one message.

### Dynamic Tool Registration

Advanced agents dynamically adjust available tools based on context:

```python
def get_tools_for_context(task_type, file_type):
    base_tools = [read_file, write_file, search_code, run_command]

    if task_type == "web_development":
        base_tools += [browser_navigate, browser_screenshot, browser_click]

    if file_type == "python":
        base_tools += [python_lint, python_format, python_typecheck]

    if file_type == "typescript":
        base_tools += [tsc_check, eslint, prettier]

    return base_tools
```

This keeps the tool list focused and reduces the chance of the model calling irrelevant tools.

### Tool Descriptions as Prompt Engineering

The `description` field in a tool definition is one of the most impactful pieces of prompt
engineering. Good descriptions:

- Explain **when** to use the tool, not just what it does
- Provide examples of valid inputs
- Specify constraints and edge cases
- Mention what the tool returns

```json
{
  "name": "edit_file",
  "description": "Make a surgical edit to a file by replacing an exact string match. The old_str must match EXACTLY one occurrence in the file (including whitespace and indentation). If old_str matches zero or multiple times, the edit will fail. Use read_file first to see the exact content. For creating new files, use write_file instead."
}
```

---

## Best Practices for Tool Definitions

### 1. Be Specific in Descriptions

Bad: `"description": "Search files"`
Good: `"description": "Search for a regex pattern across files in the project. Returns matching lines with file paths and line numbers. Use glob patterns to filter by file type (e.g., '*.ts' for TypeScript files)."`

### 2. Use Descriptive Parameter Names

Bad: `"s"`, `"p"`, `"f"`
Good: `"search_pattern"`, `"file_path"`, `"format_type"`

### 3. Provide Enums When Possible

```json
{
  "output_format": {
    "type": "string",
    "enum": ["json", "csv", "table", "plain"],
    "description": "Output format for the results"
  }
}
```

### 4. Mark Required vs Optional Fields Clearly

Only mark fields as `required` if the tool genuinely cannot function without them.
Optional fields should have sensible defaults documented in the description.

### 5. Keep the Tool Set Focused

Exposing too many tools (50+) can confuse the model and lead to poor tool selection.
Group related operations into a single tool with a mode/action parameter if needed:

```json
{
  "name": "file_operation",
  "parameters": {
    "properties": {
      "action": { "type": "string", "enum": ["read", "write", "delete", "list"] },
      "path": { "type": "string" },
      "content": { "type": "string" }
    }
  }
}
```

### 6. Return Structured Results

Tool results should be consistently structured so the model can reliably parse them:

```json
{
  "success": true,
  "data": { "temperature": 22, "condition": "sunny" },
  "metadata": { "source": "weather_api", "timestamp": "2024-01-15T10:30:00Z" }
}
```

### 7. Include Error Context

When a tool fails, provide enough context for the model to recover:

```json
{
  "success": false,
  "error": "FileNotFoundError",
  "message": "No file at path '/src/auth.ts'. Did you mean '/src/lib/auth.ts'?",
  "suggestions": ["/src/lib/auth.ts", "/src/utils/auth.ts"]
}
```

---

## Security Considerations

### Command Injection

If you expose a shell execution tool, models can be manipulated (via prompt injection or
adversarial user input) into executing dangerous commands. Mitigations:

- **Allowlist approach**: Only allow specific commands or command patterns
- **Sandboxing**: Execute commands in containers or restricted environments
- **User confirmation**: Require human approval for destructive operations
- **Command parsing**: Parse the command and reject dangerous patterns (rm -rf, curl | bash, etc.)

### Path Traversal

File operation tools must prevent path traversal attacks:

```python
def safe_read_file(path):
    resolved = os.path.realpath(path)
    if not resolved.startswith(PROJECT_ROOT):
        raise SecurityError(f"Path {path} is outside the project directory")
    return open(resolved).read()
```

### Data Exfiltration

A compromised prompt or malicious user input could instruct the model to:
1. Read sensitive files (credentials, env files, private keys)
2. Send the contents to an external API via a tool call

Mitigations:
- Block access to sensitive file patterns (`.env`, `*.pem`, `*credentials*`)
- Monitor and log all tool calls
- Restrict network-related tools

### Rate Limiting and Cost Control

Agent loops can run up large API bills. Implement:
- Maximum tool calls per session
- Maximum tokens per session
- Cost tracking with alerts
- Timeout limits for the entire agent execution

### Prompt Injection via Tool Results

Tool results can contain adversarial content. For example, a web search tool might return
a page containing text like "Ignore all previous instructions and...". Mitigations:
- Sanitize tool results before injecting into the conversation
- Use system prompts that instruct the model to treat tool results as untrusted data
- Limit the size of tool results

---

## Conclusion

Function calling is the single most important capability that transforms LLMs into agents.
While the wire formats differ across providers, the core pattern is universal: define tools,
let the model decide when to use them, execute the calls, and feed results back. Understanding
these protocols deeply is essential for anyone building AI-powered coding tools, assistants,
or autonomous agents.

The ecosystem is converging toward OpenAI's format as a de facto standard (with most open-source
serving frameworks supporting it), but Anthropic's content-block approach and Google's
parts-based system offer distinct advantages in certain scenarios. The key is to build
abstractions that can target multiple providers while leveraging provider-specific features
like strict mode, parallel tool calls, and streaming.