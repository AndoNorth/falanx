# Falanx Phase 1A+1B — Workspace Scaffold & First Agent

**Date:** 2026-05-22
**Status:** Approved

---

## Goal

A compiling two-crate workspace with all module stubs in place, a working `falanx score` command backed by `CodeQualityAgent`, and a JSONL session trail — validated entirely in dry-run mode with no LLM credentials required.

## Acceptance Criteria

- `cargo build` is green with no warnings across both crates
- `falanx score --file <path> --dry-run` prints a structured score report to stdout
- A JSONL session file is written to `~/.falanx/sessions/...` with timestamped events
- No LLM credentials required — dry-run mode returns deterministic mock scores
- `falanx --help` and `falanx score --help` render correctly
- `falanx serve` prints "not yet implemented" and exits 0

---

## Scope

Phase 1A: restructure workspace, add all module stubs, `cargo build` green.
Phase 1B: implement `CodeQualityAgent` (dry-run path only), wire `falanx score`, write session JSONL.

Orchestrator loop, `CodeReviewAgent`, `CodeWritingAgent`, and live Cersei integration are out of scope — covered in Phase 1C+1D.

---

## Workspace Structure

Restructure from the current single binary crate to two crates.

**Current state:** `rust/crates/falanx-engine/` is a binary crate (`src/main.rs` prints "falanx-engine").

**Target state:**

```
rust/
├── Cargo.toml                          ← workspace members = ["crates/falanx-engine", "crates/falanx"]
└── crates/
    ├── falanx-engine/                  ← lib crate (rename existing, change to lib)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── config.rs
    │       ├── types.rs
    │       ├── git.rs
    │       ├── session.rs
    │       ├── agents/
    │       │   ├── mod.rs
    │       │   ├── quality.rs          ← implemented in 1B
    │       │   ├── review.rs           ← stub only
    │       │   └── writing.rs          ← stub only
    │       └── orchestrator/
    │           ├── mod.rs              ← stub only
    │           ├── loop.rs             ← stub only
    │           └── horizon.rs          ← stub only
    └── falanx/                         ← new binary crate
        ├── Cargo.toml
        └── src/
            └── main.rs
```

---

## Crate Responsibilities

### `falanx-engine` (lib)

No knowledge of HTTP, MCP, or CLI arg types. All stubs compile; quality agent fully implemented.

| Module | Phase 1A/1B State |
|---|---|
| `types` | Fully implemented |
| `config` | Fully implemented |
| `git` | Fully implemented |
| `session` | Fully implemented |
| `agents/quality` | Fully implemented (dry-run path) |
| `agents/review` | Stub — `todo!()` |
| `agents/writing` | Stub — `todo!()` |
| `orchestrator` | Stub — `todo!()` |

### `falanx` (binary)

Parses args, loads config, calls `falanx_engine::agents::quality::score()` directly (no orchestrator yet), renders output, exits.

---

## Config Design

Loaded from env vars via `dotenvy`. No TOML file.

```rust
// falanx-engine/src/config.rs

pub struct FalanxConfig {
    pub provider: ProviderConfig,
    pub session: SessionConfig,
}

pub struct ProviderConfig {
    pub model: String,            // env: FALANX_MODEL, default: "opencode/big-pickle"
    pub api_key: String,          // env: OPENCODE_API_KEY, default: ""
    pub base_url: Option<String>, // env: FALANX_BASE_URL
    pub dry_run: bool,            // env: FALANX_DRY_RUN, default: false
}

pub struct SessionConfig {
    pub dir: PathBuf,             // env: FALANX_SESSION_DIR, default: ~/.falanx/sessions
}

impl FalanxConfig {
    pub fn from_env() -> Result<Self>;
    pub fn validate(&self) -> Result<()>;  // rejects empty api_key unless dry_run
}
```

`LoopConfig` and `TelemetryConfig` are deferred to Phase 1C+1D — not needed until the orchestrator exists.

---

## CLI Design

```rust
// falanx/src/main.rs

#[derive(Parser)]
#[command(name = "falanx", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Score(ScoreArgs),
    Review(ReviewArgs),     // stub — "not yet implemented"
    ListSessions,           // stub — "not yet implemented"
    Serve(ServeArgs),       // stub — "not yet implemented"
}

#[derive(Args)]
struct ScoreArgs {
    #[arg(long)] file: Option<PathBuf>,
    #[arg(long)] diff: Option<String>,
    #[command(flatten)] common: CommonArgs,
}

#[derive(Args)]
struct ReviewArgs {
    #[arg(long)] diff: Option<String>,
    #[arg(long)] file: Option<PathBuf>,
    #[command(flatten)] common: CommonArgs,
}

#[derive(Args)]
struct ServeArgs {
    #[command(flatten)] common: CommonArgs,
}

#[derive(Args)]
struct CommonArgs {
    #[arg(long, env = "FALANX_CONFIG")] config: Option<PathBuf>,
    #[arg(long, env = "FALANX_DRY_RUN")] dry_run: bool,
}
```

Tokio runtime built manually — not `#[tokio::main]`:

```rust
fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run(args))
}
```

---

## Types

