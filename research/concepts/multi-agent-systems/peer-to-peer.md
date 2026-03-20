---
title: Peer-to-Peer Agent Communication
status: complete
---

# Peer-to-Peer Agent Communication

Peer-to-peer (P2P) agent communication is a multi-agent pattern where agents operate as
equals — no central orchestrator, no fixed hierarchy. Agents communicate directly with
each other, negotiate responsibilities, share information through shared spaces, and
resolve conflicts through protocols rather than authority. Despite its theoretical appeal,
**no production coding agent we studied uses true peer-to-peer coordination**. This
document explores why the pattern exists in research, what it offers, and why it hasn't
yet reached production in coding agents.

---

## Why P2P Matters (Even Without Production Adoption)

The absence of P2P in production doesn't diminish its importance as a concept. P2P
patterns solve problems that hierarchical systems struggle with:

1. **No single point of failure** — If the orchestrator fails in an orchestrator-worker
   system, everything stops. P2P systems degrade gracefully.
2. **Dynamic team composition** — Agents can join and leave without reconfiguring a
   central coordinator.
3. **Expertise-driven routing** — The agent best suited to a task handles it, rather
   than the orchestrator guessing who's best.
4. **Debate and verification** — Peer review is fundamentally a P2P pattern — two
   equals critically examining each other's work.

As coding agents become more sophisticated and teams of agents become more common,
P2P elements will likely emerge in hybrid architectures.

---

## Core P2P Architecture

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│ Agent A   │◄───►│ Agent B   │◄───►│ Agent C   │
│           │     │           │     │           │
│ Coder     │     │ Reviewer  │     │ Tester    │
└─────┬─────┘     └─────┬─────┘     └─────┬─────┘
      │                 │                 │
      └────────┬────────┘────────┬────────┘
               │                 │
         ┌─────▼─────────────────▼─────┐
         │    Shared Communication      │
         │    Medium (Blackboard,       │
         │    Message Bus, or P2P)      │
         └─────────────────────────────┘
```

In a P2P system:
- Every agent can initiate communication with any other agent
- No agent has authority over another
- Decisions are made through protocols (voting, negotiation, consensus)
- Agents may have overlapping capabilities

---

## Agent-to-Agent Messaging

### Direct Messaging

The simplest P2P pattern — agents send messages directly to each other.

```python
class PeerAgent:
    def __init__(self, name, capabilities, peers=None):
        self.name = name
        self.capabilities = capabilities
        self.peers = peers or {}
        self.inbox = []

    def send(self, target_name, message):
        """Send a message directly to a peer"""
        self.peers[target_name].receive(Message(
            sender=self.name,
            content=message,
            timestamp=time.time(),
        ))

    def receive(self, message):
        """Process an incoming message from a peer"""
        self.inbox.append(message)

    def process_messages(self):
        """Process all pending messages"""
        for msg in self.inbox:
            response = self.llm.call(
                system=f"You are {self.name}. Process this message from {msg.sender}.",
                user=msg.content,
            )
            if response.requires_reply:
                self.send(msg.sender, response.reply)
        self.inbox.clear()
```

### Structured Message Types

P2P systems benefit from structured message types that convey intent:

```python
from enum import Enum
from dataclasses import dataclass

class MessageType(Enum):
    REQUEST = "request"          # "Can you do X?"
    INFORM = "inform"            # "Here's information about X"
    PROPOSE = "propose"          # "I suggest we do X"
    ACCEPT = "accept"            # "I agree with your proposal"
    REJECT = "reject"            # "I disagree — here's why"
    DELEGATE = "delegate"        # "You should handle X"
    QUERY = "query"              # "What do you know about X?"
    COMMIT = "commit"            # "I've completed X"

@dataclass
class AgentMessage:
    sender: str
    receiver: str
    msg_type: MessageType
    content: str
    context: dict = None          # Optional structured data
    in_reply_to: str = None       # For conversation threading
    priority: int = 0             # For message ordering
