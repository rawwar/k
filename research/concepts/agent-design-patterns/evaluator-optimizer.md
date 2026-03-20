# Evaluator-Optimizer

> Iterative refinement through generate-and-critique loops

## Overview

The Evaluator-Optimizer pattern creates a feedback loop between two distinct
LLM roles: a **generator** that produces candidate outputs and an **evaluator**
that critiques them. The generator refines its output based on evaluation
feedback until the result meets predefined quality criteria or a maximum
iteration count is reached.

Anthropic describes this pattern as "particularly effective when we have clear
evaluation criteria, and when iterative refinement provides measurable value."
The key insight is that LLMs can serve both roles — producing content and
judging content — and that separating these concerns into distinct steps often
yields better results than asking a single call to "get it right the first
time."

This pattern appears throughout coding agents in various forms: test-driven
development loops, linting feedback cycles, compilation error correction, and
explicit self-reflection prompts. It is one of the most natural patterns in
software engineering because it mirrors the human write-review-revise workflow
that every developer practices daily.

Among the 17 agents studied in this research library, the evaluator-optimizer
pattern manifests most clearly in:

- **Junie CLI** — a fixed understand-plan-implement-verify-iterate pipeline
- **ForgeCode** — enforced verification skill that mandates evaluation
- **OpenHands** — StuckDetector that monitors optimization loop health
- **Aider** — systematic edit format benchmarking across evaluation criteria
- **Claude Code** — implicit evaluation through tool result interpretation

The pattern sits at a middle point on the complexity spectrum. It is more
sophisticated than a single LLM call or a simple prompt chain, but less
complex than a fully autonomous agent with dynamic planning. This makes it
a pragmatic choice for many real-world coding tasks where "good enough on the
first try" is not reliable but "iterate until tests pass" is achievable.

---

## Architecture

The canonical evaluator-optimizer architecture consists of two LLM calls in a
loop, connected by a feedback channel:

```
+---------------------------------------------------+
|              Evaluator-Optimizer Loop               |
|                                                     |
|  +-----------+    +-----------+                    |
|  | Generator  |--->| Evaluator |                    |
|  |  (LLM 1)  |<---|  (LLM 2)  |                    |
|  +-----------+    +-----------+                    |
|       |                  |                          |
|       |    feedback      |   score/pass/fail        |
|       |<-----------------|                          |
|       |                                             |
|       v                                             |
|  [Output when criteria met or max iterations]       |
+---------------------------------------------------+
```

The generator and evaluator may be:

1. **The same model with different prompts** — most common in practice; one
   system prompt instructs generation, another instructs evaluation
2. **Different models** — a powerful model generates while a cheaper/faster
   model evaluates, or vice versa
3. **An LLM + programmatic checker** — the generator is an LLM while the
   evaluator is a test suite, compiler, or linter (the hybrid approach)

### Data Flow

```
+------------+
|   Input     |
|  (task)     |
+------+------+
       |
       v
+------------+     +--------------+     +-------------+
|  Generate   |---->|   Evaluate   |---->|  Criteria    |
|  candidate  |     |   output     |     |  met?        |
+------------+     +--------------+     +------+------+
       ^                                   |       |
       |              NO                   |       | YES
       |<----------------------------------+       |
       |     (feedback + instructions)             v
       |                                    +-------------+
       |                                    |  Return      |
       |                                    |  final       |
       |                                    |  output      |
       |                                    +-------------+
       |
       |  (also exits if max_iterations reached)
```

### In the Context of Coding Agents

For coding agents specifically, the architecture often takes a specialized form
where the evaluator is not another LLM call but the execution environment
itself:

```
+--------------------------------------------------------+
|           Test-Driven Evaluator-Optimizer                |
|                                                          |
|  +-----------+   +----------+   +------------------+    |
|  |  LLM      |-->|  Code    |-->|  Execute Tests   |    |
|  |  Generate  |   |  Write   |   |  (programmatic   |    |
|  |  /Fix Code |   |  to Disk |   |   evaluator)     |    |
|  +-----------+   +----------+   +--------+---------+    |
|       ^                                   |              |
|       |         test results              |              |
|       |   (pass/fail + error messages)    |              |
|       |<----------------------------------+              |
|                                                          |
|  Exit: all tests pass OR max iterations reached          |
+--------------------------------------------------------+
```

