# Falanx Phase 1A+1B — Workspace Scaffold & First Agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the Rust workspace into two crates, implement all module stubs, and deliver a working `falanx score --dry-run` command that writes a timestamped JSONL session trail.

**Architecture:** `falanx-engine` is a lib crate owning all domain logic (types, config, git, session, agents). `falanx` is a thin binary crate that parses CLI args, loads config, and calls the engine. No orchestrator yet — `falanx score` calls `agents::quality::score()` directly. Dry-run mode returns deterministic mock output; live provider path returns a clear error.

**Tech Stack:** Rust 2024 edition, `clap` (CLI), `anyhow` (errors), `serde`/`serde_json` (serialization), `chrono` (timestamps), `uuid` (session IDs), `dotenvy` (env loading), `tokio` (async runtime), `dirs` (home dir resolution)

---

## File Map

| File | Action | Purpose |
|---|---|---|
| `rust/Cargo.toml` | Modify | Add `crates/falanx` to workspace members |
| `rust/crates/falanx-engine/Cargo.toml` | Modify | Switch to lib crate, add dependencies |
| `rust/crates/falanx-engine/src/main.rs` | Delete | Replaced by lib.rs |
| `rust/crates/falanx-engine/src/lib.rs` | Create | Lib root, pub mod declarations |
| `rust/crates/falanx-engine/src/types.rs` | Create | ReviewScore, ReviewIssue, RewritePatch, SessionId, RunState |
| `rust/crates/falanx-engine/src/config.rs` | Create | FalanxConfig, ProviderConfig, SessionConfig, from_env, validate |
| `rust/crates/falanx-engine/src/git.rs` | Create | Diff, ReviewTarget, extract_diff |
| `rust/crates/falanx-engine/src/session.rs` | Create | Session, SessionEntry, SessionEvent, append |
| `rust/crates/falanx-engine/src/agents/mod.rs` | Create | AgentContext, pub mod declarations |
| `rust/crates/falanx-engine/src/agents/quality.rs` | Create | score(), mock_score(), dry_run branch |
| `rust/crates/falanx-engine/src/agents/review.rs` | Create | critique() stub |
| `rust/crates/falanx-engine/src/agents/writing.rs` | Create | rewrite() stub |
| `rust/crates/falanx-engine/src/orchestrator/mod.rs` | Create | pub mod declarations, RunConfig/RunResult stubs |
| `rust/crates/falanx-engine/src/orchestrator/pipeline.rs` | Create | Stub (note: spec says loop.rs but `loop` is reserved keyword) |
| `rust/crates/falanx-engine/src/orchestrator/horizon.rs` | Create | Stub |
| `rust/crates/falanx/Cargo.toml` | Create | Binary crate manifest |
| `rust/crates/falanx/src/main.rs` | Create | CLI: Cli, Commands, ScoreArgs, CommonArgs, full score flow |

---

### Task 1: Restructure Workspace (Phase 1A)

**Files:**
- Modify: `rust/Cargo.toml`
- Modify: `rust/crates/falanx-engine/Cargo.toml`
- Delete: `rust/crates/falanx-engine/src/main.rs`
- Create: `rust/crates/falanx-engine/src/lib.rs`
- Create: `rust/crates/falanx/Cargo.toml`
- Create: `rust/crates/falanx/src/main.rs`

- [ ] **Step 1: Update workspace Cargo.toml**

Replace `rust/Cargo.toml` with:

```toml
[workspace]
members = ["crates/falanx-engine", "crates/falanx"]
resolver = "2"
```

- [ ] **Step 2: Convert falanx-engine to a lib crate**

Replace `rust/crates/falanx-engine/Cargo.toml` with:

```toml
[package]
name = "falanx-engine"
version = "0.1.0"
edition = "2024"

[lib]
name = "falanx_engine"
path = "src/lib.rs"

[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
dotenvy = "0.15"
tokio = { version = "1", features = ["full"] }
dirs = "5"
```

- [ ] **Step 3: Delete the existing main.rs**

```bash
rm rust/crates/falanx-engine/src/main.rs
```

