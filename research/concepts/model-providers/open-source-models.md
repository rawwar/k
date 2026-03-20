# Open-Source and Local Models for Coding Agents

## Overview

Open-source and locally-hosted models represent the privacy-first, cost-free
alternative to cloud providers for CLI coding agents. While frontier cloud models
(Claude, GPT-4, Gemini) dominate in raw capability, open-source models have reached
a quality level where they are viable for many coding tasks—especially when self-hosted
for zero marginal cost, unlimited rate limits, and complete data privacy.

Among the 17 agents studied, **5 (29%)** explicitly support local/open-source models,
typically through Ollama or OpenAI-compatible API endpoints. Many more can use them
through LiteLLM or custom endpoint configuration.

---

## The Open-Source Model Landscape for Coding

### Tier 1: Frontier Open Models (70B+)

| Model | Parameters | Context | Coding Quality | License |
|-------|-----------|---------|---------------|---------|
| **Llama 3.1 405B** | 405B | 128K | Near GPT-4 | Llama 3.1 License |
| **Llama 3.3 70B** | 70B | 128K | Strong | Llama 3.3 License |
| **Qwen2.5-Coder-32B** | 32B | 128K | Excellent (code-specialized) | Apache 2.0 |
| **DeepSeek-R1-Distill-70B** | 70B | 128K | Very strong + reasoning | MIT |
| **Mistral Large** | ~123B | 128K | Strong | Mistral License |
| **CodeLlama 70B** | 70B | 100K | Good (code-specialized) | Llama 2 License |

### Tier 2: Efficient Models (7B-32B)

| Model | Parameters | Context | Coding Quality | License |
|-------|-----------|---------|---------------|---------|
| **Qwen2.5-Coder-7B** | 7B | 128K | Good for size | Apache 2.0 |
| **Qwen2.5-Coder-14B** | 14B | 128K | Very good for size | Apache 2.0 |
| **DeepSeek-R1-Distill-32B** | 32B | 128K | Excellent + reasoning | MIT |
| **CodeLlama 34B** | 34B | 100K | Good | Llama 2 License |
| **Mistral Nemo 12B** | 12B | 128K | Decent | Apache 2.0 |
| **StarCoder2-15B** | 15B | 16K | Good (code-only) | BigCode License |
| **Llama 3.2 3B** | 3B | 128K | Basic | Llama 3.2 License |

### Tier 3: Tiny Models (< 7B)

| Model | Parameters | Context | Coding Quality | License |
|-------|-----------|---------|---------------|---------|
| **Qwen2.5-Coder-1.5B** | 1.5B | 128K | Limited | Apache 2.0 |
| **DeepSeek-R1-Distill-1.5B** | 1.5B | 128K | Limited + reasoning | MIT |
| **StarCoder2-3B** | 3B | 16K | Basic (code-only) | BigCode License |
| **TinyLlama** | 1.1B | 2K | Minimal | Apache 2.0 |

---

## Key Model Families

### Llama 3 (Meta)

Meta's Llama 3 family is the most widely used open-source foundation model:

```bash
# Available sizes
ollama pull llama3.3:70b    # Best quality, needs ~45GB RAM
ollama pull llama3.2:3b     # Smallest, runs on laptop
ollama pull llama3.1:405b   # Largest, needs multi-GPU
```

**Strengths:**
- Strong general-purpose coding ability
- Large community and ecosystem
- Available in multiple sizes (1B to 405B)
- 128K context window across all sizes
- Good instruction following

**Limitations:**
- Not specialized for code (general-purpose)
- 405B requires significant infrastructure
- Function calling less reliable than specialized models

### Qwen2.5-Coder (Alibaba)

The best open-source code-specialized model family:

```bash
# Qwen2.5-Coder variants
ollama pull qwen2.5-coder:32b  # Best quality, ~20GB RAM
ollama pull qwen2.5-coder:14b  # Good balance, ~9GB RAM
ollama pull qwen2.5-coder:7b   # Fast, ~5GB RAM
ollama pull qwen2.5-coder:1.5b # Tiny, ~1.5GB RAM
```

