use cersei_agent::Agent;

use crate::{
    agents::{self, AgentContext},
    config::FalanxConfig,
    orchestrator::{horizon::HorizonState, RunConfig, RunResult},
    provider::MockProvider,
    session::SessionEvent,
    types::RewritePatch,
};

fn build_provider(falanx_cfg: &FalanxConfig) -> anyhow::Result<(Box<dyn cersei_provider::Provider>, String)> {
    if falanx_cfg.provider.dry_run {
        Ok((Box::new(MockProvider), falanx_cfg.provider.model.clone()))
    } else {
        let (p, model_id) = cersei_provider::from_model_string(&falanx_cfg.provider.model)?;
        Ok((p, model_id))
    }
}

fn build_agent(falanx_cfg: &FalanxConfig) -> anyhow::Result<Agent> {
    let (provider, model_id) = build_provider(falanx_cfg)?;
    Agent::builder()
        .provider_boxed(provider)
        .model(&model_id)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build agent: {}", e))
}

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    let RunConfig { target, loop_cfg, session } = config;
    let session_id = session.id().clone();

    let diff = target.extract_diff()?;

    session.append(SessionEvent::RunStarted {
        session_id: session_id.0.clone(),
        target: "review".to_string(),
    })?;

    let mut horizon = HorizonState::new();
    let mut iteration: u32 = 0;
    let mut prev_score = None;
    let mut all_patches: Vec<RewritePatch> = vec![];
    let mut plateau_streak: u32 = 0;

    // Initial score
    let mut current_score = {
        session.append(SessionEvent::AgentInvoked { agent: "quality".into(), iteration })?;
        let agent = build_agent(falanx_cfg)?;
        let ctx = AgentContext { agent: &agent, diff: &diff };
        let s = agents::quality::score(&ctx).await?;
        session.append(SessionEvent::ScoreComputed { score: s.clone(), iteration })?;
        s
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
        let issues = agents::review::critique(&ctx, &current_score).await?;
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
        let agent = build_agent(falanx_cfg)?;
        let ctx = AgentContext { agent: &agent, diff: &diff };
        let new_score = agents::quality::score(&ctx).await?;
        session.append(SessionEvent::ScoreComputed { score: new_score.clone(), iteration })?;

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
                break; // exhausted resets
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{LoopConfig, ProviderConfig, SessionConfig},
        git::ReviewTarget,
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
    async fn pipeline_runs_dry_run_and_returns_result() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = dry_run_cfg();
        cfg.loop_cfg.max_iter = 1;
        cfg.session.dir = dir.path().to_path_buf();

        let f = dir.path().join("test.rs");
        std::fs::write(&f, "fn main() {}").unwrap();

        let session = Session::new(&cfg.session.dir, "test").unwrap();
        let run_cfg = RunConfig {
            target: ReviewTarget::File(f),
            loop_cfg: cfg.loop_cfg.clone(),
            session,
        };

        let result = run(run_cfg, &cfg).await.unwrap();
        assert_eq!(result.iterations, 1);
        assert!(result.final_score.composite() > 0.0);
    }
}
