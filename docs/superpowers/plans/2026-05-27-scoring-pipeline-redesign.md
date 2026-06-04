# Scoring Pipeline Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-prompt `agents/quality.rs` with a config-driven pipeline of per-category agents followed by a synthesis agent, producing a `ScoringResult` with per-category scores and reasoning.

**Architecture:** `agents/quality/` module runs N sequential category agents (prompts and categories defined in YAML config embedded in the binary), then a synthesis agent that produces a holistic composite score. `ScoringResult` replaces `ReviewScore` throughout. Per-category `SessionEvent::CategoryScored` events fire in real time for observability.

**Tech Stack:** Rust, `serde_yaml = "0.9"`, `cersei-agent`, `cersei-provider`, existing `MockProvider`

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `rust/crates/falanx-engine/Cargo.toml` | Add `serde_yaml` dependency |
| Modify | `rust/crates/falanx-engine/src/types.rs` | Add `CategoryResult`, `ScoringResult`; update `SessionEvent`, `RunCompleted` |
| Modify | `rust/crates/falanx-engine/src/session.rs` | Update `SessionMeta.final_score` type, `Session::list()` parsing |
| Modify | `rust/crates/falanx-engine/src/provider.rs` | Add routing for category + synthesis mock responses |
| Create | `rust/crates/falanx-engine/src/agents/quality/defaults/categories.yaml` | Embedded default category definitions |
| Create | `rust/crates/falanx-engine/src/agents/quality/defaults/pipelines.yaml` | Embedded default pipeline definitions |
| Create | `rust/crates/falanx-engine/src/agents/quality/config.rs` | `CategoryConfig`, `PipelineConfig`, `ScoringConfig::load()` |
| Create | `rust/crates/falanx-engine/src/agents/quality/synthesis.rs` | Synthesis agent — fixed prompt, `synthesize()` |
| Create | `rust/crates/falanx-engine/src/agents/quality/pipeline.rs` | Sequential category execution, `run_categories()` |
| Create | `rust/crates/falanx-engine/src/agents/quality/mod.rs` | Public `score()` fn, wires config + pipeline + synthesis |
| Delete | `rust/crates/falanx-engine/src/agents/quality.rs` | Replaced by module directory |
| Modify | `rust/crates/falanx-engine/src/orchestrator/mod.rs` | `RunResult.final_score: ScoringResult` |
| Modify | `rust/crates/falanx-engine/src/orchestrator/pipeline.rs` | Load `ScoringConfig`, diff size validation, new `score()` call |
| Modify | `rust/crates/falanx/src/main.rs` | Update `cmd_score` — new score call + output |

---

## Task 1: Add serde_yaml dependency

**Files:**
- Modify: `rust/crates/falanx-engine/Cargo.toml`

- [ ] **Step 1: Add serde_yaml to dependencies**

In `rust/crates/falanx-engine/Cargo.toml`, add after `serde_json = "1"`:

```toml
serde_yaml = "0.9"
```

- [ ] **Step 2: Verify it compiles**

```bash
cd rust && cargo build -p falanx-engine
```

Expected: compiles without errors. `serde_yaml` downloads and links.

- [ ] **Step 3: Commit**

```bash
git add rust/crates/falanx-engine/Cargo.toml rust/Cargo.lock
git commit -m "chore(deps): add serde_yaml for scoring config loading"
```

---

## Task 2: Add CategoryResult type

**Files:**
- Modify: `rust/crates/falanx-engine/src/types.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` block in `types.rs`:

```rust
#[test]
fn category_result_serialises_to_json() {
    let r = CategoryResult {
        name: "readability".into(),
        score: 4,
        reasoning: "clear names".into(),
    };
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("\"name\":\"readability\""));
    assert!(json.contains("\"score\":4"));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd rust && cargo test -p falanx-engine category_result_serialises_to_json
```

Expected: compile error — `CategoryResult` not defined.

- [ ] **Step 3: Add the struct**

Add to `types.rs` before the existing `ReviewScore` definition:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CategoryResult {
    pub name: String,
    pub score: u8,
    pub reasoning: String,
}
```

- [ ] **Step 4: Run to verify it passes**

```bash
cd rust && cargo test -p falanx-engine category_result_serialises_to_json
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/types.rs
git commit -m "feat(types): add CategoryResult struct"
```

---

## Task 3: Add ScoringResult type

**Files:**
- Modify: `rust/crates/falanx-engine/src/types.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block:

```rust
#[test]
fn scoring_result_composite_returns_composite_score() {
    let r = ScoringResult {
        pipeline_name: "default".into(),
        categories: vec![],
        composite_score: 3.5,
        synthesis: "ok".into(),
    };
    assert!((r.composite() - 3.5).abs() < f32::EPSILON);
}

#[test]
fn scoring_result_delta_is_absolute_difference() {
    let a = ScoringResult {
        pipeline_name: "default".into(),
        categories: vec![],
        composite_score: 3.0,
        synthesis: "".into(),
    };
    let b = ScoringResult {
        pipeline_name: "default".into(),
        categories: vec![],
        composite_score: 4.5,
        synthesis: "".into(),
    };
    assert!((a.delta(&b) - 1.5).abs() < f32::EPSILON);
    assert!((b.delta(&a) - 1.5).abs() < f32::EPSILON);
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd rust && cargo test -p falanx-engine scoring_result
```

Expected: compile error — `ScoringResult` not defined.

- [ ] **Step 3: Add the struct and impl**

Add to `types.rs` after `CategoryResult`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScoringResult {
    pub pipeline_name: String,
    pub categories: Vec<CategoryResult>,
    pub composite_score: f32,
    pub synthesis: String,
}

impl ScoringResult {
    pub fn composite(&self) -> f32 {
        self.composite_score
    }

    pub fn delta(&self, other: &Self) -> f32 {
        (self.composite_score - other.composite_score).abs()
    }
}
```

- [ ] **Step 4: Run to verify they pass**

```bash
cd rust && cargo test -p falanx-engine scoring_result
```

Expected: both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/types.rs
git commit -m "feat(types): add ScoringResult with composite and delta"
```

---

## Task 4: Add CategoryScored and ScoringComplete session events

**Files:**
- Modify: `rust/crates/falanx-engine/src/types.rs`

- [ ] **Step 1: Write failing test**

Add to `#[cfg(test)]` in `types.rs`:

```rust
#[test]
fn session_event_category_scored_serialises() {
    let e = SessionEvent::CategoryScored {
        category: "security".into(),
        score: 2,
        reasoning: "injection risk".into(),
        iteration: 1,
    };
    let json = serde_json::to_string(&e).unwrap();
    assert!(json.contains("\"type\":\"category_scored\""));
    assert!(json.contains("\"score\":2"));
}

#[test]
fn session_event_scoring_complete_serialises() {
    let e = SessionEvent::ScoringComplete {
        pipeline_name: "default".into(),
        composite_score: 3.2,
        synthesis: "overall ok".into(),
        iteration: 0,
    };
    let json = serde_json::to_string(&e).unwrap();
    assert!(json.contains("\"type\":\"scoring_complete\""));
    assert!(json.contains("\"pipeline_name\":\"default\""));
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd rust && cargo test -p falanx-engine session_event_category session_event_scoring_complete
```

Expected: compile error — variants not defined.

- [ ] **Step 3: Add the variants to SessionEvent**

In `types.rs`, find the `SessionEvent` enum (it lives in `session.rs` — see Task 5). Skip — these are in `session.rs`, not `types.rs`. Move these tests to Task 5.

> **Note:** `SessionEvent` is defined in `session.rs`, not `types.rs`. All SessionEvent changes happen in Task 5.

- [ ] **Step 4: Discard the test additions from types.rs**

The `SessionEvent` tests written above belong in `session.rs`. Do not add them to `types.rs`. Proceed to Task 5.

---

## Task 5: Update SessionEvent — add CategoryScored + ScoringComplete, remove ScoreComputed, update RunCompleted

