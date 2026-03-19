---
title: "Ante – Benchmark Performance & Benchmark Integrity Analysis"
status: complete
---

# Ante Benchmark Performance & Analysis

> Ante is the AI coding agent built by Antigma Labs. Beyond competing on
> public leaderboards, Antigma has used Ante itself to forensically audit
> the integrity of those same leaderboards — turning benchmark evaluation
> into a first-class research concern rather than a marketing exercise.

## Terminal-Bench Scores

Ante has appeared on both generations of the Terminal-Bench leaderboard,
which evaluates autonomous coding agents on real-world software-engineering
tasks inside a terminal environment.

| Leaderboard        | Rank | Agent Configuration            | Score  |
| ------------------- | ---- | ------------------------------ | ------ |
| Terminal-Bench 2.0  | #17  | Ante + Gemini 3 Pro            | 69.4 % |
| Terminal-Bench 1.0  | #4   | Ante + claude-sonnet-4-5       | 60.3 % |

Antigma has noted that Ante "topped Terminal-Bench twice before," referring
to earlier snapshots of the leaderboard where Ante held the #1 position
before subsequent submissions displaced it. The scores above reflect the
standings at the time of writing, not those earlier peaks.

### Score Context

- **Terminal-Bench 2.0** introduced harder tasks and a broader category
  spread compared to v1.0, which accounts for the apparent score
  regression between ranks.
- Ante's 69.4 % on TB 2.0 was achieved with Google's Gemini 3 Pro as the
  backing model, while the 60.3 % on TB 1.0 used Anthropic's
  claude-sonnet-4-5.
- The shift in backing model across leaderboard versions illustrates
  Antigma's model-agnostic architecture: Ante's agent scaffolding is
  designed to work with whichever frontier model best fits the task
  distribution.

## Terminal-Bench Forensic Analysis

On March 13, 2026, Antigma published a blog post titled:

> **"How to Achieve #1 on Terminal Bench (and Why We Can't Have Nice Things)"**

The post describes how Antigma used Ante itself to forensically reverse-
engineer the #1 submission on the Terminal-Bench 2.0 leaderboard. The
analysis is both a technical exposé of that specific entry and a broader
argument about benchmark integrity in the AI-agent ecosystem.

### The Subject: `@obl-hq/ob1`

The #1 entry on TB 2.0 at the time of publication was a package called
`@obl-hq/ob1`, version `0.1.0-dev.2498638`. Antigma's forensic
investigation revealed that this was not an original agent but a lightly
modified fork of the open-source **Google Gemini CLI**.

Key identifiers of provenance:

- Google badges were still present in the package's README.
- The codebase structure, configuration patterns, and dependency graph
  matched the public Gemini CLI repository.
- The package weighed **20.9 MB**, containing a **31 MB / 616,000-line
  JavaScript bundle** and a **51 MB source map** — far larger than any
  legitimate agent wrapper would require.

### Methodology

Antigma's analysis followed a structured forensic approach, executed
largely by Ante operating autonomously against the published npm package:

1. **Package inspection** — Ante unpacked `@obl-hq/ob1` and catalogued
   every file by type, size, and purpose.
2. **Code-signature matching** — Compared the bundle against the known
   Gemini CLI codebase to establish the fork relationship.
3. **Obfuscation analysis** — Identified XOR-based string "encryption"
   used to hide model names and configuration flags from casual
   inspection.
4. **Trajectory file analysis** — Discovered and decoded a cache of
   pre-recorded solution trajectories.
5. **Behavioral reconstruction** — Traced the control flow to understand
   how the agent decided between live inference and trajectory replay.
6. **Timing analysis** — Identified artificial delays injected to make
   replay look like real-time reasoning.

### Findings

The investigation uncovered several mechanisms that, taken together,
constitute a systematic approach to gaming the benchmark.

#### XOR-Obfuscated Model Names

Real model identifiers and cheat-mode flags were hidden behind a simple
XOR cipher. When decoded, these revealed:

- The actual backing model being used (not the one declared in metadata).
- Boolean flags that toggled between "honest" inference and trajectory
  replay depending on the task.

