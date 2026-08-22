# Composable Agent Pipeline — Design Spec

**Date:** 2026-06-04
**Supersedes:** `docs/ARCHITECTURE.md`, `docs/AGENTS.md` (both must be rewritten to match this spec before implementation begins)

---

## Summary

Refactor Falanx from a hardcoded score → review → rewrite pipeline into a composable, config-driven agent execution system. Agents are defined as directories (system prompt + prompt template + config). A `workflow.yaml` describes which agents run, in what order, what they output, and what conditions control the loop. A generic `WorkflowRunner` in Rust drives execution. The hardcoded pipeline disappears.

This is a full architectural replacement — not an additive layer.

---

## Goals

- Agent definitions (prompts, config) unified in one place, decoupled from Rust orchestration logic
- Workflow sequencing, loop control, and data wiring described in `workflow.yaml` DSL
- `RunResult` and `TemplateContext` carry data between stages via prompt injection — runs are stateless (no shared Cersei sessions between agents)
- Generic session events — no scoring-specific coupling in audit trail
- CLI simplified to `falanx run` — no subcommand per pipeline stage
- Architecture and agent docs replaced to match the new model

## Non-goals

- Per-agent model configuration (all Cersei agents share the same model from `FalanxConfig`)
- External agent harnesses (Claude Code CLI, Codex, Pi) — `AgentKind::Cersei` is the only implementation; extension point exists but is not wired
- Parallel agent execution within a stage (sequential only in v1; `parallel: true` reserved)
- `workflow.yaml`-configurable synthesis strategies — multi-agent stage output is always a raw array; templates handle aggregation
- UI or MCP changes — serve mode and MCP surface unchanged in this refactor

---

## Architecture

### Module shape

```
falanx-engine/src/
  workflow/
    mod.rs        — Workflow, load from yaml or embedded default
    runner.rs     — WorkflowRunner: drives stage loop, owns TemplateContext
    stage.rs      — Stage, StageResult, StageKind
    context.rs    — TemplateContext: accumulated stage outputs, diff, loop state
  agent/
    mod.rs        — AgentDef, AgentKind
    loader.rs     — load AgentDef from directory (system.md + prompt.md + config.yaml)
    run.rs        — run(agent_def, context, cfg) → RunResult; dispatches on AgentKind
  orchestrator/
    mod.rs        — entry point, RunConfig / RunResult, unchanged public signature
    horizon.rs    — unchanged
  session.rs      — refactored SessionEvent (generic), Session unchanged
  config.rs       — FalanxConfig, LoopConfig unchanged (CLI/env override layer)
  types.rs        — RunResult added; ScoringResult, RewritePatch retained for now
  provider.rs     — unchanged
```

### Embedded defaults

All defaults compiled into the binary via `include_str!`:

```
falanx-engine/src/defaults/
  agents/
    score_readability/
      system.md
      prompt.md
      config.yaml
    score_maintainability/
      system.md
      prompt.md
      config.yaml
    score_architecture/
      system.md
      prompt.md
      config.yaml
    score_performance/
      system.md
      prompt.md
      config.yaml
    score_security/
      system.md
      prompt.md
      config.yaml
    review/
      system.md
      prompt.md
      config.yaml
    rewrite/
      system.md
      prompt.md
      config.yaml
  workflow.yaml
```

User override: place a `.falanx/agents/<name>/` directory or `.falanx/workflow.yaml` in the repo. Runtime override wins over compiled defaults.

---

## Agent Definition

### Directory structure

Each agent is a directory with three files:

```
system.md      — system prompt passed to Cersei agent builder
prompt.md      — minijinja template; {{diff}}, {{output_as_name}}, etc.
config.yaml    — max_turns and kind
```

### `config.yaml`

```yaml
kind: cersei       # optional, defaults to cersei
max_turns: 1       # maps to Cersei .max_turns(n)
```

### `AgentDef` struct

```rust
pub struct AgentDef {
    pub name: String,
    pub kind: AgentKind,
    pub system_prompt: String,
    pub prompt_template: String,
    pub max_turns: u32,
}
```

### `AgentKind`

```rust
pub enum AgentKind {
    Cersei,
    // Future: ClaudeCode { model: String }, Codex, Pi
}

impl Default for AgentKind {
    fn default() -> Self { AgentKind::Cersei }
}
```

