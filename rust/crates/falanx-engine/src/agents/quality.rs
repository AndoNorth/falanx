use super::AgentContext;
use crate::types::ReviewScore;

pub async fn score(ctx: &AgentContext<'_>) -> anyhow::Result<ReviewScore> {
    if ctx.config.provider.dry_run {
        tracing::debug!(model = %ctx.config.provider.model, "dry-run: returning mock score");
        return Ok(mock_score());
    }
    tracing::error!(model = %ctx.config.provider.model, "live provider not implemented");
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
