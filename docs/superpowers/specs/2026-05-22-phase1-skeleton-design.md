# Falanx Phase 1 — Skeleton Design

**Date:** 2026-05-22
**Status:** Superseded — split into two specs:
- [Phase 1A+1B](2026-05-22-phase1a-workspace-scaffold.md) — workspace scaffold + first agent
- [Phase 1C+1D](2026-05-22-phase1c-orchestrator-pipeline.md) — orchestrator + full pipeline

---

## Goal

A working CLI engine that orchestrates the three-agent pipeline in dry-run mode — validating the full workflow (state machine, session JSONL, CLI, agent sequencing) without requiring a live LLM provider.

## Acceptance Criteria

- `falanx score --file <path> --dry-run` returns a deterministic mock score report
- `falanx review --diff HEAD~1 --dry-run` runs the full pipeline: score → critique → rewrite, producing a JSONL audit trail
- The pipeline exits cleanly on target score hit, max iterations, or plateau detection
- Dry-run mode uses a mock provider — agents return hardcoded deterministic responses, no LLM calls made
- Each run produces a JSONL session file with timestamped events at `~/.falanx/sessions/...`
- A static diff string is sufficient input — no real repo or LLM credentials required for acceptance testing
- `cargo build` is green with no warnings across both crates
- Real provider (OpenCode/Cersei) integration is wired but gated behind the absence of `--dry-run`

---

## Scope

Establish the foundational workspace structure and module skeleton for Falanx. This design covers Phase 1A through 1D as defined in the brainstorm — from empty scaffold to a working three-agent pipeline. Phase 1E (serve mode) is explicitly out of scope.

---

## Workspace Structure

Two crates. Same binary, same engine, two entry points (per ARCHITECTURE.md invariant).

```
rust/
├── Cargo.toml                          ← workspace, members = ["crates/falanx-engine", "crates/falanx"]
└── crates/
    ├── falanx-engine/                  ← lib crate — all logic
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── config.rs
    │       ├── types.rs
    │       ├── git.rs
    │       ├── session.rs
    │       ├── agents/
    │       │   ├── mod.rs
    │       │   ├── quality.rs
    │       │   ├── review.rs
    │       │   └── writing.rs
    │       └── orchestrator/
    │           ├── mod.rs
    │           ├── loop.rs
    │           └── horizon.rs
    └── falanx/                         ← binary crate — thin CLI shell
        ├── Cargo.toml
        └── src/
            └── main.rs
```

The existing `crates/falanx-engine` binary crate is restructured: it becomes the lib crate and a new `falanx` binary crate is added. The current placeholder `main.rs` is replaced.

---

## Crate Responsibilities

### `falanx-engine` (lib)

Owns all runtime logic. Has no knowledge of HTTP, MCP, or argument parsing.

Modules:

| Module | Owns |
|---|---|
| `types` | Domain types — `ReviewScore`, `ReviewIssue`, `RewritePatch`, `SessionId`, `RunState` |
| `config` | `FalanxConfig` and sub-structs, env loading, `validate()` |
| `git` | `ReviewTarget`, `Diff`, diff extraction |
| `session` | `Session`, `SessionEvent`, append-only JSONL writes |
| `agents` | `AgentContext`, one async fn per agent (quality, review, writing) |
| `orchestrator` | `run()`, `RunState` machine, loop, plateau detection, horizon reset |

### `falanx` (binary)

Owns: clap arg parsing, config resolution, tokio runtime construction, calling `falanx_engine::orchestrator::run()`, rendering output to stdout. Nothing else.

`falanx serve` is a stubbed subcommand that prints "not yet implemented" and exits — reserving the command surface for Phase 1E.

---

## Config Design

Config is loaded from environment variables (via `dotenvy`) — not a TOML file. Secrets (API keys) stay in `.env` / env vars. Loop defaults are also env-configurable with CLI override.

```rust
pub struct FalanxConfig {
    pub provider: ProviderConfig,
    pub loop_cfg: LoopConfig,
    pub session: SessionConfig,
    pub telemetry: TelemetryConfig,
}

pub struct ProviderConfig {
    pub model: String,            // e.g. "opencode/big-pickle"
    pub api_key: String,          // from OPENCODE_API_KEY or equivalent
    pub base_url: Option<String>, // local endpoint for Ollama / proxy
    pub dry_run: bool,            // if true, use mock provider — no LLM calls
}

pub struct LoopConfig {
    pub max_iter: u32,           // default: 3, env: FALANX_MAX_ITER
    pub target_score: f32,       // default: 4.5, env: FALANX_TARGET_SCORE
    pub plateau_threshold: f32,  // default: 0.1, env: FALANX_PLATEAU_THRESHOLD
}

pub struct SessionConfig {
    pub dir: PathBuf,            // default: ~/.falanx/sessions
}
```

