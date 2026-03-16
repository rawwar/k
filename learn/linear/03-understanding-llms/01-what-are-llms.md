---
title: What Are LLMs
description: A practical introduction to large language models for agent builders, covering architecture intuition without requiring ML expertise.
---

# What Are LLMs

> **What you'll learn:**
> - How transformer-based language models generate text through next-token prediction
> - Why LLMs can follow instructions, write code, and use tools despite being trained on prediction
> - The practical mental model for reasoning about LLM capabilities and limitations as an agent builder

You are about to build a coding agent that runs on top of a large language model. Before you write your first API call, you need an accurate mental model of what is actually happening inside that model. Not the linear algebra -- the practical behavior. How does it generate text? Why does it sometimes hallucinate? Why can it write working Rust code one moment and confidently produce a function that does not exist the next? Understanding the machine you are building on top of is what separates an agent that works reliably from one that fails in mysterious ways.

## The Core Idea: Next-Token Prediction

At its heart, every large language model does one thing: given a sequence of tokens (roughly, pieces of words), it predicts the most likely next token. That is the entire mechanism. There is no "understanding" module, no "reasoning" engine, no database of facts. There is a neural network that has learned statistical patterns across an enormous corpus of text, and it uses those patterns to predict what comes next.

Here is the intuition. Suppose you see the text:

```
The function returns a Result<T,
```

A well-trained model assigns high probability to `E>` as the next token, because it has seen this Rust pattern millions of times during training. It has learned that `Result<T,` is almost always followed by `E>` or a concrete error type. The model does not "know" what a Result type is in the way you do. It has learned the statistical structure of code so thoroughly that its predictions are functionally equivalent to understanding.

This next-token prediction happens autoregressively -- one token at a time, feeding each generated token back in as input for predicting the next one. When you ask a model to write a function, it generates the function signature token by token, then the body, then the closing brace. Each token is conditioned on everything that came before it: your prompt, the system message, and all previously generated tokens.

## From Prediction to Instruction Following

Raw next-token prediction on internet text would give you a model that continues text plausibly but does not follow instructions. If you typed "Write a Rust function that sorts a vector," a base model might continue with "...is a common interview question" because that is a likely continuation of that sentence on the internet.

The key step that makes models useful for agents is **instruction tuning** and **reinforcement learning from human feedback (RLHF)**. During these training phases, the model learns to treat the input as an instruction and generate a helpful response rather than just a plausible continuation. The model learns the pattern: "when a human asks me to do something, I should do it and present the result clearly."

This is why the message format matters so much (we will cover this in detail in [Message Formats](/linear/03-understanding-llms/06-message-formats)). When you send a message with `role: "user"`, you are activating the instruction-following behavior the model learned during fine-tuning. The model has been trained on millions of examples of `user says X, assistant responds with Y`, and it has internalized that pattern deeply.

## Why LLMs Can Write Code

Code generation is not a special feature bolted onto language models -- it emerges naturally from training on code. Modern LLMs are trained on datasets that include billions of lines of source code from GitHub, Stack Overflow, documentation sites, and technical books. The model learns:

- **Syntax rules** -- it has seen so many examples of valid Rust/Python/JavaScript that it rarely produces syntax errors
- **Idioms and patterns** -- it knows that Rust error handling uses `?` and `match`, that Python uses `try/except`
- **API usage** -- it has seen thousands of examples of how popular libraries are used
- **Logical structure** -- it has learned that if a function opens a file, it typically does something with the contents and handles the error case

This is also why models sometimes hallucinate API calls that do not exist. The model has learned the pattern of "import library, call function" so well that it can generate plausible-looking but fictitious function names. It is pattern matching, not looking things up in documentation.

::: python Coming from Python
If you have used GitHub Copilot or asked ChatGPT to write Python code, you have already experienced LLM code generation. The same mechanism that completes your Python list comprehensions is what will power your Rust coding agent. The difference is that your agent will orchestrate multiple rounds of generation, inspect the results, and iterate -- turning a single-shot code generator into a persistent collaborator.
:::

