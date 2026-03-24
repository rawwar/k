export interface GlossaryEntry {
  definition: string
  category: string
}

export const glossary: Record<string, GlossaryEntry> = {
  // ── Core LLM ────────────────────────────────────────────────────────────────
  'llm': {
    definition: 'A Large Language Model is a neural network trained on vast text corpora to predict and generate human-like text. LLMs learn statistical patterns across billions of parameters, enabling them to answer questions, write code, and reason through complex problems.',
    category: 'Core LLM',
  },
  'large language model': {
    definition: 'A neural network trained on vast text corpora to predict and generate human-like text. LLMs learn statistical patterns across billions of parameters, enabling them to answer questions, write code, and reason through complex problems.',
    category: 'Core LLM',
  },
  'token': {
    definition: 'The basic unit of text that an LLM processes. Tokens are roughly word fragments — "tokenization" splits into ~3-4 characters each on average. A 1,000-word document is typically 1,300–1,500 tokens. Models have a maximum token limit (context window).',
    category: 'Core LLM',
  },
  'tokenization': {
    definition: 'The process of splitting input text into tokens before an LLM can process it. Common algorithms include Byte-Pair Encoding (BPE) and SentencePiece. Tokenization affects cost, context window usage, and how the model perceives punctuation and whitespace.',
    category: 'Core LLM',
  },
  'embedding': {
    definition: 'A dense vector representation of text in high-dimensional space, where semantically similar texts are positioned closer together. Embeddings are used for semantic search, clustering, and as inputs to downstream models. OpenAI\'s text-embedding-3-large produces 3072-dimensional vectors.',
    category: 'Core LLM',
  },
  'embeddings': {
    definition: 'Dense vector representations of text in high-dimensional space, where semantically similar texts are positioned closer together. Used for semantic search, clustering, and RAG retrieval. The quality of embeddings directly affects retrieval accuracy.',
    category: 'Core LLM',
  },
  'context window': {
    definition: 'The maximum number of tokens an LLM can process in a single request, including both the input prompt and the generated output. GPT-4o supports 128K tokens; Claude 3.5 supports 200K. Exceeding the limit truncates the input or raises an error.',
    category: 'Core LLM',
  },
  'context length': {
    definition: 'The total number of tokens an LLM can "see" at once, spanning both input and output. Longer context lengths allow processing entire codebases or long documents but increase latency and cost. Models with longer context windows require more memory.',
    category: 'Core LLM',
  },
  'prompt': {
    definition: 'The input text sent to an LLM to elicit a response. Prompts can include instructions, examples (few-shot), conversation history, and retrieved context. Prompt design significantly impacts output quality — this is the domain of prompt engineering.',
    category: 'Core LLM',
  },
  'system prompt': {
    definition: 'A special prompt injected at the beginning of an LLM conversation that sets behavior, persona, and constraints for the entire session. System prompts are invisible to the user but guide the model\'s responses. Agents use system prompts to define tool use, output format, and safety rules.',
    category: 'Core LLM',
  },
  'inference': {
    definition: 'The process of running a trained model to generate output from new input. Unlike training, inference uses the model\'s fixed weights to produce predictions. Inference speed (tokens per second) and cost are key production concerns.',
    category: 'Core LLM',
  },
  'hallucination': {
    definition: 'When an LLM generates confidently-stated but factually incorrect or fabricated information. Hallucinations occur because models predict likely text rather than retrieving stored facts. Grounding, RAG, and tool use are common mitigation strategies.',
    category: 'Core LLM',
  },
  'grounding': {
    definition: 'Techniques that anchor LLM outputs to verifiable, external sources of truth rather than relying on training-data memory. RAG, web search, and code execution are common grounding mechanisms. Grounding reduces hallucination and improves factual accuracy.',
    category: 'Core LLM',
  },
  'temperature': {
    definition: 'A sampling parameter (0.0–2.0) that controls randomness in LLM outputs. Low temperature (near 0) produces deterministic, focused responses; high temperature increases creativity and variation. Most production agents use 0.0–0.3 for consistency.',
    category: 'Core LLM',
  },
  'top-p': {
    definition: 'Nucleus sampling: the model considers only the smallest set of tokens whose cumulative probability exceeds p. Combined with temperature to control output diversity. A top-p of 0.9 means only tokens making up 90% of the probability mass are sampled.',
    category: 'Core LLM',
  },
  'top-k': {
    definition: 'Limits sampling to the k most probable next tokens at each step. A top-k of 40 means only the 40 highest-probability tokens are candidates. Often used alongside temperature and top-p for fine-grained control over output randomness.',
    category: 'Core LLM',
  },
  'transformer': {
    definition: 'The neural network architecture underlying virtually all modern LLMs, introduced in "Attention Is All You Need" (2017). Transformers use self-attention to relate all tokens in an input simultaneously, enabling parallel training and long-range dependency modeling.',
    category: 'Core LLM',
  },
  'attention': {
    definition: 'A mechanism in transformers that allows the model to weigh the relevance of each input token when processing any given token. Multi-head attention runs several attention operations in parallel, enabling the model to capture different types of relationships simultaneously.',
    category: 'Core LLM',
  },
  'self-attention': {
    definition: 'An attention mechanism where a sequence attends to itself — every token can "look at" every other token to determine contextual meaning. Self-attention is the core operation in transformer layers that gives LLMs their ability to understand context.',
    category: 'Core LLM',
  },
  'chain-of-thought': {
    definition: 'A prompting technique where the model is instructed (or observed) to reason step-by-step before giving a final answer. Chain-of-thought significantly improves performance on math, logic, and multi-step reasoning tasks. Often abbreviated CoT.',
    category: 'Core LLM',
  },
  'cot': {
    definition: 'Chain-of-Thought prompting: a technique where the model reasons step-by-step before answering. Improves performance on math, logic, and complex reasoning. Can be elicited with phrases like "Let\'s think step by step" or built into system prompts.',
    category: 'Core LLM',
  },
  'few-shot': {
    definition: 'A prompting strategy where a small number of input-output examples are provided in the prompt to guide the model\'s behavior. Few-shot prompting can dramatically improve format adherence and task-specific performance without any weight updates.',
    category: 'Core LLM',
  },
  'zero-shot': {
    definition: 'Using an LLM for a task without any examples in the prompt, relying purely on the model\'s pretrained knowledge and instruction-following ability. Zero-shot performance has improved dramatically with larger, RLHF-trained models.',
    category: 'Core LLM',
  },
  'one-shot': {
    definition: 'Providing exactly one example in the prompt to guide model behavior. A middle ground between zero-shot (no examples) and few-shot (multiple examples). Useful when a single clear example conveys the desired pattern.',
    category: 'Core LLM',
  },
  'context management': {
    definition: 'Strategies for fitting relevant information into an LLM\'s finite context window. Includes summarization, truncation, sliding windows, and retrieval. Effective context management is critical for long-running agent sessions.',
    category: 'Core LLM',
  },
  'foundation model': {
    definition: 'A large model trained on broad data at scale, intended as a base for downstream tasks. Foundation models (GPT-4, Claude, Gemini) are fine-tuned or prompted for specific applications. The term emphasizes the model as infrastructure others build upon.',
    category: 'Core LLM',
  },
  'multimodal': {
    definition: 'Models that process multiple types of inputs — text, images, audio, and/or video — in a single model. GPT-4o and Claude 3.5 Sonnet are multimodal. Multimodality enables richer tasks like analyzing screenshots, diagrams, or describing images.',
    category: 'Core LLM',
  },

  // ── Agents ──────────────────────────────────────────────────────────────────
  'agent': {
    definition: 'An LLM-powered system that can take actions in the world — running code, browsing the web, editing files, calling APIs — in pursuit of a goal. Unlike a chatbot, an agent operates in a loop: observe, think, act, repeat until the task is complete.',
    category: 'Agents',
  },
  'ai agent': {
    definition: 'A software system that uses an LLM as its reasoning engine to autonomously plan and execute tasks. AI agents combine language understanding, tool use, and multi-step planning to complete goals that require more than a single LLM response.',
    category: 'Agents',
  },
  'agentic loop': {
    definition: 'The core control flow of an AI agent: perceive inputs → reason with an LLM → select and call tools → observe results → repeat. The loop continues until the agent determines the task is complete or a stopping condition is met.',
    category: 'Agents',
  },
  'react': {
    definition: 'Reasoning + Acting: a prompting pattern where the agent alternates between Thought (reasoning about the current situation) and Action (calling a tool or taking a step). Introduced in the ReAct paper (Yao et al., 2022), it\'s the basis for many agent frameworks.',
    category: 'Agents',
  },
  'tool use': {
    definition: 'The ability of an LLM to invoke external functions — search engines, code interpreters, APIs, file systems — to extend beyond what\'s in its training data. Tool use turns a chatbot into an agent capable of taking real-world actions.',
    category: 'Agents',
  },
  'tool call': {
    definition: 'A structured request from an LLM to invoke an external function, expressed as a JSON object with a function name and arguments. The caller executes the function, captures the result, and feeds it back to the LLM to continue generation.',
    category: 'Agents',
  },
  'function calling': {
    definition: 'OpenAI\'s term for structured tool use: the model outputs a JSON object specifying a function name and parameters instead of (or in addition to) plain text. The calling application executes the function and returns the result as a message.',
    category: 'Agents',
  },
  'orchestrator': {
    definition: 'In a multi-agent system, the orchestrator is the top-level agent that breaks down tasks, delegates to sub-agents, and aggregates results. The orchestrator manages control flow, handles errors, and determines when the overall goal is achieved.',
    category: 'Agents',
  },
  'sub-agent': {
    definition: 'A specialized agent invoked by an orchestrator to handle a specific sub-task. Sub-agents typically have narrower tools and context than the orchestrator. Examples: a search agent, a code-writing agent, a review agent operating in parallel.',
    category: 'Agents',
  },
  'scaffold': {
    definition: 'The surrounding code and infrastructure that wraps an LLM to create an agent: the loop that calls the model, routes tool calls, handles errors, manages context, and enforces safety policies. Frameworks like LangChain, AutoGen, and CrewAI provide scaffolding.',
    category: 'Agents',
  },
  'scaffolding': {
    definition: 'The infrastructure code surrounding an LLM that enables agentic behavior: the control loop, tool dispatch, memory management, error handling, and logging. Good scaffolding is invisible to the LLM but critical for reliable agent operation.',
    category: 'Agents',
  },
  'planning': {
    definition: 'The agent capability to decompose a high-level goal into a sequence of concrete steps before acting. Planning reduces hallucination by forcing the model to think ahead. Approaches include explicit plan-then-execute, tree-of-thought, and dynamic replanning.',
    category: 'Agents',
  },
  'reflection': {
    definition: 'A meta-cognitive step where an agent reviews its own outputs or trajectory to identify errors and improve. Self-reflection can happen within a single turn (critique-and-revise) or across turns (reviewing memory before acting).',
    category: 'Agents',
  },
  'memory': {
    definition: 'How an agent stores and retrieves information across time. Types: in-context memory (the current conversation), episodic memory (past experiences in a vector store), semantic memory (general facts), and procedural memory (learned skills/rules).',
    category: 'Agents',
  },
  'working memory': {
    definition: 'The information currently in the LLM\'s context window — the "active" memory. Limited by context window size. Managing what goes in working memory (what to keep, summarize, or discard) is a core challenge in long-horizon agent tasks.',
    category: 'Agents',
  },
  'episodic memory': {
    definition: 'An agent\'s stored record of past actions, observations, and outcomes, typically persisted in a vector database. Episodic memory allows the agent to recall relevant past experiences using semantic search rather than keeping everything in context.',
    category: 'Agents',
  },
  'semantic memory': {
    definition: 'General world knowledge stored externally (in a vector DB or knowledge base) and retrieved as needed. Unlike episodic memory (past events), semantic memory contains facts, concepts, and documentation that inform the agent\'s reasoning.',
    category: 'Agents',
  },
  'handoff': {
    definition: 'In multi-agent systems, transferring control of a task from one agent to another. A handoff passes context, conversation history, and current state to the receiving agent. Clean handoffs are critical for multi-agent reliability.',
    category: 'Agents',
  },
  'human-in-the-loop': {
    definition: 'A design pattern where humans are given opportunities to review, approve, or redirect agent actions before they are executed. HITL provides a safety valve for high-stakes tasks and is essential for building user trust in autonomous systems.',
    category: 'Agents',
  },
  'hitl': {
    definition: 'Human-in-the-loop: a design pattern where humans review or approve agent actions at key decision points. Reduces risk in autonomous systems. Can range from approving every action to only intervening when the agent is uncertain.',
    category: 'Agents',
  },
  'approval': {
    definition: 'In agentic systems, a checkpoint where a human must confirm before the agent proceeds with an action. Common for destructive operations (file deletion, sending emails, deploying code). Balances automation speed with human oversight.',
    category: 'Agents',
  },
  'multi-agent': {
    definition: 'A system where multiple specialized AI agents collaborate to complete a complex task. Agents can run in parallel, sequentially, or as a hierarchy with an orchestrator. Multi-agent systems improve performance by dividing labor and specialization.',
    category: 'Agents',
  },
  'multi-agent system': {
    definition: 'An architecture using multiple cooperating AI agents to solve tasks that exceed any single agent\'s capability. Key patterns: orchestrator-subagent, peer-to-peer, pipeline, and blackboard. Communication is typically via structured messages.',
    category: 'Agents',
  },
  'coding agent': {
    definition: 'An AI agent specialized for software development tasks: writing, reading, editing, and running code. Coding agents typically have tools for file system access, terminal execution, and code search. Examples: Claude Code, Codex, OpenHands.',
    category: 'Agents',
  },
  'agent loop': {
    definition: 'The iterative execution cycle of an AI agent: receive input → think (LLM call) → act (tool call) → observe result → repeat. The loop continues until a stop condition: task complete, max iterations reached, or user interrupt.',
    category: 'Agents',
  },
  'trajectory': {
    definition: 'The complete sequence of observations, thoughts, and actions taken by an agent to complete a task. Trajectories are analyzed for debugging, evaluation, and training data generation. A successful trajectory is a ground-truth demonstration.',
    category: 'Agents',
  },
  'autonomy': {
    definition: 'The degree to which an agent can operate without human intervention. Fully autonomous agents execute entire tasks end-to-end; semi-autonomous agents pause for human approval at key steps. Higher autonomy requires better error handling and safety measures.',
    category: 'Agents',
  },

  // ── RAG ─────────────────────────────────────────────────────────────────────
  'rag': {
    definition: 'Retrieval-Augmented Generation: augmenting LLM responses with dynamically retrieved documents. The query is embedded, relevant chunks are fetched from a vector store, and the chunks are injected into the prompt. RAG dramatically reduces hallucination on knowledge-intensive tasks.',
    category: 'RAG',
  },
  'retrieval-augmented generation': {
    definition: 'A technique that improves LLM accuracy by fetching relevant documents at query time and including them in the prompt context. RAG separates the knowledge base from the model weights, making updates easy without retraining.',
    category: 'RAG',
  },
  'vector database': {
    definition: 'A specialized database that stores embeddings (dense vectors) and supports fast approximate nearest-neighbor (ANN) search. Examples: Pinecone, Weaviate, Chroma, Qdrant, pgvector. The backbone of RAG and semantic search systems.',
    category: 'RAG',
  },
  'vector store': {
    definition: 'Storage for embeddings (high-dimensional vectors) with support for similarity search. Vector stores power RAG retrieval by finding the most semantically similar chunks to a query. Can be in-memory (Chroma) or fully managed (Pinecone).',
    category: 'RAG',
  },
  'chunking': {
    definition: 'Splitting documents into smaller pieces before embedding for RAG. Chunk size (typically 256–1024 tokens) balances retrieval precision (smaller) vs. context completeness (larger). Overlap between chunks prevents losing information at boundaries.',
    category: 'RAG',
  },
  'reranking': {
    definition: 'A second-stage retrieval step that re-scores initially retrieved chunks using a more powerful (but slower) model. Cross-encoders compare query and document together, improving ranking accuracy beyond initial embedding similarity.',
    category: 'RAG',
  },
  'semantic search': {
    definition: 'Finding documents by meaning rather than exact keyword match. Queries and documents are embedded, and similarity is measured in vector space. Semantic search handles synonyms, paraphrases, and conceptual relationships that keyword search misses.',
    category: 'RAG',
  },
  'hybrid search': {
    definition: 'Combining dense vector search (semantic) with sparse keyword search (BM25/TF-IDF). Hybrid search outperforms either method alone by capturing both semantic similarity and exact keyword matches. Reciprocal Rank Fusion (RRF) is a common combination strategy.',
    category: 'RAG',
  },
  'bm25': {
    definition: 'Best Match 25: a classic TF-IDF-based ranking function for keyword search. BM25 scores documents by term frequency, document frequency, and document length. Despite being decades old, it\'s still competitive for exact-match retrieval and widely used in hybrid search.',
    category: 'RAG',
  },

  // ── Training ─────────────────────────────────────────────────────────────────
  'fine-tuning': {
    definition: 'Continuing training a pretrained model on a smaller, task-specific dataset to adapt its behavior. Fine-tuning updates model weights to improve performance on narrow domains. More expensive than prompting but yields better results for specialized tasks.',
    category: 'Training',
  },
  'rlhf': {
    definition: 'Reinforcement Learning from Human Feedback: training a reward model from human preference judgments, then using it to fine-tune the LLM via RL. RLHF is responsible for the "helpful, harmless, honest" alignment of models like ChatGPT and Claude.',
    category: 'Training',
  },
  'sft': {
    definition: 'Supervised Fine-Tuning: training a model on labeled input-output pairs to teach a specific behavior or style. SFT is typically the first step in alignment pipelines (before RLHF) and is simpler but less flexible than RL-based methods.',
    category: 'Training',
  },
  'supervised fine-tuning': {
    definition: 'Training a pretrained model on curated input-output demonstrations to teach a specific skill or behavior. SFT is computationally cheaper than pretraining and is used for instruction-following, format adherence, and domain adaptation.',
    category: 'Training',
  },
  'lora': {
    definition: 'Low-Rank Adaptation: a parameter-efficient fine-tuning method that inserts small trainable weight matrices into frozen model layers. LoRA reduces fine-tuning memory by 10–100x compared to full fine-tuning, making it practical on consumer GPUs.',
    category: 'Training',
  },
  'qlora': {
    definition: 'Quantized LoRA: fine-tuning with 4-bit quantized base model weights plus LoRA adapters. QLoRA enables fine-tuning 65B-parameter models on a single 48GB GPU, democratizing LLM customization.',
    category: 'Training',
  },
  'quantization': {
    definition: 'Reducing model weight precision from 32-bit floats to smaller formats (16-bit, 8-bit, 4-bit). Quantization shrinks model size and speeds up inference at a small cost to quality. 4-bit quantization (GGUF, AWQ, GPTQ) enables running 70B models on consumer hardware.',
    category: 'Training',
  },
  'perplexity': {
    definition: 'A metric for language model quality: how surprised the model is by a test corpus. Lower perplexity means the model better predicts the text. Perplexity is the exponential of cross-entropy loss and is used to compare models on the same dataset.',
    category: 'Training',
  },
  'overfitting': {
    definition: 'When a model memorizes training data instead of learning generalizable patterns, leading to poor performance on unseen data. In LLM fine-tuning, overfitting causes the model to repeat training examples verbatim rather than generalizing the learned style.',
    category: 'Training',
  },
  'gradient': {
    definition: 'The direction and magnitude of change needed to reduce the model\'s loss. Gradients flow backward through the network during backpropagation, and the optimizer uses them to update weights. Gradient accumulation and clipping are key training stability techniques.',
    category: 'Training',
  },
  'pre-training': {
    definition: 'The initial large-scale training phase where an LLM learns language patterns from a massive corpus (trillions of tokens). Pre-training is extremely compute-intensive (thousands of GPUs for months) and produces the base model that is then fine-tuned.',
    category: 'Training',
  },
  'alignment': {
    definition: 'The process of training LLMs to behave in ways that are helpful, harmless, and honest. Alignment techniques (RLHF, constitutional AI, DPO) shape the model\'s values and behavior to match human intentions and avoid harmful outputs.',
    category: 'Training',
  },
  'dpo': {
    definition: 'Direct Preference Optimization: an alternative to RLHF that directly optimizes the LLM on human preference data without training a separate reward model. DPO is simpler, more stable, and computationally cheaper than PPO-based RLHF.',
    category: 'Training',
  },

  // ── Tools / MCP ──────────────────────────────────────────────────────────────
  'mcp': {
    definition: 'Model Context Protocol: an open standard (by Anthropic) for connecting LLMs to tools, data sources, and services. MCP defines a client-server protocol where MCP servers expose tools/resources, and MCP clients (agents) invoke them.',
    category: 'Tools & Protocols',
  },
  'model context protocol': {
    definition: 'An open standard by Anthropic for connecting LLM applications to external tools and data sources. MCP uses a JSON-RPC-based client-server architecture, enabling any LLM client to discover and call any MCP-compatible tool server.',
    category: 'Tools & Protocols',
  },
  'lsp': {
    definition: 'Language Server Protocol: a JSON-RPC protocol (from Microsoft) for editor-language server communication. LSP servers provide code intelligence (completions, go-to-definition, diagnostics) that coding agents can leverage for precise code navigation.',
    category: 'Tools & Protocols',
  },
  'tool': {
    definition: 'A function an LLM agent can invoke to take actions: read/write files, run shell commands, search the web, call APIs, query databases. Tools are defined by a schema (name, description, parameters) that the LLM uses to decide when and how to call them.',
    category: 'Tools & Protocols',
  },
  'json schema': {
    definition: 'A vocabulary for annotating and validating JSON documents. In agent tool use, JSON Schema describes the structure of tool parameters, enabling the LLM to construct valid tool calls and the runtime to validate inputs before execution.',
    category: 'Tools & Protocols',
  },
  'structured output': {
    definition: 'Constrained LLM generation that produces valid JSON or other structured formats, typically conforming to a provided schema. Structured output makes parsing LLM responses reliable and is essential for tool calls, API integrations, and data extraction.',
    category: 'Tools & Protocols',
  },
  'function': {
    definition: 'In the context of LLM tool use, a callable unit exposed to the model with a name, description, and parameter schema. The LLM "calls" functions by generating JSON; the runtime executes them and returns results. OpenAI\'s API uses "functions" and now "tools" interchangeably.',
    category: 'Tools & Protocols',
  },

  // ── Infrastructure ────────────────────────────────────────────────────────────
  'streaming': {
    definition: 'Delivering LLM output token-by-token as it is generated, rather than waiting for the full response. Streaming dramatically improves perceived latency and enables agents to start processing output before generation completes. Implemented via SSE or WebSocket.',
    category: 'Infrastructure',
  },
  'latency': {
    definition: 'The time from sending a request to receiving the first (or full) response. In LLM systems, time-to-first-token (TTFT) is the key latency metric for interactive use. Latency is affected by model size, hardware, batching, and network distance.',
    category: 'Infrastructure',
  },
  'throughput': {
    definition: 'The number of tokens (or requests) a system can process per unit time. Throughput is the key metric for batch processing and high-load production systems. Batching multiple requests together improves GPU utilization and throughput.',
    category: 'Infrastructure',
  },
  'sse': {
    definition: 'Server-Sent Events: a simple HTTP-based protocol for pushing data from server to client in real time. SSE is widely used for LLM streaming, as it supports one-way text-based event streams over standard HTTP connections without WebSocket overhead.',
    category: 'Infrastructure',
  },
  'api': {
    definition: 'Application Programming Interface: a defined contract for how software components communicate. LLM APIs (OpenAI, Anthropic, Gemini) expose model capabilities over HTTP, typically using JSON request/response with streaming support.',
    category: 'Infrastructure',
  },
  'rate limiting': {
    definition: 'Restrictions on how many API requests a client can make in a given time period (e.g., 60 req/min, 100K tokens/min). Rate limits require agents to implement retry logic, exponential backoff, and request queuing for reliability.',
    category: 'Infrastructure',
  },
  'context window management': {
    definition: 'Strategies for operating within an LLM\'s finite context limit during long tasks: summarizing old messages, evicting less relevant content, and using retrieval to restore needed context. Essential for agents running 100s of steps.',
    category: 'Infrastructure',
  },
  'prompt caching': {
    definition: 'Reusing the KV cache from a previous request when the beginning of the prompt is identical. Prompt caching reduces cost and latency for requests with long shared prefixes (e.g., system prompt + large codebase). Anthropic and OpenAI both support this.',
    category: 'Infrastructure',
  },

  // ── Code / Dev ────────────────────────────────────────────────────────────────
  'ast': {
    definition: 'Abstract Syntax Tree: a tree representation of source code\'s grammatical structure, where each node represents a construct. ASTs are used by coding agents for precise code navigation, transformation, and analysis without brittle text-based matching.',
    category: 'Code & Dev',
  },
  'linter': {
    definition: 'A tool that analyzes source code for potential errors, style violations, and anti-patterns without executing it. Linters (ESLint, Pylint, Clippy) provide fast automated feedback. Coding agents use linter output as a feedback signal in their fix loop.',
    category: 'Code & Dev',
  },
  'diff': {
    definition: 'A compact representation of changes between two versions of a file, showing lines added (+) and removed (-). LLM coding agents often generate diffs rather than full files for efficiency. Unified diff format is the standard for patch files.',
    category: 'Code & Dev',
  },
  'sandbox': {
    definition: 'An isolated execution environment that runs untrusted code with restricted access to the host system. Coding agents use sandboxes (containers, VMs, e2b.dev) to safely execute generated code without risk to the host machine.',
    category: 'Code & Dev',
  },
  'code understanding': {
    definition: 'An agent\'s ability to comprehend existing codebases: understanding structure, finding relevant files, tracing call graphs, and identifying dependencies. Good code understanding requires a combination of file search, AST analysis, and semantic embeddings.',
    category: 'Code & Dev',
  },
  'terminal': {
    definition: 'A text-based interface for executing shell commands. Coding agents interact with terminals to run tests, install packages, execute scripts, and observe program output. Full terminal access is one of the most powerful (and risky) agent capabilities.',
    category: 'Code & Dev',
  },

  // ── Prompt Engineering ────────────────────────────────────────────────────────
  'prompt engineering': {
    definition: 'The practice of designing and refining prompts to elicit desired behaviors from LLMs. Prompt engineering includes instruction design, few-shot examples, chain-of-thought elicitation, persona setting, and output format specification.',
    category: 'Prompt Engineering',
  },
  'in-context learning': {
    definition: 'Learning from examples provided directly in the prompt, without any weight updates. LLMs perform in-context learning by recognizing patterns in few-shot examples. The quality and diversity of examples significantly affects learning effectiveness.',
    category: 'Prompt Engineering',
  },
  'instruction tuning': {
    definition: 'Fine-tuning a base LLM on a dataset of instruction-response pairs to improve its ability to follow natural language instructions. Instruction-tuned models (InstructGPT, Llama-3-Instruct) are dramatically more useful than base models for most applications.',
    category: 'Prompt Engineering',
  },
  'persona': {
    definition: 'Assigning a specific role or character to an LLM in the system prompt to shape its responses. Personas ("You are an expert Python developer") improve relevance and consistency. They can also be used to simulate domain experts or stakeholders.',
    category: 'Prompt Engineering',
  },
  'output format': {
    definition: 'Specifying the desired structure of an LLM\'s response in the prompt — markdown, JSON, bullet points, code blocks. Explicit format instructions dramatically improve parsability and consistency, especially for programmatic consumers.',
    category: 'Prompt Engineering',
  },
  'tree of thought': {
    definition: 'A prompting technique where the model explores multiple reasoning paths simultaneously, like a tree, before selecting the best continuation. Tree of Thought (ToT) outperforms linear chain-of-thought on tasks requiring search or backtracking.',
    category: 'Prompt Engineering',
  },

  // ── Research / Architecture ───────────────────────────────────────────────────
  'agent design pattern': {
    definition: 'A reusable solution to a common agent architecture problem. Examples: tool executor, reflection loop, critic-actor, plan-and-execute, parallel sub-agents. Design patterns provide battle-tested templates for building reliable agentic systems.',
    category: 'Research',
  },
  'critic': {
    definition: 'A component (often a second LLM call) that evaluates the main agent\'s output for correctness, safety, or quality. The critic-actor pattern uses the critic\'s feedback to iteratively refine outputs. Critics can be self-critiquing (same model) or independent.',
    category: 'Research',
  },
  'actor': {
    definition: 'In agent architecture, the component responsible for taking actions — generating text, calling tools, producing code. In critic-actor patterns, the actor proposes actions and the critic evaluates them before execution.',
    category: 'Research',
  },
  'verifier': {
    definition: 'A component that checks whether an agent\'s output or action satisfies given constraints — tests pass, invariants hold, policies are met. Verifiers provide objective feedback for agent self-improvement and are critical for reliable code generation.',
    category: 'Research',
  },
  'evaluation': {
    definition: 'Measuring an LLM or agent system\'s performance on a defined set of tasks or metrics. Evals include automated benchmarks (HumanEval, SWE-bench), LLM-as-judge, and human raters. Good evals are necessary to detect regressions and guide development.',
    category: 'Research',
  },
  'benchmark': {
    definition: 'A standardized test suite for measuring LLM/agent capabilities. Examples: MMLU (knowledge), HumanEval (code), SWE-bench (GitHub issue resolution), GAIA (real-world tasks). Benchmarks enable apples-to-apples comparison between models.',
    category: 'Research',
  },
  'swe-bench': {
    definition: 'A benchmark that measures AI coding agent performance on real GitHub issues: given a repository and an issue, can the agent produce a patch that passes the existing test suite? SWE-bench is the standard for evaluating practical coding agent capability.',
    category: 'Research',
  },
  'parallelism': {
    definition: 'Running multiple agent operations simultaneously to reduce total wall-clock time. Parallel tool calls, parallel sub-agents, and speculative execution are key patterns. Most modern agent frameworks support async/parallel execution for independent tasks.',
    category: 'Research',
  },
  'idempotency': {
    definition: 'A property where performing the same operation multiple times produces the same result. Idempotent tool calls are safe to retry on failure. Designing idempotent tools is essential for reliable agent error recovery.',
    category: 'Research',
  },
  'rollback': {
    definition: 'Undoing a set of actions to restore a previous state. In coding agents, rollback means reverting file changes when a task fails. Git provides natural rollback capabilities; agents should commit checkpoints to enable clean rollback.',
    category: 'Research',
  },
  'interruption': {
    definition: 'A mechanism for stopping or pausing an agent\'s execution mid-task. Interruptions can be user-triggered (cancel) or system-triggered (error, timeout). Clean interruption handling preserves partial work and provides meaningful status to the user.',
    category: 'Research',
  },

  // ── Model Providers ───────────────────────────────────────────────────────────
  'openai': {
    definition: 'The company behind GPT-4, GPT-4o, o1, and the Codex model family. OpenAI\'s API is the most widely adopted LLM API, and its function calling format has become an informal industry standard for agent tool use.',
    category: 'Model Providers',
  },
  'anthropic': {
    definition: 'The AI safety company behind the Claude model family (Claude 3 Haiku/Sonnet/Opus, Claude 3.5 Sonnet). Anthropic focuses on AI safety research, introduced Constitutional AI, and created the Model Context Protocol (MCP).',
    category: 'Model Providers',
  },
  'claude': {
    definition: 'Anthropic\'s family of LLMs, including Claude 3.5 Sonnet (high capability), Claude 3 Haiku (fast/cheap), and Claude 3 Opus (highest capability). Claude is known for strong coding, long context handling (200K tokens), and instruction following.',
    category: 'Model Providers',
  },
  'gpt-4': {
    definition: 'OpenAI\'s most capable GPT model family, including GPT-4, GPT-4 Turbo, and GPT-4o (omni, with multimodal capabilities). GPT-4 set the benchmark for reasoning, coding, and general task performance when released in 2023.',
    category: 'Model Providers',
  },
  'gpt-4o': {
    definition: 'OpenAI\'s multimodal model that processes text, images, and audio in a single model. GPT-4o (omni) is faster and cheaper than GPT-4 Turbo while matching or exceeding its text performance. It supports real-time voice interaction.',
    category: 'Model Providers',
  },
  'gemini': {
    definition: 'Google\'s family of multimodal LLMs (Gemini Pro, Ultra, Flash, Nano). Gemini Ultra 1.0 was the first model to surpass human experts on MMLU. Gemini 1.5 Pro supports a 1M-token context window.',
    category: 'Model Providers',
  },
  'llama': {
    definition: 'Meta\'s family of open-weight LLMs (Llama 2, Llama 3, Llama 3.1 405B). Llama models are freely available for research and commercial use, enabling local deployment, fine-tuning, and customization without API costs.',
    category: 'Model Providers',
  },
  'ollama': {
    definition: 'A tool for running LLMs locally on macOS, Linux, and Windows. Ollama manages model downloads, quantization, and serving via a simple CLI and HTTP API. Popular for running Llama, Mistral, and other open models without a cloud dependency.',
    category: 'Model Providers',
  },
  'mistral': {
    definition: 'A French AI company producing efficient open-weight models (Mistral 7B, Mixtral 8x7B MoE, Mistral Large). Mistral models are known for strong performance-per-parameter, making them popular for local and edge deployment.',
    category: 'Model Providers',
  },

  // ── Misc / General ────────────────────────────────────────────────────────────
  'neural network': {
    definition: 'A computational model inspired by biological neurons: layers of interconnected nodes that transform inputs through learned weights. Deep neural networks with many layers are the foundation of modern AI, including all LLMs.',
    category: 'Core LLM',
  },
  'parameter': {
    definition: 'A learnable weight in a neural network updated during training. Model size is measured in parameters (7B, 70B, 405B). More parameters generally improve capability but increase memory, compute cost, and inference latency.',
    category: 'Core LLM',
  },
  'weights': {
    definition: 'The numerical values (parameters) that define a trained model\'s behavior. Weights are updated during training and fixed during inference. Model weights are the "knowledge" encoded by the training process.',
    category: 'Core LLM',
  },
  'gpu': {
    definition: 'Graphics Processing Unit: massively parallel hardware used for training and running LLMs. A100 and H100 GPUs are the standard for large-scale AI compute. GPUs accelerate matrix operations that dominate neural network computation.',
    category: 'Infrastructure',
  },
  'vram': {
    definition: 'Video RAM: GPU memory used to store model weights, activations, and KV cache during inference. A 70B parameter model in 16-bit precision requires ~140GB VRAM. Quantization reduces VRAM requirements significantly.',
    category: 'Infrastructure',
  },
  'kv cache': {
    definition: 'Key-Value cache: stores intermediate attention computations for previously processed tokens, avoiding recomputation on each generation step. KV cache is the primary memory bottleneck for long-context inference, growing linearly with sequence length.',
    category: 'Infrastructure',
  },
  'speculative decoding': {
    definition: 'A technique where a small draft model generates candidate tokens that a larger verifier model approves in parallel, reducing latency. When the draft is usually correct, speculative decoding achieves near-draft-model speed at large-model quality.',
    category: 'Infrastructure',
  },
  'peft': {
    definition: 'Parameter-Efficient Fine-Tuning: methods that update a small subset of parameters (or add a tiny set of new parameters) instead of all model weights. LoRA, prefix tuning, and prompt tuning are common PEFT methods, enabling fine-tuning on consumer hardware.',
    category: 'Training',
  },
  'constitutional ai': {
    definition: 'Anthropic\'s alignment technique where the model is trained to critique and revise its own outputs according to a set of principles (a "constitution"). Constitutional AI reduces reliance on human feedback for harmful content detection.',
    category: 'Training',
  },
}