`FalanxConfig::from_env()` — loads all fields from env, applies defaults. `validate()` — skips `api_key` check when `dry_run = true`. Fails loudly at startup otherwise.

**Phase 1 provider strategy:**
- `--dry-run` flag (or `FALANX_DRY_RUN=true`) activates mock provider — agents return hardcoded deterministic responses, no Cersei/LLM involved
- Mock responses are realistic enough to exercise the full state machine and session trail
- Real provider (Cersei + OpenCode) is wired as the non-dry-run path but not required to pass acceptance criteria for Phase 1

---

## CLI Design

Uses `clap`. Config path resolution: `--config` > `$FALANX_CONFIG` > `~/.config/falanx/.env`.

```rust
enum Commands {
    Review(ReviewArgs),
    Score(ScoreArgs),
    ListSessions,
    Serve(ServeArgs),   // stub
}

struct ReviewArgs {
    #[arg(long)] diff: Option<String>,      // "HEAD~1", "main..feature"
    #[arg(long)] file: Option<PathBuf>,
    #[arg(long)] max_iter: Option<u32>,     // overrides config
    #[arg(long)] target: Option<f32>,
    #[arg(long)] continue_session: Option<String>,
    #[command(flatten)] common: CommonArgs,
}

struct ScoreArgs {
    #[arg(long)] diff: Option<String>,
    #[arg(long)] file: Option<PathBuf>,
    #[command(flatten)] common: CommonArgs,
}

struct CommonArgs {
    #[arg(long, env = "FALANX_CONFIG")] config: Option<PathBuf>,
    #[arg(long, env = "FALANX_DRY_RUN")]  dry_run: bool,
}
```

Tokio runtime is built manually from config (not `#[tokio::main]`) so worker threads are tunable. Pattern taken from `workflow-server`.

---

## Types

```rust
pub struct ReviewScore {
    pub readability: u8,
    pub maintainability: u8,
    pub performance: u8,
    pub security: u8,
    pub architecture: u8,
}
impl ReviewScore {
    pub fn composite(&self) -> f32 { /* arithmetic mean */ }
    pub fn delta(&self, other: &Self) -> f32 { /* |composite diff| */ }
}

pub struct ReviewIssue {
    pub location: String,
    pub problem: String,
    pub fix: String,
}

pub struct RewritePatch {
    pub original: String,
    pub revised: String,
    pub issue_ref: String,
}

pub struct SessionId(pub String);  // newtype

pub enum RunState {
    Initializing,
    Scoring,
    Reviewing,
    Rewriting,
    ReScoring,
    PlateauCheck,
    HorizonReset,
    Completed,
    Failed(String),
}
```

---

## Git Module

```rust
pub struct Diff(pub String);

pub enum ReviewTarget {
    GitRange(String),   // "HEAD~1", "main..feature"
    File(PathBuf),
}

impl ReviewTarget {
    pub fn extract_diff(&self) -> Result<Diff>;
}
```

Phase 1: shell out to `git diff`. No `git2` crate unless shelling proves insufficient. No GitHub/GitLab API.

---

## Session Module

Falanx's own run-level audit trail. Separate from Cersei's internal JSONL sessions.

Events are wrapped in an envelope that adds timestamp and any future metadata in one place:

```rust
pub struct SessionEntry {
    pub timestamp: DateTime<Utc>,
    pub event: SessionEvent,
}

pub enum SessionEvent {
    RunStarted { session_id: SessionId, target: String },
    AgentInvoked { agent: String, iteration: u32 },
    ScoreComputed { score: ReviewScore, iteration: u32 },
    IssuesFound { count: usize, iteration: u32 },
    RewriteApplied { patches: usize, iteration: u32 },
    HorizonReset { iteration: u32 },
    RunCompleted { final_score: ReviewScore, iterations: u32 },
    RunFailed { reason: String },
}

pub struct Session {
    path: PathBuf,  // ~/.falanx/sessions/<repo-hash>/<branch>/<id>.jsonl
}

impl Session {
    pub fn new(dir: &Path, target: &ReviewTarget) -> Result<(Self, SessionId)>;
    pub fn append(&self, event: SessionEvent) -> Result<()>;  // wraps in SessionEntry internally
    pub fn list(dir: &Path) -> Result<Vec<SessionMeta>>;
}
```