This hybrid approach — LLM generation with programmatic evaluation — is the
dominant form in production coding agents because test results provide
unambiguous, deterministic feedback that is far more reliable than LLM-based
evaluation for code correctness.

---

## Core Concepts

### Generate-Then-Critique Loop

The fundamental mechanism is straightforward: produce an output, judge it,
and if the judgment says it's not good enough, try again with the judgment
as additional context. This mirrors how humans naturally work — write a draft,
review it, revise it.

The power of the pattern comes from the observation that **LLMs are often
better at evaluating than generating**. A model that struggles to produce
perfect code in one shot may reliably identify bugs when shown the code and
asked "what's wrong with this?" This asymmetry makes the two-step process
more capable than either step alone.

### Self-Reflection and Self-Correction

A specific instantiation where the same model both generates and evaluates.
The model is prompted to critique its own output, identify weaknesses, and
produce an improved version. Research shows this works when:

- The model has the capability to recognize errors it makes
- The evaluation prompt is specific enough to guide useful critique
- The task has objectively identifiable quality dimensions

Self-correction can fail when the model is confident in incorrect outputs
and cannot recognize its own errors — a well-documented limitation.

### Evaluation Criteria Design

The quality of the evaluator-optimizer loop depends entirely on the quality
of the evaluation criteria. Vague criteria ("is this code good?") produce
vague feedback. Specific criteria produce actionable feedback:

- **Functional correctness** — does the code produce expected outputs?
- **Test passage** — do all unit/integration tests pass?
- **Compilation** — does the code compile without errors?
- **Lint compliance** — does the code pass linting rules?
- **Performance** — does the code meet performance benchmarks?
- **Security** — does the code pass security scanning?

### Convergence and Stopping Conditions

Not all evaluation loops converge. A loop might:

1. **Converge** — quality improves each iteration until criteria are met
2. **Oscillate** — alternates between two or more solutions without improving
3. **Diverge** — quality degrades as the model "overthinks" or introduces
   new bugs while fixing old ones
4. **Plateau** — quality stops improving but criteria aren't met

Stopping conditions must account for all four cases:

- **Success exit** — all evaluation criteria met
- **Iteration limit** — maximum attempts reached
- **No improvement** — score hasn't improved in N iterations
- **Oscillation detection** — same or similar outputs repeating

### Analogy to Human Iterative Process

The pattern is not novel in concept — it is how every experienced developer
works. The innovation is in automating both sides:

| Human Process           | Evaluator-Optimizer Equivalent         |
|------------------------|----------------------------------------|
| Write code             | LLM generates code                     |
| Run tests              | Programmatic evaluator runs tests      |
| Read error messages    | Error messages fed back to LLM         |
| Think about what's     | LLM receives feedback context and      |
| wrong and fix it       | generates revised code                 |
| Ask colleague for      | Second LLM evaluates with fresh        |
| code review            | perspective                            |

---

## Evaluation Strategies

### LLM-as-Judge

A second LLM call (or the same model with a different prompt) evaluates the
generated output. This approach is flexible but non-deterministic:

```python
def llm_evaluate(generated_code: str, task: str) -> dict:
    """Use an LLM to evaluate generated code."""
    evaluation_prompt = f"""
    Evaluate the following code against the task requirements.

    Task: {task}
    Code: {generated_code}

    Rate on these dimensions (1-5):
    1. Correctness: Does it solve the task?
    2. Completeness: Does it handle edge cases?
    3. Clarity: Is it readable and well-structured?
    4. Efficiency: Is it reasonably performant?

    For any score below 4, provide specific feedback on what
    to improve.

    Respond in JSON format.
    """
    return call_llm(evaluation_prompt)
```

**Strengths**: Can evaluate subjective qualities (readability, design).
**Weaknesses**: Non-deterministic, can miss bugs, adds latency and cost.

### Programmatic Evaluation

The evaluator is code, not an LLM. For coding agents, this is the most
reliable strategy:

```python
def programmatic_evaluate(code: str, test_suite: str) -> dict:
    """Evaluate code by running tests."""
    write_file("solution.py", code)
    result = run_command(f"python -m pytest {test_suite}")
    return {
        "passed": result.returncode == 0,
        "output": result.stdout,
        "errors": result.stderr,
        "score": count_passing_tests(result.stdout)
    }
```

