# Falanx Agents

High-level behavioural descriptions of the agent pipeline. Implementation detail lives alongside the engine code. This document describes what each agent is responsible for, what it receives, and what it must produce.

---

## Pipeline Overview

```
CodeReviewOrchestrationAgent
├── CodeQualityAgent          ← scores the diff
├── CodeReviewAgent           ← critiques the diff
└── CodeWritingAgent          ← rewrites based on critique
```

Agents are sequenced by the orchestrator. Each agent is fully isolated — it receives explicit inputs and returns explicit outputs. No agent shares context with another.

The invariant: **no rewrite without prior critique**. `CodeWritingAgent` never fires without `CodeReviewAgent` output.

---

## CodeReviewOrchestrationAgent

**Role:** Entry point and pipeline coordinator.

**Receives:** Target input (file, diff, or branch range), loop configuration, session state.

**Responsibilities:**
- Sequences the three sub-agents in order
- Aggregates outputs into a combined report
- Owns loop control: iterates toward a target score, respects `--max-iter` ceiling, exits on plateau
- Detects reasoning degradation and triggers horizon reset (discards agent, seeds fresh context from original diff + current scores, continues loop)
- Produces and appends to the JSONL audit trail throughout

**Produces:** Combined score + critique + rewrite report. Full JSONL audit trail.

---

## CodeQualityAgent

**Role:** Score the diff across defined quality categories.

**Receives:** The target diff or file content.

**Responsibilities:**
- Scores each category independently on a 1–5 scale
- Produces a structured score report with per-category ratings and a composite score
- Does not critique or suggest changes — scoring only

**Default scoring categories:**

| Category | What it measures |
|---|---|
| Readability | Naming, structure, documentation |
| Maintainability | Coupling, complexity, test coverage |
| Performance | Algorithmic efficiency, resource usage |
| Security | Input validation, unsafe patterns, dependency risk |
| Architecture | Separation of concerns, abstraction quality |

**Produces:** Structured score report.

---

## CodeReviewAgent

**Role:** Critique the diff. Produce actionable feedback.

**Receives:** The target diff or file content, score report from `CodeQualityAgent`.

**Responsibilities:**
- Identifies specific issues: code smells, anti-patterns, logic errors, edge cases, style violations, architectural concerns
- Each issue includes: location, what is wrong, why it matters, how to fix it
- Scoped to the blast radius of the diff only — not whole-codebase analysis

**Produces:** Ordered list of issues with location, problem, and fix recommendation.

---

## CodeWritingAgent

**Role:** Apply changes based on the critique.

**Receives:** Original source, critique output from `CodeReviewAgent`.

**Responsibilities:**
- Applies the minimum changes required to address each critique item
- Every change maps to a specific critique — nothing is changed silently
- Produces a diff-auditable output; changes are reviewable before write-back

**Does not:**
- Run linters, formatters, or tests (post-v1)
- Make changes beyond the scope of the critique
- Invent improvements not grounded in the review

**Produces:** Modified source with traceable diff.

---

## Behavioural Contracts

These hold across all agents:

- Agents do not call LLMs directly — all LLM access goes through the model abstraction layer
- Agents do not share context windows — sub-agents are fully isolated from the orchestrator
- Every agent output is captured in the JSONL audit trail
- Scoring categories are configurable — defaults above are the starting point
