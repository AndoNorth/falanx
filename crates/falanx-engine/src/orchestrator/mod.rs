pub mod horizon;

use crate::{
    config::{FalanxConfig, LoopConfig},
    git::ReviewTarget,
    session::{Session, SessionEvent},
    types::SessionId,
    workflow::{WorkflowConfig, runner::WorkflowRunner},
};

pub struct RunConfig {
    pub target: ReviewTarget,
    pub loop_cfg: LoopConfig,
    pub session: Session,
    pub agents_dir: Option<std::path::PathBuf>,
}

pub struct RunResult {
    pub iterations: u32,
    pub session_id: SessionId,
    pub summary: serde_json::Value,
}

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    let agents_dir = config.agents_dir.clone().unwrap_or_else(resolve_agents_dir);
    let workflow = WorkflowConfig::load(Some(&agents_dir))
        .map_err(|e| anyhow::anyhow!("failed to load workflow: {}", e))?;
    run_with_workflow_and_agents(config, falanx_cfg, workflow, &agents_dir).await
}

pub async fn run_with_workflow(
    config: RunConfig,
    falanx_cfg: &FalanxConfig,
    workflow_yaml: &str,
    agents_dir: &std::path::Path,
) -> anyhow::Result<RunResult> {
    let workflow = WorkflowConfig::from_yaml(workflow_yaml)?;
    run_with_workflow_and_agents(config, falanx_cfg, workflow, agents_dir).await
}

async fn run_with_workflow_and_agents(
    config: RunConfig,
    falanx_cfg: &FalanxConfig,
    mut workflow: WorkflowConfig,
    agents_dir: &std::path::Path,
) -> anyhow::Result<RunResult> {
    let RunConfig {
        target,
        loop_cfg,
        session,
        ..
    } = config;
    let session_id = session.id().clone();

    let diff = target.extract_diff()?;
    let max_diff_chars: usize = std::env::var("FALANX_MAX_DIFF_CHARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    let diff_text: String = if diff.0.len() > max_diff_chars {
        tracing::warn!(
            original = diff.0.len(),
            truncated_to = max_diff_chars,
            "diff truncated"
        );
        diff.0.chars().take(max_diff_chars).collect()
    } else {
        diff.0.clone()
    };

    // Apply CLI loop overrides onto workflow defaults
    workflow.loop_cfg.max_iterations = loop_cfg.max_iter;
    workflow.loop_cfg.target_score = loop_cfg.target_score;
    workflow.loop_cfg.plateau_threshold = loop_cfg.plateau_threshold;

    session.append(SessionEvent::RunStarted {
        session_id: session_id.0.clone(),
        target: "run".to_string(),
        workflow: "default".to_string(),
    })?;

    let runner = WorkflowRunner::new(&workflow, falanx_cfg, &session, agents_dir);
    match runner.run(&diff_text).await {
        Ok(result) => Ok(result),
        Err(err) => {
            // Best-effort: if the JSONL write itself fails, the original run error is still
            // what gets returned - we don't want a logging failure to mask the real one.
            let _ = session.append(SessionEvent::RunFailed {
                reason: err.to_string(),
            });
            Err(err)
        }
    }
}

fn resolve_agents_dir() -> std::path::PathBuf {
    let local = std::path::PathBuf::from(".falanx/agents");
    if local.exists() {
        return local;
    }
    // Fall back to defaults dir relative to binary location
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let candidate = parent.join("defaults/agents");
        if candidate.exists() {
            return candidate;
        }
    }
    local
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{FalanxConfig, LoopConfig, ProviderConfig, SessionConfig},
        git::ReviewTarget,
        session::Session,
    };

    fn dry_cfg(dir: &std::path::Path) -> FalanxConfig {
        FalanxConfig {
            provider: ProviderConfig {
                model: "mock".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: SessionConfig {
                dir: dir.to_path_buf(),
            },
            loop_cfg: LoopConfig::default(),
        }
    }

    #[tokio::test]
    async fn run_returns_result_with_session_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = dry_cfg(dir.path());
        cfg.loop_cfg.max_iter = 1;

        let f = dir.path().join("test.rs");
        std::fs::write(&f, "fn main() {}").unwrap();

        // Create a minimal agents dir for this test
        let agents_dir = dir.path().join(".falanx/agents");
        std::fs::create_dir_all(agents_dir.join("score_readability")).unwrap();
        std::fs::write(
            agents_dir.join("score_readability/system.md"),
            "You are a code quality reviewer.",
        )
        .unwrap();
        std::fs::write(agents_dir.join("score_readability/prompt.md"), "{{ diff }}").unwrap();
        std::fs::write(
            agents_dir.join("score_readability/config.yaml"),
            "kind: cersei\nmax_turns: 1",
        )
        .unwrap();

        let session = Session::new(&cfg.session.dir, "test").unwrap();
        let run_cfg = RunConfig {
            target: ReviewTarget::File(f),
            loop_cfg: cfg.loop_cfg.clone(),
            session,
            agents_dir: Some(agents_dir.clone()),
        };

        // Run with a minimal workflow that only has one agent
        let result = run_with_workflow(
            run_cfg,
            &cfg,
            r#"
loop:
  max_iterations: 1
  target_score: 5.0
  plateau_threshold: 0.1
stages:
  - id: score
    agents: [score_readability]
    output_format: json_object
    output_as: score_result
"#,
            &agents_dir,
        )
        .await
        .unwrap();

        assert_eq!(result.iterations, 1);
        assert!(!result.session_id.0.is_empty());
    }

    #[tokio::test]
    async fn run_appends_run_failed_event_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dry_cfg(dir.path());

        let f = dir.path().join("test.rs");
        std::fs::write(&f, "fn main() {}").unwrap();

        // Deliberately empty - no agent directories exist here, so load_from_dir inside
        // run_stage will fail and the run should end in an Err.
        let agents_dir = dir.path().join(".falanx/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let session = Session::new(&cfg.session.dir, "failure-test").unwrap();
        let session_path = session.path().to_path_buf();
        let run_cfg = RunConfig {
            target: ReviewTarget::File(f),
            loop_cfg: cfg.loop_cfg.clone(),
            session,
            agents_dir: Some(agents_dir.clone()),
        };

        let result = run_with_workflow(
            run_cfg,
            &cfg,
            r#"
loop:
  max_iterations: 1
  target_score: 5.0
  plateau_threshold: 0.1
stages:
  - id: score
    agents: [score_readability]
    output_format: json_object
    output_as: score_result
"#,
            &agents_dir,
        )
        .await;

        assert!(result.is_err());

        let content = std::fs::read_to_string(&session_path).unwrap();
        assert!(
            content.contains("run_failed"),
            "expected a run_failed event in the session file, got: {content}"
        );
    }
}
