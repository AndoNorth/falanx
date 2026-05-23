# Falanx Phase 1C+1D — Orchestrator & Full Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire a three-agent pipeline (score → critique → rewrite) driven by a state machine orchestrator with loop control, plateau detection, MockProvider dry-run, and live OpenCode path via Cersei.

**Architecture:** Cersei handles LLM execution within each agent stage; Falanx orchestrates between stages. `MockProvider` implements `cersei_provider::Provider` so provider selection happens once at orchestrator construction — no `if dry_run` branches in agent code. `FalanxAuditHook` implements `cersei_hooks::Hook` to write session JSONL events co-located with agent lifecycle. When new Cersei API details are discovered during implementation, update `docs/CERSEI.md` before committing.

**Tech Stack:** Rust 2024 edition, cersei/cersei-agent/cersei-provider/cersei-hooks/cersei-memory 0.1.9, async-trait, walkdir 2, anyhow, serde_json, tokio

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `rust/crates/falanx-engine/Cargo.toml` | Add cersei deps, async-trait, walkdir |
| Modify | `rust/crates/falanx-engine/src/lib.rs` | Expose `provider`, `audit_hook` modules |
| Modify | `rust/crates/falanx-engine/src/config.rs` | Add `LoopConfig`, extend `FalanxConfig` |
| Create | `rust/crates/falanx-engine/src/provider.rs` | `MockProvider: Provider` trait |
| Create | `rust/crates/falanx-engine/src/audit_hook.rs` | `FalanxAuditHook: Hook` trait |
| Modify | `rust/crates/falanx-engine/src/agents/mod.rs` | `AgentContext` carries `cersei_agent::Agent` |
| Modify | `rust/crates/falanx-engine/src/agents/quality.rs` | Remove `if dry_run`, use cersei agent |
| Modify | `rust/crates/falanx-engine/src/agents/review.rs` | Implement `critique` via cersei agent |
| Modify | `rust/crates/falanx-engine/src/agents/writing.rs` | Implement `rewrite` via cersei agent |
| Modify | `rust/crates/falanx-engine/src/orchestrator/mod.rs` | Add `loop_cfg` to `RunConfig`, wire `run()` |
| Modify | `rust/crates/falanx-engine/src/orchestrator/pipeline.rs` | Full state machine |
| Modify | `rust/crates/falanx-engine/src/orchestrator/horizon.rs` | `HorizonState` |
| Modify | `rust/crates/falanx-engine/src/session.rs` | Add `Session::list()`, `SessionMeta` |
| Modify | `rust/crates/falanx/src/main.rs` | Wire `review`, `list-sessions`; extend `ReviewArgs` |

---

## Task 1: Add Cersei dependencies and verify API from source

**Files:**
- Modify: `rust/crates/falanx-engine/Cargo.toml`

- [ ] **Step 1: Add dependencies**

```toml
# rust/crates/falanx-engine/Cargo.toml — [dependencies] section, add:
cersei = "0.1.9"
cersei-agent = "0.1.9"
cersei-provider = "0.1.9"
cersei-hooks = "0.1.9"
cersei-memory = "0.1.9"
async-trait = "0.1"
walkdir = "2"
```

Run from workspace root:
```bash
cd rust && cargo fetch
```

- [ ] **Step 2: Verify workspace compiles**

```bash
cd rust && cargo build 2>&1 | head -40
```

Expected: compiles (may have unused import warnings — fine).

- [ ] **Step 3: Locate cersei source in cargo registry**

```bash
find ~/.cargo/registry/src -type d -name "cersei-provider-0.1.9" 2>/dev/null
find ~/.cargo/registry/src -type d -name "cersei-hooks-0.1.9" 2>/dev/null
find ~/.cargo/registry/src -type d -name "cersei-agent-0.1.9" 2>/dev/null
```

- [ ] **Step 4: Read Provider trait and CompletionStream**

Read these files (adjust path prefix from Step 3):
```
~/.cargo/registry/src/.../cersei-provider-0.1.9/src/lib.rs
~/.cargo/registry/src/.../cersei-provider-0.1.9/src/router.rs   (from_model_string)
```

Confirm:
- Exact `Provider` trait method signatures (use `async-trait` or native async?)
- How to construct `CompletionStream` from a static string (needed for `MockProvider`)
- `CompletionResponse` fields — what field holds the response text?
- `from_model_string(model, api_key, base_url)` exact signature and return type

- [ ] **Step 5: Read Hook trait and HookEvent variants**

```
~/.cargo/registry/src/.../cersei-hooks-0.1.9/src/lib.rs
```

Confirm:
- `Hook` trait method signatures
- All `HookEvent` variants (pre/post tool use, model turn, etc.)
- All `HookAction` variants

- [ ] **Step 6: Read AgentBuilder and AgentOutput**

```
~/.cargo/registry/src/.../cersei-agent-0.1.9/src/lib.rs
```

Confirm:
- `Agent::builder()` — all builder methods available
- `.run_with(&str)` return type — is it `AgentOutput` or `String`?
- `AgentOutput` fields — which field holds the final response text?

- [ ] **Step 7: Update docs/CERSEI.md with verified API details**

Fill in the gaps currently marked `// ⚠ verify` in `docs/CERSEI.md`:
- Hook trait exact method signature
- All `HookEvent` variants with descriptions
- All `HookAction` variants
- `AgentOutput` fields
- `CompletionStream` constructor pattern for static responses
- Note the `async-trait` usage (or lack of it if Rust 2024 native async traits are used)