- [ ] **Step 4: Create lib.rs placeholder**

Create `rust/crates/falanx-engine/src/lib.rs`:

```rust
pub mod agents;
pub mod config;
pub mod git;
pub mod orchestrator;
pub mod session;
pub mod types;
```

- [ ] **Step 5: Create placeholder module files so lib.rs compiles**

Create `rust/crates/falanx-engine/src/types.rs`:
```rust
// placeholder — implemented in Task 2
```

Create `rust/crates/falanx-engine/src/config.rs`:
```rust
// placeholder — implemented in Task 3
```

Create `rust/crates/falanx-engine/src/git.rs`:
```rust
// placeholder — implemented in Task 4
```

Create `rust/crates/falanx-engine/src/session.rs`:
```rust
// placeholder — implemented in Task 5
```

Create `rust/crates/falanx-engine/src/agents/mod.rs`:
```rust
// placeholder — implemented in Task 6
pub mod quality;
pub mod review;
pub mod writing;
```

Create `rust/crates/falanx-engine/src/agents/quality.rs`:
```rust
// placeholder
```

Create `rust/crates/falanx-engine/src/agents/review.rs`:
```rust
// placeholder
```

Create `rust/crates/falanx-engine/src/agents/writing.rs`:
```rust
// placeholder
```

Create `rust/crates/falanx-engine/src/orchestrator/mod.rs`:
```rust
// placeholder — implemented in Task 6
pub mod horizon;
pub mod pipeline;
```

Create `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`:
```rust
// placeholder
```

Create `rust/crates/falanx-engine/src/orchestrator/horizon.rs`:
```rust
// placeholder
```

- [ ] **Step 6: Create the falanx binary crate**

Create `rust/crates/falanx/Cargo.toml`:

```toml
[package]
name = "falanx"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "falanx"
path = "src/main.rs"

[dependencies]
falanx-engine = { path = "../falanx-engine" }
clap = { version = "4", features = ["derive", "env"] }
anyhow = "1"
tokio = { version = "1", features = ["full"] }
```

Create `rust/crates/falanx/src/main.rs`:

```rust
fn main() {
    println!("falanx");
}
```

- [ ] **Step 7: Verify workspace compiles**

```bash
cd rust && cargo build
```

Expected: compiles with no errors. Warnings about unused imports/placeholders are acceptable at this stage.

- [ ] **Step 8: Commit**

```bash
git add rust/
git commit -m "chore: restructure workspace — falanx-engine lib crate + falanx binary crate"
```

---

### Task 2: Domain Types

**Files:**
- Modify: `rust/crates/falanx-engine/src/types.rs`

- [ ] **Step 1: Write failing tests**

Replace `rust/crates/falanx-engine/src/types.rs` with:

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewScore {
    pub readability: u8,
    pub maintainability: u8,
    pub performance: u8,
    pub security: u8,
    pub architecture: u8,
}

