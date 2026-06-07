use std::path::{Path, PathBuf};

use crate::{
    agent::{loader::load_from_dir, run::run_agent},
    config::FalanxConfig,
    orchestrator,
    session::{Session, SessionEvent},
    types::AgentRunResult,
    workflow::{
        context::TemplateContext,
        stage::{StageConfig, StageResult},
        WorkflowConfig,
    },
};
use crate::orchestrator::horizon::HorizonState;

pub struct WorkflowRunner<'a> {
    workflow: &'a WorkflowConfig,
    cfg: &'a FalanxConfig,
    session: &'a Session,
    agents_dir: PathBuf,
}

impl<'a> WorkflowRunner<'a> {
    pub fn new(
        workflow: &'a WorkflowConfig,
        cfg: &'a FalanxConfig,
        session: &'a Session,
        agents_dir: &Path,
    ) -> Self {
        Self { workflow, cfg, session, agents_dir: agents_dir.to_path_buf() }
    }

    pub async fn run(&self, diff: &str) -> anyhow::Result<orchestrator::RunResult> {
        let session_id = self.session.id().clone();
        let mut horizon = HorizonState::new();
        let mut iteration = 0u32;
        let mut prev_composite: Option<f32> = None;
        let mut plateau_streak = 0u32;
        let mut final_ctx = TemplateContext::new(diff.to_string());

        loop {
            let mut ctx = TemplateContext::new(diff.to_string());
            ctx.loop_iteration = iteration;

            for stage_cfg in &self.workflow.stages {
                if let Some(skip_if) = &stage_cfg.skip_if {
                    if skip_if.evaluate(&ctx, self.workflow.loop_cfg.target_score) {
                        self.session.append(SessionEvent::StageSkipped {
                            stage_id: stage_cfg.id.clone(),
                            condition: format!("{:?}", skip_if),
                            iteration,
                        })?;
                        continue;
                    }
                }
                let stage_result = self.run_stage(stage_cfg, &ctx).await?;
                ctx.stages.insert(stage_cfg.output_as.clone(), stage_result.synthesised.clone());
                self.session.append(SessionEvent::StageCompleted {
                    stage_id: stage_cfg.id.clone(),
                    output_as: stage_cfg.output_as.clone(),
                    result: stage_result.synthesised,
                    iteration,
                })?;
            }

            final_ctx = ctx.clone();
            let composite = composite_from_ctx(&final_ctx);

            if composite >= self.workflow.loop_cfg.target_score {
                break;
            }
            if iteration >= self.workflow.loop_cfg.max_iterations {
                break;
            }

            if let Some(prev) = prev_composite {
                if (composite - prev).abs() < self.workflow.loop_cfg.plateau_threshold {
                    plateau_streak += 1;
                } else {
                    plateau_streak = 0;
                }
            }
            prev_composite = Some(composite);

            if plateau_streak >= 2 {
                if horizon.should_reset(true) {
                    horizon.record_reset();
                    plateau_streak = 0;
                    prev_composite = None;
                    self.session.append(SessionEvent::HorizonReset { iteration })?;
                } else {
                    break;
                }
            }

            iteration += 1;
        }

        let summary = serde_json::to_value(&final_ctx.stages).unwrap_or_default();
        self.session.append(SessionEvent::RunCompleted {
            iterations: iteration,
            summary: summary.clone(),
        })?;

        Ok(orchestrator::RunResult { iterations: iteration, session_id, summary })
    }