**Strengths:**
- **Best-in-class coding** for open models at each size
- Trained specifically on code across 92 programming languages
- 128K context window
- Apache 2.0 license (most permissive)
- Outperforms many larger general-purpose models on coding tasks
- Strong function calling support

**Why Qwen2.5-Coder stands out:**

| Model | Size | HumanEval | MBPP | SWE-bench Lite |
|-------|------|-----------|------|----------------|
| Qwen2.5-Coder-32B | 32B | 92.7% | 83.5% | ~25% |
| CodeLlama-34B | 34B | 67.8% | 72.6% | ~15% |
| DeepSeek-Coder-33B | 33B | 79.3% | 80.4% | ~20% |
| Llama 3.1-70B | 70B | 80.5% | 82.3% | ~22% |

The 32B Qwen2.5-Coder matches or exceeds models 2x its size.

### CodeLlama (Meta)

Meta's code-specialized variant of Llama 2:

```bash
ollama pull codellama:70b      # Best quality
ollama pull codellama:34b      # Good balance
ollama pull codellama:13b      # Lightweight
ollama pull codellama:7b       # Fastest
```

**Strengths:**
- Specifically trained for code generation and understanding
- Fill-in-the-middle (FIM) support for code completion
- 100K context window (larger than base Llama 2)
- Instruct variants for conversational coding

**Limitations:**
- Based on older Llama 2 architecture
- Largely superseded by Qwen2.5-Coder and newer models
- Smaller community momentum

### StarCoder2 (BigCode)

Open-source model trained exclusively on code:

```bash
ollama pull starcoder2:15b
ollama pull starcoder2:7b
ollama pull starcoder2:3b
```

**Strengths:**
- Trained on The Stack v2 (massive code dataset)
- Strong code completion (FIM native)
- Available in 3B, 7B, 15B sizes
- Good for IDE-style code completion

**Limitations:**
- 16K context (much smaller than alternatives)
- Less capable at instruction following
- Better suited for completion than conversation
- Not ideal for agentic tool use

### Mistral (Mistral AI)

Mistral offers competitive open-source models:

```bash
ollama pull mistral:7b         # Mistral 7B
ollama pull mistral-nemo:12b   # Nemo 12B
ollama pull mixtral:8x7b       # Mixtral MoE
```

**Strengths:**
- Efficient architectures (sliding window attention)
- Mixtral pioneered open-source MoE models
- Good multilingual support
- Solid instruction following

---

## Local Inference Engines

### Ollama

The most popular way for coding agents to run local models:

```bash
# Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh

# Pull and run a model
ollama pull qwen2.5-coder:32b
ollama run qwen2.5-coder:32b

# Ollama exposes an OpenAI-compatible API
curl http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen2.5-coder:32b",
    "messages": [
      {"role": "user", "content": "Write a binary search in Python"}
    ]
  }'
```

**Why agents love Ollama:**
- **OpenAI-compatible API** — Works with any agent that supports custom endpoints
- **Easy installation** — Single command setup on Mac, Linux, Windows
- **Model management** — Pull, run, and manage models like Docker images
- **GPU acceleration** — Automatic CUDA/Metal/ROCm detection
- **Multiple models** — Run different models on the same server
- **No API keys** — Completely local, no authentication needed

**Agent integration examples:**

```bash
# Codex CLI with Ollama
export OPENAI_BASE_URL="http://localhost:11434/v1"
export OPENAI_API_KEY="ollama"
codex --model qwen2.5-coder:32b

# Aider with Ollama (via LiteLLM)
aider --model ollama/qwen2.5-coder:32b

# Goose with Ollama
export GOOSE_PROVIDER=ollama
export GOOSE_MODEL=qwen2.5-coder:32b
```

### vLLM

High-performance inference engine for production deployments:

