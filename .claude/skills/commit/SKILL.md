---
name: commit
description: Generates a conventional commit message from your current changes. Uses conversation context plus git diffs to produce intent-focused summaries. Always presents the draft for review and loops until you approve before committing.
---

# Commit Skill

Generate a conventional commit message based on the current changes. The
skill drafts the message, presents it for review, loops on feedback, and
only commits once you approve.

The goal is **intent-focused** messages — what the change accomplishes and why,
not a description of which files were touched. Conversation context (what you
asked Claude to do this session) is the strongest signal; git diffs are
supplementary.

Messages follow [Conventional Commits](https://www.conventionalcommits.org/)
and are structured for machine-readable changelog generation (e.g. `git-cliff`).

## Inputs

No required arguments. The skill reads:
- Current conversation context (the strongest signal for intent)
- `git status`, `git diff`, `git diff --cached`
- Current branch name

If invoked with extra text (e.g. `/commit fix scope to scoring`), treat it
as a hint or instruction for the message draft.

## Setup

Before starting, call `ToolSearch` with `query: "select:AskUserQuestion"` to load the tool schema. `AskUserQuestion` is a deferred tool and will not be callable until its schema is fetched.

## Performance expectations

Speed matters — this skill should feel comparable to a manual `git commit` flow,
not slower. To keep it snappy:

- **Run all gathering commands in parallel** (single message, multiple tool
  calls). Never run them sequentially.
- **Use `AskUserQuestion` for yes/no and approval prompts** — interactive
  options are faster to answer than free-text responses.
- **Be concise in presentation** — show the draft, ask for approval, no preamble.
- **Don't re-read the diff between iterations** — keep gathered context in memory.

## Procedure

### Step 1 — Gather changes (in parallel)

```bash
git status
git diff
git diff --cached
git branch --show-current
```

| State | Action |
|-------|--------|
| No staged or unstaged changes | Inform the user, stop |
| On `master` branch | Warn but continue if user wants to proceed |
| On feature branch with changes | Continue normally |

### Step 2 — Analyse changes

Combine diffs with conversation context to understand:

- **What** changed — the technical substance
- **Why** it changed — the intent (from conversation, or commit log)
- **Domain area** — which module or agent the change is *about* (for scope)
- **Change type** — feat, fix, refactor, chore, docs, test, perf, ci, build
- **Breaking change** — any change to agent contracts, scoring schemas, or audit
  trail format must be flagged with `BREAKING CHANGE` in the footer

### Step 3 — Draft the commit message

Format:

```
<type>(<scope>): <subject>

<optional body>

<optional footer — BREAKING CHANGE: <description>>
```

**Types:** `feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `perf`, `ci`, `build`.

**Scope — AI-inferred from domain area:**

| Changes | Scope | Why |
|---------|-------|-----|
| `rust/crates/falanx-engine/` core pipeline | `engine` | Core orchestration |
| Scoring logic or rubric | `scoring` | Scoring subsystem |
| A specific agent (characterizer, reviewer, rewriter) | `agents` | Agent layer |
| CLI entry point or argument handling | `cli` | User-facing interface |
| `flake.nix`, `rust-toolchain.toml`, `Cargo.toml` | `nix` or `build` | Dev environment / build |
| `docs/` only | `docs` | Documentation |
| `.claude/` config, skills, hooks | `dx` | Developer experience |
| Audit trail format or output schema | `audit` | Traceability layer |
| Cross-cutting or ambiguous | *ask user* | Can't infer safely |

Conversation context is the strongest signal — if the user said "working on
the reviewer agent", changes to shared types are still scoped to `agents`.

**Subject line rules:**
- Imperative mood ("add", "fix", "refactor" — not "added"/"adds")
- ≤ 70 characters
- No trailing period
- Lowercase first word (after the type/scope prefix)

**Body:** Only if subject alone isn't enough. Wrap at ~72 chars. Explain the
*why* if it isn't obvious from the subject.

**Breaking changes:** Use footer `BREAKING CHANGE: <description>` for any
change that alters agent interfaces, scoring output format, or audit trail
schema. These are picked up by changelog generators as major version bumps.

### Step 4 — Present draft for review

Show the full drafted commit message (no preamble).

Then use `AskUserQuestion`:

- **Question:** "Approve and commit?"
- **Header:** "Approve commit"
- **Options:**
  - **Approve** — "Commit with this message as shown"
  - **Edit message** — "I want to change the commit message"
  - **Cancel** — "Stop without committing"

- **Approve** → Step 5
- **Edit message** → ask what to change, regenerate, loop back to Step 4
- **Cancel** → stop, leave files unstaged

**Never commit without approval.**

### Step 5 — Stage and commit

1. Stage relevant changed files (never `.env*`, secrets, build artefacts
   unless already tracked)
2. Commit via HEREDOC:

   ```bash
   git commit -m "$(cat <<'EOF'
   <type>(<scope>): <subject>

   <body if any>

   <BREAKING CHANGE footer if any>
   EOF
   )"
   ```

Do **not** use `--no-verify`. Surface pre-commit hook failures to the user.

### Step 6 — Report back

- Short SHA + commit subject
- Reminder: committed but not pushed

If commit failed, report clearly and leave staged state intact.

## Notes

- Never amends — each invocation is a new commit
- Never force-pushes, never bypasses hooks
- `BREAKING CHANGE` footer is required for any change to agent contracts,
  scoring schemas, or audit trail format — changelog generators depend on it
