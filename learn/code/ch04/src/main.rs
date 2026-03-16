// Chapter 4: Building a Tool System — Code snapshot
//
// Builds on Chapter 3's agentic loop by adding an extensible tool architecture:
// - A Tool trait defining name(), description(), input_schema(), and execute()
// - A ToolRegistry backed by HashMap<String, Box<dyn Tool>> for O(1) lookup
// - Tool dispatch that matches tool_use calls from the LLM to registered tools
// - Two example tools: get_current_time and calculator
// - Formatting tool definitions as JSON for the API's `tools` parameter
// - An agentic loop that sends tools to the API, dispatches tool calls, and
//   feeds results back as observations

use std::collections::HashMap;
use std::fmt;
use std::panic;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// ToolError — categorizes the ways tool execution can fail
// ---------------------------------------------------------------------------

#[derive(Debug)]
#[allow(dead_code)]
enum ToolError {
    /// The model sent arguments that do not match the tool's expectations.
    InvalidInput(String),
    /// The tool ran but the operation failed (e.g., file not found).
    ExecutionFailed(String),
    /// An infrastructure-level problem (panic, timeout, resource exhaustion).
    SystemError(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            ToolError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            ToolError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

impl std::error::Error for ToolError {}

// ---------------------------------------------------------------------------
// Tool trait — the contract every tool must satisfy
// ---------------------------------------------------------------------------

trait Tool: Send + Sync {
    /// Returns the unique name of this tool (must match the API tool definition).
    fn name(&self) -> &str;

    /// Returns a human-readable description the LLM reads to decide when to
    /// use this tool.
    fn description(&self) -> &str;

    /// Returns the JSON Schema describing valid input parameters.
    fn input_schema(&self) -> Value;

    /// Execute the tool with the given input and return the result.
    fn execute(&self, input: &Value) -> Result<String, ToolError>;
}

// ---------------------------------------------------------------------------
// ToolRegistry — stores registered tools and generates API definitions
// ---------------------------------------------------------------------------

struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    fn new() -> Self {
        ToolRegistry {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Returns the previous tool if one with the same name
    /// was already registered.
    fn register(&mut self, tool: Box<dyn Tool>) -> Option<Box<dyn Tool>> {
        let name = tool.name().to_string();
        self.tools.insert(name, tool)
    }

    /// Look up a tool by name.
    fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Return the number of registered tools.
    fn len(&self) -> usize {
        self.tools.len()
    }

    /// Return an iterator over all registered tool names.
    fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(|s| s.as_str())
    }

    /// Generate the `tools` array for the Anthropic Messages API request.
    /// Each element contains `name`, `description`, and `input_schema`.
    fn tool_definitions(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|tool| {
                json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.input_schema()
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Dispatch types — connect LLM tool_use requests to tool implementations
// ---------------------------------------------------------------------------

/// Represents a tool_use content block from the assistant's response.
#[derive(Debug, Deserialize)]
struct ToolUse {
    id: String,
    name: String,
    input: Value,
}

/// Represents the result of executing a tool, ready to send back as a
/// tool_result content block.
#[derive(Debug, Serialize)]
struct ToolResult {
    tool_use_id: String,
    content: String,
    is_error: bool,
}

/// Maximum characters in a tool result before truncation.
const MAX_OUTPUT_CHARS: usize = 50_000;

/// Truncate a string to a maximum number of characters, appending a notice
/// if truncation occurred.
fn truncate_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        output.to_string()
    } else {
        let truncated = &output[..max_chars];
        format!(
            "{}\n\n[Output truncated. Showing first {} of {} characters.]",
            truncated, max_chars, output.len()
        )
    }
}

/// Dispatch a single tool call: look up the tool, execute it with panic
/// recovery and timing, truncate the output, and return a ToolResult.
fn dispatch_tool_call(registry: &ToolRegistry, tool_use: &ToolUse) -> ToolResult {
    let start = Instant::now();

    // Stage 1: Registry lookup
    let tool = match registry.get(&tool_use.name) {
        Some(t) => t,
        None => {
            let available: Vec<&str> = registry.tool_names().collect();
            eprintln!(
                "[dispatch] Unknown tool '{}' (available: {:?}) [{:?}]",
                tool_use.name, available, start.elapsed()
            );
            return ToolResult {
                tool_use_id: tool_use.id.clone(),
                content: format!(
                    "Error: Unknown tool '{}'. Available tools: {:?}",
                    tool_use.name, available
                ),
                is_error: true,
            };
        }
    };

    // Stage 2: Execute with panic recovery
    let exec_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        tool.execute(&tool_use.input)
    }));