**Strengths**: Deterministic, reliable, objective, fast.
**Weaknesses**: Can only evaluate what tests cover. Requires tests to exist.

### Hybrid Evaluation

Combines programmatic checks with LLM evaluation — the most common approach
in production coding agents:

1. First, run programmatic checks (compile, test, lint)
2. If programmatic checks pass, optionally run LLM evaluation for
   subjective quality (readability, design patterns, documentation)
3. Combine scores for overall assessment

This is what agents like Junie CLI effectively do: run tests (programmatic),
then use the LLM to interpret results and decide next steps (LLM evaluation).

### Rubric-Based Scoring

Define a detailed rubric and have the evaluator score against it:

```
Rubric for Code Quality:
- [ ] All tests pass (0 or 50 points)
- [ ] No linting errors (0 or 15 points)
- [ ] Type-safe (0 or 15 points)
- [ ] Handles edge cases (0-10 points)
- [ ] Has docstrings (0-10 points)

Threshold: 80/100 to pass
```

### Binary Pass/Fail vs Scored Evaluation

**Binary**: Simple to implement, clear stopping condition. "Do all tests
pass? Yes -> done. No -> iterate."

**Scored**: Provides gradient information. "Score went from 60 to 75 — we're
improving. Score went from 75 to 74 — we might be oscillating."

Binary evaluation is preferred when clear pass/fail criteria exist (tests).
Scored evaluation is useful for subjective or multi-dimensional quality
assessment.

---

## Test-Driven Optimization in Coding Agents

The most prevalent instantiation of the evaluator-optimizer pattern in coding
agents is the **test-driven development loop**: generate code, run tests, fix
failures, repeat.

### Junie CLI: The Structured Pipeline

Junie CLI implements the most explicit evaluator-optimizer pipeline among the
17 agents studied. Its fixed pipeline is:

```
understand -> plan -> implement -> verify -> iterate -> present
                                     |         ^
                                     |         |
                                     +---------+
                                   (loop until tests
                                    pass or limit hit)
```

The verify step runs the project's test suite. If tests fail, the iterate
step feeds test output back to the LLM for code revision. This continues
until tests pass or a maximum iteration count is reached. The pipeline is
**fixed** — the agent always follows this sequence, making it a prompt chain
with an evaluator-optimizer sub-loop rather than a fully autonomous agent.

This design choice reflects a key insight: for coding tasks, the
evaluate-then-fix loop is predictable enough to hard-code. You don't need
dynamic planning to know that "run tests, fix if failing" is the right
next step.

### ForgeCode: Enforced Verification

ForgeCode takes a more prescriptive approach with its enforced verification
skill. The agent is required to verify its work — it cannot skip the
evaluation step. This addresses a common failure mode where agents declare
success without actually checking their work.

The enforcement mechanism ensures that every code generation cycle includes
at least one evaluation pass, preventing the "write and pray" anti-pattern
that plagues simpler agent implementations.

### OpenHands: StuckDetector

OpenHands adds a meta-evaluator — the StuckDetector — that monitors the
evaluation loop itself. While other agents iterate until tests pass or a
limit is hit, OpenHands actively watches for pathological loop behavior:

- Repeated identical actions (the agent is stuck)
- Oscillation between two states (fix A breaks B, fix B breaks A)
- Monotonically increasing error counts (making things worse)

When the StuckDetector fires, it breaks the loop and forces a different
approach. This is a crucial innovation because naive evaluator-optimizer
loops can waste enormous amounts of tokens and time when they don't converge.

### Aider: Edit Format Benchmarking

Aider implements the evaluator-optimizer pattern at a meta-level: rather
than optimizing individual code outputs, it optimizes its own **edit format**
across benchmark suites. Aider systematically tests different ways of
representing code changes (whole file, search/replace, unified diff) and
evaluates which format produces the best results across standardized
benchmarks.

This is evaluator-optimizer applied to the agent's own design:

```
+-----------------------------------------------------+
|        Aider's Meta-Level Optimization               |
|                                                       |
|  +--------------+   +----------------+               |
|  | Edit Format   |-->| Run SWE-bench  |              |
|  | Configuration |   | Benchmark      |              |
|  +--------------+   +-------+--------+               |
|         ^                    |                        |
|         |    benchmark       |                        |
|         |    results         |                        |
|         |<-------------------+                        |
|         |                                             |
|   [Adjust edit format, model settings, prompts]       |
|   [Repeat across benchmark suite]                     |
+-----------------------------------------------------+
```

### Claude Code: Implicit Evaluation

Claude Code does not have an explicit evaluator-optimizer loop, but it
achieves similar results through its tool-use feedback cycle. When Claude
Code runs a command and gets an error, the error output naturally becomes
feedback for the next generation attempt. The "evaluator" is the execution
environment, and the "feedback" is the tool result.

This implicit pattern is common across single-loop agents: the environment
provides evaluation signals (errors, test results, type-check output) that
feed back into the agent's context window, driving iterative improvement
without explicit evaluator architecture.

---

## Self-Reflection Patterns

### Model Critiques Its Own Output

The simplest self-reflection pattern appends a critique prompt after
generation:

```python
def generate_with_self_reflection(task: str) -> str:
    # Step 1: Generate
    code = call_llm(f"Write code to: {task}")

    # Step 2: Self-critique
    critique = call_llm(f"""
    Review this code for bugs, edge cases, and improvements:
    {code}

    List specific issues found.
    """)

    # Step 3: Revise based on critique
    revised = call_llm(f"""
    Original code: {code}
    Issues found: {critique}

    Write an improved version addressing all issues.
    """)

    return revised
```

This adds two LLM calls but often catches bugs that the first pass missed.

### Chain-of-Thought Evaluation

Instead of a simple "is this good?" evaluation, the evaluator reasons
step-by-step through the output:

```python
evaluation_prompt = """
Evaluate this code step by step:

1. Trace through the main function with a typical input.
   What happens at each step?
2. Now trace through with an edge case (empty input).
   What happens?
3. Check each function: does it handle errors properly?
4. Are there any type mismatches or undefined variables?
5. Based on your analysis, what specific changes are needed?
"""
```

Chain-of-thought evaluation catches more subtle issues because it forces
the model to simulate execution rather than pattern-match on code quality.

### Error Analysis and Targeted Fix

When test failures provide specific error messages, the most effective
self-reflection pattern is targeted error analysis:

```python
def targeted_fix(code: str, error: str) -> str:
    """Fix code based on specific error output."""
    return call_llm(f"""
    This code produces an error:

    Code:
    {code}

    Error:
    {error}

    Analyze the error, identify the root cause, and provide
    a corrected version. Explain what was wrong.
    """)
```

This pattern works well because error messages provide precise, actionable
feedback — the model doesn't need to figure out *what's wrong*, only
*how to fix* the identified problem.

### "What Went Wrong?" Prompting

A broader reflection pattern useful when the error isn't immediately clear:

```python
reflection_prompt = f"""
I attempted to solve this task: {task}
My solution: {code}
The result was: {result}
Expected result: {expected}

What went wrong? Analyze the discrepancy between expected
and actual results. Then provide a corrected solution.
"""
```

---

## Quality Scoring

### Code Quality Metrics as Evaluation Signals

Coding agents can use a variety of automated quality signals as evaluation
criteria:

| Signal              | Tool              | Binary/Scored |
|---------------------|-------------------|---------------|
| Tests pass          | pytest, jest      | Binary        |
| Code compiles       | gcc, tsc, rustc   | Binary        |
| Lint clean          | eslint, ruff      | Scored        |
| Type-check clean    | mypy, tsc         | Binary        |
| Coverage threshold  | coverage.py       | Scored        |
| Complexity limit    | radon, lizard     | Scored        |
| Security scan       | bandit, semgrep   | Scored        |

### Test Coverage as Optimization Target

Some agents optimize not just for test passage but for test coverage:

```
Iteration 1: 60% coverage, 3 tests failing
Iteration 2: 75% coverage, 1 test failing
Iteration 3: 82% coverage, 0 tests failing -> exit
```

This is particularly relevant for agents like ForgeCode that enforce
verification — the verification can check not just "do tests pass" but
"is the code adequately tested."

### Performance Benchmarks as Criteria

For performance-sensitive tasks, the evaluator can include runtime
measurements:

