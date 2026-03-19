# Batch Processing APIs for LLMs

## Introduction

Batch processing APIs allow you to submit large volumes of LLM requests as a single
job rather than making individual synchronous API calls. This matters for several reasons:

- **Cost efficiency**: Both OpenAI and Anthropic offer a 50% discount on batch requests
  compared to their synchronous counterparts.
- **Throughput**: Batch APIs have separate, often higher, rate limits — you can process
  thousands of requests without hitting per-minute token or request caps.
- **Reliability**: The provider handles retries, queuing, and orchestration internally.
  You submit a job and poll for results instead of managing concurrent connections.
- **Simplicity**: For offline workloads (evals, bulk analysis, embedding generation),
  batch processing eliminates the need for complex client-side concurrency, retry logic,
  and rate-limit backoff code.

The trade-off is latency. Batch requests are processed at lower priority than real-time
API calls, typically completing within a 24-hour window. This makes them ideal for any
workflow where you do not need results in real time.

---

## OpenAI Batch API

### Overview

The OpenAI Batch API lets you send groups of requests as a single batch job. Each batch
is backed by a JSONL file where every line is an independent API request. OpenAI processes
the file asynchronously and produces an output file with the results.

**Base endpoint**: `POST https://api.openai.com/v1/batches`

### Supported Endpoints

Batches can target the following OpenAI API endpoints:

| Endpoint | Description |
|---|---|
| `/v1/chat/completions` | Chat completions (GPT-4o, GPT-4.1, etc.) |
| `/v1/embeddings` | Text embeddings (text-embedding-3-small, etc.) |
| `/v1/responses` | Responses API (structured outputs, tool use) |

### How It Works

The workflow has three phases:

1. **Upload** a JSONL input file via the Files API.
2. **Create** a batch referencing that file.
3. **Poll** the batch status until it completes, then download results.

```
┌──────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│  Upload  │────▶│ Create Batch │────▶│ Poll Status │────▶│  Download    │
│  JSONL   │     │   (POST)     │     │   (GET)     │     │  Results     │
└──────────┘     └──────────────┘     └─────────────┘     └──────────────┘
```

### Input File Format

The input file is JSONL (one JSON object per line). Each line must include:

| Field | Type | Description |
|---|---|---|
| `custom_id` | string | Your unique identifier for this request (for correlating results) |
| `method` | string | HTTP method — always `"POST"` |
| `url` | string | The API endpoint path, e.g. `"/v1/chat/completions"` |
| `body` | object | The request body, identical to what you would send synchronously |

**Example input file** (`batch_input.jsonl`):

```jsonl
{"custom_id": "req-1", "method": "POST", "url": "/v1/chat/completions", "body": {"model": "gpt-4o", "messages": [{"role": "user", "content": "Explain the CAP theorem in 2 sentences."}], "max_tokens": 200}}
{"custom_id": "req-2", "method": "POST", "url": "/v1/chat/completions", "body": {"model": "gpt-4o", "messages": [{"role": "user", "content": "What is a monad in functional programming?"}], "max_tokens": 200}}
{"custom_id": "req-3", "method": "POST", "url": "/v1/embeddings", "body": {"model": "text-embedding-3-small", "input": "Batch processing is efficient for large-scale LLM workloads."}}
```

### Step 1: Upload the Input File

```bash
curl https://api.openai.com/v1/files \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -F purpose="batch" \
  -F file="@batch_input.jsonl"
```

Response (truncated):

```json
{
  "id": "file-abc123",
  "object": "file",
  "purpose": "batch",
  "filename": "batch_input.jsonl",
  "bytes": 1024,
  "status": "processed"
}
```

### Step 2: Create the Batch

```bash
curl https://api.openai.com/v1/batches \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "input_file_id": "file-abc123",
    "endpoint": "/v1/chat/completions",
    "completion_window": "24h"
  }'
```

Request body fields:

