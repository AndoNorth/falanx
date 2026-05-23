use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

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

#[derive(Debug)]
pub struct SessionMeta {
    pub id: SessionId,
    pub path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub final_score: Option<ReviewScore>,
    pub iterations: Option<u32>,
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

    /// List all completed sessions in a directory, sorted newest-first.
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

            // First line → RunStarted (has timestamp and session_id)
            let mut first_line = String::new();
            if reader.read_line(&mut first_line).is_err() || first_line.trim().is_empty() {
                continue;
            }
            let first: serde_json::Value = match serde_json::from_str(first_line.trim()) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let started_at: DateTime<Utc> = match first
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
            {
                Some(t) => t,
                None => continue,
            };
            let id_str = first
                .get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Read all remaining lines, keep the last non-empty one
            let mut last_line = String::new();
            let mut buf = String::new();
            loop {
                buf.clear();
                match reader.read_line(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if !buf.trim().is_empty() {
                            last_line = buf.trim().to_string();
                        }
                    }
                    Err(_) => break,
                }
            }

            let (final_score, iterations) = if last_line.is_empty() {
                (None, None)
            } else {
                match serde_json::from_str::<SessionEntry>(&last_line) {
                    Ok(entry) => match entry.event {
                        SessionEvent::RunCompleted {
                            final_score,
                            iterations,
                        } => (Some(final_score), Some(iterations)),
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

    #[test]
    fn list_returns_empty_for_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = Session::list(dir.path()).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn list_finds_completed_session() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::new(dir.path(), "test").unwrap();

        session
            .append(SessionEvent::RunStarted {
                session_id: session.id().0.clone(),
                target: "test.rs".into(),
            })
            .unwrap();
        session
            .append(SessionEvent::RunCompleted {
                final_score: crate::types::ReviewScore {
                    readability: 3,
                    maintainability: 3,
                    performance: 3,
                    security: 3,
                    architecture: 3,
                },
                iterations: 1,
            })
            .unwrap();

        let sessions = Session::list(dir.path()).unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].final_score.is_some());
        assert_eq!(sessions[0].iterations, Some(1));
    }

    #[test]
    fn list_skips_malformed_files() {
        let dir = tempfile::tempdir().unwrap();
        let bad_dir = dir.path().join("bad");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(bad_dir.join("bad.jsonl"), "not json\n").unwrap();

        let sessions = Session::list(dir.path()).unwrap();
        assert!(sessions.is_empty());
    }
}
