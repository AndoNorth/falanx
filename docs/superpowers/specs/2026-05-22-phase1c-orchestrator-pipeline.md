# Falanx Phase 1C+1D — Orchestrator & Full Pipeline

**Date:** 2026-05-22
**Status:** Ready — Phase 1A+1B complete

---

## Goal

A working three-agent pipeline driven by a state machine orchestrator — score → critique → rewrite — with loop control, plateau detection, and Cersei live provider integration, validated first in dry-run mode then against OpenCode.

## Acceptance Criteria

- `falanx review --diff HEAD~1 --dry-run` runs the full pipeline and exits cleanly
- Pipeline exits on: target score hit, max iterations reached, or plateau detected
- `falanx review --dry-run` produces a JSONL session trail with all agent stages recorded
- `CodeWritingAgent` never fires without `CodeReviewAgent` output — enforced by state machine order
- `falanx list-sessions` lists session files with timestamps and final scores
- Live Cersei + OpenCode path works end-to-end for at least `falanx score` (not gated on `--dry-run`)

## Prerequisites

Phase 1A+1B complete ✓:
- Two-crate workspace compiles clean
- `falanx score --dry-run` works and writes session JSONL
- `agents/quality.rs`, `session.rs`, `git.rs`, `config.rs`, `types.rs` all implemented
- `falanx review` currently stubs with `exit(1)` — to be wired here
- `falanx list-sessions` currently stubs with `exit(1)` — to be wired here

---

## Scope

Phase 1C: Cersei dependency added, `LoopConfig` added to `FalanxConfig`, orchestrator state machine implemented, `falanx review` wired end-to-end in dry-run mode.

Phase 1D: `CodeReviewAgent` + `CodeWritingAgent` dry-run implementations, `falanx list-sessions` implemented, live Cersei + OpenCode path validated for all three agents.

Serve mode (MCP, HTTP API) remains out of scope.

---

## Cersei Dependency & Trait Composition (Phase 1C first step)

Add to `rust/crates/falanx-engine/Cargo.toml`:

```toml
cersei = "0.1.9"
cersei-agent = "0.1.9"
cersei-provider = "0.1.9"
cersei-hooks = "0.1.9"
cersei-memory = "0.1.9"
```

**Explore before implementing.** Before writing any integration code, read Cersei source to confirm:
- `Provider` trait signature and how `from_model_string()` works
- `Agent::builder()` API and `run_with()` return type (`AgentOutput`)
- `Hook` trait signature and `HookEvent` variants
- `InMemory` backend API for test use
- Whether `cersei-memory::JsonlMemory` overlaps with Falanx's `Session` (they likely serve different layers — Cersei tracks message history, Falanx tracks run-level audit events)

### Trait Composition Points

Three places where Falanx should inherit Cersei behaviour rather than reimplement it:

#### 1. `MockProvider` implements `Provider` trait

The `if dry_run { return mock() }` branch in every agent is a smell — it's manual provider switching. Instead, implement `MockProvider` that satisfies Cersei's `Provider` trait and returns hardcoded deterministic responses. Provider selection happens once at construction time in the CLI/config layer.

```rust
// falanx-engine/src/provider.rs (new file)

use cersei_provider::Provider;

pub struct MockProvider;

// Implement cersei_provider::Provider for MockProvider
// returning fixed CompletionResponse with JSON matching each agent's expected output shape.
// Exact impl depends on Provider trait signature — read source first.
```

Agent functions become clean — zero `if dry_run` branches:

```rust
// agents/quality.rs — after Provider trait integration
pub async fn score(ctx: &AgentContext<'_>) -> anyhow::Result<ReviewScore> {
    let prompt = build_score_prompt(&ctx.diff.0);
    let response = ctx.agent.run_with(&prompt).await?;
    parse_score_response(&response)
}
```

`AgentContext` carries the pre-built Cersei agent (with provider already selected). Orchestrator constructs the agent with either `MockProvider` or the live provider based on `config.provider.dry_run` — once, not per-call.

#### 2. `FalanxAuditHook` implements `Hook` trait

Manual `session.append(AgentInvoked { ... })` calls scattered through the orchestrator should be a `FalanxAuditHook` implementing Cersei's `Hook` trait. The hook fires on `HookEvent` lifecycle events and writes to the session JSONL.

```rust
// falanx-engine/src/audit_hook.rs (new file)

use cersei_hooks::{Hook, HookContext, HookEvent, HookAction};
use crate::session::{Session, SessionEvent};

pub struct FalanxAuditHook {
    session: Arc<Session>,
    iteration: u32,
}

// Implement cersei_hooks::Hook for FalanxAuditHook.
// Map HookEvent variants to SessionEvent variants and call session.append().
// Exact HookEvent variants — confirm against cersei-hooks source before implementing.
```

The orchestrator registers `FalanxAuditHook` with the Cersei agent builder rather than manually appending events. This keeps session writing co-located with the agent lifecycle rather than spread through orchestrator logic.

