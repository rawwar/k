// Chapter 2: First LLM Call — Code snapshot

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    // TODO: Define response fields matching the API schema
    id: Option<String>,
}

#[tokio::main]
async fn main() {
    println!("Chapter 2: First LLM Call");

    // TODO: Read API key from environment variable
    // TODO: Build the HTTP request to the LLM API
    // TODO: Send the request using reqwest
    // TODO: Parse and display the response

    let _client = reqwest::Client::new();

    println!("TODO: Make first API call to the LLM provider");
}