```python
def evaluate_performance(code: str, benchmark: str) -> dict:
    """Evaluate code against performance criteria."""
    write_file("solution.py", code)
    result = run_command(f"python benchmark.py")
    return {
        "runtime_ms": parse_runtime(result.stdout),
        "memory_mb": parse_memory(result.stdout),
        "meets_criteria": (
            parse_runtime(result.stdout) < 100 and
            parse_memory(result.stdout) < 256
        )
    }
```

### Style and Convention Adherence

Less critical but sometimes relevant, especially for agents generating code
that humans will maintain. Linters and formatters provide automated style
evaluation that feeds cleanly into the evaluator-optimizer loop.

---

## Implementation Patterns

### Simple Retry Loop with Evaluation

The most basic evaluator-optimizer implementation:

```python
def evaluator_optimizer(
    task: str,
    max_iterations: int = 5
) -> str:
    """Basic evaluator-optimizer loop."""
    code = generate_code(task)

    for i in range(max_iterations):
        # Evaluate
        result = run_tests(code)
        if result.all_passed:
            return code

        # Generate improved version
        code = call_llm(f"""
        Task: {task}
        Current code: {code}
        Test results: {result.output}

        Fix the failing tests. Return the complete corrected code.
        """)

    # Return best effort after max iterations
    return code
```

### Multi-Criteria Evaluation

When multiple quality dimensions matter:

```python
def multi_criteria_evaluate(code: str) -> dict:
    """Evaluate code across multiple dimensions."""
    scores = {}

    # Programmatic checks
    scores["tests"] = run_tests(code).pass_rate
    scores["lint"] = run_linter(code).score
    scores["typecheck"] = run_typecheck(code).clean

    # LLM evaluation for subjective quality
    llm_eval = call_llm(f"Rate readability 1-10: {code}")
    scores["readability"] = parse_score(llm_eval)

    # Composite score
    scores["overall"] = (
        scores["tests"] * 0.5 +
        scores["lint"] * 0.2 +
        (10 if scores["typecheck"] else 0) * 0.1 +
        scores["readability"] * 0.2
    )

    return scores
```

### Progressive Refinement with Diminishing Iterations

Allocate more effort to early iterations when improvement potential is
highest:

```python
def progressive_refinement(task: str) -> str:
    """Refine with decreasing effort per iteration."""
    code = generate_code(task)

    # First pass: comprehensive review
    eval_1 = full_evaluation(code)
    if eval_1["passed"]:
        return code
    code = revise(code, eval_1, detail="comprehensive")

    # Second pass: focused on remaining issues
    eval_2 = focused_evaluation(code, eval_1["issues"])
    if eval_2["passed"]:
        return code
    code = revise(code, eval_2, detail="targeted")

    # Third pass: quick check
    eval_3 = quick_check(code)
    return code  # Return regardless
```

### Early Exit on Success

Always check for success before iterating — avoid unnecessary LLM calls:

```python
def optimize_with_early_exit(task: str) -> str:
    """Exit as soon as criteria are met."""
    code = generate_code(task)
    best_code = code
    best_score = 0.0

    for i in range(MAX_ITERATIONS):
        result = evaluate(code)

        # Early exit conditions
        if result.score >= THRESHOLD:
            return code
        if i > 0 and result.score <= prev_score:
            return best_code

        prev_score = result.score
        if result.score > best_score:
            best_score = result.score
            best_code = code
        code = refine(code, result.feedback)

    return best_code
```

---

## The Stuck Problem

### When Optimization Loops Don't Converge

The most dangerous failure mode of the evaluator-optimizer pattern is the
infinite (or very long) loop that consumes tokens without making progress.
This happens more often than practitioners expect:

**Scenario 1: Whack-a-mole bugs**
The LLM fixes one bug but introduces another. Each iteration has the same
number of failing tests, just different ones.

**Scenario 2: Fundamental misunderstanding**
The LLM doesn't understand the task requirements. No amount of iteration
will help because it's optimizing toward the wrong goal.

**Scenario 3: Capability ceiling**
The task requires capabilities beyond the model's ability. The model keeps
trying variations but can't produce a correct solution.

**Scenario 4: Oscillation**
The model alternates between two approaches, never settling on one:
```
Iteration 1: Uses approach A -> fails test X
Iteration 2: Switches to approach B -> fails test Y
Iteration 3: Switches back to approach A -> fails test X
... forever
```