## The Transformer Architecture (Just Enough)

You do not need to understand backpropagation or attention head mathematics to build a coding agent. But knowing the basic shape of the architecture helps you reason about behavior.

A transformer processes input through these conceptual stages:

1. **Tokenization** -- your text is split into tokens (covered in [Tokens and Tokenization](/linear/03-understanding-llms/02-tokens-and-tokenization))
2. **Embedding** -- each token is converted to a high-dimensional vector
3. **Attention layers** -- the model processes all tokens in parallel, allowing each token to "attend to" every other token. This is how the model knows that the `it` in "the function failed because it received null" refers to "the function"
4. **Feed-forward layers** -- dense neural network layers that transform the attended representations
5. **Output projection** -- the final layer produces a probability distribution over all possible next tokens

The attention mechanism is what makes transformers powerful for code. When generating token 500, the model can directly attend to token 3 -- it does not need to pass information through a chain of intermediate states like older architectures (RNNs) did. This means it can maintain coherence across long sequences, which is critical for generating functions that correctly reference variables defined hundreds of tokens earlier.

## What This Means for Agent Design

Understanding the prediction mechanism gives you practical insights for building agents:

**The model has no persistent memory.** Every API call starts fresh. The model does not remember your previous conversation unless you send the entire conversation history in every request. This is why context window management (Chapter 3.3) is an essential agent design concern.

**The model does not execute code.** When it writes a function, it is predicting what a correct function looks like based on patterns. It cannot run the code to verify it works. This is precisely why coding agents need a tool for executing shell commands -- the agent writes code, runs it, reads the output, and iterates. The execution feedback loop is what makes agents dramatically more capable than single-shot code generation.

**The model is probabilistic.** The same prompt can produce different outputs on different runs (controlled by temperature, covered in [Temperature and Sampling](/linear/03-understanding-llms/04-temperature-and-sampling)). Your agent needs to handle this variability gracefully.

**The model responds to framing.** How you describe tools, what examples you provide in the system prompt, and how you format previous tool results all significantly affect the model's behavior. This is not a deterministic function call -- it is a system that responds to context and phrasing.

::: wild In the Wild
Claude Code and other production coding agents treat the LLM as one component in a larger system. The agent provides the model with tools (file reading, shell execution, code search), feeds back real execution results, and maintains conversation history across turns. The model's job is to decide what to do next and generate tool calls or text -- the agent handles everything else. This separation of concerns -- LLM for decision-making, agent for execution -- is the foundational architecture you will build throughout this course.
:::

## The Practical Mental Model

Here is the mental model that will serve you throughout this course:

An LLM is a **stateless text-processing function**. You send it a sequence of messages (conversation history, system prompt, tool definitions), and it returns the most likely continuation. That continuation might be natural language, code, a tool call request, or a mix of all three. The model chooses what to generate based on the patterns it learned during training and the specific context you provide in your request.

Your job as an agent builder is to:
1. **Provide good context** -- clear system prompts, relevant conversation history, well-described tools
2. **Parse the output** -- detect whether the model wants to call a tool, write text, or both
3. **Execute and feed back** -- run the requested tool, capture the result, and send it back
4. **Loop until done** -- repeat until the model indicates the task is complete

This is the agentic loop, and every concept in this chapter -- tokens, context windows, tool use, streaming -- is something you need to understand to build it well.

## Key Takeaways

- LLMs generate text through next-token prediction, producing one token at a time conditioned on all previous tokens
- Instruction tuning and RLHF transform raw text predictors into systems that follow user instructions, which is what enables the message-based API format agents rely on
- Code generation works because models are trained on massive code corpora and learn syntax, idioms, and API patterns -- but they can also hallucinate nonexistent APIs for the same reason
- The model is stateless: it has no memory between API calls, which means your agent must manage conversation history explicitly
- Your mental model should be: the LLM is a stateless text-processing function, and the agent is the stateful system built around it