Append-only. Write-only during a run. `serde_json` one line per `SessionEntry`.

**Cersei log association:** Cersei maintains its own session/audit logs for agent invocations. How Falanx session events associate with Cersei's logs (e.g. via shared IDs, correlated timestamps, or co-located paths) is to be determined during implementation once the Cersei API is understood. The `SessionEntry` envelope is designed to accommodate additional correlation fields when that pattern becomes clear.

Session path: `~/.falanx/sessions/<repo-hash>/<branch-name>/<session-id>.jsonl`

Repo hash: SHA256 of the git repo root path, truncated to 8 chars.

---

## Agents Module

Cersei owns the agentic loop, context management, and LLM provider. Each Falanx agent is a thin wrapper that constructs a `cersei::Agent`, sends a prompt, and parses structured output.

No `Agent` trait in Phase 1 — there are exactly 3 agents, not N. Plain async functions.

```rust
pub struct AgentContext<'a> {
    pub config: &'a FalanxConfig,
    pub diff: &'a Diff,
}

// agents/quality.rs
pub async fn score(ctx: &AgentContext<'_>) -> Result<ReviewScore>;

// agents/review.rs
pub async fn critique(ctx: &AgentContext<'_>, score: &ReviewScore) -> Result<Vec<ReviewIssue>>;

// agents/writing.rs
pub async fn rewrite(ctx: &AgentContext<'_>, issues: &[ReviewIssue]) -> Result<Vec<RewritePatch>>;
```

Each function checks `config.provider.dry_run`:
- **dry-run:** returns hardcoded deterministic output (fixed scores, canned issues, no-op patches) — no Cersei involved
- **live:** builds a `cersei::Agent` with provider from config, calls `.run_with(prompt)`, parses JSON response into typed output, returns `Err` on parse failure

Horizon reset is a no-op at this layer — each call already constructs a fresh Cersei agent.

---

## Orchestrator Module

Drives the `RunState` machine. Owns loop control, convergence, plateau detection.

```rust
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

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> Result<RunResult>;
```

State machine loop (in `orchestrator/loop.rs`):

```
Initializing
  → Scoring          (CodeQualityAgent)
  → Reviewing        (CodeReviewAgent, only if score < target)
  → Rewriting        (CodeWritingAgent, only if issues found)
  → ReScoring        (CodeQualityAgent again)
  → PlateauCheck     (compare delta to plateau_threshold)
      → HorizonReset if stalled (resets to Scoring with fresh context)
      → Completed if target hit or max_iter reached
      → Scoring if continuing
  → Completed / Failed
```

Exit conditions (all active simultaneously when looping):
- Target score hit (`composite >= target_score`)
- Max iterations reached (`iteration >= max_iter`)
- Plateau detected (`delta < plateau_threshold` for two consecutive passes)

`CodeWritingAgent` never fires without `CodeReviewAgent` output — enforced by state ordering, not a runtime check.

---

## Phase Delivery Order

| Phase | Deliverable | Success Criteria |
|---|---|---|
| 1A | Workspace restructure + all module stubs compile | `cargo build` green, all modules present |
| 1B | `CodeQualityAgent` + `falanx score` works | `falanx score --file src/main.rs` returns a score |
| 1C | Orchestrator loop + `falanx review` (single agent) | Loop exits on target/plateau/max-iter |
| 1D | `CodeReviewAgent` + `CodeWritingAgent` | Full pipeline: score → critique → rewrite |

---

## Key Constraints

- `falanx-engine` has no knowledge of HTTP, MCP, or CLI arg types
- `CodeWritingAgent` never fires without `CodeReviewAgent` output
- Every run produces a JSONL audit trail regardless of exit condition
- Loop always has a hard `max_iter` ceiling — no unbounded LLM spend
- Cersei manages context per agent call; Falanx manages run-level state
- Agents are stateless across calls — each invocation builds a fresh Cersei agent

---

## Out of Scope (Phase 1)

- Serve mode (MCP server, HTTP API)
- Post-rewrite validation (cargo check, clippy)
- Manual horizon reset (`--reset-horizon` flag)
- Platform API input (`--pr 123`)
- Session continuation (`--continue <session-id>`) — listed-sessions only
- GitHub/GitLab diff fetching