| Field | Type | Required | Description |
|---|---|---|---|
| `input_file_id` | string | Yes | The uploaded JSONL file ID |
| `endpoint` | string | Yes | Target API endpoint |
| `completion_window` | string | Yes | Currently only `"24h"` is supported |
| `metadata` | object | No | Optional key-value metadata for your records |

### Step 3: Check Batch Status

```bash
curl https://api.openai.com/v1/batches/batch_xyz789 \
  -H "Authorization: Bearer $OPENAI_API_KEY"
```

### Batch Status Lifecycle

```
validating ──▶ in_progress ──▶ completed
                    │              │
                    ▼              ▼
                 failed       (download results)
                    │
                    ▼
              cancelling ──▶ cancelled
                               │
                               ▼
                            expired
```

| Status | Description |
|---|---|
| `validating` | Input file is being validated |
| `in_progress` | Batch is actively being processed |
| `completed` | All requests finished; results available |
| `failed` | The batch could not be processed (e.g. invalid input file) |
| `expired` | Did not complete within the 24-hour window |
| `cancelling` | Cancellation requested; in-flight requests finishing |
| `cancelled` | Batch was cancelled; partial results may be available |

### Step 4: Retrieve Results

Once the batch status is `completed`, the response object includes an `output_file_id`.
Download it via the Files API:

```bash
curl https://api.openai.com/v1/files/file-output456/content \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -o batch_output.jsonl
```

Each line of the output file contains:

```json
{
  "id": "batch_req_abc123",
  "custom_id": "req-1",
  "response": {
    "status_code": 200,
    "request_id": "req_abc123",
    "body": {
      "id": "chatcmpl-xyz",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "The CAP theorem states that..."
          }
        }
      ]
    }
  },
  "error": null
}
```

### Error Handling

Errors can occur at two levels:

1. **Batch-level errors**: The entire batch fails (e.g. malformed JSONL). Check
   the `errors` field on the batch object and the `error_file_id` for details.
2. **Request-level errors**: Individual requests within the batch fail. These appear
   in the output file with a non-200 `status_code` and a populated `error` field.

```json
{
  "id": "batch_req_fail",
  "custom_id": "req-99",
  "response": null,
  "error": {
    "code": "content_filter",
    "message": "Content was flagged by the content filter."
  }
}
```

Always iterate through the output file and check both `response.status_code` and the
`error` field for each result.

### Cost Savings and Rate Limits

- **50% discount**: Batch API requests cost half the price of synchronous requests
  for the same model and token count.
- **Separate rate limits**: Batch API has its own token-per-minute and request-per-day
  quotas that do not count against your synchronous rate limits. This effectively
  doubles your available throughput.
- **24-hour completion window**: OpenAI guarantees that batches complete within 24 hours.
  In practice many batches finish in minutes to a few hours depending on load.

---

## Anthropic Message Batches API

### Overview

Anthropic's Message Batches API provides similar batch processing capabilities for
Claude models. You submit an array of message requests and retrieve results once
processing completes.

**Base endpoint**: `POST https://api.anthropic.com/v1/messages/batches`

### Request Format

Unlike OpenAI (which uses a file upload), Anthropic accepts batch requests directly
in the request body:

```json
{
  "requests": [
    {
      "custom_id": "eval-001",
      "params": {
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [
          {
            "role": "user",
            "content": "Explain the difference between TCP and UDP."
          }
        ]
      }
    },
    {
      "custom_id": "eval-002",
      "params": {
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [
          {
            "role": "user",
            "content": "What is the time complexity of merge sort?"
          }
        ]
      }
    }
  ]
}
```

Each item in the `requests` array has:

| Field | Type | Description |
|---|---|---|
| `custom_id` | string | Your unique identifier (max 64 chars) |
| `params` | object | A complete Messages API request body (model, messages, max_tokens, etc.) |

The `params` object supports everything the synchronous Messages API supports:
system prompts, tool use, images, streaming parameters (ignored in batch), and so on.

### Creating a Batch

