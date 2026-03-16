// Chapter 4: Tool System — Code snapshot

use serde_json::Value;

/// The core trait that all tools must implement.
trait Tool {
    /// The unique name of this tool (used in API tool definitions).
    fn name(&self) -> &str;

    /// A human-readable description for the LLM.
    fn description(&self) -> &str;

    /// The JSON Schema for this tool's input parameters.
    fn input_schema(&self) -> Value;

    /// Execute the tool with the given input and return the result.
    fn execute(&self, input: &Value) -> Result<String, String>;
}

// TODO: Implement a ToolRegistry that holds registered tools
// TODO: Implement tool dispatch — match tool name from API response to registered tool
// TODO: Convert tool definitions to the API's expected format

fn main() {
    println!("Chapter 4: Tool System");
    println!("TODO: Define tools and wire them into the agentic loop");
}