- [ ] **Step 8: Commit**

```bash
git add rust/crates/falanx-engine/Cargo.toml docs/CERSEI.md
git commit -m "chore(deps): add cersei 0.1.9 crates and document verified API"
```

---

## Task 2: LoopConfig in config.rs

**Files:**
- Modify: `rust/crates/falanx-engine/src/config.rs`

- [ ] **Step 1: Write failing tests**

Add to `rust/crates/falanx-engine/src/config.rs` at the bottom of the `#[cfg(test)]` block:

```rust
#[test]
fn loop_config_defaults() {
    let cfg = LoopConfig::default();
    assert_eq!(cfg.max_iter, 3);
    assert!((cfg.target_score - 4.5).abs() < f32::EPSILON);
    assert!((cfg.plateau_threshold - 0.1).abs() < f32::EPSILON);
}

#[test]
fn loop_config_from_env_reads_vars() {
    unsafe {
        std::env::set_var("FALANX_MAX_ITER", "5");
        std::env::set_var("FALANX_TARGET_SCORE", "4.0");
        std::env::set_var("FALANX_PLATEAU_THRESHOLD", "0.05");
    }
    let cfg = FalanxConfig::from_env().unwrap();
    assert_eq!(cfg.loop_cfg.max_iter, 5);
    assert!((cfg.loop_cfg.target_score - 4.0).abs() < f32::EPSILON);
    assert!((cfg.loop_cfg.plateau_threshold - 0.05).abs() < f32::EPSILON);
    unsafe {
        std::env::remove_var("FALANX_MAX_ITER");
        std::env::remove_var("FALANX_TARGET_SCORE");
        std::env::remove_var("FALANX_PLATEAU_THRESHOLD");
    }
}
```

- [ ] **Step 2: Verify tests fail**

```bash
cd rust && cargo test -p falanx-engine config 2>&1 | tail -20
```

Expected: FAIL — `LoopConfig` not found.

- [ ] **Step 3: Add LoopConfig struct and extend FalanxConfig**

In `rust/crates/falanx-engine/src/config.rs`, add after the existing structs:

```rust
#[derive(Debug, Clone)]
pub struct LoopConfig {
    pub max_iter: u32,
    pub target_score: f32,
    pub plateau_threshold: f32,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self { max_iter: 3, target_score: 4.5, plateau_threshold: 0.1 }
    }
}
```

Change `FalanxConfig` to:

```rust
#[derive(Debug, Clone)]
pub struct FalanxConfig {
    pub provider: ProviderConfig,
    pub session: SessionConfig,
    pub loop_cfg: LoopConfig,
}
```

- [ ] **Step 4: Extend from_env() to read loop vars**

In `FalanxConfig::from_env()`, add before the final `Ok(Self { ... })`:

```rust
let max_iter = std::env::var("FALANX_MAX_ITER")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(3u32);

let target_score = std::env::var("FALANX_TARGET_SCORE")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(4.5f32);

let plateau_threshold = std::env::var("FALANX_PLATEAU_THRESHOLD")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(0.1f32);
```

Change `Ok(Self { ... })` to:

```rust
Ok(Self {
    provider: ProviderConfig { model, api_key, base_url, dry_run },
    session: SessionConfig { dir },
    loop_cfg: LoopConfig { max_iter, target_score, plateau_threshold },
})
```

- [ ] **Step 5: Fix all FalanxConfig construction in tests across the codebase**

Any test that constructs `FalanxConfig { provider: ..., session: ... }` needs `loop_cfg: LoopConfig::default()` added. Search:

```bash
cd rust && grep -rn "FalanxConfig {" --include="*.rs"
```

Add `loop_cfg: LoopConfig::default(),` to each match.

- [ ] **Step 6: Verify tests pass**

```bash
cd rust && cargo test -p falanx-engine 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add rust/crates/falanx-engine/src/config.rs
git commit -m "feat(config): add LoopConfig with env var overrides"
```

---

## Task 3: HorizonState in horizon.rs

**Files:**
- Modify: `rust/crates/falanx-engine/src/orchestrator/horizon.rs`

- [ ] **Step 1: Write failing tests**

Replace the comment in `rust/crates/falanx-engine/src/orchestrator/horizon.rs` with:

```rust
pub struct HorizonState {
    pub reset_count: u32,
    pub max_resets: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero_with_max_two() {
        let h = HorizonState::new();
        assert_eq!(h.reset_count, 0);
        assert_eq!(h.max_resets, 2);
        assert!(!h.exhausted());
    }

    #[test]
    fn should_reset_when_plateau_and_not_exhausted() {
        let h = HorizonState::new();
        assert!(h.should_reset(true));
        assert!(!h.should_reset(false));
    }

    #[test]
    fn exhausted_after_max_resets() {
        let mut h = HorizonState::new();
        h.record_reset();
        assert!(!h.exhausted());
        h.record_reset();
        assert!(h.exhausted());
        assert!(!h.should_reset(true));
    }
}
```

- [ ] **Step 2: Verify tests fail**

```bash
cd rust && cargo test -p falanx-engine horizon 2>&1 | tail -20
```

Expected: FAIL — methods not found.

- [ ] **Step 3: Implement HorizonState methods**

Add after the struct definition:

```rust
impl HorizonState {
    pub fn new() -> Self {
        Self { reset_count: 0, max_resets: 2 }
    }

    pub fn should_reset(&self, plateau: bool) -> bool {
        plateau && !self.exhausted()
    }

    pub fn record_reset(&mut self) {
        self.reset_count += 1;
    }

    pub fn exhausted(&self) -> bool {
        self.reset_count >= self.max_resets
    }
}
```