#### 3. `InMemory` for tests

Use `cersei_memory::InMemory` for test fixtures instead of touching the filesystem. Agents under test get a Cersei agent backed by `MockProvider` + `InMemory` — fully in-process, no temp dirs needed.

---

## Config Additions (Phase 1C)

Add `LoopConfig` to `FalanxConfig`. Existing `ProviderConfig` and `SessionConfig` unchanged.

```rust
// falanx-engine/src/config.rs — additions

pub struct FalanxConfig {
    pub provider: ProviderConfig,  // unchanged
    pub session: SessionConfig,    // unchanged
    pub loop_cfg: LoopConfig,      // NEW
}

pub struct LoopConfig {
    pub max_iter: u32,           // env: FALANX_MAX_ITER, default: 3
    pub target_score: f32,       // env: FALANX_TARGET_SCORE, default: 4.5
    pub plateau_threshold: f32,  // env: FALANX_PLATEAU_THRESHOLD, default: 0.1
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self { max_iter: 3, target_score: 4.5, plateau_threshold: 0.1 }
    }
}
```

`FalanxConfig::from_env()` reads `FALANX_MAX_ITER`, `FALANX_TARGET_SCORE`, `FALANX_PLATEAU_THRESHOLD` with the defaults above.

---

## CLI Additions (Phase 1C)

`ReviewArgs` gains loop override args. Currently `ReviewArgs` only has `file`, `diff`, and `common` fields — extend it:

```rust
// falanx/src/main.rs — ReviewArgs extension

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

`cmd_review` replaces the current `exit(1)` stub — resolves target, merges CLI overrides into `LoopConfig`, creates session, calls `orchestrator::run()`, prints result.

`cmd_list_sessions` replaces the current `exit(1)` stub — calls `Session::list()`, prints table to stdout.

---

## Orchestrator Module (Phase 1C)

### `orchestrator/mod.rs` — extend existing stub

`RunConfig` gets `loop_cfg`. Current stub has no `loop_cfg` field — add it:

```rust
// falanx-engine/src/orchestrator/mod.rs

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
    pub loop_cfg: LoopConfig,   // NEW — was missing from Phase 1A+1B stub
    pub session: Session,
}

pub struct RunResult {
    pub final_score: ReviewScore,
    pub iterations: u32,
    pub patches: Vec<RewritePatch>,
    pub session_id: SessionId,
}

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult>;
```

### `orchestrator/pipeline.rs` — state machine

Note: file is named `pipeline.rs` not `loop.rs` — `loop` is a Rust keyword.

State machine transitions:

```
Initializing → Scoring
Scoring → Reviewing        (score.composite() < loop_cfg.target_score)
        → Completed        (score.composite() >= loop_cfg.target_score)
Reviewing → Rewriting      (issues non-empty)
          → ReScoring      (no issues found)
Rewriting → ReScoring
ReScoring → PlateauCheck
PlateauCheck → HorizonReset  (plateau detected, resets remaining)
             → Completed     (target hit, max_iter reached, or resets exhausted)
             → Scoring       (continue loop)
HorizonReset → Scoring
Completed / Failed → terminal
```

Exit conditions — checked at `PlateauCheck`:
1. `score.composite() >= loop_cfg.target_score` → `Completed`
2. `iteration >= loop_cfg.max_iter` → `Completed`
3. `score.delta(&prev_score) < loop_cfg.plateau_threshold` for two consecutive passes → `HorizonReset`, then `Completed` if `HorizonState::exhausted()`

### `orchestrator/horizon.rs` — extend empty stub

```rust
// falanx-engine/src/orchestrator/horizon.rs

pub struct HorizonState {
    pub reset_count: u32,
    pub max_resets: u32,  // hard cap: 2 per run
}

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

Agents are stateless across calls (each builds a fresh Cersei agent), so horizon reset = session event + counter increment. No agent state to discard.

---

## Agent Implementations (Phase 1D)

With `MockProvider` satisfying Cersei's `Provider` trait, agent functions have no `if dry_run` branches. Provider selection is done once at construction in the orchestrator.

`AgentContext` is updated to carry a pre-built Cersei agent rather than raw config:

```rust
// falanx-engine/src/agents/mod.rs — updated

pub struct AgentContext<'a> {
    pub agent: &'a cersei_agent::Agent,   // pre-built, provider already selected
    pub diff: &'a crate::git::Diff,
}
```

### `agents/review.rs` — replace bail! stub

```rust
// falanx-engine/src/agents/review.rs

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
```

### `agents/writing.rs` — replace bail! stub

```rust
// falanx-engine/src/agents/writing.rs

use super::AgentContext;
use crate::types::{ReviewIssue, RewritePatch};

pub async fn rewrite(
    ctx: &AgentContext<'_>,
    issues: &[ReviewIssue],
) -> anyhow::Result<Vec<RewritePatch>> {
    let prompt = build_rewrite_prompt(&ctx.diff.0, issues);
    let response = ctx.agent.run_with(&prompt).await?;
    parse_patches_response(&response)
}
```

