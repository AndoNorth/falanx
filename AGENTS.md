# Repository Knowledge System

This project uses a overarching documentation system under `/docs/*.md`,
anything that lives in a subdirectory are supplementary docs for tools and skills

## Core idea

The code speaks for itself, tests will often provide additional context and use-cases

Docs are the source of truth for:
- product intent
- architecture decisions

Subdirectories may contain:
- feature specifications
- implementation plans
- test strategies

There will be supplementary *.md docs alongside core components to grant high level context of modules

## Key documents

`docs/NORTHSTAR.md` 

- High-level product vision and direction.
- This defines *why the system exists* and long-term goals.

`docs/ARCHITECTURE.md`

- System design, what we're building conceptually
- Constraints, invariants, and structural decisions

`docs/FALANX_AGENTS.md`

- Rules for the autonomous/AI agent behavior, workflows, and collaboration patterns for the engine itself.

## How to use these docs

When working:

- If direction is unclear → check NORTHSTAR.md
- If system constraints matter → check ARCHITECTURE.md
- If behavior or autonomy is involved → check FALANX_AGENTS.md

Always prefer existing docs over assumptions.