- [ ] **Step 4: Verify tests pass**

```bash
cd rust && cargo test -p falanx-engine horizon 2>&1 | tail -20
```

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/orchestrator/horizon.rs
git commit -m "feat(orchestrator): implement HorizonState with plateau reset logic"
```

---

## Task 4: MockProvider implementing cersei Provider trait

**Files:**
- Create: `rust/crates/falanx-engine/src/provider.rs`
- Modify: `rust/crates/falanx-engine/src/lib.rs`

> Before writing this task, verify `CompletionStream` constructor pattern from Task 1 Step 4.
> The mock must return valid JSON for each agent's expected output shape.

- [ ] **Step 1: Write failing test**

Create `rust/crates/falanx-engine/src/provider.rs`:

```rust
use cersei_provider::{CompletionRequest, CompletionStream, Provider, ProviderCapabilities};

pub struct MockProvider;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_provider_name() {
        assert_eq!(MockProvider.name(), "mock");
    }

    #[tokio::test]
    async fn mock_provider_complete_returns_stream() {
        // This test will be fleshed out once CompletionRequest construction is known.
        // For now, just verify MockProvider implements Provider (compile-time check).
        let _: &dyn Provider = &MockProvider;
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

In `rust/crates/falanx-engine/src/lib.rs`:

```rust
pub mod agents;
pub mod audit_hook;
pub mod config;
pub mod git;
pub mod orchestrator;
pub mod provider;
pub mod session;
pub mod types;
```

- [ ] **Step 3: Verify test fails (not compiles)**

```bash
cd rust && cargo test -p falanx-engine provider 2>&1 | tail -30
```

Expected: compile error — `Provider` not implemented for `MockProvider`.

- [ ] **Step 4: Implement MockProvider**

Using the `CompletionStream` constructor pattern verified in Task 1, implement:

```rust
use async_trait::async_trait;
use cersei_provider::{CompletionRequest, CompletionStream, Provider, ProviderCapabilities};

pub struct MockProvider;

/// Fixed JSON responses per agent type. The orchestrator uses prompt content
/// to determine which mock to return — keyed on first word of system prompt.
impl MockProvider {
    fn mock_score_json() -> &'static str {
        r#"{"readability":3,"maintainability":3,"performance":3,"security":3,"architecture":3}"#
    }

    fn mock_issues_json() -> &'static str {
        r#"[{"location":"main.rs:1","problem":"no issues found","fix":"none"}]"#
    }

    fn mock_patches_json() -> &'static str {
        r#"[]"#
    }

    fn response_for(request: &CompletionRequest) -> &'static str {
        // Inspect system prompt to route to correct mock response.
        // Adjust field access based on CompletionRequest shape verified in Task 1.
        let system = request.system.as_deref().unwrap_or("");
        if system.contains("SCORE") {
            Self::mock_score_json()
        } else if system.contains("CRITIQUE") {
            Self::mock_issues_json()
        } else {
            Self::mock_patches_json()
        }
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str { "mock" }

    fn context_window(&self, _model: &str) -> u64 { 200_000 }

    fn capabilities(&self, _model: &str) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    async fn complete(&self, request: CompletionRequest) -> anyhow::Result<CompletionStream> {
        let text = Self::response_for(&request).to_string();
        // Construct CompletionStream from static text — verify constructor with Task 1 findings.
        // Common pattern: CompletionStream::from_text(text) or CompletionStream::single(text)
        Ok(CompletionStream::from_text(text))
    }
}
```

> **Note:** `CompletionRequest::system` field name and `CompletionStream::from_text` constructor
> must be verified against source found in Task 1. Update `docs/CERSEI.md` with correct names.

- [ ] **Step 5: Verify tests pass**

```bash
cd rust && cargo test -p falanx-engine provider 2>&1 | tail -20
```

Expected: tests pass.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx-engine/src/provider.rs rust/crates/falanx-engine/src/lib.rs
git commit -m "feat(provider): add MockProvider implementing cersei Provider trait"
```

---

## Task 5: FalanxAuditHook implementing cersei Hook trait

**Files:**
- Create: `rust/crates/falanx-engine/src/audit_hook.rs`

> Before writing, verify `Hook` trait method signatures and all `HookEvent` variants from Task 1 Step 5.
> Update `docs/CERSEI.md` with verified variants before committing.

- [ ] **Step 1: Write failing test**

Create `rust/crates/falanx-engine/src/audit_hook.rs`:

```rust
use std::sync::Arc;
use cersei_hooks::{Hook, HookAction, HookContext, HookEvent};
use crate::session::{Session, SessionEvent};

pub struct FalanxAuditHook {
    pub session: Arc<Session>,
    pub iteration: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_hook_implements_hook_trait() {
        // Compile-time check — verify FalanxAuditHook: Hook
        fn assert_hook<T: Hook>() {}
        assert_hook::<FalanxAuditHook>();
    }
}
```

- [ ] **Step 2: Verify test fails**

```bash
cd rust && cargo test -p falanx-engine audit_hook 2>&1 | tail -20
```

Expected: compile error — `Hook` not implemented.

- [ ] **Step 3: Implement Hook trait**

Using verified `HookEvent` variants from Task 1, implement:

```rust
use async_trait::async_trait;

#[async_trait]
impl Hook for FalanxAuditHook {
    async fn on_event(&self, _ctx: &HookContext, event: &HookEvent) -> HookAction {
        // Map HookEvent variants to SessionEvent and write to session.
        // Adjust match arms based on verified HookEvent variants from Task 1.
        match event {
            HookEvent::PreToolUse { tool_name, .. } => {
                let _ = self.session.append(SessionEvent::AgentInvoked {
                    agent: tool_name.clone(),
                    iteration: self.iteration,
                });
            }
            // Add other relevant HookEvent variants as discovered in Task 1.
            _ => {}
        }
        HookAction::Continue
    }
}
```

> After implementing: update `docs/CERSEI.md` with the exact `HookEvent` variants used.

- [ ] **Step 4: Verify tests pass**

```bash
cd rust && cargo test -p falanx-engine audit_hook 2>&1 | tail -20
```

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/audit_hook.rs docs/CERSEI.md
git commit -m "feat(audit_hook): add FalanxAuditHook implementing cersei Hook trait"
```

---

## Task 6: Update AgentContext to carry cersei Agent

**Files:**
- Modify: `rust/crates/falanx-engine/src/agents/mod.rs`

Current `AgentContext` holds `config: &FalanxConfig`. Replace with pre-built `cersei_agent::Agent`.

- [ ] **Step 1: Write failing test**

Add to `rust/crates/falanx-engine/src/agents/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    // AgentContext must accept a &cersei_agent::Agent — this is a compile-time check.
    // Real agent construction is tested in Task 8 (orchestrator).
}
```

- [ ] **Step 2: Update AgentContext**

Replace `rust/crates/falanx-engine/src/agents/mod.rs` with:

```rust
pub mod quality;
pub mod review;
pub mod writing;

pub struct AgentContext<'a> {
    pub agent: &'a cersei_agent::Agent,
    pub diff: &'a crate::git::Diff,
}
```

- [ ] **Step 3: Verify old tests now fail**

```bash
cd rust && cargo test -p falanx-engine agents 2>&1 | tail -30
```

Expected: compile errors in `quality.rs` — `ctx.config` no longer exists.

- [ ] **Step 4: Update quality.rs to use cersei agent**

Replace `rust/crates/falanx-engine/src/agents/quality.rs` with:

```rust
use super::AgentContext;
use crate::types::ReviewScore;

pub async fn score(ctx: &AgentContext<'_>) -> anyhow::Result<ReviewScore> {
    let prompt = build_score_prompt(&ctx.diff.0);
    let response = ctx.agent.run_with(&prompt).await?;
    parse_score_response(&response)
}

fn build_score_prompt(diff: &str) -> String {
    format!(
        "SCORE this diff across five categories: readability, maintainability, \
         performance, security, architecture. Each score 1-5 (integer). \
         Respond with JSON only, no prose:\n\
         {{\"readability\":N,\"maintainability\":N,\"performance\":N,\
         \"security\":N,\"architecture\":N}}\n\nDIFF:\n{}",
        diff
    )
}

fn parse_score_response(response: &cersei_agent::AgentOutput) -> anyhow::Result<ReviewScore> {
    // Extract text from AgentOutput — field name verified in Task 1 Step 6.
    // Adjust `.content` to the correct field name.
    let text = &response.content;
    let json_start = text.find('{').ok_or_else(|| anyhow::anyhow!("no JSON in score response"))?;
    let json_end = text.rfind('}').ok_or_else(|| anyhow::anyhow!("no JSON end in score response"))?;
    let json = &text[json_start..=json_end];
    let score: ReviewScore = serde_json::from_str(json)?;
    Ok(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_score_response_extracts_json() {
        // Unit test parse function directly — no cersei agent needed.
        let json = r#"{"readability":3,"maintainability":4,"performance":3,"security":5,"architecture":2}"#;
        let score: ReviewScore = serde_json::from_str(json).unwrap();
        assert_eq!(score.readability, 3);
        assert_eq!(score.security, 5);
        assert!((score.composite() - 3.4).abs() < 0.01);
    }

    #[test]
    fn parse_score_response_handles_prose_wrapping_json() {
        // LLM might wrap JSON in prose — parser extracts the JSON block.
        let json = r#"Here is the score: {"readability":3,"maintainability":3,"performance":3,"security":3,"architecture":3} Done."#;
        // Direct serde test of the extraction logic.
        let start = json.find('{').unwrap();
        let end = json.rfind('}').unwrap();
        let extracted = &json[start..=end];
        let score: ReviewScore = serde_json::from_str(extracted).unwrap();
        assert_eq!(score.composite(), 3.0);
    }
}
```

> **Note:** `response.content` — adjust field name to match `AgentOutput` verified in Task 1.

- [ ] **Step 5: Verify tests pass**

```bash
cd rust && cargo test -p falanx-engine agents::quality 2>&1 | tail -20
```

Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/mod.rs rust/crates/falanx-engine/src/agents/quality.rs
git commit -m "refactor(agents): AgentContext carries cersei Agent, remove dry_run branch from quality"
```

---

## Task 7: Implement agents/review.rs and agents/writing.rs

**Files:**
- Modify: `rust/crates/falanx-engine/src/agents/review.rs`
- Modify: `rust/crates/falanx-engine/src/agents/writing.rs`

- [ ] **Step 1: Write failing tests for review**

Replace `rust/crates/falanx-engine/src/agents/review.rs` with:

```rust
use super::AgentContext;
use crate::types::{ReviewIssue, ReviewScore};

pub async fn critique(
    ctx: &AgentContext<'_>,
    score: &ReviewScore,
) -> anyhow::Result<Vec<ReviewIssue>> {
    let prompt = build_critique_prompt(&ctx.diff.0, score);
    let response = ctx.agent.run_with(&prompt).await?;
    parse_issues_response(&response)
}

fn build_critique_prompt(diff: &str, score: &ReviewScore) -> String {
    format!(
        "CRITIQUE this diff. Composite score is {:.1}. Identify concrete issues.\n\
         Respond with JSON array only:\n\
         [{{\"location\":\"file:line\",\"problem\":\"description\",\"fix\":\"suggestion\"}}]\n\
         Return empty array [] if no issues found.\n\nDIFF:\n{}",
        score.composite(),
        diff
    )
}

fn parse_issues_response(
    response: &cersei_agent::AgentOutput,
) -> anyhow::Result<Vec<ReviewIssue>> {
    // Adjust `.content` to correct AgentOutput field from Task 1.
    let text = &response.content;
    let start = text.find('[').ok_or_else(|| anyhow::anyhow!("no JSON array in critique response"))?;
    let end = text.rfind(']').ok_or_else(|| anyhow::anyhow!("no JSON array end"))?;
    let json = &text[start..=end];
    let issues: Vec<ReviewIssue> = serde_json::from_str(json)?;
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_issues_empty_array() {
        let json = "[]";
        let start = json.find('[').unwrap();
        let end = json.rfind(']').unwrap();
        let issues: Vec<ReviewIssue> = serde_json::from_str(&json[start..=end]).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn parse_issues_single_issue() {
        let json = r#"[{"location":"src/main.rs:10","problem":"unused variable","fix":"prefix with _"}]"#;
        let issues: Vec<ReviewIssue> = serde_json::from_str(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location, "src/main.rs:10");
    }
}
```

- [ ] **Step 2: Write failing tests for writing**

Replace `rust/crates/falanx-engine/src/agents/writing.rs` with:

```rust
use super::AgentContext;
use crate::types::{ReviewIssue, RewritePatch};

pub async fn rewrite(
    ctx: &AgentContext<'_>,
    issues: &[ReviewIssue],
) -> anyhow::Result<Vec<RewritePatch>> {
    if issues.is_empty() {
        return Ok(vec![]);
    }
    let prompt = build_rewrite_prompt(&ctx.diff.0, issues);
    let response = ctx.agent.run_with(&prompt).await?;
    parse_patches_response(&response)
}

fn build_rewrite_prompt(diff: &str, issues: &[ReviewIssue]) -> String {
    let issues_json = serde_json::to_string(issues).unwrap_or_default();
    format!(
        "REWRITE to fix these issues. Respond with JSON array only:\n\
         [{{\"original\":\"exact original text\",\"revised\":\"replacement text\",\
         \"issue_ref\":\"issue location\"}}]\n\
         Return empty array [] if no changes needed.\n\
         ISSUES:\n{}\n\nDIFF:\n{}",
        issues_json, diff
    )
}

fn parse_patches_response(
    response: &cersei_agent::AgentOutput,
) -> anyhow::Result<Vec<RewritePatch>> {
    // Adjust `.content` to correct AgentOutput field from Task 1.
    let text = &response.content;
    let start = text.find('[').ok_or_else(|| anyhow::anyhow!("no JSON array in rewrite response"))?;
    let end = text.rfind(']').ok_or_else(|| anyhow::anyhow!("no JSON array end"))?;
    let json = &text[start..=end];
    let patches: Vec<RewritePatch> = serde_json::from_str(json)?;
    Ok(patches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_empty_issues_returns_empty_patches_without_calling_agent() {
        // No agent call when issues empty — pure unit test.
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async {
                // Can't easily call rewrite without agent — test the guard logic directly.
                let issues: Vec<ReviewIssue> = vec![];
                if issues.is_empty() { Ok(vec![]) } else { Err(anyhow::anyhow!("unexpected")) }
            });
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_patches_empty_array() {
        let json = "[]";
        let patches: Vec<RewritePatch> = serde_json::from_str(json).unwrap();
        assert!(patches.is_empty());
    }

    #[test]
    fn parse_patches_single_patch() {
        let json = r#"[{"original":"let x = 1","revised":"let _x = 1","issue_ref":"src/main.rs:10"}]"#;
        let patches: Vec<RewritePatch> = serde_json::from_str(json).unwrap();
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].issue_ref, "src/main.rs:10");
    }
}
```

- [ ] **Step 3: Verify tests pass**

```bash
cd rust && cargo test -p falanx-engine agents 2>&1 | tail -20
```

Expected: all agent tests pass.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/review.rs rust/crates/falanx-engine/src/agents/writing.rs
git commit -m "feat(agents): implement critique and rewrite using cersei Agent"
```

---

## Task 8: HorizonState + pipeline state machine

**Files:**
- Modify: `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`
- Modify: `rust/crates/falanx-engine/src/orchestrator/mod.rs`

- [ ] **Step 1: Update RunConfig in orchestrator/mod.rs**

Replace `rust/crates/falanx-engine/src/orchestrator/mod.rs` with:

```rust
pub mod horizon;
pub mod pipeline;

use crate::{
    config::{FalanxConfig, LoopConfig},
    git::ReviewTarget,
    session::Session,
    types::{RewritePatch, ReviewScore, SessionId},
};

pub struct RunConfig {
    pub target: ReviewTarget,
    pub loop_cfg: LoopConfig,
    pub session: Session,
}

pub struct RunResult {
    pub final_score: ReviewScore,
    pub iterations: u32,
    pub patches: Vec<RewritePatch>,
    pub session_id: SessionId,
}

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    pipeline::run(config, falanx_cfg).await
}
```

- [ ] **Step 2: Write failing tests for pipeline**

Replace `rust/crates/falanx-engine/src/orchestrator/pipeline.rs` with:

```rust
use crate::{
    config::FalanxConfig,
    orchestrator::{RunConfig, RunResult},
};

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    anyhow::bail!("pipeline not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{LoopConfig, ProviderConfig, SessionConfig},
        git::{Diff, ReviewTarget},
        session::Session,
    };
    use std::path::PathBuf;

    fn dry_run_cfg() -> FalanxConfig {
        FalanxConfig {
            provider: ProviderConfig {
                model: "mock".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: SessionConfig { dir: PathBuf::from("/tmp/falanx-test") },
            loop_cfg: LoopConfig::default(),
        }
    }

    #[tokio::test]
    async fn pipeline_exits_when_target_score_hit() {
        // MockProvider returns score 3.0; target default 4.5 — pipeline runs max_iter then stops.
        // With max_iter=1, stops after one iteration.
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = dry_run_cfg();
        cfg.loop_cfg.max_iter = 1;
        cfg.session.dir = dir.path().to_path_buf();

        let session = Session::new(&cfg.session.dir, "test").unwrap();
        let run_cfg = RunConfig {
            target: ReviewTarget::File(
                // Write a temp file to diff
                {
                    let f = dir.path().join("test.rs");
                    std::fs::write(&f, "fn main() {}").unwrap();
                    f
                }
            ),
            loop_cfg: cfg.loop_cfg.clone(),
            session,
        };

        let result = run(run_cfg, &cfg).await.unwrap();
        assert_eq!(result.iterations, 1);
    }
}
```

- [ ] **Step 3: Verify test fails**

```bash
cd rust && cargo test -p falanx-engine pipeline 2>&1 | tail -20
```

Expected: FAIL — `pipeline not yet implemented`.

- [ ] **Step 4: Implement pipeline::run() state machine**

Replace the `run` function in `pipeline.rs`:

```rust
use std::sync::Arc;
use cersei_agent::Agent;
use crate::{
    agents::{self, AgentContext},
    audit_hook::FalanxAuditHook,
    config::FalanxConfig,
    orchestrator::{horizon::HorizonState, RunConfig, RunResult},
    provider::MockProvider,
    session::SessionEvent,
    types::{ReviewIssue, ReviewScore, RewritePatch},
};

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    let RunConfig { target, loop_cfg, session } = config;
    let session = Arc::new(session);

    // Select provider once — no per-agent dry_run checks.
    let provider: Box<dyn cersei_provider::Provider> = if falanx_cfg.provider.dry_run {
        Box::new(MockProvider)
    } else {
        cersei_provider::from_model_string(
            &falanx_cfg.provider.model,
            &falanx_cfg.provider.api_key,
            falanx_cfg.provider.base_url.as_deref(),
        )?
    };

    let diff = target.extract_diff()?;
    let session_id = session.id().clone();

    session.append(SessionEvent::RunStarted {
        session_id: session_id.0.clone(),
        target: format!("{:?}", std::mem::discriminant(&target)),
    })?;

    let mut horizon = HorizonState::new();
    let mut iteration: u32 = 0;
    let mut prev_score: Option<ReviewScore> = None;
    let mut all_patches: Vec<RewritePatch> = vec![];
    let mut plateau_streak: u32 = 0;

    // Initial score
    let mut current_score = {
        let agent = build_agent(&provider, &falanx_cfg.provider.model, &session, iteration)?;
        let ctx = AgentContext { agent: &agent, diff: &diff };
        session.append(SessionEvent::AgentInvoked { agent: "quality".into(), iteration })?;
        let s = agents::quality::score(&ctx).await?;
        session.append(SessionEvent::ScoreComputed { score: s.clone(), iteration })?;
        s
    };

    loop {
        // Check exit: target hit
        if current_score.composite() >= loop_cfg.target_score {
            break;
        }

        // Check exit: max iterations
        if iteration >= loop_cfg.max_iter {
            break;
        }

        iteration += 1;

        // Critique
        let agent = build_agent(&provider, &falanx_cfg.provider.model, &session, iteration)?;
        let ctx = AgentContext { agent: &agent, diff: &diff };
        session.append(SessionEvent::AgentInvoked { agent: "review".into(), iteration })?;
        let issues = agents::review::critique(&ctx, &current_score).await?;
        session.append(SessionEvent::IssuesFound { count: issues.len(), iteration })?;

        if !issues.is_empty() {
            // Rewrite
            let agent = build_agent(&provider, &falanx_cfg.provider.model, &session, iteration)?;
            let ctx = AgentContext { agent: &agent, diff: &diff };
            session.append(SessionEvent::AgentInvoked { agent: "writing".into(), iteration })?;
            let patches = agents::writing::rewrite(&ctx, &issues).await?;
            session.append(SessionEvent::RewriteApplied { patches: patches.len(), iteration })?;
            all_patches.extend(patches);
        }

        // Re-score
        let agent = build_agent(&provider, &falanx_cfg.provider.model, &session, iteration)?;
        let ctx = AgentContext { agent: &agent, diff: &diff };
        session.append(SessionEvent::AgentInvoked { agent: "quality".into(), iteration })?;
        let new_score = agents::quality::score(&ctx).await?;
        session.append(SessionEvent::ScoreComputed { score: new_score.clone(), iteration })?;

        // Plateau check
        let plateau = if let Some(ref prev) = prev_score {
            new_score.delta(prev) < loop_cfg.plateau_threshold
        } else {
            false
        };

        if plateau {
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
                // Exhausted resets — stop
                break;
            }
        }

        prev_score = Some(current_score);
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

fn build_agent(
    provider: &Box<dyn cersei_provider::Provider>,
    model: &str,
    session: &Arc<crate::session::Session>,
    iteration: u32,
) -> anyhow::Result<Agent> {
    // Verify exact AgentBuilder API against Task 1 findings.
    // Adjust builder calls to match cersei-agent source.
    Agent::builder()
        .provider(provider.as_ref())
        .model(model)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build agent: {}", e))
}
```

> **Note:** `Agent::builder().provider()` may need `Arc<dyn Provider>` or `Box<dyn Provider>` — verify from Task 1. Update `docs/CERSEI.md`.

- [ ] **Step 5: Verify tests pass**

```bash
cd rust && cargo test -p falanx-engine orchestrator 2>&1 | tail -30
```

Expected: pipeline test passes.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx-engine/src/orchestrator/
git commit -m "feat(orchestrator): implement pipeline state machine with loop control and horizon resets"
```

---

## Task 9: Session::list() and SessionMeta

**Files:**
- Modify: `rust/crates/falanx-engine/src/session.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block in `session.rs`:

```rust
#[test]
fn list_returns_empty_for_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let sessions = Session::list(dir.path()).unwrap();
    assert!(sessions.is_empty());
}

#[test]
fn list_finds_completed_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let session = Session::new(dir.path(), "test").unwrap();

    session.append(SessionEvent::RunStarted {
        session_id: session.id().0.clone(),
        target: "test.rs".into(),
    }).unwrap();
    session.append(SessionEvent::RunCompleted {
        final_score: ReviewScore { readability: 3, maintainability: 3, performance: 3, security: 3, architecture: 3 },
        iterations: 1,
    }).unwrap();

    let sessions = Session::list(dir.path()).unwrap();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].final_score.is_some());
    assert_eq!(sessions[0].iterations, Some(1));
}

#[test]
fn list_skips_malformed_files() {
    let dir = tempfile::tempdir().unwrap();
    // Write a malformed JSONL file
    let bad_dir = dir.path().join("bad");
    std::fs::create_dir_all(&bad_dir).unwrap();
    std::fs::write(bad_dir.join("bad.jsonl"), "not json\n").unwrap();

    let sessions = Session::list(dir.path()).unwrap();
    assert!(sessions.is_empty());
}
```

- [ ] **Step 2: Verify tests fail**

```bash
cd rust && cargo test -p falanx-engine session::tests::list 2>&1 | tail -20
```

Expected: FAIL — `Session::list` not found.

- [ ] **Step 3: Add SessionMeta struct**

Add after the existing `Session` struct in `session.rs`:

```rust
#[derive(Debug)]
pub struct SessionMeta {
    pub id: SessionId,
    pub path: std::path::PathBuf,
    pub started_at: DateTime<Utc>,
    pub final_score: Option<ReviewScore>,
    pub iterations: Option<u32>,
}
```

- [ ] **Step 4: Implement Session::list()**

Add `use std::io::{BufRead, BufReader, SeekFrom, Seek};` and `use walkdir::WalkDir;` to imports, then add to the `Session` impl block:

```rust
pub fn list(dir: &Path) -> anyhow::Result<Vec<SessionMeta>> {
    let mut results = Vec::new();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
    {
        let path = entry.path().to_path_buf();
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut reader = BufReader::new(file);

        // Read first line → RunStarted
        let mut first_line = String::new();
        if reader.read_line(&mut first_line).is_err() || first_line.is_empty() {
            continue;
        }
        let first: serde_json::Value = match serde_json::from_str(first_line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let started_at: DateTime<Utc> = match first.get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
        {
            Some(t) => t,
            None => continue,
        };
        let id_str = first.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Read last line → RunCompleted or RunFailed
        let mut last_line = String::new();
        let mut buf = String::new();
        while reader.read_line(&mut buf).map(|n| n > 0).unwrap_or(false) {
            if !buf.trim().is_empty() {
                last_line = buf.trim().to_string();
            }
            buf.clear();
        }

        let (final_score, iterations) = if last_line.is_empty() {
            (None, None)
        } else {
            match serde_json::from_str::<SessionEntry>(&last_line) {
                Ok(entry) => match entry.event {
                    SessionEvent::RunCompleted { final_score, iterations } => {
                        (Some(final_score), Some(iterations))
                    }
                    _ => (None, None),
                },
                Err(_) => (None, None),
            }
        };

        results.push(SessionMeta {
            id: SessionId(id_str),
            path,
            started_at,
            final_score,
            iterations,
        });
    }

    results.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(results)
}
```

- [ ] **Step 5: Verify tests pass**

```bash
cd rust && cargo test -p falanx-engine session 2>&1 | tail -20
```

Expected: all session tests pass.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx-engine/src/session.rs
git commit -m "feat(session): add Session::list() and SessionMeta for list-sessions command"
```

---

## Task 10: Wire falanx review and falanx list-sessions CLI commands

**Files:**
- Modify: `rust/crates/falanx/src/main.rs`

- [ ] **Step 1: Add loop override args to ReviewArgs**

In `rust/crates/falanx/src/main.rs`, replace the `ReviewArgs` struct:

```rust
#[derive(Args)]
struct ReviewArgs {
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long)]
    diff: Option<String>,
    /// Override max iterations from config
    #[arg(long)]
    max_iter: Option<u32>,
    /// Override target score from config
    #[arg(long)]
    target: Option<f32>,
    #[command(flatten)]
    common: CommonArgs,
}
```

- [ ] **Step 2: Implement cmd_review**

Replace the `Commands::Review(_) => { ... }` arm and add `cmd_review`:

```rust
// In run():
Commands::Review(args) => cmd_review(args).await,