```

---

## Negotiation Protocols

In P2P systems, agents must negotiate who does what. Several protocols address this:

### Contract Net Protocol (CNP)

A classic multi-agent negotiation protocol adapted for coding:

```
┌─────────────┐          ┌─────────────┐          ┌─────────────┐
│  Announcer   │          │  Bidder A    │          │  Bidder B    │
│  (any agent) │          │  (peer)      │          │  (peer)      │
└──────┬──────┘          └──────┬──────┘          └──────┬──────┘
       │                        │                        │
       │  ANNOUNCE: "Need to    │                        │
       │  fix auth bug in       │                        │
       │  routes/users.ts"      │                        │
       │───────────────────────►│                        │
       │──────────────────────────────────────────────►  │
       │                        │                        │
       │         BID: "I can    │                        │
       │         fix it. I've   │                        │
       │         worked on auth │                        │
       │         before."       │                        │
       │◄───────────────────────│                        │
       │                        │                        │
       │                  BID: "I can fix it.            │
       │                  Lower confidence but           │
       │                  I know the test suite."        │
       │◄──────────────────────────────────────────────  │
       │                        │                        │
       │  AWARD: "Agent A       │                        │
       │  wins — best match"    │                        │
       │───────────────────────►│                        │
       │                        │                        │
       │         COMMIT:        │                        │
       │         "Fix applied   │                        │
       │         and tested"    │                        │
       │◄───────────────────────│                        │
```

```python
class ContractNetProtocol:
    def announce_task(self, task, peers):
        """Announce a task and collect bids from peers"""
        bids = []
        for peer in peers:
            bid = peer.evaluate_bid(task)
            if bid.willing:
                bids.append(bid)

        # Select best bidder based on expertise match
        winner = max(bids, key=lambda b: b.confidence)
        winner.agent.assign(task)
        return winner

    def evaluate_bid(self, task):
        """Evaluate whether this agent should bid on a task"""
        relevance = self.llm.call(
            system=f"You are {self.name} with expertise in {self.capabilities}. "
                   f"Rate your confidence (0-1) in completing this task.",
            user=f"Task: {task.description}",
        )
        return Bid(
            agent=self,
            confidence=relevance.confidence,
            willing=relevance.confidence > 0.5,
            rationale=relevance.explanation,
        )
```

### Debate Protocol

Two or more agents argue for different approaches, with a judge (which could be
another peer) deciding:

```python
def debate_protocol(question, proponent, opponent, judge, max_rounds=3):
    """Two agents debate an approach; a third judges"""
    debate_history = []

    for round in range(max_rounds):
        # Proponent argues FOR the approach
        pro_argument = proponent.llm.call(
            system="Argue FOR this approach. Consider previous counterarguments.",
            user=f"Question: {question}\nHistory: {debate_history}",
        )
        debate_history.append({"role": "proponent", "argument": pro_argument})

        # Opponent argues AGAINST
        con_argument = opponent.llm.call(
            system="Argue AGAINST this approach. Find weaknesses and alternatives.",
            user=f"Question: {question}\nHistory: {debate_history}",
        )
        debate_history.append({"role": "opponent", "argument": con_argument})

    # Judge evaluates the debate
    verdict = judge.llm.call(
        system="You are a neutral judge. Evaluate both sides and decide "
               "which approach is better. Explain your reasoning.",
        user=f"Debate transcript: {debate_history}",
    )
    return verdict
```

**Application to code review:** Two agents — one defending the implementation, one
attacking it — produce a more thorough review than a single reviewer. The defender
forces the attacker to be specific; the attacker forces the defender to justify
every decision.

---

## The Shared Blackboard Pattern

A **blackboard** is a shared workspace where agents post information, read each
other's contributions, and build toward a solution collaboratively:

```
┌─────────────────────────────────────────────────────┐
│                    BLACKBOARD                       │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │ TASK: Refactor auth module to use JWT       │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │ [Agent A - Researcher]                      │   │
│  │ Current auth uses passport.js with local    │   │
│  │ and OAuth strategies. 15 routes depend on   │   │
│  │ req.user populated by passport middleware.  │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │ [Agent B - Architect]                       │   │
│  │ Proposed plan:                              │   │
│  │ 1. Create JWT utility module                │   │
│  │ 2. Replace passport middleware              │   │
│  │ 3. Update all 15 route handlers             │   │
│  │ Risk: OAuth refresh flow needs rethinking   │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │ [Agent C - Security Expert]                 │   │
│  │ ⚠ Agent B's plan doesn't address token     │   │
│  │ rotation or refresh token storage.          │   │
│  │ Recommend: Add Step 2.5 for refresh flow    │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │ [Agent B - Architect] REVISED               │   │
│  │ Updated plan incorporating C's feedback...  │   │
│  └─────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

