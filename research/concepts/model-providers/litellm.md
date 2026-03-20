# LiteLLM — Unified Provider Interface

## Overview

LiteLLM is an open-source Python library and proxy server that provides a single,
unified interface to call 100+ LLM providers using the OpenAI format. For CLI coding
agents, LiteLLM serves as a critical abstraction layer that decouples the agent's
code from any specific model provider, enabling instant multi-provider support with
a single integration.

Among the 17 agents studied, **5 (29%)** use LiteLLM as their core LLM integration
layer:
- **Aider** — All model calls go through LiteLLM
- **OpenHands** — LLM layer wraps LiteLLM
- **mini-SWE-agent** — Single `litellm.completion()` call
- **Goose** — LiteLLM as one of many supported gateways
- Others access LiteLLM-supported models through the proxy

---

## Core Architecture

### SDK vs. Proxy

LiteLLM operates in two modes:

```
Mode 1: Python SDK (Direct Integration)
┌─────────────┐     ┌──────────┐     ┌──────────────┐
│   Agent     │────▶│ litellm  │────▶│ Provider API │
│ (Python)    │     │ .completion()│  │              │
└─────────────┘     └──────────┘     └──────────────┘

Mode 2: Proxy Server (Any Language)
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│   Agent     │────▶│ LiteLLM      │────▶│ Provider API │
│ (Any lang)  │     │ Proxy Server │     │              │
│             │     │ :4000        │     │              │
└─────────────┘     └──────────────┘     └──────────────┘
   OpenAI SDK           Translates          100+ providers
```

### How It Works

LiteLLM translates a single unified API call into the correct provider-specific format:

```python
from litellm import completion

# Same function, different providers
# The model string format is: "provider/model-name"

# OpenAI
response = completion(model="openai/gpt-4.1", messages=messages)

# Anthropic
response = completion(model="anthropic/claude-sonnet-4-6", messages=messages)

# Google
response = completion(model="gemini/gemini-2.5-pro", messages=messages)

# DeepSeek
response = completion(model="deepseek/deepseek-chat", messages=messages)

# Ollama (local)
response = completion(model="ollama/qwen2.5-coder:32b", messages=messages)

# AWS Bedrock
response = completion(model="bedrock/anthropic.claude-sonnet-4-6", messages=messages)

# Google Vertex AI
response = completion(model="vertex_ai/gemini-2.5-pro", messages=messages)

# Azure OpenAI
response = completion(model="azure/my-gpt4-deployment", messages=messages)
```

**Every response follows the same OpenAI format**, regardless of the underlying
provider. This is the key value proposition.

---

## Supported Providers (100+)

### Major Providers

| Provider | Model Prefix | Example |
|----------|-------------|---------|
| OpenAI | `openai/` | `openai/gpt-4.1` |
| Anthropic | `anthropic/` | `anthropic/claude-sonnet-4-6` |
| Google (AI Studio) | `gemini/` | `gemini/gemini-2.5-pro` |
| Google (Vertex AI) | `vertex_ai/` | `vertex_ai/gemini-2.5-pro` |
| AWS Bedrock | `bedrock/` | `bedrock/anthropic.claude-sonnet-4-6` |
| Azure OpenAI | `azure/` | `azure/my-deployment` |
| DeepSeek | `deepseek/` | `deepseek/deepseek-chat` |
| Mistral | `mistral/` | `mistral/mistral-large-latest` |
| Groq | `groq/` | `groq/llama-3.3-70b-versatile` |
| Together AI | `together_ai/` | `together_ai/meta-llama/Llama-3-70b` |
| Fireworks AI | `fireworks_ai/` | `fireworks_ai/llama-v3p3-70b` |
| Ollama | `ollama/` | `ollama/qwen2.5-coder:32b` |
| OpenRouter | `openrouter/` | `openrouter/anthropic/claude-sonnet-4-6` |

### Cloud Platforms

