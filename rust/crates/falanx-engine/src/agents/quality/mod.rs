pub mod config;
pub use config::ScoringConfig;
pub(crate) mod pipeline;
pub(crate) mod synthesis;

use crate::{
    config::FalanxConfig,
    git::Diff,
    session::{Session, SessionEvent},
    types::ScoringResult,
};

pub async fn score(
    diff: &Diff,
    falanx_cfg: &FalanxConfig,
    scoring_cfg: &ScoringConfig,
    session: &Session,
    iteration: u32,
) -> anyhow::Result<ScoringResult> {
    tracing::info!(
        pipeline = %scoring_cfg.pipeline.name,
        categories = scoring_cfg.pipeline.categories.len(),
        iteration = iteration,
        "scoring pipeline started"
    );

    let category_results = pipeline::run_categories(
        diff,
        falanx_cfg,
        &scoring_cfg.pipeline,
        &scoring_cfg.categories,
        session,
        iteration,
    )
    .await?;

    let (provider, model_id) = build_provider(falanx_cfg)?;
    let synthesis_agent = cersei_agent::Agent::builder()
        .provider_boxed(provider)
        .model(&model_id)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build synthesis agent: {}", e))?;

    let result = synthesis::synthesize(
        &synthesis_agent,
        &scoring_cfg.pipeline.name,
        category_results,
    )
    .await?;

    session.append(SessionEvent::ScoringComplete {
        pipeline_name: result.pipeline_name.clone(),
        composite_score: result.composite_score,
        synthesis: result.synthesis.clone(),
        iteration,
    })?;

    tracing::info!(
        composite = result.composite_score,
        pipeline = %result.pipeline_name,
        iteration = iteration,
        "scoring pipeline complete"
    );

    Ok(result)
}

fn build_provider(
    cfg: &FalanxConfig,
) -> anyhow::Result<(Box<dyn cersei_provider::Provider>, String)> {
    if cfg.provider.dry_run {
        Ok((
            Box::new(crate::provider::MockProvider),
            cfg.provider.model.clone(),
        ))
    } else {
        cersei_provider::from_model_string(&cfg.provider.model)
            .map_err(|e| anyhow::anyhow!("provider error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FalanxConfig, LoopConfig, ProviderConfig, SessionConfig};

    #[tokio::test]
    async fn score_runs_full_pipeline_with_mock_provider() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::new(dir.path(), "test").unwrap();

        let falanx_cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "mock".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: SessionConfig { dir: dir.path().to_path_buf() },
            loop_cfg: LoopConfig::default(),
        };

        unsafe {
            std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
            std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
            std::env::set_var("FALANX_SCORING_PIPELINE", "quick");
        }
        let scoring_cfg = ScoringConfig::load().unwrap();
        unsafe { std::env::remove_var("FALANX_SCORING_PIPELINE"); }

        let diff = Diff("fn add(a: i32, b: i32) -> i32 { a + b }".into());
        let result = score(&diff, &falanx_cfg, &scoring_cfg, &session, 0).await.unwrap();

        assert_eq!(result.pipeline_name, "quick");
        assert_eq!(result.categories.len(), 2);
        assert!(result.composite_score > 0.0);
        assert!(!result.synthesis.is_empty());

        let content = std::fs::read_to_string(session.path()).unwrap();
        assert!(content.contains("category_scored"));
        assert!(content.contains("scoring_complete"));
    }
}