impl ReviewScore {
    pub fn composite(&self) -> f32 {
        (self.readability as u32
            + self.maintainability as u32
            + self.performance as u32
            + self.security as u32
            + self.architecture as u32) as f32
            / 5.0
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionId(pub String);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn score(v: u8) -> ReviewScore {
        ReviewScore {
            readability: v,
            maintainability: v,
            performance: v,
            security: v,
            architecture: v,
        }
    }

    #[test]
    fn composite_is_mean_of_five_fields() {
        let s = ReviewScore {
            readability: 4,
            maintainability: 3,
            performance: 5,
            security: 2,
            architecture: 1,
        };
        assert!((s.composite() - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn composite_uniform_score() {
        assert!((score(3).composite() - 3.0).abs() < f32::EPSILON);
        assert!((score(5).composite() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn delta_is_absolute_difference_of_composites() {
        let a = score(3);
        let b = score(5);
        assert!((a.delta(&b) - 2.0).abs() < f32::EPSILON);
        assert!((b.delta(&a) - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn delta_same_score_is_zero() {
        let a = score(4);
        assert!(a.delta(&a) < f32::EPSILON);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd rust && cargo test -p falanx-engine types
```

Expected: all 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add rust/crates/falanx-engine/src/types.rs
git commit -m "feat(engine): domain types — ReviewScore, ReviewIssue, RewritePatch, SessionId, RunState"
```

---

### Task 3: Config Module

**Files:**
- Modify: `rust/crates/falanx-engine/src/config.rs`

- [ ] **Step 1: Write failing tests first**

Add test module at bottom of `rust/crates/falanx-engine/src/config.rs` (write tests before implementation):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_env_empty() {
        // Clear relevant env vars for this test
        std::env::remove_var("FALANX_MODEL");
        std::env::remove_var("OPENCODE_API_KEY");
        std::env::remove_var("FALANX_BASE_URL");
        std::env::remove_var("FALANX_DRY_RUN");
        std::env::remove_var("FALANX_SESSION_DIR");

        let cfg = FalanxConfig::from_env().unwrap();
        assert_eq!(cfg.provider.model, "opencode/big-pickle");
        assert_eq!(cfg.provider.api_key, "");
        assert!(!cfg.provider.dry_run);
        assert!(cfg.provider.base_url.is_none());
    }

    #[test]
    fn validate_rejects_empty_api_key_when_not_dry_run() {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "opencode/big-pickle".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: false,
            },
            session: SessionConfig {
                dir: std::path::PathBuf::from("/tmp/falanx-test"),
            },
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_accepts_empty_api_key_in_dry_run() {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "opencode/big-pickle".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: SessionConfig {
                dir: std::path::PathBuf::from("/tmp/falanx-test"),
            },
        };
        assert!(cfg.validate().is_ok());
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd rust && cargo test -p falanx-engine config 2>&1 | head -20
```

Expected: compile error — `FalanxConfig` not defined yet.

- [ ] **Step 3: Implement config module**

Replace `rust/crates/falanx-engine/src/config.rs` with:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FalanxConfig {
    pub provider: ProviderConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub dir: PathBuf,
}

impl FalanxConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        // Load .env file if present, ignore error if missing
        let _ = dotenvy::dotenv();

        let model = std::env::var("FALANX_MODEL")
            .unwrap_or_else(|_| "opencode/big-pickle".into());

        let api_key = std::env::var("OPENCODE_API_KEY").unwrap_or_default();

        let base_url = std::env::var("FALANX_BASE_URL").ok();

        let dry_run = std::env::var("FALANX_DRY_RUN")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let dir = std::env::var("FALANX_SESSION_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".falanx")
                    .join("sessions")
            });

        Ok(Self {
            provider: ProviderConfig { model, api_key, base_url, dry_run },
            session: SessionConfig { dir },
        })
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.provider.dry_run && self.provider.api_key.is_empty() {
            anyhow::bail!(
                "OPENCODE_API_KEY is required when not in dry-run mode. \
                 Set it in your environment or use --dry-run for testing."
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_env_empty() {
        std::env::remove_var("FALANX_MODEL");
        std::env::remove_var("OPENCODE_API_KEY");
        std::env::remove_var("FALANX_BASE_URL");
        std::env::remove_var("FALANX_DRY_RUN");
        std::env::remove_var("FALANX_SESSION_DIR");

        let cfg = FalanxConfig::from_env().unwrap();
        assert_eq!(cfg.provider.model, "opencode/big-pickle");
        assert_eq!(cfg.provider.api_key, "");
        assert!(!cfg.provider.dry_run);
        assert!(cfg.provider.base_url.is_none());
    }

    #[test]
    fn validate_rejects_empty_api_key_when_not_dry_run() {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "opencode/big-pickle".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: false,
            },
            session: SessionConfig {
                dir: PathBuf::from("/tmp/falanx-test"),
            },
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_accepts_empty_api_key_in_dry_run() {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "opencode/big-pickle".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: SessionConfig {
                dir: PathBuf::from("/tmp/falanx-test"),
            },
        };
        assert!(cfg.validate().is_ok());
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd rust && cargo test -p falanx-engine config
```

Expected:
```
test config::tests::defaults_when_env_empty ... ok
test config::tests::validate_accepts_empty_api_key_in_dry_run ... ok
test config::tests::validate_rejects_empty_api_key_when_not_dry_run ... ok
```

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/config.rs
git commit -m "feat(engine): config module — FalanxConfig, ProviderConfig, SessionConfig, from_env, validate"
```

---

### Task 4: Git Module

**Files:**
- Modify: `rust/crates/falanx-engine/src/git.rs`

- [ ] **Step 1: Implement git module with inline tests**

Replace `rust/crates/falanx-engine/src/git.rs` with:

```rust
use std::path::PathBuf;

pub struct Diff(pub String);

pub enum ReviewTarget {
    GitRange(String),
    File(PathBuf),
}

impl ReviewTarget {
    pub fn extract_diff(&self) -> anyhow::Result<Diff> {
        match self {
            ReviewTarget::File(path) => {
                let content = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path.display(), e))?;
                Ok(Diff(content))
            }
            ReviewTarget::GitRange(range) => {
                let output = std::process::Command::new("git")
                    .args(["diff", range])
                    .output()
                    .map_err(|e| anyhow::anyhow!("failed to run git diff: {}", e))?;
                if !output.status.success() {
                    anyhow::bail!(
                        "git diff {} failed: {}",
                        range,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                Ok(Diff(String::from_utf8(output.stdout)?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_target_reads_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        std::fs::write(&path, "fn main() {}").unwrap();

        let target = ReviewTarget::File(path);
        let diff = target.extract_diff().unwrap();
        assert_eq!(diff.0, "fn main() {}");
    }

    #[test]
    fn file_target_missing_file_returns_error() {
        let target = ReviewTarget::File(PathBuf::from("/nonexistent/path/file.rs"));
        assert!(target.extract_diff().is_err());
    }
}
```

- [ ] **Step 2: Add `tempfile` dev-dependency**

Add to `rust/crates/falanx-engine/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Run tests**

```bash
cd rust && cargo test -p falanx-engine git
```

Expected:
```
test git::tests::file_target_reads_content ... ok
test git::tests::file_target_missing_file_returns_error ... ok
```

- [ ] **Step 4: Commit**

```bash
git add rust/crates/falanx-engine/src/git.rs rust/crates/falanx-engine/Cargo.toml
git commit -m "feat(engine): git module — ReviewTarget, Diff, extract_diff"
```

---

### Task 5: Session Module

**Files:**
- Modify: `rust/crates/falanx-engine/src/session.rs`

- [ ] **Step 1: Implement session module**

Replace `rust/crates/falanx-engine/src/session.rs` with:

```rust
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::types::{ReviewScore, SessionId};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionEntry {
    pub timestamp: DateTime<Utc>,
    #[serde(flatten)]
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
    /// Create a new session file at `<dir>/<label>/<uuid>.jsonl`.
    /// `label` is used as a subdirectory name — sanitised to filesystem-safe chars.
    pub fn new(dir: &Path, label: &str) -> anyhow::Result<Self> {
        let safe_label = label
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();

        let session_dir = dir.join(&safe_label);
        std::fs::create_dir_all(&session_dir)?;

        let id = SessionId(uuid::Uuid::new_v4().to_string());
        let path = session_dir.join(format!("{}.jsonl", id.0));

        Ok(Self { path, id })
    }

    pub fn id(&self) -> &SessionId {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one event to the JSONL file. Each call writes and flushes one line.
    pub fn append(&self, event: SessionEvent) -> anyhow::Result<()> {
        let entry = SessionEntry {
            timestamp: Utc::now(),
            event,
        };
        let line = serde_json::to_string(&entry)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", line)?;
        file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_directory_and_file_path() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::new(dir.path(), "test-label").unwrap();
        // Directory created
        assert!(dir.path().join("test-label").exists());
        // Path has .jsonl extension
        assert_eq!(session.path().extension().unwrap(), "jsonl");
        // ID is a valid UUID
        assert_eq!(session.id().0.len(), 36);
    }

    #[test]
    fn label_sanitised_for_filesystem() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::new(dir.path(), "HEAD~1..main/branch").unwrap();
        // Slashes and tildes replaced with underscores
        let subdir = session.path().parent().unwrap().file_name().unwrap();
        assert!(!subdir.to_string_lossy().contains('/'));
        assert!(!subdir.to_string_lossy().contains('~'));
    }

    #[test]
    fn append_writes_valid_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::new(dir.path(), "test").unwrap();

        session
            .append(SessionEvent::RunStarted {
                session_id: session.id().0.clone(),
                target: "test.rs".into(),
            })
            .unwrap();

        session
            .append(SessionEvent::AgentInvoked {
                agent: "quality".into(),
                iteration: 0,
            })
            .unwrap();

        let content = std::fs::read_to_string(session.path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        // Each line is valid JSON with a timestamp and type field
        for line in &lines {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(v.get("timestamp").is_some());
            assert!(v.get("type").is_some());
        }

        // First line type is run_started
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["type"], "run_started");

        // Second line type is agent_invoked
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["type"], "agent_invoked");
    }

    #[test]
    fn append_flushes_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::new(dir.path(), "flush-test").unwrap();

        session
            .append(SessionEvent::RunFailed {
                reason: "test failure".into(),
            })
            .unwrap();

        // File is readable immediately after append without closing
        let content = std::fs::read_to_string(session.path()).unwrap();
        assert!(!content.is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd rust && cargo test -p falanx-engine session
```

Expected:
```
test session::tests::new_creates_directory_and_file_path ... ok
test session::tests::label_sanitised_for_filesystem ... ok
test session::tests::append_writes_valid_jsonl ... ok
test session::tests::append_flushes_immediately ... ok
```

- [ ] **Step 3: Commit**

```bash
git add rust/crates/falanx-engine/src/session.rs
git commit -m "feat(engine): session module — append-only JSONL audit trail with timestamped events"
```

---

### Task 6: Module Stubs (agents + orchestrator)

**Files:**
- Modify: `rust/crates/falanx-engine/src/agents/mod.rs`
- Modify: `rust/crates/falanx-engine/src/agents/review.rs`
- Modify: `rust/crates/falanx-engine/src/agents/writing.rs`
- Modify: `rust/crates/falanx-engine/src/orchestrator/mod.rs`
- Modify: `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`
- Modify: `rust/crates/falanx-engine/src/orchestrator/horizon.rs`

- [ ] **Step 1: Implement agents/mod.rs**

Replace `rust/crates/falanx-engine/src/agents/mod.rs` with:

```rust
pub mod quality;
pub mod review;
pub mod writing;

pub struct AgentContext<'a> {
    pub config: &'a crate::config::FalanxConfig,
    pub diff: &'a crate::git::Diff,
}
```

- [ ] **Step 2: Implement review stub**

Replace `rust/crates/falanx-engine/src/agents/review.rs` with:

```rust
use super::AgentContext;
use crate::types::{ReviewIssue, ReviewScore};

pub async fn critique(
    _ctx: &AgentContext<'_>,
    _score: &ReviewScore,
) -> anyhow::Result<Vec<ReviewIssue>> {
    anyhow::bail!("CodeReviewAgent not yet implemented — planned for Phase 1D")
}
```

- [ ] **Step 3: Implement writing stub**

Replace `rust/crates/falanx-engine/src/agents/writing.rs` with:

```rust
use super::AgentContext;
use crate::types::{ReviewIssue, RewritePatch};

pub async fn rewrite(
    _ctx: &AgentContext<'_>,
    _issues: &[ReviewIssue],
) -> anyhow::Result<Vec<RewritePatch>> {
    anyhow::bail!("CodeWritingAgent not yet implemented — planned for Phase 1D")
}
```

- [ ] **Step 4: Implement orchestrator stubs**

Replace `rust/crates/falanx-engine/src/orchestrator/mod.rs` with:

```rust
pub mod horizon;
pub mod pipeline;

use crate::{config::FalanxConfig, git::ReviewTarget, session::Session,
            types::{RewritePatch, ReviewScore, SessionId}};

pub struct RunConfig {
    pub target: ReviewTarget,
    pub session: Session,
}

pub struct RunResult {
    pub final_score: ReviewScore,
    pub iterations: u32,
    pub patches: Vec<RewritePatch>,
    pub session_id: SessionId,
}

pub async fn run(_config: RunConfig, _falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    anyhow::bail!("Orchestrator not yet implemented — planned for Phase 1C")
}
```

Replace `rust/crates/falanx-engine/src/orchestrator/pipeline.rs` with:

```rust
// Orchestrator state machine — Phase 1C
// Note: this file is named pipeline.rs (not loop.rs) because `loop` is a Rust keyword.
```

Replace `rust/crates/falanx-engine/src/orchestrator/horizon.rs` with:

```rust
// Horizon reset logic — Phase 1C
```

- [ ] **Step 5: Verify full build**

```bash
cd rust && cargo build
```

Expected: compiles with no errors. Dead code warnings on stubs are acceptable.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/ rust/crates/falanx-engine/src/orchestrator/
git commit -m "feat(engine): agent and orchestrator stubs — review, writing, pipeline, horizon"
```

---

### Task 7: Quality Agent (Phase 1B)

**Files:**
- Modify: `rust/crates/falanx-engine/src/agents/quality.rs`

- [ ] **Step 1: Write failing tests**

Replace `rust/crates/falanx-engine/src/agents/quality.rs` with tests first:

```rust
use super::AgentContext;
use crate::types::ReviewScore;

pub async fn score(_ctx: &AgentContext<'_>) -> anyhow::Result<ReviewScore> {
    anyhow::bail!("not implemented yet")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FalanxConfig, ProviderConfig, SessionConfig};
    use crate::git::Diff;

    fn dry_run_ctx_fixture() -> (FalanxConfig, Diff) {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "opencode/big-pickle".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: SessionConfig {
                dir: std::path::PathBuf::from("/tmp/falanx-test"),
            },
        };
        let diff = Diff("fn main() {}".into());
        (cfg, diff)
    }

    #[tokio::test]
    async fn dry_run_returns_mock_score() {
        let (cfg, diff) = dry_run_ctx_fixture();
        let ctx = AgentContext { config: &cfg, diff: &diff };
        let score = score(&ctx).await.unwrap();
        assert_eq!(score.readability, 3);
        assert_eq!(score.composite(), 3.0);
    }

    #[tokio::test]
    async fn dry_run_is_deterministic() {
        let (cfg, diff) = dry_run_ctx_fixture();
        let ctx = AgentContext { config: &cfg, diff: &diff };
        let a = score(&ctx).await.unwrap();
        let b = score(&ctx).await.unwrap();
        assert_eq!(a.readability, b.readability);
        assert_eq!(a.composite(), b.composite());
    }

    #[tokio::test]
    async fn live_mode_returns_clear_error() {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "opencode/big-pickle".into(),
                api_key: "test-key".into(),
                base_url: None,
                dry_run: false,
            },
            session: SessionConfig {
                dir: std::path::PathBuf::from("/tmp/falanx-test"),
            },
        };
        let diff = Diff("fn main() {}".into());
        let ctx = AgentContext { config: &cfg, diff: &diff };
        let err = score(&ctx).await.unwrap_err();
        assert!(err.to_string().contains("dry-run"));
    }
}
```

- [ ] **Step 2: Run to confirm tests fail**

```bash
cd rust && cargo test -p falanx-engine agents::quality 2>&1 | tail -10
```

Expected: `dry_run_returns_mock_score` and `dry_run_is_deterministic` FAIL (returns error), `live_mode_returns_clear_error` PASS.

- [ ] **Step 3: Implement quality agent**

Replace the `score` function in `rust/crates/falanx-engine/src/agents/quality.rs`:

```rust
use super::AgentContext;
use crate::types::ReviewScore;

pub async fn score(ctx: &AgentContext<'_>) -> anyhow::Result<ReviewScore> {
    if ctx.config.provider.dry_run {
        return Ok(mock_score());
    }
    anyhow::bail!(
        "live provider not yet implemented — use --dry-run. \
         Cersei integration is planned for Phase 1C."
    )
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

- [ ] **Step 4: Run tests**

```bash
cd rust && cargo test -p falanx-engine agents::quality
```

Expected:
```
test agents::quality::tests::dry_run_returns_mock_score ... ok
test agents::quality::tests::dry_run_is_deterministic ... ok
test agents::quality::tests::live_mode_returns_clear_error ... ok
```

- [ ] **Step 5: Commit**

```bash
git add rust/crates/falanx-engine/src/agents/quality.rs
git commit -m "feat(engine): CodeQualityAgent — dry-run mock scoring, live path stub"
```

---

### Task 8: CLI & falanx score (Phase 1B)

**Files:**
- Modify: `rust/crates/falanx/src/main.rs`

- [ ] **Step 1: Implement full main.rs**

Replace `rust/crates/falanx/src/main.rs` with:

```rust
use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use falanx_engine::{
    agents::{self, AgentContext},
    config::FalanxConfig,
    git::ReviewTarget,
    session::{Session, SessionEvent},
};

#[derive(Parser)]
#[command(
    name = "falanx",
    version,
    about = "Automated code review runtime"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Score code quality across five categories
    Score(ScoreArgs),
    /// Run the full review pipeline (score → critique → rewrite)
    Review(ReviewArgs),
    /// List past sessions for the current context
    ListSessions,
    /// Start falanx in serve mode (MCP + HTTP API)
    Serve(ServeArgs),
}

#[derive(Args)]
struct ScoreArgs {
    /// Path to a source file to score
    #[arg(long)]
    file: Option<PathBuf>,
    /// Git range to diff (e.g. HEAD~1, main..feature)
    #[arg(long)]
    diff: Option<String>,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct ReviewArgs {
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long)]
    diff: Option<String>,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct ServeArgs {
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct CommonArgs {
    /// Path to .env config file
    #[arg(long, env = "FALANX_CONFIG")]
    config: Option<PathBuf>,
    /// Use mock provider — no LLM calls, deterministic output
    #[arg(long, env = "FALANX_DRY_RUN")]
    dry_run: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;
    rt.block_on(run(args))
}

async fn run(args: Cli) -> anyhow::Result<()> {
    match args.command {
        Commands::Score(args) => cmd_score(args).await,
        Commands::Review(_) => {
            eprintln!("falanx review: not yet implemented — planned for Phase 1C");
            std::process::exit(1);
        }
        Commands::ListSessions => {
            eprintln!("falanx list-sessions: not yet implemented — planned for Phase 1C");
            std::process::exit(1);
        }
        Commands::Serve(_) => {
            eprintln!("falanx serve: not yet implemented — planned for Phase 1E");
            std::process::exit(0);
        }
    }
}

async fn cmd_score(args: ScoreArgs) -> anyhow::Result<()> {
    // Load and patch config
    let mut cfg = FalanxConfig::from_env()?;
    if args.common.dry_run {
        cfg.provider.dry_run = true;
    }
    cfg.validate()?;

    // Resolve review target
    let target = resolve_target(args.file, args.diff)?;
    let label = target_label(&target);

    // Open session
    let session = Session::new(&cfg.session.dir, &label)?;
    session.append(SessionEvent::RunStarted {
        session_id: session.id().0.clone(),
        target: label.clone(),
    })?;

    // Extract diff
    let diff = target.extract_diff()?;

    // Run quality agent
    session.append(SessionEvent::AgentInvoked {
        agent: "quality".into(),
        iteration: 0,
    })?;

    let ctx = AgentContext { config: &cfg, diff: &diff };
    let score = agents::quality::score(&ctx).await?;

    session.append(SessionEvent::ScoreComputed {
        score: score.clone(),
        iteration: 0,
    })?;

    session.append(SessionEvent::RunCompleted {
        final_score: score.clone(),
        iterations: 1,
    })?;

    // Print report
    println!("falanx score report");
    println!("-------------------");
    println!("readability:     {}/5", score.readability);
    println!("maintainability: {}/5", score.maintainability);
    println!("performance:     {}/5", score.performance);
    println!("security:        {}/5", score.security);
    println!("architecture:    {}/5", score.architecture);
    println!("-------------------");
    println!("composite:       {:.1}/5", score.composite());
    println!();
    println!("session: {}", session.path().display());

    Ok(())
}

fn resolve_target(
    file: Option<PathBuf>,
    diff: Option<String>,
) -> anyhow::Result<ReviewTarget> {
    match (file, diff) {
        (Some(path), None) => Ok(ReviewTarget::File(path)),
        (None, Some(range)) => Ok(ReviewTarget::GitRange(range)),
        (Some(_), Some(_)) => anyhow::bail!("specify --file or --diff, not both"),
        (None, None) => anyhow::bail!("specify --file <path> or --diff <range>"),
    }
}

fn target_label(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::File(path) => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        ReviewTarget::GitRange(range) => range.clone(),
    }
}
```

- [ ] **Step 2: Build**

```bash
cd rust && cargo build
```

Expected: compiles with no errors.

- [ ] **Step 3: Manual acceptance test — score a file**

```bash
# Create a test file
echo 'fn add(a: i32, b: i32) -> i32 { a + b }' > /tmp/test.rs

# Run score in dry-run mode
cd rust && cargo run --bin falanx -- score --file /tmp/test.rs --dry-run
```

Expected output:
```
falanx score report
-------------------
readability:     3/5
maintainability: 3/5
performance:     3/5
security:        3/5
architecture:    3/5
-------------------
composite:       3.0/5

session: /root/.falanx/sessions/test.rs/<uuid>.jsonl
```

- [ ] **Step 4: Verify session file written**

```bash
cat ~/.falanx/sessions/test.rs/*.jsonl
```

Expected: 4 JSON lines, each with `timestamp` and `type` fields:
```json
{"timestamp":"...","type":"run_started","session_id":"...","target":"test.rs"}
{"timestamp":"...","type":"agent_invoked","agent":"quality","iteration":0}
{"timestamp":"...","type":"score_computed","score":{...},"iteration":0}
{"timestamp":"...","type":"run_completed","final_score":{...},"iterations":1}
```

- [ ] **Step 5: Verify help renders**

```bash
cd rust && cargo run --bin falanx -- --help
cd rust && cargo run --bin falanx -- score --help
```

Expected: clap-generated help text with all subcommands and flags listed.

- [ ] **Step 6: Verify serve stub exits 0**

```bash
cd rust && cargo run --bin falanx -- serve
```

Expected: prints "falanx serve: not yet implemented — planned for Phase 1E" and exits 0.

- [ ] **Step 7: Verify live mode rejects missing key**

```bash
cd rust && cargo run --bin falanx -- score --file /tmp/test.rs
```

Expected: error message containing "OPENCODE_API_KEY is required".

- [ ] **Step 8: Run all tests**

```bash
cd rust && cargo test
```

Expected: all tests pass, no failures.

- [ ] **Step 9: Commit**

```bash
git add rust/crates/falanx/src/main.rs
git commit -m "feat(cli): falanx score command — dry-run scoring, session JSONL, CLI help"
```

---

### Task 9: Final verification

- [ ] **Step 1: Clean build**

```bash
cd rust && cargo build --release 2>&1 | grep -E "^error"
```

Expected: no output (no errors).

- [ ] **Step 2: Full test suite**

```bash
cd rust && cargo test 2>&1 | tail -20
```

Expected: all tests pass. Count shown at end like `test result: ok. N passed; 0 failed`.

- [ ] **Step 3: Clippy**

```bash
cd rust && cargo clippy -- -D warnings
```

Expected: no warnings promoted to errors.

- [ ] **Step 4: Final acceptance test**

```bash
echo 'pub fn greet(name: &str) -> String { format!("Hello, {}!", name) }' > /tmp/acceptance.rs
cd rust && cargo run --bin falanx -- score --file /tmp/acceptance.rs --dry-run
ls -la ~/.falanx/sessions/acceptance.rs/
```

Expected: score report printed, session file present.

- [ ] **Step 5: Final commit**

```bash
git add -u
git commit -m "chore: phase 1A+1B complete — falanx score --dry-run working end-to-end"
```