### OpenHands' StuckDetector

OpenHands addresses this with a dedicated StuckDetector component that
monitors the agent's action history for pathological patterns:

```
StuckDetector checks:
  - Repeated identical actions (exact same code submitted)
  - Oscillation (alternating between N states)
  - Empty responses (model returns nothing useful)
  - Monotonically increasing errors
  - Token budget approaching limit with no progress
```

When stuck behavior is detected, the StuckDetector:
1. Breaks the current loop
2. Injects a "you appear to be stuck" prompt
3. Suggests a fundamentally different approach
4. May escalate to a different strategy or model

This meta-evaluation layer is critical for production reliability.

### Maximum Iteration Limits

The simplest convergence guarantee: hard-cap the number of iterations.
Every production agent uses this, but the choice of limit matters:

- **Too low** (1-2): Misses easy fixes that need one more iteration
- **Sweet spot** (3-5): Catches most fixable issues without waste
- **Too high** (10+): Wastes tokens on unfixable problems

Empirically, most code fixes that will converge do so within 3 iterations.
After 5 iterations without convergence, the probability of convergence
drops sharply.

### Fallback Strategies

When the evaluator-optimizer loop fails to converge:

1. **Return best-so-far** — the iteration with the highest score
2. **Escalate** — hand off to a more capable model
3. **Decompose** — break the task into smaller sub-tasks
4. **Reset** — discard all iterations and start from scratch with a
   different initial approach
5. **Human escalation** — ask the user for guidance

### Detecting Oscillation

```python
def detect_oscillation(history: list, window: int = 4) -> bool:
    """Detect if outputs are oscillating between states."""
    if len(history) < window:
        return False

    recent = history[-window:]
    unique = set(recent)

    # If only 2 unique outputs in last 4 iterations -> oscillating
    if len(unique) <= 2:
        return True

    # Check for similarity-based oscillation
    similarities = [
        similarity(recent[i], recent[i-2])
        for i in range(2, len(recent))
    ]
    return all(s > 0.95 for s in similarities)
```

---

## When to Use

The evaluator-optimizer pattern is appropriate when:

1. **Clear, measurable evaluation criteria exist** — you can write a function
   that scores or pass/fails the output. Tests, compilation, linting, and
   type-checking all qualify.

2. **Iterative refinement provides measurable value** — the second attempt
   is reliably better than the first, given evaluation feedback.

3. **The LLM can provide useful self-critique** — the model is capable of
   identifying errors when shown its output alongside the failure signal.

4. **The task is within model capability but not reliably one-shot** — the
   model can solve it but doesn't always get it right on the first try.

5. **The evaluation is cheaper than generation** — running tests is much
   cheaper than generating code, making the loop economically viable.

**Best use cases in coding agents:**
- Fix failing tests (clear pass/fail criteria)
- Resolve compilation errors (binary success/failure)
- Meet linting standards (scored evaluation)
- Match expected output format (programmatic comparison)

---

## When NOT to Use

The evaluator-optimizer pattern is counterproductive when:

1. **No clear evaluation criteria** — if you can't write an evaluation
   function, the loop has no signal to optimize toward.

2. **First attempt is usually good enough** — if the LLM reliably produces
   correct output in one shot, the evaluation overhead is pure waste.

3. **Latency-sensitive applications** — each iteration adds a full LLM
   call's worth of latency. For real-time applications, a single optimized
   call is better.

4. **Evaluation is as hard as generation** — if judging the output requires
   as much intelligence as producing it, the evaluator won't be more reliable
   than the generator. This is common in creative or subjective tasks.

5. **The task exceeds model capability** — no amount of iteration will help
   if the model fundamentally cannot solve the problem. The loop will just
   burn tokens.

6. **Error feedback is uninformative** — if failures produce vague error
   messages ("something went wrong"), the feedback loop has no actionable
   signal.

Anthropic's own guidance applies: "We recommend finding the simplest solution
possible, and only increasing complexity when needed." If a single LLM call
with good prompting achieves 95% accuracy, adding an evaluation loop for the
remaining 5% may not be worth the cost.

---

## Code Examples

### Complete Evaluator-Optimizer for Code Generation

