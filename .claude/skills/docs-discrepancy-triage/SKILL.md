---
name: docs-discrepancy-triage
description: Capture an impactful documentation gap to be resolved later in a batch. Writes a date-prefixed file to docs/discrepancy/. Use only when the gap affects system correctness, architecture, or user-facing behavior — not for minor agent confusion or implementation ambiguity. Triggers: missing doc, outdated doc, conflicting docs, uncaptured architectural decision.
---

# Docs Discrepancy Triage

## Purpose

Quickly mark a point of context confusion or documentation gap so it can be addressed later in a batch. Speed matters more than perfection here — the goal is to capture it now, resolve it later. This should only apply to `*.md` files — the code speaks for itself.

## Triage gate — invoke this skill only if the discrepancy affects:

- **System correctness** — wrong or missing docs could cause incorrect behavior
- **Architecture** — a structural decision is undocumented or contradicted
- **User-facing behavior** — product intent or API contract is unclear or missing

If the confusion is local to the coding agent (e.g. unclear naming, minor ambiguity in implementation detail) — skip the skill and continue. Low-friction means high signal.

## Workflow

### 1. Identify the discrepancy type

Pick one:

- `missing-document` — a doc that should exist does not
- `outdated-information` — doc content contradicts current code, decisions, or reality
- `ambiguous-definition` — a term, scope, or concept is undefined or overloaded
- `conflicting-documentation` — two docs disagree on the same thing
- `uncaptured-architectural-decision` — a decision was made in code or conversation with no corresponding ADR or doc

### 2. Identify the location

- Which file(s) are affected (use full paths relative to repo root)
- Which section, heading, or concept is impacted
- If the doc doesn't exist yet, where it *should* live

### 3. Describe the issue

- What is wrong or missing — be specific
- Why it matters — what decision, behavior, or understanding is at risk without resolution

### 4. Propose a resolution

One of:
- Create new file (give exact suggested path)
- Update existing file (give exact section heading and what to add/change)
- Split or merge docs (explain what moves where and why)

### 5. Classify the resolution type

Pick one:
- `adr-required` — architectural decision with trade-offs that needs formal record
- `spec-update` — a feature or behavior spec needs updating
- `architecture-update` — ARCHITECTURE.md needs amending
- `new-document` — a net-new doc is needed
- `minor-clarification` — small wording/scope fix in existing doc

### 6. Assign priority

- `high` — blocks understanding of core system behavior or causes active confusion
- `medium` — creates risk of wrong assumptions in non-critical areas
- `low` — cosmetic, stylistic, or edge-case clarification

## Output

Write a file to `docs/discrepancy/` using this filename pattern:

```
YYYY-MM-DD-<short-kebab-title>.md
```

Example: `2026-05-21-missing-agent-retry-policy.md`

Create `docs/discrepancy/` if it doesn't exist. Use today's date.

File content:

```markdown
# [Short descriptive title]

**Type:** [discrepancy type from step 1]
**Location:** [file path(s) and section/concept affected]
**Issue:** [what is wrong or missing and why it matters]
**Suggested Fix:** [specific action — file to create, section to update, doc to split]
**Resolution Type:** [resolution class from step 5]
**Priority:** [low | medium | high]
```

After writing, tell the user the file path created.

## Rules

- One file per distinct discrepancy — don't bundle unrelated issues
- If multiple discrepancies, write multiple files
- Be specific about file paths and section headings — vague reports create rework
- Check `/docs/NORTHSTAR.md`, `/docs/ARCHITECTURE.md`, and `/docs/AGENTS.md` as reference when assessing severity or suggesting fix location