```bash
curl https://api.anthropic.com/v1/messages/batches \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d '{
    "requests": [
      {
        "custom_id": "req-1",
        "params": {
          "model": "claude-sonnet-4-20250514",
          "max_tokens": 512,
          "messages": [{"role": "user", "content": "Hello, Claude!"}]
        }
      }
    ]
  }'
```

Response:

```json
{
  "id": "msgbatch_abc123",
  "type": "message_batch",
  "processing_status": "in_progress",
  "request_counts": {
    "processing": 1,
    "succeeded": 0,
    "errored": 0,
    "canceled": 0,
    "expired": 0
  },
  "created_at": "2024-09-01T12:00:00Z",
  "ended_at": null,
  "expires_at": "2024-09-02T12:00:00Z",
  "results_url": null
}
```

### Batch Lifecycle

Anthropic uses a simpler status model:

```
created ──▶ processing ──▶ ended
```

| Status | Description |
|---|---|
| `created` | Batch has been accepted |
| `processing` | Requests are being processed |
| `ended` | All requests have been processed (check individual result types) |

When a batch reaches `ended`, each individual result has one of these types:
- `succeeded` — the request completed normally
- `errored` — the request encountered an error
- `canceled` — the request was canceled (if you canceled the batch)
- `expired` — the request expired before processing

### Checking Batch Status

```bash
curl https://api.anthropic.com/v1/messages/batches/msgbatch_abc123 \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01"
```

### Retrieving Results

Results are streamed as JSONL from a dedicated results endpoint:

```bash
curl https://api.anthropic.com/v1/messages/batches/msgbatch_abc123/results \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -o batch_results.jsonl
```

Each line of the results file:

```json
{
  "custom_id": "eval-001",
  "result": {
    "type": "succeeded",
    "message": {
      "id": "msg_abc",
      "type": "message",
      "role": "assistant",
      "content": [
        {
          "type": "text",
          "text": "TCP is a connection-oriented protocol..."
        }
      ],
      "model": "claude-sonnet-4-20250514",
      "stop_reason": "end_turn",
      "usage": {
        "input_tokens": 18,
        "output_tokens": 150
      }
    }
  }
}
```

### Cost Savings and Priority

- **50% discount**: Like OpenAI, Anthropic offers batch requests at half the price
  of synchronous Messages API calls.
- **Lower priority**: Batch requests are processed at lower priority than real-time
  requests. During peak load, batch processing may slow down while interactive
  requests are prioritized.
- **Expiration**: Batches that cannot complete within the allotted time window will
  have remaining requests marked as `expired`.

---

## Comparison: OpenAI vs Anthropic Batch APIs

| Feature | OpenAI Batch API | Anthropic Message Batches API |
|---|---|---|
| **Endpoint** | `POST /v1/batches` | `POST /v1/messages/batches` |
| **Input method** | Upload JSONL file, reference by file ID | Inline JSON array in request body |
| **Max requests per batch** | 50,000 | 100,000 |
| **Request format** | `{custom_id, method, url, body}` per line | `{custom_id, params}` per item |
| **Supported models** | GPT-4o, GPT-4.1, GPT-4o-mini, etc. | Claude Opus, Sonnet, Haiku |
| **Supported endpoints** | Chat completions, embeddings, responses | Messages only |
| **Status model** | validating → in_progress → completed/failed/expired/cancelled | created → processing → ended |
| **Result delivery** | Download output file via Files API | Stream JSONL from results endpoint |
| **Completion window** | 24 hours | 24 hours |
| **Cost discount** | 50% | 50% |
| **Rate limits** | Separate from sync limits | Separate from sync limits |
| **Cancellation** | Yes (partial results available) | Yes (unprocessed requests marked canceled) |
| **Metadata support** | Yes (key-value on batch object) | No |

---

## Use Cases for Coding Agents

### Evaluation and Benchmark Runs

Running models against benchmark suites like SWE-bench, HumanEval, or MBPP involves
thousands of independent prompts. Batch APIs are ideal here: submit the entire eval
set as one batch and collect results in a single output file. The 50% cost savings
compound significantly at scale — a 2,000-problem eval that costs $200 synchronously
costs $100 via batch.

