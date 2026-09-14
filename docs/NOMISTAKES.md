# no-mistakes — Architecture Reference & Falanx Mapping

This document covers two things:

1. **no-mistakes architecture** — what it is, its gate model, pipeline stages, agent integration, and configuration surface.
2. **Falanx mapping** — how no-mistakes' concepts correspond to falanx/cersei primitives, and where the two genuinely diverge rather than just overlap.

Reference repo: [`kunchenguid/no-mistakes`](https://github.com/kunchenguid/no-mistakes)

Sources for Part 1: the locally installed Claude Code skill at `/home/ando/.agents/skills/no-mistakes/SKILL.md`, the project README (`raw.githubusercontent.com/kunchenguid/no-mistakes/main/README.md`), and three pages of its docs site (`kunchenguid.github.io/no-mistakes/`) — Introduction, The Gate Model, and the repo-config reference. The docs site's navigation is minimal (three pages total under "start-here" and "concepts"); no dedicated "pipeline stages" or "agent definitions" page exists there beyond what's summarized below, so some detail (e.g. exact per-stage prompt content, TUI internals) is not publicly documented and is not guessed at here.

---

## Part 1 — no-mistakes Architecture

### What It Is

no-mistakes is a local **gate**, not an orchestration framework you write code against. It sits between your working repo and your configured push target (a forge remote), intercepting pushes through a local bare "gate" repository and running a fixed pipeline before the change is allowed through. You don't compose steps in a script the way you do with Sandcastle's `run()` — the pipeline shape is fixed; what you configure is thresholds, commands, and escalation policy around it.

### The Gate Model — Push Flow

```
git push no-mistakes <branch>
        │
        ▼
gate repo's pre-receive hook  ──► daemon authorization check
        │
        ▼
push admitted, written to bare gate repo
        │
        ▼
post-receive hook notifies daemon
        │
        ▼
daemon creates a disposable detached worktree
 (~/.no-mistakes/worktrees/ by default)
        │
        ▼
nine-step pipeline: intent → rebase → review → test → document → lint → push → pr → ci
```

The `no-mistakes` remote is deliberately separate from `origin` — the project's stated philosophy is that it is "an opt-in gate, not a trap door that silently rewires normal Git behavior." Each run's worktree is disposable and isolated, so the daemon can modify files, run tests, and commit fixes without touching the developer's actual working directory.

### Pipeline Stages

| Stage | Role |
|---|---|
| `intent` | Captures what the change is meant to accomplish, supplied by the caller (e.g. the invoking coding agent's `--intent` text) |
| `rebase` | Brings the branch up to date against the base before validation |
| `review` | AI-driven critique of the diff against `review.path_instructions` and project conventions |
| `test` | Runs configured test commands; can auto-fix within limits |
| `document` | Checks/updates documentation per `document.instructions` |
| `lint` | Runs configured lint/format commands |
| `push` | Pushes the validated branch toward the real push target |
| `pr` | Opens/updates a pull request (title, template, base branch from config) |
| `ci` | Watches the forge's CI checks, rebases and re-validates on conflict, keeps monitoring until merged/closed/timeout |

Each stage is a **checkpoint**: it either passes independently or halts with findings that require a decision. This is the "gated push" positioning the docs use explicitly — deliberately placed *between* pre-commit hooks (block the working tree, before any push) and remote CI (runs only after the push is already public).

### Gating and Auto-Fix Policy

Findings surfaced at a gate carry an `action` classification:

| Action | Meaning |
|---|---|
| `auto-fix` | Mechanical, low-risk — the driving agent (or a human) can authorize the fix without user consultation |
| `no-op` | Informational only |
| `ask-user` | Touches product behavior or challenges stated intent — must be escalated to the human, never auto-resolved (except under an explicit standing `--yes` consent) |

Review auto-fix is disabled by default (`auto_fix.review: 0`) — review findings always park for a decision unless a repo or global override raises the limit. Test, lint, and other steps may auto-fix within their own configured attempt budgets and re-run before ever reaching a gate. This is the core design idea stated in the docs: *"safe, mechanical fixes are applied automatically; anything that touches your intent is escalated."*

A `protected_paths` mechanism additionally forces specific files/globs to require an explicit human response regardless of `--yes` — a `protected-path-refusal` gate cannot be auto-approved.

### Agent Integration

no-mistakes does not define its own per-stage LLM agents with prompts and schemas the way falanx does. Instead it drives **one external coding-agent CLI**, chosen through an ordered-fallback `agent:` config field, to actually perform each step's reasoning (review a diff, decide on a fix, write docs, etc.). Supported agent backends: Claude, Codex, Grok, RovoDev, OpenCode, Pi, Copilot, Antigravity, or an explicit `acp:<target>`/Cursor ACP alias. There is no agent-directory concept (`system.md`/`prompt.md`/`config.yaml`) and no per-stage prompt template file — the pipeline steps are fixed, built into the daemon, and delegate their actual judgment calls to whichever single coding-agent CLI is configured.

Invocation happens three ways: a direct `git push no-mistakes`, an interactive TUI, or (the path documented in the local skill) the `/no-mistakes` Claude Code skill, which drives the pipeline via a `no-mistakes axi` CLI that emits machine-readable [TOON](https://toonformat.dev) output — `gate:` objects with a `findings` table to react to, looping until an `outcome:`.

### Configuration — `.no-mistakes.yaml`

A single YAML file at the repo root. Notable sections (from the repo-config reference):

```yaml
agent: claude               # which coding-agent CLI drives the pipeline
commands:                   # shell commands for prepare/test/lint/format
  test: ...
  lint: ...
allow_repo_commands: false  # opt-in to trust pushed-branch command overrides

gates: []                   # custom checks after core pipeline steps
protected_paths: []         # paths requiring operator approval before auto-commit
no_ci: false                # declare a repo with no CI pipeline

pr:
  base_branch: ...
  template: ...
  title_format: "{{.Branch}}: {{.Title}}"
  publish_intent: true

document:
  instructions: ...
review:
  path_instructions: []     # glob-scoped review rules

auto_fix:                   # per-step attempt limits
  rebase: 1
  review: 0                 # disabled by default
  test: 2
  document: 1
  lint: 2
  ci: 2

ci:
  rerun_transient: 2
  revalidate_repairs: true

commit:
  fix_message: "..."
  branch_pattern: "..."

intent:
  enabled: true
  threshold: ...
  slack_days: ...

test:
  instructions: ...
  allow_approve_over_failure: ...
  evidence: { branch: ..., ... }

providers: {}                # per-forge draft-PR settings
```

**Security model:** a fixed set of "gate-control" fields (`commands`, `agent`, `document.instructions`, `review.path_instructions`, `gates`, `protected_paths`, `disable_project_settings`, `no_ci`, `ci.rerun_transient`, `ci.revalidate_repairs`, `test.instructions`, `test.allow_approve_over_failure`, `test.evidence.branch`, `pr.template`, `pr.base_branch`, `pr.publish_intent`) are read **only from the default branch**, never from the pushed branch — this stops a contributor from injecting arbitrary shell execution or swapping the agent via a feature branch. "Non-executing" fields (`ignore_patterns`, `auto_fix`, `commit`, `intent`, `test` metadata, `pr.title_format`, `providers`) are read from the pushed branch instead, since they're conventions rather than a security boundary. `allow_repo_commands: true` (itself default-branch-only) re-enables pushed-branch command/agent reading for single-developer setups.

### Scope — Single-Repo Assumption

Nothing in the docs discusses monorepo layout, multi-package scoping, or repository size. The gate model is built around **one repo, one push target, one PR base**: config is read from "the default branch," there is one `pr.base_branch`, and fork-based development (`--fork-url`) still treats the parent repo's `origin` as the single PR-base authority. `review.path_instructions` gives glob-scoped review rules within that one repo, but there's no first-class concept of "which package/workspace does this diff belong to" the way a monorepo tool typically needs.

> **Not independently verifiable beyond this:** the docs site has no page dedicated to monorepo support one way or the other — this is read from silence plus the single-`pr.base_branch`/single-default-branch design, not a documented limitation.

---

## Part 2 — Falanx Mapping

### Concept Mapping

| no-mistakes concept | Falanx / Cersei equivalent |
|---|---|
| Gate repo + pre/post-receive hooks intercepting `git push` | Not present — falanx is invoked directly (`falanx run --diff ...`), not wired into the push path |
| Disposable worktree per run (`~/.no-mistakes/worktrees/`) | Not present — falanx runs against the working tree/diff in place; no worktree isolation |
| Nine-step fixed pipeline (`intent → rebase → review → test → document → lint → push → pr → ci`) | `workflow.yaml` `stages: [...]` — but falanx's default stages are `score → review → rewrite` only; no `test`, `document`, `lint`, `push`, `pr`, or `ci` stages exist today |
| Gate `findings` table with `auto-fix` / `no-op` / `ask-user` classification | Not present — falanx's `review` agent emits issues (`location`/`problem`/`fix`); nothing routes them by risk class or pauses for human approval |
| `axi respond --action fix/approve/skip` (human-in-the-loop checkpoint) | Not present — `skip_if` predicates (`score_meets_target`, `no_review_issues`) are the only conditional gating, and they're Rust-enum-closed, not interactive |
| `protected_paths` (forced human approval on sensitive files) | Not present |
| Single external coding-agent CLI drives every step's judgment (`agent: claude/codex/...`) | `AgentDef` per pipeline stage — each falanx agent is its own directory (`system.md`/`prompt.md`/`config.yaml`/`output.schema.json`), dispatched through Cersei, not a single external CLI doing everything |
| `.no-mistakes.yaml` single config file | `workflow.yaml` (loop + stages) + per-agent `config.yaml` files + `.env` — config is split across files rather than one root YAML |
| `pr` stage (open/update PR) | Not present — falanx has no forge integration at all |
| `ci` stage (watch checks, rebase on conflict, re-validate) | Not present — falanx's own CI (`.github/workflows/ci.yml`) is a separate, minimal thing: three jobs (fmt+clippy, `cargo test`, `cargo audit`) mirroring falanx's *own* local git hooks. It validates falanx's source, not a downstream repo's diffs, and has no PR-gating behavior of its own |
| `intent` extraction/threshold | Falanx has no equivalent concept — no notion of "the goal behind this change" separate from the diff itself |
| TOON-formatted `gate:`/`outcome:` machine-readable protocol over `axi` | Falanx's JSONL `SessionEvent` audit trail (`RunStarted`, `StageStarted`, `AgentCompleted`, `HorizonReset`, `RunCompleted`/`RunFailed`, etc.) — both are structured, machine-consumable records, but no-mistakes' is a live control-loop protocol (drives a stateful conversation with the caller), falanx's is a passive append-only log read after the fact |
| Output-format enforcement (implicit — each backend coding-agent CLI decides its own output shape) | Falanx's schema-validated **output contract**: an agent can declare `output.schema.json`, and `run_agent` wraps/validates/retries (bounded, `MAX_OUTPUT_RETRIES = 2`) before the result reaches `TemplateContext` — see `docs/superpowers/specs/2026-09-14-agent-output-contract-design.md`. no-mistakes has no analogous per-step schema contract; it relies entirely on whichever coding-agent CLI is configured to interpret its own instructions correctly |
| Horizon reset / plateau detection | Not present in no-mistakes — it has retry/auto-fix *attempt limits* per step (`auto_fix.*`), but no notion of discarding a poisoned context and restarting with a clean one; it also doesn't run a scored quality loop at all |

### Where They Genuinely Diverge

**Push-path integration vs. standalone tool.** no-mistakes' entire architecture is built around intercepting `git push` through a gate repo with hooks. Falanx has nothing like this — it's invoked directly against a diff and produces a report. Adopting no-mistakes' gate-repo mechanism into falanx would be a large, separate piece of infrastructure (bare repo, hooks, worktree lifecycle, daemon), not a pattern that folds into the existing `WorkflowRunner`.

**Single external agent vs. many declared agents.** This is the most structural difference, and it cuts against a "just borrow their approach" read: no-mistakes hands *every* step's judgment to one configured coding-agent CLI (Claude Code, Codex, etc.) and trusts that CLI's own tool-use loop to decide what "review" or "document" means. Falanx instead declares each step as its own `AgentDef` — a directory with its own system prompt, its own minijinja template, and (as of the output-contract work) its own JSON Schema. Falanx's approach gives per-stage control and machine-checkable output at the cost of writing and maintaining seven-plus prompt/schema pairs; no-mistakes' approach is far less code to configure, but the *quality* of every step is only as good as whatever single external CLI was picked, with no schema enforcement of what it returns.

**Mono-repo / mixed-context fit — the user's own concern.** The user has used no-mistakes before and abandoned it in a mono-repo with mixed context, describing the fit as "not really clean and/or structured." That matches what the docs actually show: `.no-mistakes.yaml` is one file at the repo root, config is read from "the default branch" as a single source of truth, and there is exactly one `pr.base_branch`. `review.path_instructions` gives glob-scoped rules, which is the one knob that partially addresses mixed-context repos, but there is no first-class notion of "which package/service does this diff belong to," no per-package agent selection, and no per-package PR base. Falanx's diff-scoped, git-native input (`--diff`, `--file`) doesn't solve monorepo package-awareness either — its agents are also scoped to "the diff only," not a package — but falanx at least doesn't assume a single push target or single PR base, since it has no push/PR concept at all yet. Neither tool has actually solved monorepo package-awareness; no-mistakes just has more surface area (a single default-branch config, a single PR base) where that assumption shows.

**Capabilities no-mistakes has that falanx currently lacks.** Per the constraints of this comparison (falanx's own CI just landed, and is minimal: fmt+clippy, `cargo test`, `cargo audit`, mirroring falanx's local git hooks — it has no PR-gating agent behavior of its own):
- A `pr` stage: opening/updating a pull request with templated title and body.
- A `ci` stage: watching forge checks, auto-rebasing on conflict, re-validating, and only then reporting done.
- A `push` stage: actually landing the validated branch on a configured remote.
- A `document` stage as a first-class pipeline step (falanx has no docs-generation/verification stage at all).
- Human-in-the-loop gating: the `ask-user`/`auto-fix`/`no-op` finding classification and the pause-for-approval protocol. Falanx's `skip_if` is the closest analog and it is a closed, non-interactive Rust enum — flagged as an explicit gap in `docs/superpowers/specs/2026-09-14-agent-output-contract-design.md`'s "Out of Scope" section ("Custom `skip_if` gate predicates" and "Human-in-the-loop pipeline checkpoints (no pause/approve stage type exists)").
- `protected_paths` — forced escalation on sensitive files regardless of automation consent.
- Multi-tool agent fallback — the ability to pick from eight-plus coding-agent CLI backends per repo, rather than one hardcoded provider abstraction.

> **These are real, acknowledged gaps, not aspirational parity claims** — the falanx design spec for the output contract explicitly names `skip_if` predicates and human-in-the-loop checkpoints as out-of-scope items identified during design review, not yet designed.

**Capabilities falanx has that no-mistakes does not.** Falanx's schema-validated output contract (`output.schema.json` per agent, wrap → dispatch → validate → bounded retry, `SessionEvent::AgentOutputInvalid` on failure) has no equivalent in no-mistakes, which delegates output interpretation entirely to whichever external coding-agent CLI is configured. Falanx's JSONL audit trail is a full structured record of every stage/agent/iteration/horizon-reset for later inspection (`GET /runs/:id/audit` in serve mode); no-mistakes' TOON output is a live control protocol for driving the *current* run, not an offline audit format, and its logs are inspected per-step (`no-mistakes axi logs --step <name>`) rather than as one queryable session file. Falanx's kind-agnostic `AgentDef.kind` dispatch (`agent/run.rs` wraps `match def.kind { AgentKind::Cersei => ... }`) is designed so a future non-Cersei backend still satisfies the same output contract — no-mistakes' provider list is a fixed fallback chain of whole coding-agent CLIs with no shared per-step contract between them at all.

---

_Reference: [no-mistakes README](https://github.com/kunchenguid/no-mistakes) · [no-mistakes docs — Introduction](https://kunchenguid.github.io/no-mistakes/start-here/introduction/) · [no-mistakes docs — The Gate Model](https://kunchenguid.github.io/no-mistakes/concepts/gate-model/) · [no-mistakes docs — repo-config reference](https://kunchenguid.github.io/no-mistakes/reference/repo-config/) · local skill: `/home/ando/.agents/skills/no-mistakes/SKILL.md` · [Falanx ARCHITECTURE.md](ARCHITECTURE.md) · [Falanx Agents](FALANX_AGENTS.md) · [Agent Output Contract spec](superpowers/specs/2026-09-14-agent-output-contract-design.md) · [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)_