    async fn run_stage(&self, stage_cfg: &StageConfig, ctx: &TemplateContext) -> anyhow::Result<StageResult> {
        self.session.append(SessionEvent::StageStarted {
            stage_id: stage_cfg.id.clone(),
            iteration: ctx.loop_iteration,
        })?;

        let mut runs: Vec<AgentRunResult> = Vec::new();

        for agent_name in stage_cfg.agent_names() {
            let def = load_from_dir(&self.agents_dir, &agent_name)?;
            let rendered = ctx.render(&def.prompt_template)?;

            self.session.append(SessionEvent::AgentStarted {
                stage_id: stage_cfg.id.clone(),
                agent_name: agent_name.clone(),
                iteration: ctx.loop_iteration,
            })?;

            let result = match run_agent(&def, &rendered, self.cfg, &stage_cfg.output_format).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(agent = %agent_name, error = %e, "agent run failed; using null output");
                    AgentRunResult {
                        agent_name: agent_name.clone(),
                        raw_output: String::new(),
                        extracted: serde_json::Value::Null,
                        turns_used: 0,
                    }
                }
            };

            self.session.append(SessionEvent::AgentCompleted {
                stage_id: stage_cfg.id.clone(),
                agent_name: agent_name.clone(),
                turns_used: result.turns_used,
                output: result.extracted.clone(),
                iteration: ctx.loop_iteration,
            })?;

            runs.push(result);
        }

        let synthesised = synthesise_runs(&runs);

        Ok(StageResult { stage_id: stage_cfg.id.clone(), runs, synthesised })
    }
}

fn synthesise_runs(runs: &[AgentRunResult]) -> serde_json::Value {
    if runs.len() == 1 {
        runs[0].extracted.clone()
    } else {
        let arr = runs.iter().map(|r| {
            let mut obj = r.extracted.as_object().cloned().unwrap_or_default();
            obj.insert("agent".to_string(), serde_json::Value::String(r.agent_name.clone()));
            serde_json::Value::Object(obj)
        }).collect();
        serde_json::Value::Array(arr)
    }
}

fn composite_from_ctx(ctx: &TemplateContext) -> f32 {
    let arr = match ctx.stages.get("score_result").and_then(|v| v.as_array()) {
        Some(a) if !a.is_empty() => a,
        _ => return 0.0,
    };
    let sum: f32 = arr.iter()
        .filter_map(|v| v.get("score").and_then(|s| s.as_f64()))
        .map(|s| s as f32)
        .sum();
    sum / arr.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{FalanxConfig, LoopConfig, ProviderConfig, SessionConfig},
        session::Session,
        workflow::WorkflowConfig,
    };

    fn dry_cfg() -> FalanxConfig {
        FalanxConfig {
            provider: ProviderConfig { model: "mock".into(), api_key: "".into(), base_url: None, dry_run: true },
            session: SessionConfig { dir: std::path::PathBuf::from("/tmp") },
            loop_cfg: LoopConfig::default(),
        }
    }

    #[tokio::test]
    async fn runner_executes_single_stage_dry_run() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dry_cfg();
        let session = Session::new(dir.path(), "test").unwrap();

        let yaml = r#"
loop:
  max_iterations: 1
  target_score: 5.0
  plateau_threshold: 0.1
stages:
  - id: score
    agents: [score_readability]
    output_format: json_object
    output_as: score_result
"#;
        let workflow = WorkflowConfig::from_str(yaml).unwrap();

        let agents_dir = dir.path().join("agents");
        std::fs::create_dir_all(agents_dir.join("score_readability")).unwrap();
        std::fs::write(agents_dir.join("score_readability/system.md"), "sys").unwrap();
        std::fs::write(agents_dir.join("score_readability/prompt.md"), "score: {{ diff }}").unwrap();
        std::fs::write(agents_dir.join("score_readability/config.yaml"), "kind: cersei\nmax_turns: 1").unwrap();

        let runner = WorkflowRunner::new(&workflow, &cfg, &session, &agents_dir);
        let result = runner.run("fn foo() {}").await.unwrap();

        assert_eq!(result.iterations, 1);

        let content = std::fs::read_to_string(session.path()).unwrap();
        assert!(content.contains("stage_started"));
        assert!(content.contains("agent_completed"));
        assert!(content.contains("stage_completed"));
        assert!(content.contains("run_completed"));
    }
}
