---
title: Temperature and Sampling
description: How temperature, top-p, and other sampling parameters control LLM output randomness and their optimal settings for coding agents.
---

# Temperature and Sampling

> **What you'll learn:**
> - How temperature scales the probability distribution over tokens and affects output creativity vs determinism
> - What top-p (nucleus sampling) and top-k do and when to use them alongside temperature
> - The recommended sampling settings for coding agents where correctness matters more than creativity

When the model predicts the next token, it does not just pick the single most likely one. It produces a probability distribution across its entire vocabulary -- tens of thousands of possible tokens, each with an associated probability. Sampling parameters control how the model selects from this distribution. Getting these settings right for a coding agent is important: too much randomness and you get unreliable code, too little and you get repetitive, unimaginative solutions.

## How Temperature Works

Temperature is a single number, typically between 0 and 2, that controls the "sharpness" of the probability distribution. Here is the intuition:

**Temperature = 0 (or very close to 0):** The model always picks the single highest-probability token. This is called "greedy decoding." The output is deterministic -- the same input always produces the same output.

**Temperature = 1.0:** The model samples directly from the learned probability distribution. If token A has probability 0.7 and token B has probability 0.2, the model picks A roughly 70% of the time and B roughly 20% of the time.

**Temperature > 1.0:** The distribution is flattened, making less-likely tokens more probable. This produces more creative, surprising, and often less coherent output.

**Temperature < 1.0:** The distribution is sharpened, making the most-likely tokens even more dominant. This produces more predictable, conservative output.

Mathematically, temperature divides the logits (raw model scores) before the softmax function:

```
probability(token_i) = exp(logit_i / temperature) / sum(exp(logit_j / temperature))
```

You do not need to memorize that formula. Just remember: **lower temperature = more focused and deterministic, higher temperature = more varied and creative**.

Let's see a concrete example. Suppose the model is generating the next token after `let result = ` and the top candidates are:

| Token | Probability (T=1.0) | Probability (T=0.3) | Probability (T=1.5) |
|---|---|---|---|
| `vec` | 0.35 | 0.72 | 0.25 |
| `String` | 0.25 | 0.18 | 0.21 |
| `Ok` | 0.20 | 0.07 | 0.19 |
| `HashMap` | 0.10 | 0.02 | 0.16 |
| `fetch` | 0.05 | 0.001 | 0.10 |
| other | 0.05 | 0.001 | 0.09 |

At T=0.3, `vec` is chosen about 72% of the time -- the model is very confident. At T=1.5, the distribution is much flatter, and even unlikely tokens like `fetch` get a 10% chance. This is how temperature controls the randomness of generation.

## Top-p (Nucleus Sampling)

Top-p is an alternative (or complement) to temperature that works differently. Instead of scaling the entire distribution, it truncates it.

With top-p = 0.9, the model considers only the smallest set of tokens whose cumulative probability is at least 90%, and samples from just those tokens. Everything below that threshold is excluded.

Using our example above with T=1.0:

```
top_p = 0.9:
  vec (0.35) + String (0.25) + Ok (0.20) + HashMap (0.10) = 0.90
  → Sample from {vec, String, Ok, HashMap} only
  → "fetch" and all rarer tokens are excluded
```

Top-p dynamically adapts to the model's confidence. When the model is very confident (one token has 0.95 probability), top-p = 0.9 effectively selects just that one token. When the model is uncertain (many tokens with similar probability), top-p = 0.9 keeps many candidates in the pool.

## Top-k Sampling

Top-k is simpler: it considers only the k most probable tokens. With top-k = 40, the model samples from the 40 highest-probability tokens regardless of their actual probabilities.

Top-k is less commonly used in modern APIs. OpenAI does not expose a top-k parameter. Anthropic supports it but recommends using top-p instead for most use cases.

## How Parameters Interact

You can combine temperature and top-p. When both are set, they apply sequentially:

1. Temperature first adjusts the distribution
2. Top-p then truncates the adjusted distribution

The API docs for both Anthropic and OpenAI recommend **changing one or the other, not both simultaneously**. In practice, for coding agents:

```json
{
  "temperature": 0.0,
  "top_p": 1.0
}
```

or equivalently, just set temperature to 0 and leave top-p at its default.

