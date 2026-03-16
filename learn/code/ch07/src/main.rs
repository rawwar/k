// Chapter 7: Streaming — Code snapshot

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    // TODO: Define all SSE event types:
    //   message_start, content_block_start, content_block_delta,
    //   content_block_stop, message_delta, message_stop
}

/// Process a streaming response from the API.
async fn handle_stream(_response: reqwest::Response) {
    // TODO: Read the response body as a stream of SSE events
    // TODO: Parse each event and handle by type
    // TODO: Print text deltas as they arrive
    // TODO: Accumulate tool_use blocks for execution

    println!("TODO: Process streaming SSE events");
}

#[tokio::main]
async fn main() {
    println!("Chapter 7: Streaming");

    let _client = reqwest::Client::new();

    // TODO: Send a request with "stream": true
    // TODO: Pass the response to handle_stream
    // TODO: Integrate streaming into the agentic loop

    println!("TODO: Stream LLM responses token by token");
}
