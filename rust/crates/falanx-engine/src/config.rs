use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FalanxConfig {
    pub provider: ProviderConfig,
    pub session: SessionConfig,
    pub loop_cfg: LoopConfig,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub dry_run: bool,
}

impl ProviderConfig {
    /// Returns true for providers that don't require an API key (e.g. local Ollama).
    /// Matches the `env_keys: &[]` entries in cersei-provider's registry.
    pub fn is_keyless_local(&self) -> bool {
        self.model.starts_with("ollama/")
    }
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LoopConfig {
    pub max_iter: u32,
    pub target_score: f32,
    pub plateau_threshold: f32,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self { max_iter: 3, target_score: 4.5, plateau_threshold: 0.1 }
    }
}

impl FalanxConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        // Load .env file if present, ignore error if missing
        let _ = dotenvy::dotenv();

        let model = std::env::var("FALANX_MODEL")
            .unwrap_or_else(|_| "anthropic/claude-opus-4-7".into());

        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();

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

        let max_iter = std::env::var("FALANX_MAX_ITER")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3u32);

        let target_score = std::env::var("FALANX_TARGET_SCORE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4.5f32);

        let plateau_threshold = std::env::var("FALANX_PLATEAU_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.1f32);

        Ok(Self {
            provider: ProviderConfig { model, api_key, base_url, dry_run },
            session: SessionConfig { dir },
            loop_cfg: LoopConfig { max_iter, target_score, plateau_threshold },
        })
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.provider.dry_run && self.provider.api_key.is_empty() && !self.provider.is_keyless_local() {
            anyhow::bail!(
                "ANTHROPIC_API_KEY is required when not in dry-run mode. \
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
        unsafe {
            std::env::remove_var("FALANX_MODEL");
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("FALANX_BASE_URL");
            std::env::remove_var("FALANX_DRY_RUN");
            std::env::remove_var("FALANX_SESSION_DIR");
        }

        let cfg = FalanxConfig::from_env().unwrap();
        assert_eq!(cfg.provider.model, "anthropic/claude-opus-4-7");
        assert_eq!(cfg.provider.api_key, "");
        assert!(!cfg.provider.dry_run);
        assert!(cfg.provider.base_url.is_none());
    }

    #[test]
    fn validate_rejects_empty_api_key_when_not_dry_run() {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "anthropic/claude-opus-4-7".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: false,
            },
            session: SessionConfig {
                dir: PathBuf::from("/tmp/falanx-test"),
            },
            loop_cfg: LoopConfig::default(),
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_accepts_empty_api_key_in_dry_run() {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "anthropic/claude-opus-4-7".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: SessionConfig {
                dir: PathBuf::from("/tmp/falanx-test"),
            },
            loop_cfg: LoopConfig::default(),
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_accepts_empty_api_key_for_ollama() {
        let cfg = FalanxConfig {
            provider: ProviderConfig {
                model: "ollama/llama3.1".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: false,
            },
            session: SessionConfig {
                dir: PathBuf::from("/tmp/falanx-test"),
            },
            loop_cfg: LoopConfig::default(),
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn is_keyless_local_matches_ollama_prefix() {
        let ollama = ProviderConfig { model: "ollama/llama3.1".into(), api_key: "".into(), base_url: None, dry_run: false };
        assert!(ollama.is_keyless_local());
        let remote = ProviderConfig { model: "anthropic/claude-opus-4-7".into(), api_key: "".into(), base_url: None, dry_run: false };
        assert!(!remote.is_keyless_local());
    }

    #[test]
    fn loop_config_defaults() {
        let cfg = LoopConfig::default();
        assert_eq!(cfg.max_iter, 3);
        assert!((cfg.target_score - 4.5).abs() < f32::EPSILON);
        assert!((cfg.plateau_threshold - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn loop_config_from_env_reads_vars() {
        unsafe {
            std::env::set_var("FALANX_MAX_ITER", "5");
            std::env::set_var("FALANX_TARGET_SCORE", "4.0");
            std::env::set_var("FALANX_PLATEAU_THRESHOLD", "0.05");
        }
        let cfg = FalanxConfig::from_env().unwrap();
        assert_eq!(cfg.loop_cfg.max_iter, 5);
        assert!((cfg.loop_cfg.target_score - 4.0).abs() < f32::EPSILON);
        assert!((cfg.loop_cfg.plateau_threshold - 0.05).abs() < f32::EPSILON);
        unsafe {
            std::env::remove_var("FALANX_MAX_ITER");
            std::env::remove_var("FALANX_TARGET_SCORE");
            std::env::remove_var("FALANX_PLATEAU_THRESHOLD");
        }
    }
}