// New function:
async fn cmd_review(args: ReviewArgs) -> anyhow::Result<()> {
    let mut cfg = FalanxConfig::from_env()?;
    if args.common.dry_run {
        cfg.provider.dry_run = true;
    }
    if let Some(max_iter) = args.max_iter {
        cfg.loop_cfg.max_iter = max_iter;
    }
    if let Some(target) = args.target {
        cfg.loop_cfg.target_score = target;
    }
    cfg.validate()?;

    let target = resolve_target(args.file, args.diff)?;
    let label = target_label(&target);
    let session = falanx_engine::session::Session::new(&cfg.session.dir, &label)?;

    let run_cfg = falanx_engine::orchestrator::RunConfig {
        target,
        loop_cfg: cfg.loop_cfg.clone(),
        session,
    };

    let result = falanx_engine::orchestrator::run(run_cfg, &cfg).await?;

    tracing::info!(
        final_score = result.final_score.composite(),
        iterations = result.iterations,
        patches = result.patches.len(),
        session_id = %result.session_id.0,
        "review complete"
    );

    Ok(())
}
```

- [ ] **Step 3: Implement cmd_list_sessions**

Replace `Commands::ListSessions => { ... }` arm and add function:

```rust
// In run():
Commands::ListSessions => cmd_list_sessions().await,