```rust
// falanx-engine/src/types.rs

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewScore {
    pub readability: u8,
    pub maintainability: u8,
    pub performance: u8,
    pub security: u8,
    pub architecture: u8,
}

impl ReviewScore {
    pub fn composite(&self) -> f32 {
        (self.readability + self.maintainability + self.performance
            + self.security + self.architecture) as f32 / 5.0
    }
    pub fn delta(&self, other: &Self) -> f32 {
        (self.composite() - other.composite()).abs()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewIssue {
    pub location: String,
    pub problem: String,
    pub fix: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewritePatch {
    pub original: String,
    pub revised: String,
    pub issue_ref: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionId(pub String);

// RunState is used by orchestrator — stub for now, needed for session events
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
// falanx-engine/src/git.rs

pub struct Diff(pub String);

pub enum ReviewTarget {
    GitRange(String),   // "HEAD~1", "main..feature"
    File(PathBuf),
}

impl ReviewTarget {
    pub fn extract_diff(&self) -> anyhow::Result<Diff> {
        match self {
            ReviewTarget::File(path) => {
                let content = std::fs::read_to_string(path)?;
                Ok(Diff(content))
            }
            ReviewTarget::GitRange(range) => {
                let output = std::process::Command::new("git")
                    .args(["diff", range])
                    .output()?;
                if !output.status.success() {
                    anyhow::bail!("git diff failed: {}", String::from_utf8_lossy(&output.stderr));
                }
                Ok(Diff(String::from_utf8(output.stdout)?))
            }
        }
    }
}
```

Phase 1A+1B: `--file` path reads file content as the diff. `GitRange` shells out to `git diff`.

---

## Session Module

```rust
// falanx-engine/src/session.rs

use chrono::{DateTime, Utc};
use crate::types::{SessionId, ReviewScore};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionEntry {
    pub timestamp: DateTime<Utc>,
    pub event: SessionEvent,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    RunStarted { session_id: String, target: String },
    AgentInvoked { agent: String, iteration: u32 },
    ScoreComputed { score: ReviewScore, iteration: u32 },
    IssuesFound { count: usize, iteration: u32 },
    RewriteApplied { patches: usize, iteration: u32 },
    HorizonReset { iteration: u32 },
    RunCompleted { final_score: ReviewScore, iterations: u32 },
    RunFailed { reason: String },
}

pub struct Session {
    path: PathBuf,
    id: SessionId,
}

impl Session {
    pub fn new(dir: &Path, label: &str) -> anyhow::Result<Self>;
    pub fn id(&self) -> &SessionId;
    pub fn append(&self, event: SessionEvent) -> anyhow::Result<()>;
}
```

Session path: `<dir>/<label>/<session-id>.jsonl` where `session-id` is a UUID v4 and `label` is derived from the review target (file path or git range, sanitised for filesystem use).

`Session::append` wraps the event in `SessionEntry` with `Utc::now()` before writing.

No `Session::list()` in Phase 1A+1B — deferred to when `falanx list-sessions` is implemented.

---

## Agents Module

### `agents/mod.rs`

```rust
// falanx-engine/src/agents/mod.rs

pub mod quality;
pub mod review;
pub mod writing;

pub struct AgentContext<'a> {
    pub config: &'a crate::config::FalanxConfig,
    pub diff: &'a crate::git::Diff,
}
```

### `agents/quality.rs` — implemented in 1B

```rust
// falanx-engine/src/agents/quality.rs

use super::AgentContext;
use crate::types::ReviewScore;

pub async fn score(ctx: &AgentContext<'_>) -> anyhow::Result<ReviewScore> {
    if ctx.config.provider.dry_run {
        return Ok(mock_score());
    }
    // Phase 1C+1D: Cersei integration goes here
    anyhow::bail!("live provider not yet implemented — use --dry-run")
}

fn mock_score() -> ReviewScore {
    ReviewScore {
        readability: 3,
        maintainability: 3,
        performance: 3,
        security: 3,
        architecture: 3,
    }
}
```

### `agents/review.rs` and `agents/writing.rs` — stubs

```rust
pub async fn critique(_ctx: &AgentContext<'_>, _score: &ReviewScore) -> anyhow::Result<Vec<ReviewIssue>> {
    anyhow::bail!("not yet implemented")
}

pub async fn rewrite(_ctx: &AgentContext<'_>, _issues: &[ReviewIssue]) -> anyhow::Result<Vec<RewritePatch>> {
    anyhow::bail!("not yet implemented")
}
```

---

## `falanx score` Flow (Phase 1B)

```
main()
  → parse args → load config (merge --dry-run from CommonArgs)
  → resolve ReviewTarget from --file or --diff
  → Session::new(config.session.dir, label)
  → session.append(RunStarted)
  → diff = target.extract_diff()
  → session.append(AgentInvoked { agent: "quality", iteration: 0 })
  → score = agents::quality::score(&ctx).await
  → session.append(ScoreComputed { score, iteration: 0 })
  → session.append(RunCompleted { final_score: score, iterations: 1 })
  → print score report to stdout
  → exit 0
```

No loop, no review/rewrite. Orchestrator not involved.

---

## Key Constraints

- `falanx-engine` has no knowledge of HTTP, MCP, or CLI arg types
- Dry-run path returns deterministic output — same input always produces same output
- Live provider path (`!dry_run`) returns `bail!("not yet implemented")` — not panics
- All stubs use `anyhow::bail!` not `todo!()` — panics are not acceptable in a CLI tool
- Every session file write is flushed immediately — no buffering

---

## Out of Scope

- Orchestrator loop, convergence, plateau detection (Phase 1C)
- `CodeReviewAgent`, `CodeWritingAgent` (Phase 1D)
- Live Cersei provider integration (Phase 1C+1D)
- `falanx list-sessions`, `falanx review`, `falanx serve` (stubs only)
- `LoopConfig`, `TelemetryConfig` (Phase 1C)
- `Session::list()` (Phase 1C)