#### Pre-Recorded Trajectories

The package contained **48 pre-recorded trajectory JSON files** totaling
**5.8 MB** and representing approximately **8.3 hours** of pre-computed
solutions. These trajectories covered a wide range of Terminal-Bench task
categories:

- `chess-best-move` — optimal move selection from board states
- `password-recovery` — digital-forensics-style credential extraction
- `protein-assembly` — computational biology structure prediction
- `feal-differential-cryptanalysis` — breaking the FEAL cipher via
  differential methods
- `corewars` — writing Redcode warriors for the Core War game
- `compression-reverse-engineering` — decompressing unknown formats
- `xss-bypass` — evading HTML sanitizers
- `pytorch-model-recovery` — reconstructing PyTorch model architectures
- `tensor-parallelism` — distributed tensor computation
- `adaptive-rejection-sampling` — statistical sampling methods
- `ray-tracing` — rendering scenes via ray tracing
- `complicert-compilation` — verified compilation tasks

When the agent detected that a task matched one of these trajectories, it
replayed the cached solution instead of performing live reasoning.

#### Hand-Crafted "Bench-Skills" Cheat Sheets

In addition to full trajectories, the package included **8 hand-crafted
skill files** totaling roughly **60 KB**. These were domain-specific
reference documents covering:

- Binary data handling
- Extracting filter JS from HTML (`break-filter-js-from-html`)
- CoreWars / Redcode strategy
- Metacircular evaluator construction
- PyTorch internals
- Ray-tracing algorithms

These skill files gave the agent an unfair informational advantage on
tasks where competitors had to derive the same knowledge from scratch or
from the model's training data.

#### Randomized Sleep Timer

To make trajectory replay appear indistinguishable from genuine reasoning,
the entry injected artificial delays:

```javascript
// Pseudocode reconstructed from the deobfuscated bundle
const delay = 0.8 + Math.random() * 0.7;  // 0.8–1.5 seconds per step
await sleep(delay * 1000);
```

This randomized sleep pattern was designed to mimic the variable latency
of real LLM inference, making it harder for leaderboard operators to
detect replay from timing data alone.

#### Reasoning Effort Downgrade

When the agent determined it could use a pre-recorded trajectory, it
downgraded the reasoning effort parameter from `"xhigh"` to `"medium"`.
This reduced API costs and latency for the replayed tasks, since the
model's output was being discarded in favor of the cached solution anyway.
The downgrade also served as an internal signal within the control flow
to switch code paths.

### Terminal-Bench Task Categories

The forensic analysis provided incidental insight into the breadth of
Terminal-Bench 2.0's task distribution. Categories identified from the
trajectory and skill files include:

| Category                        | Description                                      |
| ------------------------------- | ------------------------------------------------ |
| Cryptanalysis                   | FEAL differential cryptanalysis                  |
| Chess                           | Best-move analysis from board positions          |
| Password Recovery               | Digital forensics credential extraction           |
| Protein Assembly                | Computational biology structure tasks            |
| CoreWars                        | Redcode warrior programming                      |
| Compression Reverse Engineering | Decompressing unknown / proprietary formats      |
| XSS Bypass                      | HTML sanitizer evasion                           |
| PyTorch Model Recovery          | Reconstructing model architectures from artifacts|
| Tensor Parallelism              | Distributed tensor computation                   |
| Adaptive Rejection Sampling     | Statistical sampling algorithm implementation    |
| Ray Tracing                     | Scene rendering via ray tracing                  |
| CompCert Compilation            | Verified / certified compilation tasks           |

### Implications for the Field

Antigma's analysis highlights several systemic risks in current agent
benchmarking practices:

1. **Lookup tables defeat leaderboards.** When a top-ranked submission is
   effectively a replay engine with a sleep timer, the leaderboard ceases
   to measure agent capability and instead measures willingness to
   pre-compute answers.

2. **Obfuscation is trivial.** XOR encoding, bundle minification, and
   source maps make it easy to hide cheating mechanisms in plain sight.
   Most leaderboard operators do not perform forensic audits of
   submissions.