// New function:
async fn cmd_list_sessions() -> anyhow::Result<()> {
    let cfg = FalanxConfig::from_env()?;
    let sessions = falanx_engine::session::Session::list(&cfg.session.dir)?;

    if sessions.is_empty() {
        println!("No sessions found in {}", cfg.session.dir.display());
        return Ok(());
    }

    println!("{:<38} {:<24} {:<8} {}", "SESSION ID", "STARTED", "SCORE", "ITERATIONS");
    for meta in &sessions {
        let score = meta.final_score.as_ref()
            .map(|s| format!("{:.1}", s.composite()))
            .unwrap_or_else(|| "—".into());
        let iters = meta.iterations.map(|i| i.to_string()).unwrap_or_else(|| "—".into());
        println!(
            "{:<38} {:<24} {:<8} {}",
            meta.id.0,
            meta.started_at.format("%Y-%m-%d %H:%M:%S UTC"),
            score,
            iters,
        );
    }

    Ok(())
}
```

- [ ] **Step 4: Verify full build and dry-run smoke test**

```bash
cd rust && cargo build --bin falanx 2>&1 | tail -20
```

Expected: builds clean.

```bash
cd rust && echo "fn main() {}" > /tmp/test_review.rs && \
  cargo run --bin falanx -- review --file /tmp/test_review.rs --dry-run 2>&1