`AgentKind` is the extension seam for future harnesses. For Cersei, `system_prompt` maps to `.system_prompt()`, `max_turns` to `.max_turns(n)`. For a future `ClaudeCode` kind the same fields map to CLI flags — the interface is consistent.

### Cersei agent construction

```rust
// run.rs — AgentKind::Cersei path
let (provider, model_id) = build_provider(cfg)?;
let agent = Agent::builder()
    .provider_boxed(provider)
    .model(&model_id)
    .system_prompt(&def.system_prompt)
    .max_turns(def.max_turns)
    .build()?;
let output = agent.run(&rendered_prompt).await?;
```

Model and provider always come from `FalanxConfig` — not from `AgentDef`.

---

## Workflow DSL

### `workflow.yaml` schema

```yaml
loop:
  max_iterations: 5          # outer workflow loop ceiling — mandatory
  target_score: 4.5          # exit when score_result composite reaches this
  plateau_threshold: 0.1     # horizon reset trigger

stages:
  - id: score
    agents:                  # multiple agents → array output, sequential
      - score_readability
      - score_maintainability
      - score_architecture
      - score_performance
      - score_security
    output_format: json_object   # extract first {...} from each agent response
    output_as: score_result      # name in TemplateContext

  - id: review
    agent: review            # single agent
    output_format: json_array
    output_as: review_result
    skip_if: score_below_target  # named predicate

  - id: rewrite
    agent: rewrite
    output_format: json_array
    output_as: rewrite_result
    skip_if: no_review_issues
```

### `output_format`

| Value | Extraction behaviour |
|---|---|
| `json_object` | Find first `{` … `}`, parse as JSON object |
| `json_array` | Find first `[` … `]`, parse as JSON array |
| `text` | Raw agent output, no extraction — `serde_json::Value::String` |

Replaces the current ad-hoc `find('{')` / `find('[')` pattern in `review.rs` and `writing.rs`. Makes extraction intent explicit per-agent.

### `skip_if` — predicate vocabulary

Named predicates only. The runner errors on unknown values — no arbitrary expression evaluation.

| Value | Rust predicate |
|---|---|
| `score_below_target` | `composite(score_result) < loop.target_score` |
| `no_review_issues` | `review_result.as_array().map(Vec::is_empty).unwrap_or(true)` |

New predicates added as named Rust functions when needed. Same pattern as `skip_if` conditions.

### Multi-agent stage output

When a stage has multiple agents, each runs independently with the same inputs. The runner collects their extracted outputs into an array, tagging each with the agent name:

```json
// score_result after scoring stage
[
  {"agent": "score_readability",     "score": 4, "reasoning": "..."},
  {"agent": "score_maintainability", "score": 3, "reasoning": "..."},
  {"agent": "score_architecture",    "score": 4, "reasoning": "..."},
  {"agent": "score_performance",     "score": 4, "reasoning": "..."},
  {"agent": "score_security",        "score": 5, "reasoning": "..."}
]
```

No synthesis function. Templates handle aggregation via minijinja filters.

---

## Template Engine

**Crate: `minijinja`** — Rust port of Jinja2. Handles nested context, filters, and conditionals.

### TemplateContext

```rust
pub struct TemplateContext {
    pub diff: String,
    pub stages: HashMap<String, serde_json::Value>,  // keyed by output_as
    pub loop_iteration: u32,
}
```

Available in all templates as:

| Template variable | Value |
|---|---|
| `{{ diff }}` | Raw diff text |
| `{{ loop_iteration }}` | Current outer loop iteration |
| `{{ score_result }}` | Array from scoring stage |
| `{{ score_result \| selectattr("agent", "equalto", "score_security") \| first }}` | Single agent result |
| `{{ review_result }}` | Array from review stage |
| `{{ review_result \| tojson }}` | JSON-serialised for injection into rewrite prompt |

### Context growth and reset

Context grows as stages complete — each stage appends its `output_as` value before the next stage renders. No stage can access a future stage's output.

On each outer loop iteration the context resets to `{ diff }` only. Prior iteration outputs do not bleed across iterations.

### Example: review `prompt.md`

