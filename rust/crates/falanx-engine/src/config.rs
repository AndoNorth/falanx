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
        unsafe {
            std::env::remove_var("FALANX_MODEL");
            std::env::remove_var("OPENCODE_API_KEY");
            std::env::remove_var("FALANX_BASE_URL");
            std::env::remove_var("FALANX_DRY_RUN");
            std::env::remove_var("FALANX_SESSION_DIR");
        }

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