```bash
# Install vLLM
pip install vllm

# Start server
python -m vllm.entrypoints.openai.api_server \
    --model Qwen/Qwen2.5-Coder-32B-Instruct \
    --tensor-parallel-size 2 \
    --max-model-len 32768 \
    --port 8000

# vLLM also exposes an OpenAI-compatible API
curl http://localhost:8000/v1/chat/completions \
    -H "Content-Type: application/json" \
    -d '{"model": "Qwen/Qwen2.5-Coder-32B-Instruct", "messages": [...]}'
```

**Advantages over Ollama:**
- **Higher throughput** — Optimized for concurrent requests
- **PagedAttention** — More efficient memory management
- **Tensor parallelism** — Easy multi-GPU scaling
- **Continuous batching** — Better utilization under load
- **Speculative decoding** — Faster generation with draft models

**When to use vLLM over Ollama:**
- Multiple users sharing the same model
- Production deployment with SLA requirements
- Need for maximum tokens/second throughput
- Multi-GPU setups

### llama.cpp

The most efficient option for running models on consumer hardware:

```bash
# Build llama.cpp
git clone https://github.com/ggerganov/llama.cpp
cd llama.cpp && make -j

# Run a server (OpenAI-compatible)
./llama-server \
    -m models/qwen2.5-coder-32b-q4_k_m.gguf \
    --port 8080 \
    --n-gpu-layers 35 \
    --ctx-size 32768

# Use with any OpenAI-compatible agent
export OPENAI_BASE_URL="http://localhost:8080/v1"
```

**Why llama.cpp matters:**
- **CPU inference** — Runs without a GPU (slower but works)
- **Quantization** — Q4/Q5/Q8 quantized models use 2-4x less memory
- **Metal acceleration** — Excellent on Apple Silicon Macs
- **GGUF format** — Universal model format, widely available
- **Minimal dependencies** — Pure C/C++, no Python runtime needed

### Comparison of Inference Engines

| Feature | Ollama | vLLM | llama.cpp |
|---------|--------|------|-----------|
| **Setup difficulty** | Very easy | Moderate | Moderate |
| **OpenAI compat API** | ✅ | ✅ | ✅ |
| **GPU support** | CUDA, Metal, ROCm | CUDA | CUDA, Metal, ROCm, Vulkan |
| **CPU inference** | ✅ | ❌ | ✅ (optimized) |
| **Quantization** | ✅ (GGUF) | ✅ (GPTQ, AWQ) | ✅ (GGUF, native) |
| **Multi-GPU** | ✅ | ✅ (best) | ✅ |
| **Throughput** | Good | Best | Good |
| **Memory efficiency** | Good | Best | Good |
| **Model management** | Built-in | Manual | Manual |
| **Best for** | Development | Production | Consumer hardware |

---

## Performance vs. Cloud Models

### Coding Benchmark Comparison

| Model | Size | HumanEval | MBPP | SWE-bench Verified |
|-------|------|-----------|------|--------------------|
| **Claude Sonnet 4.6** | Cloud | ~92% | ~88% | ~55% |
| **GPT-4.1** | Cloud | ~90% | ~87% | ~54% |
| **Gemini 2.5 Pro** | Cloud | ~91% | ~87% | ~53% |
| **Qwen2.5-Coder-32B** | 32B | ~93% | ~84% | ~25% |
| **DeepSeek-R1-Distill-70B** | 70B | ~85% | ~82% | ~28% |
| **Llama 3.3-70B** | 70B | ~81% | ~82% | ~22% |
| **Qwen2.5-Coder-7B** | 7B | ~82% | ~76% | ~12% |
| **CodeLlama-34B** | 34B | ~68% | ~73% | ~15% |

### Key Observations

1. **Simple code generation** — Local models are competitive (HumanEval/MBPP)
2. **Complex multi-step tasks** — Cloud models still dominate (SWE-bench)
3. **Sweet spot** — Qwen2.5-Coder-32B offers the best local coding quality
4. **Function calling** — Cloud models are significantly more reliable
5. **Long context** — Cloud models handle 1M tokens; local models often struggle >32K

