use std::collections::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CategoryConfig {
    pub system_prompt: String,
    pub prompt: String,
    pub max_reasoning_chars: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawPipelineConfig {
    pub parallel: bool,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub name: String,
    pub parallel: bool,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScoringConfig {
    pub categories: HashMap<String, CategoryConfig>,
    pub pipeline: PipelineConfig,
}

impl ScoringConfig {
    pub fn load() -> anyhow::Result<Self> {
        let categories_yaml = Self::load_categories_yaml()?;
        let pipelines_yaml = Self::load_pipelines_yaml()?;
        let pipeline_name = std::env::var("FALANX_SCORING_PIPELINE")
            .unwrap_or_else(|_| "default".into());

        let categories: HashMap<String, CategoryConfig> =
            serde_yaml::from_str(&categories_yaml)
                .map_err(|e| anyhow::anyhow!("invalid categories config: {}", e))?;

        let pipelines: HashMap<String, RawPipelineConfig> =
            serde_yaml::from_str(&pipelines_yaml)
                .map_err(|e| anyhow::anyhow!("invalid pipelines config: {}", e))?;

        let raw_pipeline = pipelines
            .get(&pipeline_name)
            .ok_or_else(|| anyhow::anyhow!("pipeline '{}' not found in pipelines config", pipeline_name))?;

        for cat_name in &raw_pipeline.categories {
            if !categories.contains_key(cat_name) {
                anyhow::bail!(
                    "category '{}' in pipeline '{}' not found in categories registry",
                    cat_name,
                    pipeline_name
                );
            }
        }

        Ok(Self {
            categories,
            pipeline: PipelineConfig {
                name: pipeline_name,
                parallel: raw_pipeline.parallel,
                categories: raw_pipeline.categories.clone(),
            },
        })
    }

    fn load_categories_yaml() -> anyhow::Result<String> {
        if let Ok(path) = std::env::var("FALANX_SCORING_CATEGORIES_CONFIG") {
            std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read categories config '{}': {}", path, e))
        } else {
            Ok(include_str!("defaults/categories.yaml").to_string())
        }
    }

    fn load_pipelines_yaml() -> anyhow::Result<String> {
        if let Ok(path) = std::env::var("FALANX_SCORING_PIPELINES_CONFIG") {
            std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read pipelines config '{}': {}", path, e))
        } else {
            Ok(include_str!("defaults/pipelines.yaml").to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn category_config_deserialises_from_yaml() {
        let yaml = r#"
readability:
  system_prompt: "test system"
  prompt: "test prompt"
  max_reasoning_chars: 200
"#;
        let cats: HashMap<String, CategoryConfig> = serde_yaml::from_str(yaml).unwrap();
        let r = cats.get("readability").unwrap();
        assert_eq!(r.system_prompt, "test system");
        assert_eq!(r.max_reasoning_chars, 200);
    }

    #[test]
    fn pipeline_config_deserialises_from_yaml() {
        let yaml = r#"
default:
  parallel: false
  categories: [readability, maintainability]
"#;
        let pipelines: HashMap<String, RawPipelineConfig> = serde_yaml::from_str(yaml).unwrap();
        let p = pipelines.get("default").unwrap();
        assert_eq!(p.categories.len(), 2);
        assert!(!p.parallel);
    }

    #[test]
    #[serial]
    fn scoring_config_loads_embedded_defaults() {
        unsafe {
            std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
            std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
            std::env::remove_var("FALANX_SCORING_PIPELINE");
        }
        let cfg = ScoringConfig::load().unwrap();
        assert!(cfg.categories.contains_key("readability"));
        assert!(cfg.categories.contains_key("security"));
        assert_eq!(cfg.pipeline.name, "default");
        assert_eq!(cfg.pipeline.categories.len(), 5);
    }

    #[test]
    #[serial]
    fn scoring_config_selects_quick_pipeline() {
        unsafe {
            std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
            std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
            std::env::set_var("FALANX_SCORING_PIPELINE", "quick");
        }
        let cfg = ScoringConfig::load().unwrap();
        assert_eq!(cfg.pipeline.name, "quick");
        assert_eq!(cfg.pipeline.categories.len(), 2);
        unsafe { std::env::remove_var("FALANX_SCORING_PIPELINE"); }
    }

    #[test]
    #[serial]
    fn scoring_config_rejects_unknown_pipeline() {
        unsafe {
            std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
            std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
            std::env::set_var("FALANX_SCORING_PIPELINE", "nonexistent");
        }
        let err = ScoringConfig::load().unwrap_err();
        assert!(err.to_string().contains("not found in pipelines config"));
        unsafe { std::env::remove_var("FALANX_SCORING_PIPELINE"); }
    }

    #[test]
    #[serial]
    fn scoring_config_rejects_pipeline_with_unknown_category() {
        let dir = tempfile::tempdir().unwrap();
        let cats_path = dir.path().join("cats.yaml");
        let pipes_path = dir.path().join("pipes.yaml");
        std::fs::write(&cats_path, "readability:\n  system_prompt: s\n  prompt: p\n  max_reasoning_chars: 100\n").unwrap();
        std::fs::write(&pipes_path, "custom:\n  parallel: false\n  categories: [readability, nonexistent]\n").unwrap();

        unsafe {
            std::env::set_var("FALANX_SCORING_CATEGORIES_CONFIG", cats_path.to_str().unwrap());
            std::env::set_var("FALANX_SCORING_PIPELINES_CONFIG", pipes_path.to_str().unwrap());
            std::env::set_var("FALANX_SCORING_PIPELINE", "custom");
        }
        let err = ScoringConfig::load().unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
        unsafe {
            std::env::remove_var("FALANX_SCORING_CATEGORIES_CONFIG");
            std::env::remove_var("FALANX_SCORING_PIPELINES_CONFIG");
            std::env::remove_var("FALANX_SCORING_PIPELINE");
        }
    }
}