| Platform | Prefix | Notes |
|----------|--------|-------|
| AWS Bedrock | `bedrock/` | Full AWS IAM auth |
| AWS SageMaker | `sagemaker/` | Custom endpoints |
| Google Vertex AI | `vertex_ai/` | GCP IAM auth |
| Azure OpenAI | `azure/` | Azure AD auth |
| Azure AI | `azure_ai/` | Azure AI Studio |

### Local/Self-Hosted

| Engine | Prefix | Notes |
|--------|--------|-------|
| Ollama | `ollama/` | Auto-detect local server |
| vLLM | `hosted_vllm/` | OpenAI-compatible |
| llama.cpp | — | Via OpenAI-compatible |
| HuggingFace TGI | `huggingface/` | Text Generation Inference |

---

## Python SDK Usage

### Installation

```bash
pip install litellm
# Or with proxy support
pip install 'litellm[proxy]'
```

### Basic Completion

```python
import litellm
import os

# Set API keys via environment variables
os.environ["OPENAI_API_KEY"] = "sk-..."
os.environ["ANTHROPIC_API_KEY"] = "sk-ant-..."
os.environ["GEMINI_API_KEY"] = "..."

# Simple completion
response = litellm.completion(
    model="anthropic/claude-sonnet-4-6",
    messages=[
        {"role": "system", "content": "You are a coding assistant."},
        {"role": "user", "content": "Write a binary search in Python"}
    ],
    max_tokens=4096,
    temperature=0.1
)

print(response.choices[0].message.content)
print(f"Cost: ${response._hidden_params.get('response_cost', 0):.4f}")
```

### Streaming

```python
for chunk in litellm.completion(
    model="openai/gpt-4.1",
    messages=messages,
    stream=True
):
    content = chunk.choices[0].delta.content
    if content:
        print(content, end="", flush=True)
```

### Function Calling

```python
# Function calling works consistently across providers
tools = [
    {
        "type": "function",
        "function": {
            "name": "edit_file",
            "description": "Edit a file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }
        }
    }
]

# Same tool definition works for OpenAI, Anthropic, Google, etc.
response = litellm.completion(
    model="anthropic/claude-sonnet-4-6",
    messages=messages,
    tools=tools,
    tool_choice="auto"
)

# Tool calls are returned in OpenAI format regardless of provider
for tool_call in response.choices[0].message.tool_calls:
    print(f"Function: {tool_call.function.name}")
    print(f"Arguments: {tool_call.function.arguments}")
```

### Error Handling

LiteLLM maps all provider errors to OpenAI exception types:

```python
import litellm

try:
    response = litellm.completion(model="anthropic/claude-sonnet-4-6", messages=messages)
except litellm.AuthenticationError as e:
    print(f"Bad API key: {e}")
except litellm.RateLimitError as e:
    print(f"Rate limited: {e}")
except litellm.Timeout as e:
    print(f"Timeout: {e}")
except litellm.ServiceUnavailableError as e:
    print(f"Provider down: {e}")
except litellm.APIError as e:
    print(f"API error: {e}")
```

### Cost Tracking

```python
# Track cost per response
def track_cost(kwargs, completion_response, start_time, end_time):
    cost = kwargs.get("response_cost", 0)
    model = kwargs.get("model", "unknown")
    print(f"Model: {model}, Cost: ${cost:.6f}")

litellm.success_callback = [track_cost]

# Or use the completion_cost function
from litellm import completion_cost

response = litellm.completion(model="openai/gpt-4.1", messages=messages)
cost = completion_cost(completion_response=response)
print(f"This call cost: ${cost:.6f}")
```

---

## Proxy Server

The LiteLLM proxy server creates an OpenAI-compatible gateway that any agent (in any
programming language) can use:

### Starting the Proxy

```bash
# Simple start with a single model
litellm --model anthropic/claude-sonnet-4-6 --port 4000

# Start with a config file
litellm --config config.yaml --port 4000
```

### Proxy Configuration

