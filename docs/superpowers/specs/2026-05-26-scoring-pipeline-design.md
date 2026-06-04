# Scoring Pipeline — Config-Driven Design

**Date:** 2026-05-26
**Scope:** `falanx-engine` scoring agent only — `agents/quality/`

---

## Scoring Philosophy

Each category scores the **quality of the changes in the diff** — not a general evaluation of the codebase. This is a quality gate on the delta, not a codebase health report. Absence of problems is a good result — a high score means the changes did not introduce issues in that category.

| Category | Ask this | Not this |
|---|---|---|
| Security | *Do these changes introduce security holes?* | *Are there security holes in this area of the code?* |
| Architecture | *Are the changes introducing new APIs or wrappers unnecessarily?* | *Were there too many APIs here already?* |
| Readability | *Do these changes make the code harder to understand?* | *Is this code well-written generally?* |

The same principle applies to every category. If the diff contains no changes relevant to a category, that category should return a high score. A low score means the changes actively introduced a problem.

---

## Problem

The current `agents/quality.rs` scores all five categories in a single LLM prompt. This:
- Exhausts small context windows (local models like `qwen2.5-14b`)
- Produces no per-category reasoning for the critique agent to work with
- Is not configurable — changing categories or prompts requires code changes

---

## Design

### Overview

The scoring step becomes a config-driven pipeline of specialised category agents, followed by a hardcoded synthesis agent. The orchestrator receives a `ScoringResult` — a richer type than the current `ReviewScore` — containing per-category scores, reasoning, and a synthesised overall verdict.

```
orchestrator
  → scoring pipeline (quality agent)
      → load ScoringConfig (once, at orchestrator setup)
      → pre-allocate CategoryResult slots
      → validate diff size against context budget
      → for each category in pipeline (sequential):
          → build prompt (system_prompt + prompt + diff)
          → agent.run()
          → parse { score, reasoning }
          → truncate reasoning to max_reasoning_chars
          → emit SessionEvent::CategoryScored
      → synthesis agent (fixed prompt):
          → receives all CategoryResults
          → returns overall_score + summary
      → emit SessionEvent::ScoringComplete
      → return ScoringResult
  ← ScoringResult
```

---

## Config Files

Two YAML files. Both ship as defaults embedded in the binary via `include_str!()`. Users can override via env vars pointing to local files.

### `falanx-scoring-categories.yaml`

Defines the registry of all available category agents. Each category is independent and reusable across pipelines.

examples:

```yaml
readability:
  system_prompt: "You are a code quality reviewer. You evaluate the quality of changes in a diff, not the codebase generally."
  prompt: |
    Do these changes introduce readability problems?
    Score 5 if the changes are clear or neutral. Score lower only if the changes actively make the code harder to read.
    Questions to answer:
    - Do the new names obscure intent?
    - Does the new control flow become harder to follow?
    - Do new abstractions add confusion rather than clarity?
    Score 1-5. Respond in JSON only:
    {"score": N, "reasoning": "max 80 words"}
  max_reasoning_chars: 400

maintainability:
  system_prompt: "You are a code quality reviewer. You evaluate the quality of changes in a diff, not the codebase generally."
  prompt: |
    Do these changes introduce maintainability problems?
    Score 5 if the changes are clean or neutral. Score lower only if the changes actively make the code harder to maintain.
    Questions to answer:
    - Do the changes blur responsibilities or mix concerns?
    - Do the changes make future modifications riskier?
    - Do the changes introduce hidden side effects or implicit dependencies?
    Score 1-5. Respond in JSON only:
    {"score": N, "reasoning": "max 80 words"}
  max_reasoning_chars: 400

architecture:
  system_prompt: "You are a code quality reviewer. You evaluate the quality of changes in a diff, not the codebase generally."
  prompt: |
    Do these changes introduce architectural problems?
    Score 5 if the changes fit the existing structure or are neutral. Score lower only if the changes actively damage boundaries or structure.
    Questions to answer:
    - Do the changes violate existing module boundaries?
    - Do the changes introduce inappropriate coupling?
    - Do the changes add complexity without necessity?
    Score 1-5. Respond in JSON only:
    {"score": N, "reasoning": "max 80 words"}
  max_reasoning_chars: 400

performance:
  system_prompt: "You are a code quality reviewer. You evaluate the quality of changes in a diff, not the codebase generally."
  prompt: |
    Do these changes introduce performance problems?
    Score 5 if the changes are efficient or neutral. Score lower only if the changes actively introduce regressions.
    Questions to answer:
    - Do the changes add unnecessary allocations or copies?
    - Do the changes introduce worse algorithmic complexity where better is feasible?
    - Do the changes block where async would be appropriate?
    Score 1-5. Respond in JSON only:
    {"score": N, "reasoning": "max 80 words"}
  max_reasoning_chars: 400

security:
  system_prompt: "You are a code quality reviewer. You evaluate the quality of changes in a diff, not the codebase generally."
  prompt: |
    Do these changes introduce security problems?
    Score 5 if the changes are safe or security is not relevant to this diff. Score lower only if the changes actively introduce risk.
    Questions to answer:
    - Do the changes create input validation gaps?
    - Do the changes expose secrets or introduce injection risks?
    - Do the changes cross trust boundaries unsafely?
    Score 1-5. Respond in JSON only:
    {"score": N, "reasoning": "max 80 words"}
  max_reasoning_chars: 400
```