```python
class Blackboard:
    def __init__(self):
        self.entries = []
        self.subscribers = []

    def post(self, agent_name, content, entry_type="info"):
        entry = {
            "id": len(self.entries),
            "agent": agent_name,
            "content": content,
            "type": entry_type,  # info, proposal, critique, revision, decision
            "timestamp": time.time(),
            "replies_to": None,
        }
        self.entries.append(entry)
        self.notify_subscribers(entry)
        return entry["id"]

    def reply(self, agent_name, reply_to_id, content, entry_type="reply"):
        entry = {
            "id": len(self.entries),
            "agent": agent_name,
            "content": content,
            "type": entry_type,
            "timestamp": time.time(),
            "replies_to": reply_to_id,
        }
        self.entries.append(entry)
        self.notify_subscribers(entry)

    def read_all(self):
        return self.entries

    def read_since(self, entry_id):
        return [e for e in self.entries if e["id"] > entry_id]

    def notify_subscribers(self, entry):
        for sub in self.subscribers:
            sub.on_new_entry(entry)
```

**Blackboard vs Message Passing:**

| Aspect | Blackboard | Direct Messages |
|--------|-----------|-----------------|
| Visibility | All agents see everything | Only sender/receiver |
| Coupling | Loose (read when ready) | Tight (must handle messages) |
| History | Naturally persistent | Must be stored separately |
| Coordination | Emergent from reading | Explicit from sending |
| Scaling | Reads scale well | Messages scale linearly |

---

## Publish-Subscribe Between Agents

Pub/sub is a middle ground between direct messaging and blackboard — agents subscribe
to topics they care about and publish events others may need:

```python
class AgentPubSub:
    def __init__(self):
        self.topics = defaultdict(list)  # topic → [subscriber callbacks]

    def subscribe(self, topic, agent, callback):
        self.topics[topic].append((agent, callback))

    def publish(self, topic, sender, data):
        for agent, callback in self.topics[topic]:
            if agent.name != sender:  # Don't notify self
                callback(data)

# Usage for coding agents
pubsub = AgentPubSub()

# Researcher publishes findings
pubsub.subscribe("research_findings", planner_agent, planner_agent.on_research)
pubsub.subscribe("research_findings", reviewer_agent, reviewer_agent.on_research)

# Implementer publishes changes
pubsub.subscribe("code_changes", tester_agent, tester_agent.on_code_change)
pubsub.subscribe("code_changes", reviewer_agent, reviewer_agent.on_code_change)

# Test runner publishes results
pubsub.subscribe("test_results", implementer_agent, implementer_agent.on_test_result)
```

### OpenHands' EventStream as Pub/Sub

OpenHands implements the closest thing to P2P pub/sub in production coding agents.
Its EventStream allows multiple independent subscribers to react to events:

```python
class EventStream:
    def add_event(self, event: Event, source: EventSource):
        event._id = self._cur_id
        self._cur_id += 1
        self.file_store.write(event)
        for subscriber in self._subscribers:
            subscriber.executor.submit(subscriber.callback, event)
```

Subscriber types: `AGENT_CONTROLLER`, `RESOLVER`, `SERVER`, `RUNTIME`, `MEMORY`,
`MAIN`, `TEST`. Each has its own `ThreadPoolExecutor`, enabling concurrent event
processing — a decentralized pattern.

---

## Conflict Resolution

When peers disagree (different approaches, conflicting edits, contradictory assessments),
the system needs conflict resolution mechanisms:

### Priority-Based Resolution

```python
def resolve_by_priority(conflicts, agent_priorities):
    """Higher-priority agent's decision wins"""
    for conflict in conflicts:
        agents = conflict.involved_agents
        winner = max(agents, key=lambda a: agent_priorities.get(a.name, 0))
        conflict.resolve(winner.position)
```

### Voting-Based Resolution