**Files:**
- Modify: `rust/crates/falanx-engine/src/session.rs`

- [ ] **Step 1: Write failing tests**

Add to `#[cfg(test)]` in `session.rs`:

```rust
#[test]
fn category_scored_event_serialises() {
    let session = Session::new(&std::path::PathBuf::from("/tmp/falanx-test-events"), "test").unwrap();
    session.append(SessionEvent::CategoryScored {
        category: "security".into(),
        score: 2,
        reasoning: "injection risk".into(),
        iteration: 0,
    }).unwrap();
    let content = std::fs::read_to_string(session.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(v["type"], "category_scored");
    assert_eq!(v["score"], 2);
}

#[test]
fn scoring_complete_event_serialises() {
    let session = Session::new(&std::path::PathBuf::from("/tmp/falanx-test-events"), "test2").unwrap();
    session.append(SessionEvent::ScoringComplete {
        pipeline_name: "default".into(),
        composite_score: 3.5,
        synthesis: "looks ok".into(),
        iteration: 0,
    }).unwrap();
    let content = std::fs::read_to_string(session.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(v["type"], "scoring_complete");
    assert_eq!(v["pipeline_name"], "default");
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd rust && cargo test -p falanx-engine category_scored_event scoring_complete_event
```

Expected: compile error — variants not defined.

- [ ] **Step 3: Update SessionEvent in session.rs**

Replace the existing `SessionEvent` enum definition in `session.rs`:

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    RunStarted { session_id: String, target: String },
    AgentInvoked { agent: String, iteration: u32 },
    CategoryScored { category: String, score: u8, reasoning: String, iteration: u32 },
    ScoringComplete { pipeline_name: String, composite_score: f32, synthesis: String, iteration: u32 },
    IssuesFound { count: usize, iteration: u32 },
    RewriteApplied { patches: usize, iteration: u32 },
    HorizonReset { iteration: u32 },
    RunCompleted { final_score: crate::types::ScoringResult, iterations: u32 },
    RunFailed { reason: String },
}
```

`ScoreComputed` is removed. `RunCompleted.final_score` changes from `ReviewScore` to `ScoringResult`.

- [ ] **Step 4: Update SessionMeta**

Replace `SessionMeta` in `session.rs`:

```rust
#[derive(Debug)]
pub struct SessionMeta {
    pub id: SessionId,
    pub path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub final_score: Option<crate::types::ScoringResult>,
    pub iterations: Option<u32>,
}
```

- [ ] **Step 5: Update Session::list() RunCompleted parsing**

In `Session::list()`, find the match arm for `SessionEvent::RunCompleted` and update it. The existing code:

```rust
Ok(entry) => match entry.event {
    SessionEvent::RunCompleted {
        final_score,
        iterations,
    } => (Some(final_score), Some(iterations)),
    _ => (None, None),
},
```

This is already correct by type — `final_score` is now `ScoringResult`. No change needed here; the pattern match works unchanged.

- [ ] **Step 6: Fix all ScoreComputed callsites**

Search for `ScoreComputed` across the codebase and remove/replace:

```bash
grep -r "ScoreComputed" rust/
```

Expected matches: `orchestrator/pipeline.rs` (2 occurrences) and `main.rs` (1 occurrence).

In `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`, replace both:
```rust
session.append(SessionEvent::ScoreComputed { score: s.clone(), iteration })?;
```
with temporary placeholders that will be replaced in Task 21:
```rust
// ScoreComputed removed — ScoringComplete emitted inside quality::score()
```

In `rust/crates/falanx/src/main.rs`, remove:
```rust
session.append(SessionEvent::ScoreComputed {
    score: score.clone(),
    iteration: 0,
})?;
```

- [ ] **Step 7: Fix ReviewScore import in orchestrator/mod.rs**

In `rust/crates/falanx-engine/src/orchestrator/mod.rs`, the import `types::{RewritePatch, ReviewScore, SessionId}` will need `ReviewScore` removed. Update it:

```rust
use crate::{
    config::{FalanxConfig, LoopConfig},
    git::ReviewTarget,
    session::Session,
    types::{RewritePatch, SessionId, ScoringResult},
};
```

Also update `RunResult`:

```rust
pub struct RunResult {
    pub final_score: ScoringResult,
    pub iterations: u32,
    pub patches: Vec<RewritePatch>,
    pub session_id: SessionId,
}
```

- [ ] **Step 8: Fix remaining ReviewScore references in orchestrator/pipeline.rs**

In `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`, update the import and all `ReviewScore` usages. Replace:

```rust
use crate::{
    agents::{self, AgentContext},
    config::FalanxConfig,
    orchestrator::{horizon::HorizonState, RunConfig, RunResult},
    provider::MockProvider,
    session::SessionEvent,
    types::RewritePatch,
};
```

Remove references to `ReviewScore` from the type of `current_score` and `prev_score` — these will become `ScoringResult` in Task 21.

For now, to keep compilation passing, stub the type. Add a temporary comment `// TODO Task 21: change to ScoringResult` and leave the body as-is (it will break — that's expected until Task 21).

- [ ] **Step 9: Run tests to check compile state**

```bash
cd rust && cargo build -p falanx-engine 2>&1 | head -40
```

Expected: compile errors in `orchestrator/pipeline.rs` about `ReviewScore` and `ScoreComputed` — these are expected and will be fixed in Tasks 20-21. The session.rs and types.rs changes should be error-free.

- [ ] **Step 10: Run session tests**

```bash
cd rust && cargo test -p falanx-engine category_scored_event scoring_complete_event
```

Expected: both PASS.

- [ ] **Step 11: Commit**

```bash
git add rust/crates/falanx-engine/src/session.rs \
        rust/crates/falanx-engine/src/orchestrator/mod.rs
git commit -m "feat(session): add CategoryScored + ScoringComplete events, RunCompleted uses ScoringResult"
```

---

## Task 6: Create default categories YAML

**Files:**
- Create: `rust/crates/falanx-engine/src/agents/quality/defaults/categories.yaml`

- [ ] **Step 1: Create the directory**

```bash
mkdir -p rust/crates/falanx-engine/src/agents/quality/defaults
```

- [ ] **Step 2: Write categories.yaml**

Create `rust/crates/falanx-engine/src/agents/quality/defaults/categories.yaml`:

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

- [ ] **Step 3: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/defaults/categories.yaml
git commit -m "feat(scoring): add default scoring categories YAML"
```

---

## Task 7: Create default pipelines YAML

**Files:**
- Create: `rust/crates/falanx-engine/src/agents/quality/defaults/pipelines.yaml`

- [ ] **Step 1: Write pipelines.yaml**

Create `rust/crates/falanx-engine/src/agents/quality/defaults/pipelines.yaml`:

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

- [ ] **Step 2: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/defaults/pipelines.yaml
git commit -m "feat(scoring): add default scoring pipelines YAML"
```

---

## Task 8: Create config.rs — structs and YAML loading

**Files:**
- Create: `rust/crates/falanx-engine/src/agents/quality/config.rs`

- [ ] **Step 1: Write failing tests**