### Bulk Code Review

Automated code review across many pull requests or files can be parallelized trivially
with batch APIs. Each request contains a file diff and a review prompt. Results can be
parsed and posted as review comments.

### Repository-Wide Refactoring Analysis

When analyzing an entire repository for refactoring opportunities (e.g. finding all
uses of a deprecated pattern, suggesting type annotations, or identifying code smells),
you can batch one request per file or module and process the entire codebase in a
single job.

### Generating Embeddings for Large Codebases

Building a code search index or RAG system requires embedding every file, function, or
chunk in a repository. The OpenAI Batch API supports `/v1/embeddings`, making it
straightforward to embed thousands of code chunks at half the cost.

### Testing Prompt Variations

When iterating on system prompts, testing multiple temperature settings, or comparing
model versions, batch APIs let you run all variations in a single job and compare
results systematically.

---

## Code Examples

### Python: OpenAI Batch API

```python
import json
import time
from openai import OpenAI

client = OpenAI()

# 1. Prepare the input JSONL file
requests = [
    {
        "custom_id": f"review-{i}",
        "method": "POST",
        "url": "/v1/chat/completions",
        "body": {
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are a code reviewer."},
                {"role": "user", "content": f"Review this code:\n```python\n{code}\n```"},
            ],
            "max_tokens": 1000,
        },
    }
    for i, code in enumerate(code_snippets)
]

input_path = "batch_input.jsonl"
with open(input_path, "w") as f:
    for req in requests:
        f.write(json.dumps(req) + "\n")

# 2. Upload the file
with open(input_path, "rb") as f:
    uploaded = client.files.create(file=f, purpose="batch")

print(f"Uploaded file: {uploaded.id}")

# 3. Create the batch
batch = client.batches.create(
    input_file_id=uploaded.id,
    endpoint="/v1/chat/completions",
    completion_window="24h",
    metadata={"project": "code-review", "run": "v1"},
)

print(f"Batch created: {batch.id}, status: {batch.status}")

# 4. Poll for completion
while batch.status not in ("completed", "failed", "expired", "cancelled"):
    time.sleep(30)
    batch = client.batches.retrieve(batch.id)
    completed = batch.request_counts.completed
    total = batch.request_counts.total
    print(f"Status: {batch.status} ({completed}/{total})")

# 5. Download and process results
if batch.status == "completed":
    result_file = client.files.content(batch.output_file_id)
    results = []
    for line in result_file.text.strip().split("\n"):
        result = json.loads(line)
        custom_id = result["custom_id"]
        if result["error"] is None:
            content = result["response"]["body"]["choices"][0]["message"]["content"]
            results.append({"id": custom_id, "review": content})
        else:
            results.append({"id": custom_id, "error": result["error"]})

    print(f"Processed {len(results)} results")

# 6. Handle errors (check error file if present)
if batch.error_file_id:
    error_file = client.files.content(batch.error_file_id)
    for line in error_file.text.strip().split("\n"):
        error = json.loads(line)
        print(f"Error for {error['custom_id']}: {error['error']}")
```

### Python: Anthropic Message Batches API