### `falanx-scoring-pipelines.yaml`

Named pipelines that reference categories by name. Pipeline selection determines which category agents run and in what order.

```yaml
default:
  parallel: false
  categories: [readability, maintainability, architecture, performance, security]

quick:
  parallel: false
  categories: [readability, maintainability]

security:
  parallel: false
  categories: [security, architecture]

strict:
  parallel: false
  categories: [readability, maintainability, architecture, performance, security]
```

`strict` runs the same categories as `default`. The distinction is in the category definitions — a `strict` variant would reference stricter category entries (e.g. `security-strict`) defined in `falanx-scoring-categories.yaml`. The pipeline name signals intent; the category registry provides the stricter prompts. This is not implemented in the defaults but the structure supports it.

`parallel: false` is the only supported value in this phase. The field is present to make parallel execution a config-only change when added later — no structural change required.

---

## Environment Variables

| Variable | Purpose | Default |
|---|---|---|
| `FALANX_SCORING_CATEGORIES_CONFIG` | Path to categories YAML override | embedded default |
| `FALANX_SCORING_PIPELINES_CONFIG` | Path to pipelines YAML override | embedded default |
| `FALANX_SCORING_PIPELINE` | Pipeline name to run | `"default"` |

---

## Types

### Replacing `ReviewScore`

```rust
pub struct CategoryResult {
    pub name: String,
    pub score: u8,
    pub reasoning: String,  // truncated to max_reasoning_chars
}

pub struct ScoringResult {
    pub pipeline_name: String,
    pub categories: Vec<CategoryResult>,
    pub composite_score: f32,   // synthesis agent's overall_score as f32
    pub synthesis: String,      // synthesis agent's summary
}

impl ScoringResult {
    pub fn composite(&self) -> f32 { self.composite_score }
    pub fn delta(&self, other: &Self) -> f32 {
        (self.composite_score - other.composite_score).abs()
    }
}
```

`ReviewScore` is removed. `ScoringResult` replaces it everywhere: `RunResult`, orchestrator loop, plateau detection, critique agent input.

### New `SessionEvent` Variants

```rust
// Replaces ScoreComputed — fires once per category, in real time
CategoryScored {
    category: String,
    score: u8,
    reasoning: String,
    iteration: u32,
}

// Fires after synthesis — replaces ScoreComputed as the completion marker
ScoringComplete {
    pipeline_name: String,
    composite_score: f32,
    synthesis: String,
    iteration: u32,
}
```

`ScoreComputed` is removed and replaced by these two variants. `CategoryScored` fires immediately after each category completes — this is the primary real-time observability event. `ScoringComplete` is the event the HTTP API will query to reconstruct per-iteration composite scores.

---

## Code Structure

`agents/quality.rs` becomes a module directory:

```
agents/quality/
  mod.rs          — public score() fn, ScoringConfig loading
  pipeline.rs     — sequential category agent execution
  synthesis.rs    — synthesis agent, fixed prompt
  defaults/
    categories.yaml   — embedded default categories
    pipelines.yaml    — embedded default pipelines
```