3. **Signal distortion propagates.** A fraudulent #1 entry doesn't just
   affect its own ranking — it shifts the perceived performance ceiling
   for the entire field, influencing funding decisions, user trust, and
   competitive strategy for honest participants.

4. **Public benchmarks are public goods.** Their value depends on the
   integrity of every participant. One bad-faith entry degrades the
   information value for everyone.

### Antigma's Stated Conclusion

From the blog post:

> "Benchmarks are public goods. When a leaderboard entry is a lookup table
> with a sleep timer, it doesn't just inflate one score — it distorts the
> entire signal."

This framing positions benchmark integrity not as a competitive concern
but as an epistemic one: if the community cannot trust leaderboard
rankings, it loses a critical tool for evaluating progress.

## Benchmark Integrity Recommendations

Antigma concluded the blog post with a set of concrete recommendations
aimed at both agent developers and benchmark operators:

### For Agent Developers

1. **Record trajectories for debugging, not replay.** Trajectory logging
   is a legitimate engineering practice for understanding agent behavior.
   Using those logs as a lookup table for benchmark submissions crosses
   the line from instrumentation into gaming.

2. **Write domain skill files openly.** If an agent benefits from curated
   domain knowledge (e.g., cryptanalysis reference material), that
   knowledge should be documented publicly — not hidden inside an
   obfuscated bundle.

3. **Scale reasoning effort dynamically, not conditionally.** Adjusting
   inference parameters based on task difficulty is reasonable. Adjusting
   them based on whether a pre-recorded answer exists is not.

4. **"Don't trust, verify."** A direct call to the community to audit
   high-performing submissions rather than accepting leaderboard rankings
   at face value.

### For Benchmark Operators

While Antigma's recommendations focused on developers, the analysis
implicitly suggests several operational improvements:

- **Submission auditing** — Automated and manual inspection of package
  contents for trajectory caches, obfuscated strings, and anomalous
  bundle sizes.
- **Timing analysis** — Statistical detection of artificial delay
  patterns that differ from genuine inference latency distributions.
- **Provenance checks** — Verifying that submissions are original work
  rather than forks of existing tools with bolted-on replay logic.
- **Hold-out tasks** — Maintaining a set of unpublished tasks that rotate
  regularly, making trajectory pre-computation infeasible.

## Ante's Benchmarking Philosophy

Antigma's approach to benchmarks, as expressed through both their
leaderboard participation and their forensic work, reflects several
principles:

### Model Agnosticism as Honest Signal

By submitting Ante with different backing models across leaderboard
versions (Gemini 3 Pro on TB 2.0, claude-sonnet-4-5 on TB 1.0), Antigma
demonstrates that their scores reflect the agent scaffolding's
contribution rather than any single model's strength. This makes their
results more informative about agent architecture quality.

### Forensic Capability as Credibility

Using Ante itself to perform the forensic analysis of a competing
submission serves a dual purpose: it demonstrates Ante's code-analysis
capabilities in a real-world scenario, and it establishes Antigma's
credibility as a benchmark participant who takes integrity seriously.

### Transparency Over Optimization

Rather than focusing exclusively on climbing the leaderboard, Antigma
invested engineering time in understanding and exposing how the
leaderboard can be gamed. This trade-off — transparency over pure
competitive optimization — is consistent with their positioning as a
research-oriented lab.

## Summary

Ante's benchmark story is defined by two threads: competitive performance
and integrity advocacy. On performance, Ante has achieved notable
rankings on both Terminal-Bench 1.0 (#4) and Terminal-Bench 2.0 (#17),
with a history of earlier #1 positions. On integrity, Antigma's forensic
exposé of the `@obl-hq/ob1` submission represents one of the most
detailed public analyses of benchmark gaming in the AI-agent space,
revealing pre-recorded trajectories, XOR obfuscation, artificial timing,
and reasoning-effort manipulation in a package that was, at its core, a
forked Google Gemini CLI. Their core message — that benchmarks are public
goods deserving of collective stewardship — positions Antigma as a
voice for accountability in an ecosystem where leaderboard rankings
carry increasing weight.