```python
import json
import time
import anthropic

client = anthropic.Anthropic()

# 1. Prepare batch requests
batch_requests = [
    {
        "custom_id": f"eval-{i}",
        "params": {
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "system": "You are an expert software engineer. Solve the given coding problem.",
            "messages": [
                {"role": "user", "content": problem["prompt"]},
            ],
        },
    }
    for i, problem in enumerate(eval_problems)
]

# 2. Create the batch
batch = client.messages.batches.create(requests=batch_requests)

print(f"Batch created: {batch.id}, status: {batch.processing_status}")

# 3. Poll for completion
while batch.processing_status != "ended":
    time.sleep(30)
    batch = client.messages.batches.retrieve(batch.id)
    counts = batch.request_counts
    print(
        f"Status: {batch.processing_status} "
        f"(succeeded={counts.succeeded}, processing={counts.processing})"
    )

# 4. Stream and process results
results = {}
for result in client.messages.batches.results(batch.id):
    custom_id = result.custom_id
    if result.result.type == "succeeded":
        message = result.result.message
        text = message.content[0].text
        results[custom_id] = {
            "response": text,
            "input_tokens": message.usage.input_tokens,
            "output_tokens": message.usage.output_tokens,
        }
    elif result.result.type == "errored":
        results[custom_id] = {"error": str(result.result.error)}
    else:
        results[custom_id] = {"status": result.result.type}

print(f"Processed {len(results)} results")
print(f"Succeeded: {sum(1 for r in results.values() if 'response' in r)}")
print(f"Errors: {sum(1 for r in results.values() if 'error' in r)}")
```

### TypeScript: OpenAI Batch API

```typescript
import OpenAI from "openai";
import * as fs from "fs";

const client = new OpenAI();

async function runBatch(prompts: { id: string; content: string }[]) {
  // 1. Write JSONL input file
  const lines = prompts.map((p) =>
    JSON.stringify({
      custom_id: p.id,
      method: "POST",
      url: "/v1/chat/completions",
      body: {
        model: "gpt-4o",
        messages: [{ role: "user", content: p.content }],
        max_tokens: 500,
      },
    })
  );
  fs.writeFileSync("batch_input.jsonl", lines.join("\n") + "\n");

  // 2. Upload the file
  const file = await client.files.create({
    file: fs.createReadStream("batch_input.jsonl"),
    purpose: "batch",
  });

  // 3. Create batch
  const batch = await client.batches.create({
    input_file_id: file.id,
    endpoint: "/v1/chat/completions",
    completion_window: "24h",
  });

  console.log(`Batch ${batch.id} created (status: ${batch.status})`);

  // 4. Poll for completion
  let current = batch;
  while (!["completed", "failed", "expired", "cancelled"].includes(current.status)) {
    await new Promise((r) => setTimeout(r, 30_000));
    current = await client.batches.retrieve(batch.id);
    console.log(`Status: ${current.status}`);
  }

  // 5. Download results
  if (current.status === "completed" && current.output_file_id) {
    const output = await client.files.content(current.output_file_id);
    const text = await output.text();
    const results = text
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));

    for (const result of results) {
      if (result.error) {
        console.error(`${result.custom_id}: ${result.error.message}`);
      } else {
        const content = result.response.body.choices[0].message.content;
        console.log(`${result.custom_id}: ${content.slice(0, 100)}...`);
      }
    }
    return results;
  }

  throw new Error(`Batch ended with status: ${current.status}`);
}
```

### TypeScript: Anthropic Message Batches API

```typescript
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic();

async function runAnthropicBatch(
  items: { id: string; prompt: string }[]
): Promise<Map<string, string>> {
  // 1. Create the batch
  const batch = await client.messages.batches.create({
    requests: items.map((item) => ({
      custom_id: item.id,
      params: {
        model: "claude-sonnet-4-20250514",
        max_tokens: 1024,
        messages: [{ role: "user", content: item.prompt }],
      },
    })),
  });

  console.log(`Batch ${batch.id} created`);

  // 2. Poll for completion
  let current = batch;
  while (current.processing_status !== "ended") {
    await new Promise((r) => setTimeout(r, 30_000));
    current = await client.messages.batches.retrieve(batch.id);
    console.log(
      `Status: ${current.processing_status}, ` +
      `succeeded: ${current.request_counts.succeeded}, ` +
      `processing: ${current.request_counts.processing}`
    );
  }

  // 3. Collect results
  const results = new Map<string, string>();
  for await (const result of client.messages.batches.results(batch.id)) {
    if (result.result.type === "succeeded") {
      const text = result.result.message.content
        .filter((block): block is Anthropic.TextBlock => block.type === "text")
        .map((block) => block.text)
        .join("");
      results.set(result.custom_id, text);
    } else {
      console.error(`${result.custom_id}: ${result.result.type}`);
    }
  }

  return results;
}
```