```python
def resolve_by_vote(conflict, all_agents, threshold=0.6):
    """Agents vote on the resolution"""
    votes = {}
    for agent in all_agents:
        vote = agent.llm.call(
            system=f"You are {agent.name}. Vote on this conflict.",
            user=f"Conflict: {conflict.description}\n"
                 f"Option A: {conflict.option_a}\n"
                 f"Option B: {conflict.option_b}",
        )
        votes[agent.name] = vote.choice

    a_votes = sum(1 for v in votes.values() if v == "A")
    b_votes = sum(1 for v in votes.values() if v == "B")

    if a_votes / len(votes) >= threshold:
        return conflict.option_a
    elif b_votes / len(votes) >= threshold:
        return conflict.option_b
    else:
        return "no_consensus"
```

### Evidence-Based Resolution

```python
def resolve_by_evidence(conflict, agents):
    """Agents provide evidence; strongest evidence wins"""
    evidence = []
    for agent in conflict.involved_agents:
        proof = agent.gather_evidence(conflict)
        evidence.append({
            "agent": agent.name,
            "position": agent.position,
            "evidence": proof,
        })

    # Evaluator (could be any peer) weighs the evidence
    evaluator = random.choice(agents)
    verdict = evaluator.llm.call(
        system="Evaluate the evidence for each position. "
               "Which position has stronger supporting evidence?",
        user=str(evidence),
    )
    return verdict
```

### Merge-Based Resolution (Code-Specific)

For conflicting code changes, merge resolution is the most natural approach:

```python
def resolve_code_conflict(change_a, change_b, merger_agent):
    """Merge conflicting code changes"""
    merged = merger_agent.llm.call(
        system="You are a code merge expert. Merge these two "
               "conflicting changes, preserving the intent of both.",
        user=f"Change A:\n{change_a.diff}\n\n"
             f"Change B:\n{change_b.diff}\n\n"
             f"Intent A: {change_a.description}\n"
             f"Intent B: {change_b.description}",
    )
    return merged
```

---

## Why P2P Hasn't Reached Production in Coding Agents

Our research identified several reasons:

### 1. Coordination Overhead

P2P negotiation requires multiple LLM round-trips before work even begins. For a
task that an orchestrator-worker system handles in 3 calls (orchestrator + 2 workers),
a P2P system might need 8+ calls (announcement, bids, selection, execution, review,
debate, revision, consensus).

### 2. Unpredictable Behavior

P2P systems are harder to debug and reason about. When something goes wrong in an
orchestrator-worker system, you inspect the orchestrator's decisions. In P2P, you
must trace messages across multiple agents to understand what happened.

### 3. Quality Enforcement

In hierarchical systems, the orchestrator can enforce quality gates — "don't
proceed until tests pass." In P2P, enforcement must be built into the protocol,
which adds complexity.

### 4. The Human Interface Problem

Users interact with a single agent. P2P systems must expose a unified interface
while internally coordinating among peers — adding a layer of complexity that
hierarchical systems avoid.

---

## Hybrid Approaches: P2P Elements in Hierarchical Systems

The most practical approach combines hierarchical orchestration with P2P elements:

```
┌───────────────────────────────────┐
│          ORCHESTRATOR             │
│  (delegates tasks, aggregates)    │
├───────────┬───────────┬───────────┤
│ Worker A  │ Worker B  │ Worker C  │
│           │◄─────────►│           │
│           │  P2P peer │           │
│           │  review   │           │
└───────────┴───────────┴───────────┘
```

In this hybrid:
- The **orchestrator** handles task decomposition and result aggregation (hierarchical)
- **Workers** peer-review each other's work (P2P)
- Conflict resolution escalates to the orchestrator (hierarchical fallback)

This is essentially what ForgeCode does — Forge (orchestrator) delegates to Sage
(researcher) and Muse (planner), and Sage's findings inform Muse's planning
(a P2P information flow within a hierarchical structure).

---

## Cross-References

- [orchestrator-worker.md](./orchestrator-worker.md) — The dominant alternative to P2P
- [swarm-patterns.md](./swarm-patterns.md) — Lightweight handoffs as a middle ground
- [communication-protocols.md](./communication-protocols.md) — Wire protocols for agent messaging
- [evaluation-agent.md](./evaluation-agent.md) — Peer review as evaluator pattern
- [context-sharing.md](./context-sharing.md) — Shared blackboard and state patterns

---

## References

- Anthropic. "Building Effective Agents." 2024. https://www.anthropic.com/research/building-effective-agents
- OpenAI. "Swarm (experimental)." 2024. https://github.com/openai/swarm
- Research files: `/research/agents/openhands/`, `/research/agents/forgecode/`