```yaml
# config.yaml
model_list:
  - model_name: gpt-4
    litellm_params:
      model: openai/gpt-4.1
      api_key: sk-...
  
  - model_name: claude
    litellm_params:
      model: anthropic/claude-sonnet-4-6
      api_key: sk-ant-...
  
  - model_name: gemini
    litellm_params:
      model: gemini/gemini-2.5-pro
      api_key: ...
  
  - model_name: local
    litellm_params:
      model: ollama/qwen2.5-coder:32b
      api_base: http://localhost:11434

# Enable load balancing
  - model_name: claude-balanced
    litellm_params:
      model: anthropic/claude-sonnet-4-6
      api_key: sk-ant-key-1
  - model_name: claude-balanced
    litellm_params:
      model: anthropic/claude-sonnet-4-6
      api_key: sk-ant-key-2

general_settings:
  master_key: sk-litellm-master-key
  database_url: postgresql://...  # Optional: for cost tracking
```

### Using the Proxy

```python
# Any OpenAI SDK client works
from openai import OpenAI

client = OpenAI(
    api_key="sk-litellm-master-key",
    base_url="http://localhost:4000"
)

response = client.chat.completions.create(
    model="claude",  # Maps to anthropic/claude-sonnet-4-6 per config
    messages=[{"role": "user", "content": "Fix this bug..."}]
)
```

```bash
# Works with any agent that supports custom OpenAI endpoints
export OPENAI_API_KEY="sk-litellm-master-key"
export OPENAI_BASE_URL="http://localhost:4000"

# Now any OpenAI-compatible agent uses the proxy
codex --model claude
aider --model openai/claude  # Through proxy
```

---

## Load Balancing and Fallbacks

### Router Configuration

```python
from litellm import Router

router = Router(
    model_list=[
        {
            "model_name": "coding-model",
            "litellm_params": {
                "model": "anthropic/claude-sonnet-4-6",
                "api_key": "sk-ant-..."
            }
        },
        {
            "model_name": "coding-model",
            "litellm_params": {
                "model": "openai/gpt-4.1",
                "api_key": "sk-..."
            }
        },
        {
            "model_name": "coding-model",
            "litellm_params": {
                "model": "deepseek/deepseek-chat",
                "api_key": "..."
            }
        }
    ],
    routing_strategy="least-busy",  # Options: simple-shuffle, least-busy,
                                    # latency-based-routing, cost-based-routing
    num_retries=3,
    fallbacks=[
        {"coding-model": ["fallback-model"]}
    ],
    context_window_fallbacks=[
        {"coding-model": ["large-context-model"]}  # Fallback for context overflow
    ]
)

response = await router.acompletion(
    model="coding-model",
    messages=messages
)
```

### Routing Strategies

| Strategy | Description | Best For |
|----------|-------------|---------|
| `simple-shuffle` | Random distribution | Basic load balancing |
| `least-busy` | Route to least loaded deployment | Even distribution |
| `latency-based-routing` | Route to fastest responding | Minimizing latency |
| `cost-based-routing` | Route to cheapest available | Minimizing cost |

### Fallback Configuration

```yaml
# config.yaml with fallbacks
model_list:
  - model_name: primary
    litellm_params:
      model: anthropic/claude-sonnet-4-6
      api_key: ${ANTHROPIC_API_KEY}

  - model_name: fallback
    litellm_params:
      model: openai/gpt-4.1
      api_key: ${OPENAI_API_KEY}

router_settings:
  routing_strategy: latency-based-routing
  num_retries: 3
  timeout: 60
  
  # If primary fails, try fallback
  fallbacks:
    - primary: [fallback]
  
  # If context too large, switch to large-context model
  context_window_fallbacks:
    - primary: [gemini-large]
```

---

## How Agents Use LiteLLM

### Aider

Aider uses LiteLLM as its core LLM abstraction:

```python
# Simplified from Aider source (aider/sendchat.py)
import litellm

def send_chat(model, messages, functions=None, stream=True):
    """Aider's core LLM call — everything goes through LiteLLM."""
    kwargs = {
        "model": model,
        "messages": messages,
        "stream": stream,
    }
    if functions:
        kwargs["functions"] = functions
    
    response = litellm.completion(**kwargs)
    return response
```

```bash
# Aider model selection (all via LiteLLM)
aider --model gpt-4.1              # OpenAI
aider --model claude-sonnet-4-6    # Anthropic
aider --model gemini/gemini-2.5-pro # Google
aider --model deepseek/deepseek-chat # DeepSeek
aider --model ollama/qwen2.5-coder:32b # Local
aider --model openrouter/anthropic/claude-sonnet-4-6 # OpenRouter
```

### OpenHands

OpenHands wraps LiteLLM for all LLM operations:

```python
# Simplified from OpenHands source (openhands/llm/llm.py)
import litellm

class LLM:
    def __init__(self, model: str, api_key: str = None, base_url: str = None):
        self.model = model
        self.api_key = api_key
        self.base_url = base_url
    
    def completion(self, messages, **kwargs):
        return litellm.completion(
            model=self.model,
            messages=messages,
            api_key=self.api_key,
            api_base=self.base_url,
            **kwargs
        )
```

### mini-SWE-agent

mini-SWE-agent has the simplest possible LiteLLM integration:

```python
# From mini-SWE-agent source (litellm_model.py)
import litellm

class LiteLLMModel:
    def __init__(self, model_name: str):
        self.model_name = model_name
    
    def generate(self, messages: list[dict]) -> str:
        response = litellm.completion(
            model=self.model_name,
            messages=messages
        )
        return response.choices[0].message.content
```

### Goose

Goose lists LiteLLM as one of its supported gateway providers:

```yaml
# Goose configuration for LiteLLM proxy
GOOSE_PROVIDER=litellm
GOOSE_API_BASE=http://localhost:4000
GOOSE_MODEL=coding-model
```

---

## Caching

LiteLLM provides built-in caching to reduce costs and latency:

```python
import litellm
from litellm.caching import Cache

# In-memory cache
litellm.cache = Cache()

# Redis cache (persistent)
litellm.cache = Cache(
    type="redis",
    host="localhost",
    port=6379,
    ttl=3600  # 1 hour
)

# S3 cache (for serverless)
litellm.cache = Cache(
    type="s3",
    s3_bucket_name="my-litellm-cache",
    s3_region_name="us-east-1"
)

# Cached responses are returned without making an API call
response1 = litellm.completion(model="openai/gpt-4.1", messages=messages)
response2 = litellm.completion(model="openai/gpt-4.1", messages=messages)
# response2 comes from cache — zero cost, instant response
```

### Cache Configuration for Proxy

```yaml
# config.yaml
litellm_settings:
  cache: True
  cache_params:
    type: redis
    host: localhost
    port: 6379
    ttl: 3600
```

---

## Observability and Logging

LiteLLM integrates with observability platforms:

```python
import litellm

# Send logs to multiple platforms simultaneously
litellm.success_callback = ["langfuse", "helicone", "lunary"]
litellm.failure_callback = ["langfuse", "helicone"]

# Custom callbacks
def my_callback(kwargs, completion_response, start_time, end_time):
    model = kwargs.get("model")
    cost = kwargs.get("response_cost", 0)
    latency = (end_time - start_time).total_seconds()
    tokens = completion_response.usage.total_tokens
    
    log_to_dashboard(model=model, cost=cost, latency=latency, tokens=tokens)

litellm.success_callback = [my_callback]
```

---

## Virtual Keys and Budget Management

The proxy server supports virtual API keys for team management:

```yaml
# config.yaml
general_settings:
  master_key: sk-litellm-master
  database_url: postgresql://...

# Create virtual keys via API
# POST /key/generate
# {
#   "models": ["claude", "gpt-4"],
#   "max_budget": 50.0,
#   "budget_duration": "monthly",
#   "metadata": {"team": "backend"}
# }
```