### Public interface

```rust
// agents/quality/mod.rs
pub async fn score(
    ctx: &AgentContext<'_>,
    scoring_cfg: &ScoringConfig,
    session: &Session,
    iteration: u32,
) -> anyhow::Result<ScoringResult>
```

`scoring_cfg` is loaded once at orchestrator setup and passed in — `score()` does not re-load config on each call. `session` and `iteration` passed in so `CategoryScored` events emit in real time from inside the pipeline — not batched and returned to the orchestrator.

### Config loading (at orchestrator setup, not per-call)

```rust
pub struct ScoringConfig {
    pub categories: HashMap<String, CategoryConfig>,
    pub pipeline: PipelineConfig,   // resolved — already looked up by name
}

pub struct CategoryConfig {
    pub system_prompt: String,
    pub prompt: String,
    pub max_reasoning_chars: usize,
}

pub struct PipelineConfig {
    pub name: String,
    pub parallel: bool,
    pub categories: Vec<String>,    // ordered names, validated against registry
}
```

`ScoringConfig::load()` is called once in orchestrator setup. It validates that all category names in the selected pipeline exist in the registry. Fails fast before any LLM call.

---

## Orchestrator Setup Sequence additions

```
1. Load FalanxConfig        (model, loop settings)
2. Load ScoringConfig       (categories + pipeline, validate, pre-allocate)
3. Extract diff             (validate size against scoring context budget)
4. Open session
5. Run pipeline
```

Steps 1–4 are pure setup — no LLM calls. The diff size check in step 3 uses `ScoringConfig` to calculate worst-case context per category agent:

the budget should be calculated based on the context size of the models, and on the orchestration setup we should check those context limits, if they aren't provided.

```
budget_per_category = system_prompt.len() + prompt.len() + diff.len() + max_reasoning_chars
```

If `diff.len()` would push any category over a configurable `FALANX_MAX_DIFF_CHARS` limit, the diff is truncated with a visible warning before any agent fires.

`Vec::with_capacity(pipeline.categories.len())` pre-allocates the `CategoryResult` slots — known exactly from config.

---

## Synthesis Agent

Fixed prompt — not in YAML config. Receives all `CategoryResult`s formatted as a summary block:

```
You are a senior code reviewer synthesising multiple category assessments.

CATEGORY SCORES:
- readability: 4 — "Names are clear, flow is easy to follow"
- maintainability: 3 — "Some coupling between modules X and Y"
- security: 2 — "Potential injection risk in input handler"

Provide an overall code quality assessment considering the relative importance
of each finding. A critical security or architecture finding should weigh
heavily even if other scores are high.

Respond in JSON only:
{"overall_score": N, "summary": "max 120 words"}
```

The synthesis agent's `overall_score` becomes `ScoringResult.composite`. Its `summary` becomes `ScoringResult.synthesis`. This is an LLM judgement — not a weighted mean.

---

## Observability

Per-category progress is visible in real time via `SessionEvent::CategoryScored`. Log output during a scoring run:

```
[quality] iteration=0 pipeline=default categories=5
[quality] category=readability score=4 iteration=0
[quality] category=maintainability score=3 iteration=0
[quality] category=architecture score=4 iteration=0
[quality] category=performance score=4 iteration=0
[quality] category=security score=2 iteration=0
[quality] synthesis composite=3.2 iteration=0
```

Future HTTP API: `GET /runs/:id/scoring/:iteration` returns the full `ScoringComplete` event including per-category breakdown — directly from the JSONL session file.

---

## What Does Not Change

- `AgentContext` struct
- Cersei agent builder pattern (one fresh `Agent` per category + one for synthesis)
- `Session::append()` mechanics
- Orchestrator loop structure (plateau detection, horizon reset, max_iter)
- Critique agent call signature (receives `ScoringResult` instead of `ReviewScore` — richer input, same boundary)
- `falanx score` CLI command (still calls `agents::quality::score`)

---

## Out of Scope

- Parallel category execution (field present in config, not implemented)
- Making synthesis prompt configurable
- Applying this pattern to critique or rewrite agents
- HTTP API implementation
- Weighted composite calculation — synthesis agent decides the overall score holistically, no mechanical weighting