### Speed Comparison

| Model | Hardware | Tokens/sec | Latency (first token) |
|-------|----------|-----------|----------------------|
| Claude Sonnet 4.6 | Cloud | ~80 t/s | ~500ms |
| GPT-4.1 | Cloud | ~70 t/s | ~300ms |
| Qwen2.5-Coder-32B Q4 | RTX 4090 | ~25 t/s | ~200ms |
| Qwen2.5-Coder-32B Q4 | M3 Max 64GB | ~20 t/s | ~150ms |
| Qwen2.5-Coder-7B Q4 | RTX 3060 12GB | ~40 t/s | ~100ms |
| Llama-3.3-70B Q4 | 2x RTX 4090 | ~15 t/s | ~500ms |
| Qwen2.5-Coder-7B Q4 | CPU (M3 Pro) | ~8 t/s | ~2s |

---

## How Agents Support Local Models

### Pattern 1: Custom OpenAI Endpoint

Most agents that support OpenAI can also support local models via endpoint override:

```bash
# Generic pattern for any OpenAI-compatible agent
export OPENAI_API_KEY="not-needed"
export OPENAI_BASE_URL="http://localhost:11434/v1"
```

### Pattern 2: LiteLLM Integration

Agents using LiteLLM get local model support automatically:

```python
# LiteLLM supports Ollama natively
import litellm

response = litellm.completion(
    model="ollama/qwen2.5-coder:32b",
    messages=[{"role": "user", "content": "Fix this bug..."}],
    api_base="http://localhost:11434"
)
```

### Pattern 3: Native Ollama Provider

Some agents implement Ollama as a first-class provider:

```go
// Goose's Ollama integration
type OllamaProvider struct {
    BaseURL string
    Model   string
}

func (p *OllamaProvider) Complete(messages []Message) (*Response, error) {
    // Uses OpenAI-compatible API at localhost:11434
    client := openai.NewClient(openai.ClientConfig{
        BaseURL: p.BaseURL + "/v1",
    })
    return client.ChatCompletion(messages)
}
```

### Agent-Specific Local Model Support

| Agent | Local Support | How |
|-------|-------------|-----|
| **Aider** | ✅ | LiteLLM → Ollama |
| **Codex CLI** | ✅ | Custom endpoint in TOML config |
| **Goose** | ✅ | Native Ollama + LM Studio + Docker Model Runner |
| **OpenHands** | ✅ | LiteLLM → Ollama |
| **mini-SWE-agent** | ✅ | LiteLLM → Ollama |
| **OpenCode** | ⚠️ | Via OpenAI-compatible endpoint |
| **ForgeCode** | ⚠️ | Via OpenAI-compatible endpoint |
| **Claude Code** | ❌ | Anthropic only |
| **Gemini CLI** | ❌ | Google only |

---

## Practical Setup Guide

### Recommended Local Model for Coding Agents

For the best local coding experience, use **Qwen2.5-Coder-32B** on hardware with
≥24GB VRAM (or ≥48GB unified memory on Apple Silicon):

```bash
# Step 1: Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh

# Step 2: Pull the model
ollama pull qwen2.5-coder:32b

# Step 3: Verify it works
curl http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen2.5-coder:32b",
    "messages": [{"role": "user", "content": "Write FizzBuzz in Rust"}]
  }'

# Step 4: Configure your agent
# For Aider:
aider --model ollama/qwen2.5-coder:32b

# For Codex CLI:
# Add to ~/.codex/config.toml:
# [providers.ollama]
# base_url = "http://localhost:11434/v1"
# model = "qwen2.5-coder:32b"
```

### If You Have Limited Hardware

