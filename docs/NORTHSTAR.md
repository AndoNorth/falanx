# Northstar

## Vision

Automated code review that behaves like a coordinated, focused engineering team — not just a lint tool, code review tool or code quality tool alone. Each agent does one job well. Quality emerges from specialization, not from a single monolithic pass.

## What This Is

A locally-run agentic pipeline, scoped to a feature within a merge request. When you finish a change, this tool takes that MR diff and runs it through a deterministic sequence of specialized reasoning agents

The general workflow is as follows: characterizing the changes into groups, scoring, reviewing, and optionally rewriting — within the blast radius of that change only, resulting in an overall quality score based on multiple categories.

It manages its own context and produces a full audit trail as it moves through the workflow, such as tool calls, prompts back and forth with the LLM. The ideal workflow for the user is fire and forget, users will review the audit log and results of each iteration.

## Core Promise

Every change is scored. Every rewrite is traceable to a critique. No rewrite happens without prior explanation.

The orchestrator owns sequencing. It can loop, re-score, and re-review. The hard constraint is traceability, not rigid linear order.

The audit trail exists so you can trust the output, not just accept it.

## Definition of Done

An MR can be scored, explained, and safely rewritten in one invocation — with a full audit trail.

Or run in loop mode: the pipeline iterates toward a target score (e.g. 5/5), exiting when the target is hit, when a max iteration ceiling is reached, or when no improvement is detected between passes. Horizon resets are available as an escape hatch when the loop stalls.

## Quality Philosophy

- **Opinionated scoring** — categories scored 1–5 by default, 1–3 (low/medium/high) where finer granularity adds no signal
- **Explain-before-change** — the rewrite agent never fires without review agent output
- **Full traceability** — every change maps to a specific critique; nothing is changed silently
- **Tools, not replacements** — linters and formatters (e.g. `clippy`, `rustfmt`) are invoked as tools, not replicated as logic — users can supplement additional tools and skills to the agents

## Agentic Principle

Separation of cognition from action. Reviewing code and rewriting code are distinct reasoning tasks. Conflating them degrades both. Specialized agents, coordinated by an orchestrator, produce better outcomes than a single agent trying to do everything.

A validation step — confirming rewritten code passes linting, formatting, and tests — is an intended part of this workflow. The exact form (agent, skill, or tool call) is not yet fixed, but the need is.

## Non-Goals

- Not a CI replacement
- Not a generic coding assistant
- Not an autonomous production deployment system
- Not a whole-codebase analyser — scope is always the MR diff
- Does not replicate linter or formatter logic — it calls them

## Forward Look

This is a forward-looking tool. The primary use case is pre-merge quality review, not retroactive codebase improvement. That may change after real usage, but scope creep starts here.

---

*If everything else disappears, this remains true: focused agents, traceable changes, no rewrite without critique.*