```markdown
Review this diff.

Scores:
{% for item in score_result %}
- {{ item.agent }}: {{ item.score }}/5 — {{ item.reasoning }}
{% endfor %}

{% set composite = score_result | map(attribute="score") | sum / score_result | length %}
Composite: {{ "%.1f" | format(composite) }}

Identify concrete issues. For each: location, problem, fix.
Respond as JSON array only:
[{"location":"file:line","problem":"...","fix":"..."}]
Return [] if no issues.

DIFF:
{{ diff }}
```

### Example: rewrite `prompt.md`

```markdown
Fix the following issues. Respond as JSON array only:
[{"original":"exact text","revised":"replacement","issue_ref":"location"}]
Return [] if nothing to change.

ISSUES:
{{ review_result | tojson }}

DIFF:
{{ diff }}
```

---

## RunResult and Data Flow

### `RunResult` — single agent run output

```rust
pub struct RunResult {
    pub agent_name: String,
    pub raw_output: String,
    pub extracted: serde_json::Value,  // json_object, json_array, or String
    pub session_id: SessionId,
    pub turns_used: u32,
}
```

### `StageResult` — one stage completion

```rust
pub struct StageResult {
    pub stage_id: String,
    pub runs: Vec<RunResult>,
    pub synthesised: serde_json::Value,  // array for multi-agent, extracted for single
}
```

### Data flow through default workflow

```
TemplateContext { diff }
    │
    ▼ Stage: score (5 agents, sequential)
      each receives {{ diff }}
      each outputs {"score": N, "reasoning": "..."}
      runner wraps: [{"agent": "score_readability", "score": 4, ...}, ...]
      → score_result added to TemplateContext
    │
    ▼ Stage: review (1 agent)
      receives {{ diff }}, {{ score_result }}
      outputs [{"location":"...","problem":"...","fix":"..."}]
      → review_result added to TemplateContext
    │
    ▼ Stage: rewrite (1 agent, max_turns=3)
      receives {{ diff }}, {{ review_result | tojson }}
      outputs [{"original":"...","revised":"...","issue_ref":"..."}]
      → rewrite_result added to TemplateContext
```

---

## Session Storage and Audit Trail

### Storage path — unchanged

```
~/.falanx/sessions/<label>/<uuid>.jsonl
```

### `SessionEvent` — refactored to remove scoring-specific events

```rust
pub enum SessionEvent {
    // Unchanged
    RunStarted   { session_id: String, target: String, workflow: String },
    RunFailed    { reason: String },
    HorizonReset { iteration: u32 },

    // Generic replacements for CategoryScored, ScoringComplete, IssuesFound, RewriteApplied, AgentInvoked
    StageStarted   { stage_id: String, iteration: u32 },
    AgentStarted   { stage_id: String, agent_name: String, iteration: u32 },
    AgentCompleted { stage_id: String, agent_name: String, turns_used: u32,
                     output: serde_json::Value, iteration: u32 },
    StageCompleted { stage_id: String, output_as: String,
                     result: serde_json::Value, iteration: u32 },
    StageSkipped   { stage_id: String, condition: String, iteration: u32 },

    // Generic summary replaces RunCompleted { final_score: ScoringResult }
    RunCompleted   { iterations: u32, summary: serde_json::Value },
}
```

**Deleted events:** `CategoryScored`, `ScoringComplete`, `IssuesFound`, `RewriteApplied`.

### `SessionMeta` updated

```rust
pub struct SessionMeta {
    pub id: SessionId,
    pub path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub summary: Option<serde_json::Value>,  // was: final_score: Option<ScoringResult>
    pub iterations: Option<u32>,
}
```

`RunCompleted.summary` carries the final `TemplateContext` state as opaque JSON. `list-sessions` displays it as compact JSON. Future UI reads it to render per-stage summaries.

### Observation

JSONL files are the primary observation mechanism in v1. Users inspect them directly to trace agent turns, stage outputs, and loop state. A UI that steps through agent turns per stage is a future build on this event stream.

---

## CLI Changes

### Commands

| Old | New |
|---|---|
| `falanx score --diff HEAD~1` | Removed |
| `falanx review --diff HEAD~1` | `falanx run --diff HEAD~1` |
| `falanx list-sessions` | Unchanged |
| `falanx serve` | Unchanged |

### `falanx run` flags