Mock behaviour lives entirely in `MockProvider` — agents are identical regardless of dry-run vs live. The orchestrator selects provider once:

```rust
// orchestrator — provider selection at construction
let provider: Box<dyn cersei_provider::Provider> = if falanx_cfg.provider.dry_run {
    Box::new(crate::provider::MockProvider)
} else {
    cersei_provider::from_model_string(
        &falanx_cfg.provider.model,
        &falanx_cfg.provider.api_key,
        falanx_cfg.provider.base_url.as_deref(),
    )?
};
```

`MockProvider` returns hardcoded JSON responses shaped to match each agent's expected output. Exact `Provider` trait impl — confirm against Cersei source before writing.

---

## Cersei Live Provider (Phase 1D)

Wire the `!dry_run` path in all three agents. `quality.rs` goes first as it already has the live stub.

**Before writing any code**, explore the Cersei crate:
- Read `cersei-agent/src/lib.rs` for `Agent::builder()` API
- Read `cersei-provider/src/lib.rs` for OpenAI-compatible provider constructor
- Understand response type returned by `.run_with()`

Sketch of expected shape (verify before implementing):

```rust
// agents/quality.rs — live path (verify API first)
pub async fn score(ctx: &AgentContext<'_>) -> anyhow::Result<ReviewScore> {
    if ctx.config.provider.dry_run {
        return Ok(mock_score());
    }

    // provider construction — verify exact API against cersei-provider
    let provider = /* cersei OpenAI-compatible provider from ctx.config.provider */;
    let prompt = build_score_prompt(&ctx.diff.0);

    let response = Agent::builder()
        .provider(provider)
        .model(&ctx.config.provider.model)
        .build()?
        .run_with(&prompt)
        .await?;

    parse_score_response(&response)
}
```

`parse_score_response` extracts a JSON block from the LLM response text and deserializes into `ReviewScore`. Prompt must instruct the model to respond with JSON only matching the `ReviewScore` shape.

Same pattern for `critique` (returns `Vec<ReviewIssue>`) and `rewrite` (returns `Vec<RewritePatch>`).

---

## Session Additions (Phase 1D)

Add `Session::list()` — deferred from Phase 1A+1B.

Note: `SessionEntry` uses `#[serde(flatten)]` on the `event` field, so JSONL lines have `timestamp` and `type` at the top level (not nested under `event`). `Session::list()` must account for this when deserializing.

```rust
// falanx-engine/src/session.rs — additions

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionMeta {
    pub id: SessionId,
    pub path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub final_score: Option<ReviewScore>,
    pub iterations: Option<u32>,
}

impl Session {
    /// Walk dir/**/*.jsonl, read first + last line of each file to build metadata.
    /// Does not load entire files into memory.
    pub fn list(dir: &Path) -> anyhow::Result<Vec<SessionMeta>>;
}
```

`list()` uses `walkdir` (already available transitively, or add `walkdir = "2"` to deps) to walk the session directory. For each `.jsonl` file: read first line as `RunStarted`, read last line as `RunCompleted` or `RunFailed`. Skip files that fail to parse.

---

## Open Questions

### Orchestration Agent Context Storage

Current spec: orchestrator holds `prev_score`, `issues`, `patches` as local Rust variables. Each sub-agent gets a fresh Cersei agent with no shared memory — stateless across iterations.

Open direction: `cersei-memory` with `features = ["graph"]` provides Grafeo-backed indexed memory. If the Cersei API makes it natural to seed sub-agents with prior iteration context (scores, critique history), this could improve review quality across iterations.

**Decision deferred to Phase 1D.** When pulling Cersei in for Phase 1C, examine `cersei-memory` API. If graph memory fits the orchestration pattern cleanly, the `SessionEntry` envelope is already designed to accommodate correlation fields. If not, pure Rust state is sufficient. Do not design for graph memory speculatively.

---

## Key Constraints

- All constraints from Phase 1A+1B carry forward
- `CodeWritingAgent` never fires without prior `CodeReviewAgent` output — state ordering enforces this
- Loop always has a hard `max_iter` ceiling — no unbounded LLM spend
- Horizon reset capped at 2 per run
- No `todo!()` in any code path — use `anyhow::bail!()` for unimplemented live paths
- Live Cersei API must be read before integration code is written — do not assume builder shape
- Live Cersei path validated manually against OpenCode before Phase 1D marked complete

---

## Out of Scope

- Serve mode (MCP server, HTTP API) — Phase 1E
- Post-rewrite validation (`cargo check`, clippy) — post-Phase 1
- Manual horizon reset (`--reset-horizon` flag) — post-Phase 1
- Platform API input (`--pr 123`) — post-Phase 1
- Session continuation (`--continue <session-id>`) — post-Phase 1