---

## Best Practices

### 1. Use Meaningful `custom_id` Values

Your `custom_id` is the only way to correlate results back to inputs. Use structured
IDs that encode useful metadata:

```
eval-humaneval-042-gpt4o-temp0.7
review-pr-1234-file-src/auth.ts
embed-repo-main-chunk-0042
```

### 2. Validate Input Before Submitting

Malformed input files cause the entire batch to fail. Validate your JSONL locally:

```python
import json

with open("batch_input.jsonl") as f:
    for i, line in enumerate(f, 1):
        try:
            obj = json.loads(line)
            assert "custom_id" in obj, "missing custom_id"
            assert "method" in obj, "missing method"
            assert "url" in obj, "missing url"
            assert "body" in obj, "missing body"
        except (json.JSONDecodeError, AssertionError) as e:
            print(f"Line {i}: {e}")
```

### 3. Implement Exponential Backoff for Polling

Don't poll too aggressively. Start with 30-second intervals and increase for larger
batches:

```python
poll_interval = 30  # seconds
max_interval = 300  # 5 minutes

while batch.status == "in_progress":
    time.sleep(poll_interval)
    batch = client.batches.retrieve(batch.id)
    poll_interval = min(poll_interval * 1.5, max_interval)
```

### 4. Handle Partial Results

When a batch is cancelled or expires, some results may still be available. Always
check for the output file even on non-`completed` statuses:

```python
if batch.output_file_id:
    # Process whatever results are available
    result_file = client.files.content(batch.output_file_id)
    # ...
```

### 5. Chunk Large Workloads

If you have more requests than the batch limit allows (50K for OpenAI, 100K for
Anthropic), split them into multiple batches and run them concurrently:

```python
CHUNK_SIZE = 50_000
batches = []
for i in range(0, len(all_requests), CHUNK_SIZE):
    chunk = all_requests[i : i + CHUNK_SIZE]
    batch = create_batch(chunk)
    batches.append(batch)

# Poll all batches
while any(b.status == "in_progress" for b in batches):
    time.sleep(60)
    batches = [client.batches.retrieve(b.id) for b in batches]
```

### 6. Store Input-Output Mappings

Save both the input and output files together so you can debug issues later:

```python
import shutil
from datetime import datetime

run_id = datetime.now().strftime("%Y%m%d_%H%M%S")
shutil.copy("batch_input.jsonl", f"runs/{run_id}_input.jsonl")
shutil.copy("batch_output.jsonl", f"runs/{run_id}_output.jsonl")
```

### 7. Monitor Costs

Track token usage from batch results to monitor spend:

```python
total_input_tokens = 0
total_output_tokens = 0

for line in open("batch_output.jsonl"):
    result = json.loads(line)
    if result.get("response") and result["response"]["status_code"] == 200:
        usage = result["response"]["body"].get("usage", {})
        total_input_tokens += usage.get("prompt_tokens", 0)
        total_output_tokens += usage.get("completion_tokens", 0)

print(f"Input tokens: {total_input_tokens:,}")
print(f"Output tokens: {total_output_tokens:,}")
```

### 8. Use Batch APIs for A/B Testing Models

Batch APIs are efficient for comparing models side-by-side. Submit the same
prompts to different models (or the same model with different parameters) as
separate batches and compare results:

```python
for model in ["gpt-4o", "gpt-4o-mini", "gpt-4.1"]:
    requests = build_requests(prompts, model=model)
    batch = create_and_run_batch(requests, metadata={"model": model})
```

---

## Summary

Batch processing APIs are essential tools for any LLM workflow that involves
processing large numbers of requests where real-time responses are not required.
Both OpenAI and Anthropic offer mature batch APIs with 50% cost savings, separate
rate limits, and straightforward workflows. The key differences are in input format
(file upload vs. inline JSON) and status model granularity, but both achieve the
same goal: efficient, cost-effective bulk LLM processing.