```
--diff <range>       Git range (e.g. HEAD~1, main..feature)
--file <path>        Single file
--workflow <name>    Workflow to run (default: embedded default workflow)
--agents <dir>       Additional agents directory to load (merged with defaults)
--max-iter <n>       Override workflow loop max_iterations
--target <f>         Override workflow loop target_score
--dry-run            Mock provider, no LLM calls
--config <path>      Path to .env config file
```

`--workflow` resolves: check `.falanx/<name>.yaml`, then embedded defaults.
`--agents` merges the given directory with compiled defaults; user agents win on name collision.

### Deleted CLI surface

```
Commands::Score
ScoreArgs
cmd_score()
FALANX_SCORING_PIPELINE env var
FALANX_SCORING_CATEGORIES_CONFIG env var
FALANX_SCORING_PIPELINES_CONFIG env var
```

**Retained env vars:** `FALANX_MODEL`, `OPENCODE_API_KEY`, `FALANX_BASE_URL`, `FALANX_DRY_RUN`, `FALANX_SESSION_DIR`, `FALANX_MAX_ITER`, `FALANX_TARGET_SCORE`, `FALANX_PLATEAU_THRESHOLD`, `FALANX_MAX_DIFF_CHARS`.

---

## Loop Control and Horizon Reset

Loop control is unchanged in behaviour — only the config source moves from `FalanxConfig` hardcoded values to `workflow.yaml` loop section (with CLI/env overrides applied on top).

| Exit condition | Source |
|---|---|
| `composite >= target_score` | `skip_if: score_below_target` predicate + `workflow.loop.target_score` |
| `iterations >= max_iterations` | `workflow.loop.max_iterations` |
| Plateau → horizon reset | `workflow.loop.plateau_threshold`; `horizon.rs` unchanged |

Horizon reset behaviour: discard live `TemplateContext`, reset to `{ diff }` only, construct fresh Cersei agents, continue loop. JSONL audit history preserved.

---

## Deletion List

### Rust modules deleted

```
rust/crates/falanx-engine/src/agents/quality/         (entire module)
rust/crates/falanx-engine/src/agents/review.rs
rust/crates/falanx-engine/src/agents/writing.rs
rust/crates/falanx-engine/src/orchestrator/pipeline.rs
```

### Types deleted from `types.rs`

```
ScoringResult      (scoring-specific, replaced by serde_json::Value in RunResult)
CategoryResult     (scoring-specific)
ReviewIssue        (scoring-specific)
RewritePatch       (scoring-specific)
RunState           (hardcoded stage names: Scoring, Reviewing, Rewriting — replaced by generic stage_id strings)
```

`SessionId` is retained — still used by `RunResult` and `Session`.

### Config files deleted

```
rust/crates/falanx-engine/src/agents/quality/defaults/categories.yaml
rust/crates/falanx-engine/src/agents/quality/defaults/pipelines.yaml
```

### Functions deleted

```
build_critique_prompt()
build_category_prompt()
build_rewrite_prompt()
parse_issues_response()
parse_patches_response()
parse_category_response()
ScoringConfig::load()
```

### Session events deleted

```
SessionEvent::CategoryScored
SessionEvent::ScoringComplete
SessionEvent::IssuesFound
SessionEvent::RewriteApplied
```

### CLI deleted

```
Commands::Score
ScoreArgs
cmd_score()
```

---

## Docs to Rewrite

The following documents are superseded by this spec and must be rewritten before implementation:

| Document | What changes |
|---|---|
| `docs/ARCHITECTURE.md` | Core engine diagram replaced with WorkflowRunner model; agent section replaced; CLI section updated to `falanx run`; session events updated |
| `docs/AGENTS.md` | `CodeQualityAgent`, `CodeReviewAgent`, `CodeWritingAgent` descriptions replaced with generic `AgentDef` model and workflow stage descriptions |
| `README.md` | Usage section: `falanx score` removed, `falanx review` → `falanx run`, new flags documented; example output updated to generic stage output |

---

## Invariants Preserved

- No rewrite without prior critique — enforced by `skip_if: no_review_issues` on rewrite stage
- Every run produces a JSONL audit trail
- Loop always has a hard iteration ceiling
- Horizon reset preserves audit history, resets only live context
- Agents do not share Cersei sessions — each run is stateless
- Model and provider always from `FalanxConfig` — not per-agent
