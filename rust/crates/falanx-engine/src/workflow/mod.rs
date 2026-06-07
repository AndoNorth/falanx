pub mod context;
pub mod runner;
pub mod stage;

use stage::StageConfig;

const DEFAULT_WORKFLOW: &str = include_str!("../defaults/workflow.yaml");

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LoopConfig {
    pub max_iterations: u32,
    pub target_score: f32,
    pub plateau_threshold: f32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorkflowConfig {
    #[serde(rename = "loop")]
    pub loop_cfg: LoopConfig,
    pub stages: Vec<StageConfig>,
}

impl WorkflowConfig {
    pub fn from_str(yaml: &str) -> anyhow::Result<Self> {
        let wf: Self = serde_yaml::from_str(yaml)
            .map_err(|e| anyhow::anyhow!("invalid workflow YAML: {}", e))?;
        for stage in &wf.stages {
            stage.validate()?;
        }
        Ok(wf)
    }

    pub fn load(agents_dir_override: Option<&std::path::Path>) -> anyhow::Result<Self> {
        let override_path = agents_dir_override
            .map(|d| d.parent().unwrap_or(d).join("workflow.yaml"))
            .unwrap_or_else(|| std::path::PathBuf::from(".falanx/workflow.yaml"));

        if override_path.exists() {
            let yaml = std::fs::read_to_string(&override_path)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", override_path.display(), e))?;
            Self::from_str(&yaml)
        } else {
            Self::load_default()
        }
    }

    pub fn load_default() -> anyhow::Result<Self> {
        Self::from_str(DEFAULT_WORKFLOW)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_YAML: &str = r#"
loop:
  max_iterations: 3
  target_score: 4.0
  plateau_threshold: 0.1
stages:
  - id: score
    agents: [score_readability]
    output_format: json_object
    output_as: score_result
  - id: review
    agent: review
    output_format: json_array
    output_as: review_result
    skip_if: score_meets_target
"#;

    #[test]
    fn parses_minimal_workflow() {
        let wf = WorkflowConfig::from_str(MINIMAL_YAML).unwrap();
        assert_eq!(wf.loop_cfg.max_iterations, 3);
        assert!((wf.loop_cfg.target_score - 4.0).abs() < f32::EPSILON);
        assert_eq!(wf.stages.len(), 2);
        assert_eq!(wf.stages[0].id, "score");
        assert_eq!(wf.stages[0].agent_names(), vec!["score_readability"]);
        assert_eq!(wf.stages[1].id, "review");
        assert!(wf.stages[1].skip_if.is_some());
    }

    #[test]
    fn validates_stages_on_load() {
        let bad = r#"
loop:
  max_iterations: 1
  target_score: 4.0
  plateau_threshold: 0.1
stages:
  - id: broken
    output_format: text
    output_as: out
"#;
        let err = WorkflowConfig::from_str(bad).unwrap_err();
        assert!(err.to_string().contains("agent"));
    }

    #[test]
    fn loads_embedded_default() {
        let wf = WorkflowConfig::load_default().unwrap();
        assert!(!wf.stages.is_empty());
        assert!(wf.stages.iter().any(|s| s.id == "score"));
    }
}