```python
import subprocess
import json
from dataclasses import dataclass


@dataclass
class EvalResult:
    passed: bool
    score: float
    feedback: str
    test_output: str


def generate_code(task: str, context: str = "") -> str:
    """Generate code for a task, optionally with feedback context."""
    prompt = f"Write Python code to: {task}"
    if context:
        prompt += f"\n\nPrevious attempt feedback:\n{context}"
    return call_llm(prompt)


def evaluate_code(code: str, test_file: str) -> EvalResult:
    """Evaluate code by running tests."""
    with open("solution.py", "w") as f:
        f.write(code)

    result = subprocess.run(
        ["python", "-m", "pytest", test_file, "-v"],
        capture_output=True, text=True, timeout=30
    )

    passed = result.returncode == 0
    total, passing = parse_test_counts(result.stdout)
    score = passing / total if total > 0 else 0.0

    return EvalResult(
        passed=passed,
        score=score,
        feedback=result.stdout + result.stderr,
        test_output=result.stdout
    )


def evaluator_optimizer_loop(
    task: str,
    test_file: str,
    max_iterations: int = 5,
    min_improvement: float = 0.05
) -> str:
    """Full evaluator-optimizer loop with convergence detection."""
    code = generate_code(task)
    best_code = code
    best_score = 0.0
    history = []

    for iteration in range(max_iterations):
        # Evaluate
        result = evaluate_code(code, test_file)
        history.append(result.score)

        # Success — all tests pass
        if result.passed:
            print(f"Converged at iteration {iteration + 1}")
            return code

        # Track best
        if result.score > best_score:
            best_score = result.score
            best_code = code

        # Check for stagnation
        if (len(history) >= 3 and
            max(history[-3:]) - min(history[-3:]) < min_improvement):
            print(f"Stagnated at iteration {iteration + 1}")
            return best_code

        # Generate improved version
        code = generate_code(
            task,
            context=f"Score: {result.score:.0%}\n{result.feedback}"
        )

    print(f"Max iterations reached, returning best attempt")
    return best_code
```

### Evaluator-Optimizer with LLM-as-Judge

```python
def llm_judge_loop(
    task: str,
    max_iterations: int = 3
) -> str:
    """Evaluator-optimizer using LLM as the evaluator."""
    code = generate_code(task)

    for i in range(max_iterations):
        # LLM evaluation
        evaluation = call_llm(f"""
        Task: {task}
        Code: {code}

        Evaluate this code. Respond in JSON:
        {{
          "score": <1-10>,
          "issues": ["issue1", "issue2"],
          "passed": <true if score >= 8>
        }}
        """)

        eval_data = json.loads(evaluation)

        if eval_data["passed"]:
            return code

        # Refine based on LLM feedback
        issues = eval_data["issues"]
        code = call_llm(f"""
        Improve this code to address these issues:
        {issues}

        Original code:
        {code}
        """)

    return code
```

---

## Key Takeaways

1. **The evaluator-optimizer pattern is the backbone of coding agents** — the
   write-test-fix loop is essentially evaluator-optimizer with programmatic
   evaluation. Nearly every coding agent studied implements some form of it.

2. **Programmatic evaluation beats LLM evaluation for code** — tests,
   compilers, and linters provide deterministic, reliable feedback that LLM
   judges cannot match for correctness verification.

3. **The stuck problem is real and under-addressed** — most agents use simple
   iteration limits, but OpenHands' StuckDetector approach of actively
   monitoring for pathological loop behavior is more robust.

4. **3-5 iterations is the sweet spot** — empirically, most fixable issues
   converge within this range. Beyond 5 iterations, you're likely stuck.

5. **Evaluation criteria quality determines loop quality** — invest in
   clear, specific, measurable evaluation criteria. Vague criteria produce
   vague improvements.

6. **The pattern scales from simple to sophisticated** — from a basic retry
   loop to Aider's meta-level benchmark optimization, the core generate-
   evaluate-refine structure adapts to many contexts.

7. **Always include an escape hatch** — no evaluation loop should run
   unbounded. Hard limits, stagnation detection, and oscillation detection
   are essential for production reliability.

8. **Anthropic's simplicity principle applies** — if a single well-prompted
   LLM call reliably succeeds, don't add an evaluation loop. "Success in
   the LLM space isn't about building the most sophisticated system. It's
   about building the right system for your needs."