```python
# Team members use their virtual keys
client = OpenAI(
    api_key="sk-team-backend-key-123",
    base_url="http://litellm-proxy:4000"
)
```

---

## Configuration Patterns for Coding Agents

### Pattern 1: Simple Direct Integration

```python
# For Python agents — just use litellm.completion()
import litellm

response = litellm.completion(
    model=os.environ.get("MODEL", "anthropic/claude-sonnet-4-6"),
    messages=messages
)
```

### Pattern 2: Multi-Model with Routing

```python
# Different models for different tasks
router = Router(model_list=[
    {"model_name": "planning", "litellm_params": {"model": "anthropic/claude-opus-4-6"}},
    {"model_name": "coding", "litellm_params": {"model": "anthropic/claude-sonnet-4-6"}},
    {"model_name": "review", "litellm_params": {"model": "deepseek/deepseek-chat"}},
])

plan = await router.acompletion(model="planning", messages=planning_messages)
code = await router.acompletion(model="coding", messages=coding_messages)
review = await router.acompletion(model="review", messages=review_messages)
```

### Pattern 3: Proxy for Non-Python Agents

```bash
# Start proxy
litellm --config config.yaml --port 4000

# Go agent connects via OpenAI SDK
# (OpenCode, for example)
export OPENAI_BASE_URL="http://localhost:4000"
export OPENAI_API_KEY="sk-litellm-key"
```

### Pattern 4: Fallback Chain for Reliability

```python
router = Router(
    model_list=[
        {"model_name": "main", "litellm_params": {"model": "anthropic/claude-sonnet-4-6"}},
        {"model_name": "backup-1", "litellm_params": {"model": "openai/gpt-4.1"}},
        {"model_name": "backup-2", "litellm_params": {"model": "deepseek/deepseek-chat"}},
    ],
    fallbacks=[{"main": ["backup-1", "backup-2"]}],
    num_retries=2
)
```

---

## Strengths and Limitations

### Strengths

| Strength | Details |
|----------|---------|
| **100+ providers** | Largest provider coverage of any abstraction |
| **OpenAI format** | Consistent response format simplifies agent code |
| **Proxy server** | Any language can use it via OpenAI SDK |
| **Cost tracking** | Built-in per-request cost calculation |
| **Fallbacks** | Automatic provider failover |
| **Load balancing** | Distribute requests across deployments |
| **Caching** | Built-in response caching |
| **Observability** | Langfuse, Helicone, etc. integrations |
| **Active development** | Frequent updates for new providers/models |

### Limitations

| Limitation | Details |
|-----------|---------|
| **Feature lag** | New provider features take time to be supported |
| **Abstraction cost** | Provider-specific optimizations may be lost |
| **Python dependency** | SDK requires Python; proxy needed for other languages |
| **Complexity** | Router/proxy config can be complex |
| **Tool call translation** | Anthropic/Google tool formats may have edge cases |
| **Extended thinking** | Provider-specific features may need special handling |
| **Prompt caching** | Anthropic's explicit cache breakpoints need careful handling |

### Provider Feature Coverage

| Feature | OpenAI | Anthropic | Google | Ollama |
|---------|--------|-----------|--------|--------|
| Basic completion | ✅ | ✅ | ✅ | ✅ |
| Streaming | ✅ | ✅ | ✅ | ✅ |
| Function calling | ✅ | ✅ | ✅ | ⚠️ |
| Vision | ✅ | ✅ | ✅ | ⚠️ |
| Prompt caching | ⚠️ | ✅ | ⚠️ | ❌ |
| Extended thinking | ❌ | ✅ | ⚠️ | ❌ |
| Batch API | ✅ | ✅ | ⚠️ | ❌ |

---

## See Also

- [Model Routing](model-routing.md) — Advanced routing strategies
- [API Patterns](api-patterns.md) — Error handling and retry logic
- [Pricing and Cost](pricing-and-cost.md) — Cost tracking and optimization
- [Agent Comparison](agent-comparison.md) — Which agents use LiteLLM