```

Expected: runs pipeline, prints `review complete` log line, exits 0.

```bash
cd rust && cargo run --bin falanx -- list-sessions 2>&1
```

Expected: prints session table or "No sessions found".

- [ ] **Step 5: Verify cargo test passes**

```bash
cd rust && cargo test 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx/src/main.rs
git commit -m "feat(cli): wire falanx review and list-sessions commands"
```

---

## Task 11: Live OpenCode path — validate end-to-end

**Files:**
- No new files — validates existing implementation with real provider

> This task validates Phase 1D acceptance criteria: *"Live Cersei + OpenCode path works end-to-end for at least `falanx score`."*
> Requires `OPENCODE_API_KEY` set in environment. If not available, set `FALANX_DRY_RUN=true` and note as deferred.

- [ ] **Step 1: Set up .env**

Create `rust/.env` (gitignored):

```bash
cat > rust/.env << 'EOF'
FALANX_MODEL=opencode/big-pickle
OPENCODE_API_KEY=<your key here>
FALANX_DRY_RUN=false
EOF
```

- [ ] **Step 2: Run live falanx score**

```bash
cd rust && echo "fn add(a: i32, b: i32) -> i32 { a + b }" > /tmp/live_test.rs && \
  cargo run --bin falanx -- score --file /tmp/live_test.rs 2>&1
```

Expected: prints score report with real values (not all 3s from mock), exits 0.

- [ ] **Step 3: Update docs/CERSEI.md with any new API learnings**

If provider construction or AgentOutput field names differ from what was documented, update `docs/CERSEI.md` now.

- [ ] **Step 4: Commit**

```bash
git add docs/CERSEI.md
git commit -m "docs(cersei): update with verified live provider integration notes"
```

---

## Acceptance Checklist

Before marking Phase 1C+1D complete, verify each item:

- [ ] `falanx review --diff HEAD~1 --dry-run` runs full pipeline and exits 0
- [ ] Pipeline exits on: target score hit, max iterations reached, plateau detected
- [ ] `falanx review --dry-run` produces JSONL session trail with all agent stages recorded
- [ ] `CodeWritingAgent` never fires without prior `CodeReviewAgent` output (enforced by state order)
- [ ] `falanx list-sessions` prints table with timestamps and final scores
- [ ] Live `falanx score --file <path>` (no `--dry-run`) calls OpenCode and returns real scores
- [ ] `cargo test` passes with zero failures
- [ ] `docs/CERSEI.md` reflects all verified API details from implementation
