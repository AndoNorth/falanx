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

        tracing::info!(session_id = %id.0, path = %path.display(), "session created");
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