    let duration = start.elapsed();
    eprintln!("[dispatch] {} completed in {:?}", tool_use.name, duration);

    // Stage 3: Format the result
    match exec_result {
        Ok(Ok(output)) => {
            let content = truncate_output(&output, MAX_OUTPUT_CHARS);
            ToolResult {
                tool_use_id: tool_use.id.clone(),
                content,
                is_error: false,
            }
        }
        Ok(Err(e)) => ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: e.to_string(),
            is_error: true,
        },
        Err(_panic) => ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: format!("System error: tool '{}' panicked during execution", tool_use.name),
            is_error: true,
        },
    }
}

/// Dispatch all tool calls from a single assistant response.
fn dispatch_all(registry: &ToolRegistry, tool_uses: &[ToolUse]) -> Vec<ToolResult> {
    tool_uses
        .iter()
        .map(|tu| dispatch_tool_call(registry, tu))
        .collect()
}

// ---------------------------------------------------------------------------
// Observation formatting — convert ToolResults into API messages
// ---------------------------------------------------------------------------

/// Convert a single ToolResult into a tool_result content block.
fn tool_result_to_content_block(result: &ToolResult) -> Value {
    let mut block = json!({
        "type": "tool_result",
        "tool_use_id": result.tool_use_id,
        "content": result.content,
    });
    if result.is_error {
        block["is_error"] = json!(true);
    }
    block
}

/// Convert multiple ToolResults into a user message containing tool_result
/// content blocks.
fn build_tool_results_message(results: &[ToolResult]) -> Value {
    let content: Vec<Value> = results.iter().map(tool_result_to_content_block).collect();
    json!({
        "role": "user",
        "content": content
    })
}

// ---------------------------------------------------------------------------
// Example Tool 1: GetCurrentTimeTool
// ---------------------------------------------------------------------------

struct GetCurrentTimeTool;

impl Tool for GetCurrentTimeTool {
    fn name(&self) -> &str {
        "get_current_time"
    }

    fn description(&self) -> &str {
        "Get the current date and time. Optionally specify a format string \
         (chrono strftime syntax). Defaults to RFC 3339 / ISO 8601."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "description": "Optional strftime format string, e.g. \"%Y-%m-%d %H:%M:%S\". Defaults to RFC 3339."
                }
            },
            "required": []
        })
    }

    fn execute(&self, input: &Value) -> Result<String, ToolError> {
        let now = chrono::Local::now();

        let formatted = match input.get("format").and_then(|v| v.as_str()) {
            Some(fmt_str) => {
                let result = now.format(fmt_str).to_string();
                if result == fmt_str {
                    // chrono::format returns the format string unchanged when
                    // it contains no valid specifiers — treat as invalid.
                    return Err(ToolError::InvalidInput(format!(
                        "Format string '{}' did not produce any formatted output. \
                         Use strftime specifiers like %Y, %m, %d, %H, %M, %S.",
                        fmt_str
                    )));
                }
                result
            }
            None => now.to_rfc3339(),
        };

        Ok(formatted)
    }
}

// ---------------------------------------------------------------------------
// Example Tool 2: CalculatorTool
// ---------------------------------------------------------------------------

struct CalculatorTool;

impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Evaluate a basic arithmetic expression with two operands. \
         Supports addition (+), subtraction (-), multiplication (*), \
         and division (/). Returns the numeric result as a string."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": {
                    "type": "number",
                    "description": "The first operand."
                },
                "b": {
                    "type": "number",
                    "description": "The second operand."
                },
                "operation": {
                    "type": "string",
                    "description": "The arithmetic operation to perform.",
                    "enum": ["+", "-", "*", "/"]
                }
            },
            "required": ["a", "b", "operation"]
        })
    }

    fn execute(&self, input: &Value) -> Result<String, ToolError> {
        let a = input
            .get("a")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                ToolError::InvalidInput(
                    "Missing or invalid required field 'a' (number).".to_string(),
                )
            })?;

        let b = input
            .get("b")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                ToolError::InvalidInput(
                    "Missing or invalid required field 'b' (number).".to_string(),
                )
            })?;

        let op = input
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidInput(
                    "Missing or invalid required field 'operation' (string: +, -, *, /).".to_string(),
                )
            })?;

        let result = match op {
            "+" => Ok(a + b),
            "-" => Ok(a - b),
            "*" => Ok(a * b),
            "/" => {
                if b == 0.0 {
                    Err(ToolError::ExecutionFailed(
                        "Division by zero is not allowed.".to_string(),
                    ))
                } else {
                    Ok(a / b)
                }
            }
            other => Err(ToolError::InvalidInput(format!(
                "Unknown operation '{}'. Supported operations: +, -, *, /",
                other
            ))),
        }?;

        // Format nicely: drop the ".0" suffix for whole numbers.
        if result.fract() == 0.0 && result.abs() < 1e15 {
            Ok(format!("{}", result as i64))
        } else {
            Ok(format!("{}", result))
        }
    }
}

// ---------------------------------------------------------------------------
// Registry construction
// ---------------------------------------------------------------------------

fn create_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(GetCurrentTimeTool));
    registry.register(Box::new(CalculatorTool));
    registry
}

// ---------------------------------------------------------------------------
// API interaction — builds on ch03's agentic loop
// ---------------------------------------------------------------------------

/// Call the Anthropic Messages API with the given messages and tool
/// definitions.
async fn call_api(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[Value],
    tools: &[Value],
) -> Result<Value, Box<dyn std::error::Error>> {
    let body = json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 4096,
        "tools": tools,
        "messages": messages
    });

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?
        .json::<Value>()
        .await?;

    Ok(response)
}

/// Extract ToolUse structs from the content array of an assistant response.
fn extract_tool_uses(response: &Value) -> Vec<ToolUse> {
    let mut tool_uses = Vec::new();

    if let Some(content) = response.get("content").and_then(|c| c.as_array()) {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                if let (Some(id), Some(name), Some(input)) = (
                    block.get("id").and_then(|v| v.as_str()),
                    block.get("name").and_then(|v| v.as_str()),
                    block.get("input"),
                ) {
                    tool_uses.push(ToolUse {
                        id: id.to_string(),
                        name: name.to_string(),
                        input: input.clone(),
                    });
                }
            }
        }
    }

    tool_uses
}

/// Extract any text content from the assistant response for display.
fn extract_text(response: &Value) -> String {
    let mut texts = Vec::new();

    if let Some(content) = response.get("content").and_then(|c| c.as_array()) {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    texts.push(text.to_string());
                }
            }
        }
    }

    texts.join("\n")
}