Create `rust/crates/falanx-engine/src/agents/quality/config.rs` with tests first:

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CategoryConfig {
    pub system_prompt: String,
    pub prompt: String,
    pub max_reasoning_chars: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawPipelineConfig {
    pub parallel: bool,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub name: String,
    pub parallel: bool,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScoringConfig {
    pub categories: HashMap<String, CategoryConfig>,
    pub pipeline: PipelineConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_config_deserialises_from_yaml() {
        let yaml = r#"
readability:
  system_prompt: "test system"
  prompt: "test prompt"
  max_reasoning_chars: 200
"#;
        let cats: HashMap<String, CategoryConfig> = serde_yaml::from_str(yaml).unwrap();
        let r = cats.get("readability").unwrap();
        assert_eq!(r.system_prompt, "test system");
        assert_eq!(r.max_reasoning_chars, 200);
    }

    #[test]
    fn pipeline_config_deserialises_from_yaml() {
        let yaml = r#"
default:
  parallel: false
  categories: [readability, maintainability]
"#;
        let pipelines: HashMap<String, RawPipelineConfig> = serde_yaml::from_str(yaml).unwrap();
        let p = pipelines.get("default").unwrap();
        assert_eq!(p.categories.len(), 2);
        assert!(!p.parallel);
    }
}
```

- [ ] **Step 2: Run to verify tests pass (structs only, no load() yet)**

```bash
cd rust && cargo test -p falanx-engine category_config_deserialises pipeline_config_deserialises
```

Expected: both PASS. The `serde_yaml` crate handles deserialization.

- [ ] **Step 3: Commit structs**

```bash
git add rust/crates/falanx-engine/src/agents/quality/config.rs
git commit -m "feat(scoring/config): add CategoryConfig, PipelineConfig, ScoringConfig structs"
```

---

## Task 9: Implement ScoringConfig::load() with embedded defaults

**Files:**
- Modify: `rust/crates/falanx-engine/src/agents/quality/config.rs`

- [ ] **Step 1: Write failing test**

Add to `#[cfg(test)]` in `config.rs`:

```rust
#[test]
fn scoring_config_loads_embedded_defaults() {
    // Clear any override env vars
    unsafe {
        std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
        std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
        std::env::remove_var("FALANX_SCORING_PIPELINE");
    }
    let cfg = ScoringConfig::load().unwrap();
    assert!(cfg.categories.contains_key("readability"));
    assert!(cfg.categories.contains_key("security"));
    assert_eq!(cfg.pipeline.name, "default");
    assert_eq!(cfg.pipeline.categories.len(), 5);
}

#[test]
fn scoring_config_selects_quick_pipeline() {
    unsafe {
        std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
        std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
        std::env::set_var("FALANX_SCORING_PIPELINE", "quick");
    }
    let cfg = ScoringConfig::load().unwrap();
    assert_eq!(cfg.pipeline.name, "quick");
    assert_eq!(cfg.pipeline.categories.len(), 2);
    unsafe { std::env::remove_var("FALANX_SCORING_PIPELINE"); }
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd rust && cargo test -p falanx-engine scoring_config_loads scoring_config_selects
```

Expected: compile error — `ScoringConfig::load()` not defined.

- [ ] **Step 3: Implement load() with embedded defaults**

Add to `config.rs` after the struct definitions:

```rust
impl ScoringConfig {
    pub fn load() -> anyhow::Result<Self> {
        let categories_yaml = Self::load_categories_yaml()?;
        let pipelines_yaml = Self::load_pipelines_yaml()?;
        let pipeline_name = std::env::var("FALANX_SCORING_PIPELINE")
            .unwrap_or_else(|_| "default".into());

        let categories: HashMap<String, CategoryConfig> =
            serde_yaml::from_str(&categories_yaml)
                .map_err(|e| anyhow::anyhow!("invalid categories config: {}", e))?;

        let pipelines: HashMap<String, RawPipelineConfig> =
            serde_yaml::from_str(&pipelines_yaml)
                .map_err(|e| anyhow::anyhow!("invalid pipelines config: {}", e))?;

        let raw_pipeline = pipelines
            .get(&pipeline_name)
            .ok_or_else(|| anyhow::anyhow!("pipeline '{}' not found in pipelines config", pipeline_name))?;

        for cat_name in &raw_pipeline.categories {
            if !categories.contains_key(cat_name) {
                anyhow::bail!(
                    "category '{}' in pipeline '{}' not found in categories registry",
                    cat_name,
                    pipeline_name
                );
            }
        }

        Ok(Self {
            categories,
            pipeline: PipelineConfig {
                name: pipeline_name,
                parallel: raw_pipeline.parallel,
                categories: raw_pipeline.categories.clone(),
            },
        })
    }

    fn load_categories_yaml() -> anyhow::Result<String> {
        if let Ok(path) = std::env::var("FALANX_SCORING_CATEGORIES_CONFIG") {
            std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read categories config '{}': {}", path, e))
        } else {
            Ok(include_str!("defaults/categories.yaml").to_string())
        }
    }

    fn load_pipelines_yaml() -> anyhow::Result<String> {
        if let Ok(path) = std::env::var("FALANX_SCORING_PIPELINES_CONFIG") {
            std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read pipelines config '{}': {}", path, e))
        } else {
            Ok(include_str!("defaults/pipelines.yaml").to_string())
        }
    }
}
```

- [ ] **Step 4: Run to verify tests pass**

```bash
cd rust && cargo test -p falanx-engine scoring_config_loads scoring_config_selects
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/config.rs
git commit -m "feat(scoring/config): implement ScoringConfig::load with embedded defaults"
```

---

## Task 10: Test ScoringConfig validation — unknown pipeline and unknown category

**Files:**
- Modify: `rust/crates/falanx-engine/src/agents/quality/config.rs`

- [ ] **Step 1: Write failing tests**

Add to `#[cfg(test)]`:

```rust
#[test]
fn scoring_config_rejects_unknown_pipeline() {
    unsafe {
        std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
        std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
        std::env::set_var("FALANX_SCORING_PIPELINE", "nonexistent");
    }
    let err = ScoringConfig::load().unwrap_err();
    assert!(err.to_string().contains("not found in pipelines config"));
    unsafe { std::env::remove_var("FALANX_SCORING_PIPELINE"); }
}

#[test]
fn scoring_config_rejects_pipeline_with_unknown_category() {
    let dir = tempfile::tempdir().unwrap();
    let cats_path = dir.path().join("cats.yaml");
    let pipes_path = dir.path().join("pipes.yaml");
    std::fs::write(&cats_path, "readability:\n  system_prompt: s\n  prompt: p\n  max_reasoning_chars: 100\n").unwrap();
    std::fs::write(&pipes_path, "custom:\n  parallel: false\n  categories: [readability, nonexistent]\n").unwrap();

    unsafe {
        std::env::set_var("FALANX_SCORING_CATEGORIES_CONFIG", cats_path.to_str().unwrap());
        std::env::set_var("FALANX_SCORING_PIPELINES_CONFIG", pipes_path.to_str().unwrap());
        std::env::set_var("FALANX_SCORING_PIPELINE", "custom");
    }
    let err = ScoringConfig::load().unwrap_err();
    assert!(err.to_string().contains("nonexistent"));
    unsafe {
        std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
        std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
        std::env::remove_var("FALANX_SCORING_PIPELINE");
    }
}
```

- [ ] **Step 2: Add tempfile to dev-dependencies if not present**

Check `rust/crates/falanx-engine/Cargo.toml` — `tempfile = "3"` is already in `[dev-dependencies]`. No change needed.

- [ ] **Step 3: Run to verify tests pass**

```bash
cd rust && cargo test -p falanx-engine scoring_config_rejects
```

Expected: both PASS (validation logic already present from Task 9).

- [ ] **Step 4: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/config.rs
git commit -m "test(scoring/config): validation tests for unknown pipeline and category"
```

---

## Task 11: Create synthesis.rs — prompt building and response parsing

**Files:**
- Create: `rust/crates/falanx-engine/src/agents/quality/synthesis.rs`

- [ ] **Step 1: Write failing tests**

Create `rust/crates/falanx-engine/src/agents/quality/synthesis.rs`:

```rust
use crate::types::{CategoryResult, ScoringResult};

pub async fn synthesize(
    agent: &cersei_agent::Agent,
    pipeline_name: &str,
    categories: Vec<CategoryResult>,
) -> anyhow::Result<ScoringResult> {
    todo!()
}

pub fn build_synthesis_prompt(categories: &[CategoryResult]) -> String {
    todo!()
}

fn parse_synthesis_response(text: &str) -> anyhow::Result<(f32, String)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_synthesis_prompt_includes_all_categories() {
        let cats = vec![
            CategoryResult { name: "readability".into(), score: 4, reasoning: "clear names".into() },
            CategoryResult { name: "security".into(), score: 2, reasoning: "injection risk".into() },
        ];
        let prompt = build_synthesis_prompt(&cats);
        assert!(prompt.contains("readability: 4"));
        assert!(prompt.contains("security: 2"));
        assert!(prompt.contains("injection risk"));
        assert!(prompt.contains("overall_score"));
    }

    #[test]
    fn parse_synthesis_response_extracts_score_and_summary() {
        let text = r#"{"overall_score": 3, "summary": "mixed results"}"#;
        let (score, summary) = parse_synthesis_response(text).unwrap();
        assert!((score - 3.0).abs() < f32::EPSILON);
        assert_eq!(summary, "mixed results");
    }

    #[test]
    fn parse_synthesis_response_handles_prose_wrapping() {
        let text = r#"Here is the result: {"overall_score": 4, "summary": "looks good"} end"#;
        let (score, summary) = parse_synthesis_response(text).unwrap();
        assert!((score - 4.0).abs() < f32::EPSILON);
        assert_eq!(summary, "looks good");
    }

    #[test]
    fn parse_synthesis_response_errors_on_missing_json() {
        let err = parse_synthesis_response("no json here").unwrap_err();
        assert!(err.to_string().contains("no JSON"));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd rust && cargo test -p falanx-engine synthesis
```

Expected: compile errors — `todo!()` panics at runtime, or compile issues. Some tests will panic.

- [ ] **Step 3: Implement build_synthesis_prompt**

Replace `build_synthesis_prompt` `todo!()`:

```rust
pub fn build_synthesis_prompt(categories: &[CategoryResult]) -> String {
    let scores_block = categories
        .iter()
        .map(|c| format!("- {}: {} — \"{}\"", c.name, c.score, c.reasoning))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are a senior code reviewer synthesising multiple category assessments.\n\n\
         CATEGORY SCORES:\n{}\n\n\
         Provide an overall code quality assessment considering the relative importance \
         of each finding. A critical security or architecture finding should weigh \
         heavily even if other scores are high.\n\n\
         Respond in JSON only:\n\
         {{\"overall_score\": N, \"summary\": \"max 120 words\"}}",
        scores_block
    )
}
```

- [ ] **Step 4: Implement parse_synthesis_response**

Replace `parse_synthesis_response` `todo!()`:

```rust
fn parse_synthesis_response(text: &str) -> anyhow::Result<(f32, String)> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("no JSON in synthesis response"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("no JSON end in synthesis response"))?;
    let json = &text[start..=end];

    #[derive(serde::Deserialize)]
    struct SynthesisResponse {
        overall_score: u8,
        summary: String,
    }

    let r: SynthesisResponse = serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("failed to parse synthesis response: {}", e))?;
    Ok((r.overall_score as f32, r.summary))
}
```

- [ ] **Step 5: Run to verify parsing tests pass**

```bash
cd rust && cargo test -p falanx-engine synthesis
```

Expected: all 4 tests PASS. `synthesize` still has `todo!()` — that's fine, it isn't tested yet.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/synthesis.rs
git commit -m "feat(scoring/synthesis): prompt building and response parsing"
```

---

## Task 12: Implement synthesis.rs — synthesize() function

**Files:**
- Modify: `rust/crates/falanx-engine/src/agents/quality/synthesis.rs`

- [ ] **Step 1: Write failing test**

Add to `#[cfg(test)]` in `synthesis.rs`. This requires MockProvider so add the import and a tokio test:

```rust
#[tokio::test]
async fn synthesize_returns_scoring_result_with_mock_provider() {
    use crate::provider::MockProvider;

    let provider = Box::new(MockProvider);
    let agent = cersei_agent::Agent::builder()
        .provider_boxed(provider)
        .model("mock")
        .build()
        .unwrap();

    let cats = vec![
        CategoryResult { name: "readability".into(), score: 3, reasoning: "ok".into() },
    ];
    let result = synthesize(&agent, "default", cats).await.unwrap();
    assert_eq!(result.pipeline_name, "default");
    assert_eq!(result.categories.len(), 1);
    assert!(result.composite_score > 0.0);
    assert!(!result.synthesis.is_empty());
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd rust && cargo test -p falanx-engine synthesize_returns_scoring_result
```

Expected: panics on `todo!()`.

- [ ] **Step 3: Implement synthesize()**

Replace the `synthesize` `todo!()`:

```rust
pub async fn synthesize(
    agent: &cersei_agent::Agent,
    pipeline_name: &str,
    categories: Vec<CategoryResult>,
) -> anyhow::Result<ScoringResult> {
    let prompt = build_synthesis_prompt(&categories);
    let output = agent.run(&prompt).await
        .map_err(|e| anyhow::anyhow!("synthesis agent failed: {}", e))?;
    tracing::debug!(response = output.text(), "synthesis agent raw response");
    let (composite_score, synthesis) = parse_synthesis_response(output.text())?;
    Ok(ScoringResult {
        pipeline_name: pipeline_name.to_string(),
        categories,
        composite_score,
        synthesis,
    })
}
```

- [ ] **Step 4: Update MockProvider to handle synthesis prompts**

The synthesis prompt contains "synthesising" — MockProvider needs to return a valid synthesis JSON. Open `rust/crates/falanx-engine/src/provider.rs` and update `response_for`:

```rust
impl MockProvider {
    fn mock_score_json() -> &'static str {
        r#"{"readability":3,"maintainability":3,"performance":3,"security":3,"architecture":3}"#
    }

    fn mock_category_json() -> &'static str {
        r#"{"score": 3, "reasoning": "mock category reasoning"}"#
    }

    fn mock_synthesis_json() -> &'static str {
        r#"{"overall_score": 3, "summary": "mock synthesis summary"}"#
    }

    fn mock_issues_json() -> &'static str {
        r#"[{"location":"mock:0","problem":"no issues found","fix":"none"}]"#
    }

    fn mock_patches_json() -> &'static str {
        r#"[]"#
    }

    fn response_for(request: &CompletionRequest) -> &'static str {
        let system = request.system.as_deref().unwrap_or("");
        let user_text = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == cersei_types::Role::User)
            .and_then(|m| m.get_text())
            .unwrap_or("");

        // New scoring pipeline routing — check system prompt first
        if system.contains("code quality reviewer") {
            return Self::mock_category_json();
        }

        // Synthesis agent — no system prompt, check user message
        if user_text.contains("synthesising") {
            return Self::mock_synthesis_json();
        }

        // Legacy routing for review + rewrite agents
        let haystack = if system.is_empty() { user_text } else { system };
        if haystack.contains("SCORE") {
            Self::mock_score_json()
        } else if haystack.contains("CRITIQUE") {
            Self::mock_issues_json()
        } else {
            Self::mock_patches_json()
        }
    }
    // static_stream unchanged
```

- [ ] **Step 5: Run to verify synthesis test passes**

```bash
cd rust && cargo test -p falanx-engine synthesize_returns_scoring_result
```

Expected: PASS.

- [ ] **Step 6: Run all provider tests**

```bash
cd rust && cargo test -p falanx-engine provider
```

Expected: all existing provider tests still PASS.

- [ ] **Step 7: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/synthesis.rs \
        rust/crates/falanx-engine/src/provider.rs
git commit -m "feat(scoring/synthesis): implement synthesize() + update MockProvider routing"
```

---

## Task 13: Create pipeline.rs — prompt building and response parsing

**Files:**
- Create: `rust/crates/falanx-engine/src/agents/quality/pipeline.rs`

- [ ] **Step 1: Write failing tests**

Create `rust/crates/falanx-engine/src/agents/quality/pipeline.rs`:

```rust
use std::collections::HashMap;
use crate::{
    config::FalanxConfig,
    git::Diff,
    session::{Session, SessionEvent},
    types::CategoryResult,
};
use super::config::{CategoryConfig, PipelineConfig};

pub async fn run_categories(
    diff: &Diff,
    falanx_cfg: &FalanxConfig,
    pipeline: &PipelineConfig,
    categories: &HashMap<String, CategoryConfig>,
    session: &Session,
    iteration: u32,
) -> anyhow::Result<Vec<CategoryResult>> {
    todo!()
}

pub fn build_category_prompt(prompt_template: &str, diff: &str) -> String {
    todo!()
}

fn parse_category_response(text: &str) -> anyhow::Result<(u8, String)> {
    todo!()
}

fn truncate_reasoning(reasoning: String, max_chars: usize) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_category_prompt_injects_diff() {
        let template = "Review this.\nDIFF:\n{diff}";
        let result = build_category_prompt(template, "fn foo() {}");
        assert!(result.contains("fn foo() {}"));
        assert!(result.contains("Review this."));
    }

    #[test]
    fn build_category_prompt_appends_diff_when_no_placeholder() {
        let template = "Review this diff.";
        let result = build_category_prompt(template, "fn foo() {}");
        assert!(result.contains("fn foo() {}"));
    }

    #[test]
    fn parse_category_response_extracts_score_and_reasoning() {
        let text = r#"{"score": 4, "reasoning": "clear names"}"#;
        let (score, reasoning) = parse_category_response(text).unwrap();
        assert_eq!(score, 4);
        assert_eq!(reasoning, "clear names");
    }

    #[test]
    fn parse_category_response_handles_prose_wrapping() {
        let text = r#"Here is the score: {"score": 3, "reasoning": "ok"} done."#;
        let (score, reasoning) = parse_category_response(text).unwrap();
        assert_eq!(score, 3);
        assert_eq!(reasoning, "ok");
    }

    #[test]
    fn parse_category_response_errors_on_missing_json() {
        let err = parse_category_response("no json").unwrap_err();
        assert!(err.to_string().contains("no JSON"));
    }

    #[test]
    fn truncate_reasoning_leaves_short_strings_unchanged() {
        let s = "short".to_string();
        assert_eq!(truncate_reasoning(s.clone(), 100), s);
    }

    #[test]
    fn truncate_reasoning_truncates_long_strings() {
        let s = "a".repeat(500);
        let result = truncate_reasoning(s, 100);
        assert_eq!(result.len(), 100);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd rust && cargo test -p falanx-engine pipeline
```

Expected: panics on `todo!()`.

- [ ] **Step 3: Implement the three helper functions**

Replace the three `todo!()` helpers:

```rust
pub fn build_category_prompt(prompt_template: &str, diff: &str) -> String {
    if prompt_template.contains("{diff}") {
        prompt_template.replace("{diff}", diff)
    } else {
        format!("{}\n\nDIFF:\n{}", prompt_template, diff)
    }
}

fn parse_category_response(text: &str) -> anyhow::Result<(u8, String)> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("no JSON in category response"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("no JSON end in category response"))?;
    let json = &text[start..=end];

    #[derive(serde::Deserialize)]
    struct CategoryResponse {
        score: u8,
        reasoning: String,
    }

    let r: CategoryResponse = serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("failed to parse category response: {}", e))?;
    Ok((r.score, r.reasoning))
}

fn truncate_reasoning(reasoning: String, max_chars: usize) -> String {
    if reasoning.len() <= max_chars {
        reasoning
    } else {
        reasoning[..max_chars].to_string()
    }
}
```

- [ ] **Step 4: Run to verify helper tests pass**

```bash
cd rust && cargo test -p falanx-engine pipeline
```

Expected: all 7 helper tests PASS. `run_categories` still has `todo!()`.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/pipeline.rs
git commit -m "feat(scoring/pipeline): prompt building, response parsing, truncation helpers"
```

---

## Task 14: Implement pipeline.rs — run_categories() sequential execution

**Files:**
- Modify: `rust/crates/falanx-engine/src/agents/quality/pipeline.rs`

- [ ] **Step 1: Write failing test**

Add to `#[cfg(test)]` in `pipeline.rs`:

```rust
#[tokio::test]
async fn run_categories_emits_category_scored_events() {
    use crate::provider::MockProvider;
    use crate::config::{FalanxConfig, LoopConfig, ProviderConfig, SessionConfig};

    let dir = tempfile::tempdir().unwrap();
    let session = Session::new(dir.path(), "test").unwrap();

    let falanx_cfg = FalanxConfig {
        provider: ProviderConfig {
            model: "mock".into(),
            api_key: "".into(),
            base_url: None,
            dry_run: true,
        },
        session: SessionConfig { dir: dir.path().to_path_buf() },
        loop_cfg: LoopConfig::default(),
    };

    let mut cats = HashMap::new();
    cats.insert("readability".to_string(), CategoryConfig {
        system_prompt: "You are a code quality reviewer. You evaluate the quality of changes in a diff, not the codebase generally.".into(),
        prompt: "Do these changes introduce readability problems?\nScore 1-5. Respond in JSON only:\n{\"score\": N, \"reasoning\": \"max 80 words\"}".into(),
        max_reasoning_chars: 400,
    });

    let pipeline = PipelineConfig {
        name: "test".into(),
        parallel: false,
        categories: vec!["readability".into()],
    };

    let diff = Diff("fn foo() {}".into());
    let results = run_categories(&diff, &falanx_cfg, &pipeline, &cats, &session, 0).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "readability");
    assert!(results[0].score > 0);

    // Verify session event was written
    let content = std::fs::read_to_string(session.path()).unwrap();
    assert!(content.contains("category_scored"));
    assert!(content.contains("readability"));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd rust && cargo test -p falanx-engine run_categories_emits
```

Expected: panics on `todo!()`.

- [ ] **Step 3: Implement run_categories()**

Replace `run_categories` `todo!()`:

```rust
pub async fn run_categories(
    diff: &Diff,
    falanx_cfg: &FalanxConfig,
    pipeline: &PipelineConfig,
    categories: &HashMap<String, CategoryConfig>,
    session: &Session,
    iteration: u32,
) -> anyhow::Result<Vec<CategoryResult>> {
    let mut results = Vec::with_capacity(pipeline.categories.len());

    for cat_name in &pipeline.categories {
        let cat_cfg = &categories[cat_name];
        let prompt = build_category_prompt(&cat_cfg.prompt, &diff.0);

        let (provider, model_id) = build_provider(falanx_cfg)?;
        let agent = cersei_agent::Agent::builder()
            .provider_boxed(provider)
            .model(&model_id)
            .system_prompt(&cat_cfg.system_prompt)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build category agent: {}", e))?;

        let output = agent
            .run(&prompt)
            .await
            .map_err(|e| anyhow::anyhow!("category agent '{}' failed: {}", cat_name, e))?;

        tracing::debug!(category = %cat_name, response = output.text(), "category agent raw response");

        let (score, reasoning) = parse_category_response(output.text())?;
        let reasoning = truncate_reasoning(reasoning, cat_cfg.max_reasoning_chars);

        session.append(SessionEvent::CategoryScored {
            category: cat_name.clone(),
            score,
            reasoning: reasoning.clone(),
            iteration,
        })?;

        tracing::info!(
            category = %cat_name,
            score = score,
            iteration = iteration,
            "category scored"
        );

        results.push(CategoryResult {
            name: cat_name.clone(),
            score,
            reasoning,
        });
    }

    Ok(results)
}
```

Also add the `build_provider` helper at the top of `pipeline.rs` (private, only used here):

```rust
fn build_provider(
    cfg: &FalanxConfig,
) -> anyhow::Result<(Box<dyn cersei_provider::Provider>, String)> {
    if cfg.provider.dry_run {
        Ok((
            Box::new(crate::provider::MockProvider),
            cfg.provider.model.clone(),
        ))
    } else {
        cersei_provider::from_model_string(&cfg.provider.model)
            .map_err(|e| anyhow::anyhow!("provider error: {}", e))
    }
}
```

And add the required imports at the top of `pipeline.rs`:

```rust
use std::collections::HashMap;
use crate::{
    config::FalanxConfig,
    git::Diff,
    session::{Session, SessionEvent},
    types::CategoryResult,
};
use super::config::{CategoryConfig, PipelineConfig};
```

- [ ] **Step 4: Run to verify test passes**

```bash
cd rust && cargo test -p falanx-engine run_categories_emits
```

Expected: PASS.

- [ ] **Step 5: Run all pipeline tests**

```bash
cd rust && cargo test -p falanx-engine pipeline
```

Expected: all 8 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/pipeline.rs
git commit -m "feat(scoring/pipeline): implement run_categories sequential execution"
```

---

## Task 15: Create quality/mod.rs — public score() function

**Files:**
- Create: `rust/crates/falanx-engine/src/agents/quality/mod.rs`

- [ ] **Step 1: Write failing test**

Create `rust/crates/falanx-engine/src/agents/quality/mod.rs`:

```rust
mod config;
mod pipeline;
mod synthesis;

pub use config::ScoringConfig;

use crate::{
    config::FalanxConfig,
    git::Diff,
    session::{Session, SessionEvent},
    types::ScoringResult,
};

pub async fn score(
    diff: &Diff,
    falanx_cfg: &FalanxConfig,
    scoring_cfg: &ScoringConfig,
    session: &Session,
    iteration: u32,
) -> anyhow::Result<ScoringResult> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FalanxConfig, LoopConfig, ProviderConfig, SessionConfig};

    #[tokio::test]
    async fn score_runs_full_pipeline_with_mock_provider() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::new(dir.path(), "test").unwrap();

        let falanx_cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "mock".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: SessionConfig { dir: dir.path().to_path_buf() },
            loop_cfg: LoopConfig::default(),
        };

        unsafe {
            std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
            std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
            std::env::set_var("FALANX_SCORING_PIPELINE", "quick");
        }
        let scoring_cfg = ScoringConfig::load().unwrap();
        unsafe { std::env::remove_var("FALANX_SCORING_PIPELINE"); }

        let diff = Diff("fn add(a: i32, b: i32) -> i32 { a + b }".into());
        let result = score(&diff, &falanx_cfg, &scoring_cfg, &session, 0).await.unwrap();

        assert_eq!(result.pipeline_name, "quick");
        assert_eq!(result.categories.len(), 2); // quick pipeline: readability + maintainability
        assert!(result.composite_score > 0.0);
        assert!(!result.synthesis.is_empty());

        // Verify session events
        let content = std::fs::read_to_string(session.path()).unwrap();
        assert!(content.contains("category_scored"));
        assert!(content.contains("scoring_complete"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd rust && cargo test -p falanx-engine score_runs_full_pipeline
```

Expected: panics on `todo!()`.

- [ ] **Step 3: Implement score()**

Replace the `todo!()` in `score()`:

```rust
pub async fn score(
    diff: &Diff,
    falanx_cfg: &FalanxConfig,
    scoring_cfg: &ScoringConfig,
    session: &Session,
    iteration: u32,
) -> anyhow::Result<ScoringResult> {
    tracing::info!(
        pipeline = %scoring_cfg.pipeline.name,
        categories = scoring_cfg.pipeline.categories.len(),
        iteration = iteration,
        "scoring pipeline started"
    );

    let category_results = pipeline::run_categories(
        diff,
        falanx_cfg,
        &scoring_cfg.pipeline,
        &scoring_cfg.categories,
        session,
        iteration,
    )
    .await?;

    let (provider, model_id) = build_provider(falanx_cfg)?;
    let synthesis_agent = cersei_agent::Agent::builder()
        .provider_boxed(provider)
        .model(&model_id)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build synthesis agent: {}", e))?;

    let result = synthesis::synthesize(
        &synthesis_agent,
        &scoring_cfg.pipeline.name,
        category_results,
    )
    .await?;

    session.append(SessionEvent::ScoringComplete {
        pipeline_name: result.pipeline_name.clone(),
        composite_score: result.composite_score,
        synthesis: result.synthesis.clone(),
        iteration,
    })?;

    tracing::info!(
        composite = result.composite_score,
        pipeline = %result.pipeline_name,
        iteration = iteration,
        "scoring pipeline complete"
    );

    Ok(result)
}
```

Add the `build_provider` helper (same as in `pipeline.rs`):

```rust
fn build_provider(
    cfg: &FalanxConfig,
) -> anyhow::Result<(Box<dyn cersei_provider::Provider>, String)> {
    if cfg.provider.dry_run {
        Ok((
            Box::new(crate::provider::MockProvider),
            cfg.provider.model.clone(),
        ))
    } else {
        cersei_provider::from_model_string(&cfg.provider.model)
            .map_err(|e| anyhow::anyhow!("provider error: {}", e))
    }
}
```

- [ ] **Step 4: Run to verify test passes**

```bash
cd rust && cargo test -p falanx-engine score_runs_full_pipeline
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality/mod.rs
git commit -m "feat(scoring): implement score() wiring pipeline + synthesis"
```

---

## Task 16: Delete old quality.rs and wire agents/mod.rs

**Files:**
- Delete: `rust/crates/falanx-engine/src/agents/quality.rs`
- Modify: `rust/crates/falanx-engine/src/agents/mod.rs`

- [ ] **Step 1: Delete old quality.rs**

```bash
rm rust/crates/falanx-engine/src/agents/quality.rs
```

- [ ] **Step 2: Verify agents/mod.rs still works**

`agents/mod.rs` already has `pub mod quality;` — Rust will now resolve it to `agents/quality/mod.rs` automatically. No change needed to `mod.rs` for the module declaration.

However, `AgentContext` is still used by `review.rs` and `writing.rs`. Keep it. `agents/mod.rs` stays as-is:

```rust
pub mod quality;
pub mod review;
pub mod writing;

pub struct AgentContext<'a> {
    pub agent: &'a cersei_agent::Agent,
    pub diff: &'a crate::git::Diff,
}
```

- [ ] **Step 3: Verify compile**

```bash
cd rust && cargo build -p falanx-engine 2>&1 | grep "^error"
```

Expected: errors only in `orchestrator/pipeline.rs` (unresolved `ReviewScore`, old `score()` call) and `main.rs`. The agents module itself should compile cleanly.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/
git commit -m "refactor(agents): replace quality.rs with quality/ module"
```

---

## Task 17: Update orchestrator/mod.rs — RunResult uses ScoringResult

**Files:**
- Modify: `rust/crates/falanx-engine/src/orchestrator/mod.rs`

- [ ] **Step 1: Update the file**

Replace the full contents of `rust/crates/falanx-engine/src/orchestrator/mod.rs`:

```rust
pub mod horizon;
pub mod pipeline;

use crate::{
    config::{FalanxConfig, LoopConfig},
    git::ReviewTarget,
    session::Session,
    types::{RewritePatch, ScoringResult, SessionId},
};

pub struct RunConfig {
    pub target: ReviewTarget,
    pub loop_cfg: LoopConfig,
    pub session: Session,
}

pub struct RunResult {
    pub final_score: ScoringResult,
    pub iterations: u32,
    pub patches: Vec<RewritePatch>,
    pub session_id: SessionId,
}

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    pipeline::run(config, falanx_cfg).await
}
```

- [ ] **Step 2: Verify compile progress**

```bash
cd rust && cargo build -p falanx-engine 2>&1 | grep "^error"
```

Expected: remaining errors only in `orchestrator/pipeline.rs` — the `RunResult` type mismatch and old `score()` call.

- [ ] **Step 3: Commit**

```bash
git add rust/crates/falanx-engine/src/orchestrator/mod.rs
git commit -m "feat(orchestrator): RunResult.final_score uses ScoringResult"
```

---

## Task 18: Update orchestrator/pipeline.rs — load ScoringConfig and validate diff size

**Files:**
- Modify: `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`

- [ ] **Step 1: Update imports**

At the top of `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`, replace the import block:

```rust
use crate::{
    agents::{self, AgentContext},
    config::FalanxConfig,
    orchestrator::{horizon::HorizonState, RunConfig, RunResult},
    provider::MockProvider,
    session::SessionEvent,
    types::RewritePatch,
};
use falanx_engine::agents::quality::ScoringConfig;
```

Wait — this is within the same crate. Use:

```rust
use crate::{
    agents::{self, AgentContext},
    agents::quality::ScoringConfig,
    config::FalanxConfig,
    orchestrator::{horizon::HorizonState, RunConfig, RunResult},
    provider::MockProvider,
    session::SessionEvent,
    types::{RewritePatch, ScoringResult},
};
```

- [ ] **Step 2: Load ScoringConfig at the start of run()**

In `orchestrator/pipeline.rs`, at the top of the `run()` function body, add before the diff extraction:

```rust
pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    let RunConfig { target, loop_cfg, session } = config;
    let session_id = session.id().clone();

    // Load scoring config once — validates pipeline + categories before any LLM call
    let scoring_cfg = ScoringConfig::load()
        .map_err(|e| anyhow::anyhow!("failed to load scoring config: {}", e))?;

    tracing::info!(
        pipeline = %scoring_cfg.pipeline.name,
        categories = scoring_cfg.pipeline.categories.len(),
        "scoring config loaded"
    );

    let diff = target.extract_diff()?;

    // Diff size guard — warn and truncate if over limit
    let max_diff_chars: usize = std::env::var("FALANX_MAX_DIFF_CHARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);

    let diff = if diff.0.len() > max_diff_chars {
        tracing::warn!(
            original_chars = diff.0.len(),
            truncated_to = max_diff_chars,
            "diff exceeds FALANX_MAX_DIFF_CHARS — truncating"
        );
        crate::git::Diff(diff.0[..max_diff_chars].to_string())
    } else {
        diff
    };

    session.append(SessionEvent::RunStarted {
        session_id: session_id.0.clone(),
        target: "review".to_string(),
    })?;

    // ... rest of function continues in Task 19
```

- [ ] **Step 3: Verify compile progress**

```bash
cd rust && cargo build -p falanx-engine 2>&1 | grep "^error" | head -20
```

- [ ] **Step 4: Commit**

```bash
git add rust/crates/falanx-engine/src/orchestrator/pipeline.rs
git commit -m "feat(orchestrator): load ScoringConfig at setup, add diff size guard"
```

---

## Task 19: Update orchestrator/pipeline.rs — use new score() call throughout

**Files:**
- Modify: `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`

This task replaces all uses of the old `agents::quality::score(&ctx)` with the new signature and fixes the full `run()` function body to use `ScoringResult`.

- [ ] **Step 1: Write the updated run() body**

Replace the full `run()` function in `orchestrator/pipeline.rs` with:

```rust
pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    let RunConfig { target, loop_cfg, session } = config;
    let session_id = session.id().clone();

    let scoring_cfg = ScoringConfig::load()
        .map_err(|e| anyhow::anyhow!("failed to load scoring config: {}", e))?;

    tracing::info!(
        pipeline = %scoring_cfg.pipeline.name,
        categories = scoring_cfg.pipeline.categories.len(),
        "scoring config loaded"
    );

    let diff = target.extract_diff()?;

    let max_diff_chars: usize = std::env::var("FALANX_MAX_DIFF_CHARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);

    let diff = if diff.0.len() > max_diff_chars {
        tracing::warn!(
            original_chars = diff.0.len(),
            truncated_to = max_diff_chars,
            "diff exceeds FALANX_MAX_DIFF_CHARS — truncating"
        );
        crate::git::Diff(diff.0[..max_diff_chars].to_string())
    } else {
        diff
    };

    session.append(SessionEvent::RunStarted {
        session_id: session_id.0.clone(),
        target: "review".to_string(),
    })?;

    let mut horizon = HorizonState::new();
    let mut iteration: u32 = 0;
    let mut prev_score: Option<ScoringResult> = None;
    let mut all_patches: Vec<RewritePatch> = vec![];
    let mut plateau_streak: u32 = 0;

    // Initial score
    let mut current_score = {
        session.append(SessionEvent::AgentInvoked { agent: "quality".into(), iteration })?;
        agents::quality::score(&diff, falanx_cfg, &scoring_cfg, &session, iteration).await?
    };

    loop {
        if current_score.composite() >= loop_cfg.target_score {
            break;
        }
        if iteration >= loop_cfg.max_iter {
            break;
        }

        iteration += 1;

        // Critique
        session.append(SessionEvent::AgentInvoked { agent: "review".into(), iteration })?;
        let agent = build_agent(falanx_cfg)?;
        let ctx = AgentContext { agent: &agent, diff: &diff };
        let issues = agents::review::critique(&ctx, &current_score.composite()).await?;
        session.append(SessionEvent::IssuesFound { count: issues.len(), iteration })?;

        if !issues.is_empty() {
            session.append(SessionEvent::AgentInvoked { agent: "writing".into(), iteration })?;
            let agent = build_agent(falanx_cfg)?;
            let ctx = AgentContext { agent: &agent, diff: &diff };
            let patches = agents::writing::rewrite(&ctx, &issues).await?;
            session.append(SessionEvent::RewriteApplied { patches: patches.len(), iteration })?;
            all_patches.extend(patches);
        }

        // Re-score
        session.append(SessionEvent::AgentInvoked { agent: "quality".into(), iteration })?;
        let new_score = agents::quality::score(&diff, falanx_cfg, &scoring_cfg, &session, iteration).await?;

        // Plateau check
        let is_plateau = prev_score
            .as_ref()
            .map(|p| new_score.delta(p) < loop_cfg.plateau_threshold)
            .unwrap_or(false);

        if is_plateau {
            plateau_streak += 1;
        } else {
            plateau_streak = 0;
        }

        if plateau_streak >= 2 {
            if horizon.should_reset(true) {
                horizon.record_reset();
                plateau_streak = 0;
                session.append(SessionEvent::HorizonReset { iteration })?;
            } else {
                break;
            }
        }

        prev_score = Some(new_score.clone());
        current_score = new_score;
    }

    session.append(SessionEvent::RunCompleted {
        final_score: current_score.clone(),
        iterations: iteration,
    })?;

    Ok(RunResult {
        final_score: current_score,
        iterations: iteration,
        patches: all_patches,
        session_id,
    })
}
```

Note: `agents::review::critique` currently takes `score: &ReviewScore` — update that call to pass `&current_score.composite()` as a float, or update the review agent signature. See step 2.

- [ ] **Step 2: Fix review agent critique signature**

Open `rust/crates/falanx-engine/src/agents/review.rs`. The current signature:

```rust
pub async fn critique(
    ctx: &AgentContext<'_>,
    score: &ReviewScore,
) -> anyhow::Result<Vec<ReviewIssue>>
```

Change to accept a float composite directly (simpler — the critique agent only uses the composite):

```rust
pub async fn critique(
    ctx: &AgentContext<'_>,
    composite: &f32,
) -> anyhow::Result<Vec<ReviewIssue>>
```

And update `build_critique_prompt` call inside:

```rust
fn build_critique_prompt(diff: &str, composite: f32) -> String {
    format!(
        "CRITIQUE this diff. Composite score is {:.1}. Identify concrete issues.\n\
         Respond with JSON array only:\n\
         [{{\"location\":\"file:line\",\"problem\":\"description\",\"fix\":\"suggestion\"}}]\n\
         Return empty array [] if no issues found.\n\nDIFF:\n{}",
        composite,
        diff
    )
}
```

Update `critique()` body:

```rust
pub async fn critique(
    ctx: &AgentContext<'_>,
    composite: &f32,
) -> anyhow::Result<Vec<ReviewIssue>> {
    let prompt = build_critique_prompt(&ctx.diff.0, *composite);
    let output = ctx.agent.run(&prompt).await?;
    parse_issues_response(output.text())
}
```

- [ ] **Step 3: Full compile check**

```bash
cd rust && cargo build -p falanx-engine 2>&1 | grep "^error"
```

Expected: no errors in falanx-engine. Possible errors in `falanx` binary (main.rs) — those are fixed in Task 20.

- [ ] **Step 4: Run orchestrator pipeline tests**

```bash
cd rust && cargo test -p falanx-engine orchestrator
```

Expected: `pipeline_runs_dry_run_and_returns_result` PASS (uses MockProvider, ScoringResult now returned).

- [ ] **Step 5: Run all falanx-engine tests**

```bash
cd rust && cargo test -p falanx-engine
```

Expected: all tests pass. Note: the pipeline test uses `max_iter = 1` so the loop runs once with `quick` pipeline scoring.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx-engine/src/orchestrator/pipeline.rs \
        rust/crates/falanx-engine/src/agents/review.rs
git commit -m "feat(orchestrator): use new score() signature throughout pipeline"
```

---

## Task 20: Update main.rs — cmd_score with new types

**Files:**
- Modify: `rust/crates/falanx/src/main.rs`

- [ ] **Step 1: Update cmd_score imports and body**

In `rust/crates/falanx/src/main.rs`, find `cmd_score` and replace its full body:

```rust
async fn cmd_score(args: ScoreArgs) -> anyhow::Result<()> {
    let mut cfg = FalanxConfig::from_env()?;
    if args.common.dry_run {
        cfg.provider.dry_run = true;
    }
    cfg.validate()?;

    let target = resolve_target(args.file, args.diff)?;
    let label = target_label(&target);

    let session = Session::new(&cfg.session.dir, &label)?;
    session.append(SessionEvent::RunStarted {
        session_id: session.id().0.clone(),
        target: label.clone(),
    })?;
    info!(session_id = %session.id().0, target = %label, "run started");

    let diff = target.extract_diff()?;
    info!(bytes = diff.0.len(), "diff extracted");

    let scoring_cfg = falanx_engine::agents::quality::ScoringConfig::load()?;

    session.append(SessionEvent::AgentInvoked {
        agent: "quality".into(),
        iteration: 0,
    })?;
    info!(agent = "quality", pipeline = %scoring_cfg.pipeline.name, iteration = 0, "scoring started");

    let score = falanx_engine::agents::quality::score(&diff, &cfg, &scoring_cfg, &session, 0).await?;

    session.append(SessionEvent::RunCompleted {
        final_score: score.clone(),
        iterations: 1,
    })?;
    info!(composite = score.composite(), iterations = 1, "run completed");

    for cat in &score.categories {
        tracing::info!(
            category = %cat.name,
            score = cat.score,
            reasoning = %cat.reasoning,
            "category score"
        );
    }
    tracing::info!(
        composite = score.composite(),
        synthesis = %score.synthesis,
        pipeline = %score.pipeline_name,
        session = %session.path().display(),
        "score report"
    );

    Ok(())
}
```

- [ ] **Step 2: Update imports in main.rs**

Ensure the import block includes `ScoringResult` is not needed directly — it's used implicitly. The `Session`, `SessionEvent` imports stay. Remove any direct reference to `ReviewScore`. The import block should include:

```rust
use falanx_engine::{
    agents::{self, AgentContext},
    config::FalanxConfig,
    git::ReviewTarget,
    provider::MockProvider,
    session::{Session, SessionEvent},
};
```

Remove `MockProvider` and `AgentContext` from the import if `cmd_score` no longer uses them directly. Check `cmd_review` still uses them via the orchestrator — it doesn't import them directly either. Clean up unused imports:

```bash
cd rust && cargo build -p falanx 2>&1 | grep "warning\|error"
```

Remove any `unused import` warnings.

- [ ] **Step 3: Full build**

```bash
cd rust && cargo build 2>&1 | grep "^error"
```

Expected: no errors.

- [ ] **Step 4: Run all tests**

```bash
cd rust && cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx/src/main.rs
git commit -m "feat(cli): update cmd_score for new scoring pipeline output"
```

---

## Task 21: Smoke test — dry run end-to-end

- [ ] **Step 1: Create a test file**

```bash
echo 'fn add(a: i32, b: i32) -> i32 { a + b }' > /tmp/test_score.rs
```

- [ ] **Step 2: Run falanx score with dry-run**

```bash
cd rust && FALANX_DRY_RUN=true OPENCODE_API_KEY=test RUST_LOG=info \
  cargo run --bin falanx -- score --file /tmp/test_score.rs
```

Expected output (tracing info lines):
```
INFO run started session_id=... target=test_score.rs
INFO diff extracted bytes=42
INFO scoring started pipeline=default iteration=0
INFO category scored category=readability score=3 iteration=0
INFO category scored category=maintainability score=3 iteration=0
INFO category scored category=architecture score=3 iteration=0
INFO category scored category=performance score=3 iteration=0
INFO category scored category=security score=3 iteration=0
INFO scoring pipeline complete composite=3.0 pipeline=default
INFO run completed composite=3.0 iterations=1
INFO category score category=readability score=3 reasoning=mock category reasoning
...
INFO score report composite=3.0 synthesis=mock synthesis summary pipeline=default
```

- [ ] **Step 3: Run falanx review with dry-run**

```bash
cd rust && FALANX_DRY_RUN=true OPENCODE_API_KEY=test RUST_LOG=info \
  cargo run --bin falanx -- review --file /tmp/test_score.rs --max-iter 1
```

Expected: exits 0, logs show category-by-category scoring, then critique, then review complete.

- [ ] **Step 4: Run falanx list-sessions**

```bash
cd rust && FALANX_DRY_RUN=true OPENCODE_API_KEY=test \
  cargo run --bin falanx -- list-sessions
```

Expected: table shows session(s) with composite score and iteration count.

- [ ] **Step 5: Inspect JSONL session file**

```bash
cat ~/.falanx/sessions/test_score_rs/*.jsonl | python3 -m json.tool --no-ensure-ascii 2>/dev/null | grep '"type"'
```

Expected event sequence:
```
"type": "run_started"
"type": "agent_invoked"
"type": "category_scored"    (x5 for default pipeline)
"type": "scoring_complete"
"type": "run_completed"
```

- [ ] **Step 6: Run with quick pipeline**

```bash
cd rust && FALANX_DRY_RUN=true OPENCODE_API_KEY=test FALANX_SCORING_PIPELINE=quick RUST_LOG=info \
  cargo run --bin falanx -- score --file /tmp/test_score.rs
```

Expected: only 2 category_scored events (readability + maintainability).

- [ ] **Step 7: Final test suite run**

```bash
cd rust && cargo test
```

Expected: all tests pass, no regressions.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "test: smoke test validation — scoring pipeline end-to-end"
```

---

## Task 22: Test with live Ollama model

- [ ] **Step 1: Run falanx score with live model**

```bash
cd rust && FALANX_MODEL="ollama/qwen2.5-14b:latest" OPENCODE_API_KEY="ollama" \
  FALANX_SCORING_PIPELINE=quick RUST_LOG=info \
  cargo run --bin falanx -- score --file crates/falanx/src/main.rs
```

Expected:
- Two category agents fire (readability + maintainability)
- Real scores (not all 3s)
- Real reasoning text
- Synthesis agent fires and returns a holistic composite
- Session JSONL written with real data

- [ ] **Step 2: Verify session JSONL has real reasoning**

```bash
cat ~/.falanx/sessions/main_rs/*.jsonl | tail -20
```

Expected: `category_scored` events with non-empty `reasoning` field, `scoring_complete` with a real `synthesis` string.

- [ ] **Step 3: Commit if live test passes**

```bash
git add docs/superpowers/plans/2026-05-27-scoring-pipeline-redesign.md
git commit -m "docs(plans): scoring pipeline redesign implementation plan"
```