| VRAM | Recommended Model | Quality |
|------|-------------------|---------|
| **48GB+** | Qwen2.5-Coder-32B (full) | Excellent |
| **24GB** | Qwen2.5-Coder-32B Q4 | Very good |
| **16GB** | Qwen2.5-Coder-14B Q4 | Good |
| **8GB** | Qwen2.5-Coder-7B Q4 | Decent |
| **4GB** | Qwen2.5-Coder-1.5B Q4 | Limited |
| **CPU only** | Qwen2.5-Coder-7B Q4 | Slow but works |

### Apple Silicon Recommendations

Apple Silicon Macs are excellent for local model inference due to unified memory:

| Mac | Unified Memory | Best Model | Quality |
|-----|---------------|-----------|---------|
| M3/M4 Pro (18GB) | 18GB | Qwen2.5-Coder-14B Q4 | Good |
| M3/M4 Pro (36GB) | 36GB | Qwen2.5-Coder-32B Q4 | Very good |
| M3/M4 Max (64GB) | 64GB | Qwen2.5-Coder-32B FP16 | Excellent |
| M3/M4 Max (128GB) | 128GB | Llama-3.3-70B Q4 | Strong |
| M2/M3 Ultra | 192GB | DeepSeek-R1-Distill-70B FP16 | Very strong |

---

## Trade-offs: Local vs. Cloud

### When Local Models Win

| Scenario | Why Local is Better |
|----------|-------------------|
| **Privacy-critical code** | Code never leaves your machine |
| **No internet access** | Air-gapped development environments |
| **Unlimited usage** | No per-token costs after hardware investment |
| **Low latency (simple tasks)** | No network round-trip, instant first token |
| **Rate limit freedom** | No RPM or TPM limits |
| **Offline development** | Works on planes, in remote areas |
| **Fine-tuning** | Can train on proprietary codebases |

### When Cloud Models Win

| Scenario | Why Cloud is Better |
|----------|-------------------|
| **Complex reasoning** | GPT-4/Claude/Gemini are 2-3x better on SWE-bench |
| **Long context** | Cloud models handle 1M tokens vs. 32-128K local |
| **Function calling** | Cloud models are much more reliable for tool use |
| **Multi-step tasks** | Cloud models maintain coherence over many turns |
| **Cost (low volume)** | $10/month API < $1000+ GPU investment |
| **Latest models** | Cloud providers update models regularly |
| **No hardware needed** | Any laptop can use cloud APIs |

### Hybrid Approach

The most practical strategy for many users:

```python
# Use local models for simple tasks, cloud for complex ones
def select_provider(task):
    if task.estimated_complexity == "simple":
        return OllamaProvider(model="qwen2.5-coder:32b")
    elif task.estimated_complexity == "moderate":
        return DeepSeekProvider(model="deepseek-chat")  # Cheap cloud
    else:
        return AnthropicProvider(model="claude-sonnet-4-6")  # Best quality
```

---

## Future Directions

### Trends to Watch

1. **Rapidly improving quality** — Open models are closing the gap with cloud models
2. **Smaller, better models** — The 7-14B range is becoming increasingly capable
3. **Better function calling** — Critical for agentic use cases
4. **Longer context** — 128K is standard, 1M coming
5. **Specialized code models** — Models trained specifically for agentic coding
6. **Hardware improvements** — NPUs, dedicated AI accelerators on laptops
7. **Model merging** — Community techniques for combining model strengths

### The Path to Local-First Agents

As open models improve, we may see a shift toward local-first agents that only fall
back to cloud providers for the hardest tasks. The economic equation tilts further
toward local as:
- Model quality at 32B approaches cloud frontier
- Consumer GPUs get more VRAM (RTX 5090: 32GB)
- Apple Silicon continues to increase unified memory
- Quantization techniques improve quality at lower precision

---

## See Also

- [DeepSeek](deepseek.md) — Open-weight frontier model with API access
- [LiteLLM](litellm.md) — Unified interface for local and cloud models
- [Pricing and Cost](pricing-and-cost.md) — Cost comparison including local hosting
- [Model Routing](model-routing.md) — Routing between local and cloud models