::: python Coming from Python
If you have used the OpenAI Python SDK, you have seen these parameters in `client.chat.completions.create(temperature=0.7, top_p=0.9)`. The Rust implementation passes them as fields in a JSON request body. The semantics are identical across languages -- these are API parameters, not SDK features.
:::

## Optimal Settings for Coding Agents

For a coding agent, the priority is **correctness and consistency** over creativity. You want the model to:

- Generate syntactically valid code reliably
- Make predictable tool use decisions
- Not introduce random variations between runs

The recommended settings:

| Parameter | Recommended Value | Reasoning |
|---|---|---|
| temperature | 0.0 - 0.2 | Minimize randomness for code generation |
| top_p | 1.0 (default) | Let temperature handle sampling; do not combine |
| max_tokens | Model maximum (e.g., 8192) | Let the model decide response length |

**Temperature 0** is the safest choice for most agent operations. When the model decides which tool to call, you want deterministic behavior -- the same situation should produce the same tool call every time. When generating code, you want the most likely (and typically most correct) completion.

There are cases where a slightly higher temperature (0.1-0.3) can help:

- **When the model gets stuck in a loop** -- repeating the same failed approach. A small amount of randomness can help it try a different path.
- **When generating creative content** -- if the agent is writing documentation or commit messages, a bit of variety is welcome.
- **When exploring multiple solutions** -- some agent architectures generate several candidate solutions at higher temperature and then evaluate them.

However, for your initial agent implementation, start with temperature 0 and only increase it if you observe specific problems that randomness would solve.

## The `stop` Parameter

Beyond sampling, you can control generation with the `stop` parameter -- a list of sequences that cause the model to stop generating when encountered:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "temperature": 0,
  "stop_sequences": ["\n```\n"],
  "messages": [...]
}
```

This is rarely needed for agents that use the tool use protocol (the model stops naturally after generating a tool call), but it can be useful if you are doing raw text generation and want to prevent runaway output.

## Seed Parameter for Reproducibility

OpenAI offers a `seed` parameter that, combined with temperature 0, provides near-deterministic outputs:

```json
{
  "model": "gpt-4o",
  "temperature": 0,
  "seed": 42,
  "messages": [...]
}
```

Anthropic does not currently offer a seed parameter, but temperature 0 is already nearly deterministic for most practical purposes. The rare cases where temperature 0 produces different outputs are due to internal floating-point nondeterminism, and they almost never change the semantic content of the response.

For testing and debugging your agent, deterministic outputs are valuable. If you can reproduce the exact same model response for a given input, debugging unexpected behavior becomes much easier.

::: wild In the Wild
Production coding agents typically use temperature 0 or very close to it. Claude Code uses a low temperature setting for tool use decisions where consistency matters. Some agents implement a dynamic temperature strategy -- using temperature 0 for code generation and tool use, but bumping it up slightly for conversational responses to feel more natural. This is a refinement you can add later, but starting with a fixed low temperature is the right approach.
:::

## Common Pitfalls

**Pitfall 1: Using high temperature for code generation.** Temperature 0.7 or above produces creative-sounding prose but introduces bugs into code. Variable names vary randomly, sometimes the model picks a less common API pattern, and occasionally it generates completely invalid syntax.

**Pitfall 2: Setting both temperature and top-p.** This creates confusing interactions. If you set temperature to 0.5 and top-p to 0.5, the effective behavior is hard to reason about. Pick one lever and leave the other at its default.

**Pitfall 3: Ignoring these parameters entirely.** Many APIs default to temperature 1.0, which is too high for agent use. Always explicitly set temperature in your agent's API calls.

## Key Takeaways

- Temperature controls the sharpness of the probability distribution: lower values (0-0.2) produce deterministic, consistent output ideal for coding agents; higher values (0.7+) increase randomness
- Top-p (nucleus sampling) truncates the distribution to the most probable tokens and adapts dynamically to model confidence -- use one or the other, not both
- For coding agents, temperature 0 is the recommended starting point because correctness and consistency matter more than creativity
- Always explicitly set temperature in your API calls -- the default (often 1.0) is too high for agent operations
- The seed parameter (OpenAI) provides additional reproducibility for testing, and temperature 0 alone is nearly deterministic in practice