/// Run the agentic loop: send messages + tool definitions to the API, dispatch
/// tool calls, feed results back, and repeat until the model stops calling
/// tools.
async fn agentic_loop(
    client: &reqwest::Client,
    api_key: &str,
    messages: &mut Vec<Value>,
    registry: &ToolRegistry,
) -> Result<String, Box<dyn std::error::Error>> {
    let tools = registry.tool_definitions();
    let max_iterations = 10;

    for iteration in 0..max_iterations {
        eprintln!("[loop] Iteration {} — sending {} message(s)", iteration, messages.len());

        // 1. Call the API
        let response = call_api(client, api_key, messages, &tools).await?;

        // Check for API errors
        if let Some(error) = response.get("error") {
            return Err(format!("API error: {}", error).into());
        }

        let stop_reason = response
            .get("stop_reason")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown");

        eprintln!("[loop] Stop reason: {}", stop_reason);

        // 2. If the model is done (end_turn), return the final text
        if stop_reason == "end_turn" {
            let text = extract_text(&response);
            if !text.is_empty() {
                println!("\nAssistant: {}", text);
            }
            return Ok(text);
        }

        // 3. If the model wants to use tools, dispatch them
        if stop_reason == "tool_use" {
            // Print any thinking text the model included alongside tool calls
            let text = extract_text(&response);
            if !text.is_empty() {
                println!("\nAssistant: {}", text);
            }

            // Extract tool_use blocks
            let tool_uses = extract_tool_uses(&response);
            eprintln!("[loop] Dispatching {} tool call(s)", tool_uses.len());

            for tu in &tool_uses {
                eprintln!("[loop]   -> {}({})", tu.name, tu.input);
            }

            // Dispatch all tool calls
            let tool_results = dispatch_all(registry, &tool_uses);

            for tr in &tool_results {
                let status = if tr.is_error { "ERROR" } else { "OK" };
                eprintln!("[loop]   <- {} [{}]: {:.120}", tr.tool_use_id, status, tr.content);
            }

            // 4. Append the assistant message to conversation history
            messages.push(json!({
                "role": "assistant",
                "content": response.get("content").cloned().unwrap_or(json!([]))
            }));

            // 5. Append tool results as a user message
            let results_message = build_tool_results_message(&tool_results);
            messages.push(results_message);

            // Loop continues — the API will see the tool results on the next call
        } else {
            // Unexpected stop reason — print what we got and exit
            let text = extract_text(&response);
            if !text.is_empty() {
                println!("\nAssistant: {}", text);
            }
            return Ok(text);
        }
    }

    Err(format!("Agentic loop exceeded {} iterations", max_iterations).into())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("Chapter 4: Building a Tool System");
    println!("==================================\n");

    // Build the tool registry
    let registry = create_registry();
    println!(
        "Registered {} tool(s):",
        registry.len()
    );
    for name in registry.tool_names() {
        println!("  - {}", name);
    }

    // Show the tool definitions that will be sent to the API
    let definitions = registry.tool_definitions();
    println!(
        "\nTool definitions for API:\n{}",
        serde_json::to_string_pretty(&definitions).unwrap()
    );

    // Demonstrate local tool dispatch without hitting the API
    println!("\n--- Local dispatch demo ---\n");

    let demo_calls = vec![
        ToolUse {
            id: "demo_01".to_string(),
            name: "get_current_time".to_string(),
            input: json!({}),
        },
        ToolUse {
            id: "demo_02".to_string(),
            name: "calculator".to_string(),
            input: json!({"a": 6, "b": 7, "operation": "*"}),
        },
        ToolUse {
            id: "demo_03".to_string(),
            name: "calculator".to_string(),
            input: json!({"a": 10, "b": 0, "operation": "/"}),
        },
        ToolUse {
            id: "demo_04".to_string(),
            name: "nonexistent".to_string(),
            input: json!({}),
        },
    ];

    let results = dispatch_all(&registry, &demo_calls);
    for result in &results {
        let status = if result.is_error { "ERROR" } else { "OK" };
        println!(
            "  [{}] {}: {}",
            status, result.tool_use_id, result.content
        );
    }

    // Show how results are formatted for the API
    let results_msg = build_tool_results_message(&results);
    println!(
        "\nFormatted tool_result message:\n{}",
        serde_json::to_string_pretty(&results_msg).unwrap()
    );

    // Run the full agentic loop if an API key is available
    println!("\n--- Agentic loop ---\n");

    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        println!(
            "Set ANTHROPIC_API_KEY to run the agentic loop with the real API.\n\
             Skipping API call. The local dispatch demo above shows the tool \n\
             system working end-to-end."
        );
        return;
    }

    let client = reqwest::Client::new();
    let mut messages = vec![json!({
        "role": "user",
        "content": "What time is it right now? Also, what is 144 divided by 12?"
    })];

    match agentic_loop(&client, &api_key, &mut messages, &registry).await {
        Ok(_) => println!("\nAgentic loop completed successfully."),
        Err(e) => eprintln!("\nAgentic loop error: {}", e),
    }
}
