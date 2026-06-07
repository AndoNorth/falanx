use crate::workflow::context::TemplateContext;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    JsonObject,
    JsonArray,
    Text,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipIf {
    ScoreMeetsTarget,
    NoReviewIssues,
}

impl SkipIf {
    pub fn evaluate(&self, ctx: &TemplateContext, target_score: f32) -> bool {
        match self {
            SkipIf::ScoreMeetsTarget => composite_from_context(ctx) >= target_score,
            SkipIf::NoReviewIssues => ctx
                .stages
                .get("review_result")
                .and_then(|v| v.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(true),
        }
    }
}

fn composite_from_context(ctx: &TemplateContext) -> f32 {
    let arr = match ctx.stages.get("score_result").and_then(|v| v.as_array()) {
        Some(a) if !a.is_empty() => a,
        _ => return 0.0,
    };
    let sum: f32 = arr
        .iter()
        .filter_map(|v| v.get("score").and_then(|s| s.as_f64()))
        .map(|s| s as f32)
        .sum();
    sum / arr.len() as f32
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StageConfig {
    pub id: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    pub output_format: OutputFormat,
    pub output_as: String,
    pub skip_if: Option<SkipIf>,
}

impl StageConfig {
    pub fn agent_names(&self) -> Vec<String> {
        if let Some(ref name) = self.agent {
            vec![name.clone()]
        } else {
            self.agents.clone()
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let has_single = self.agent.is_some();
        let has_multi = !self.agents.is_empty();
        match (has_single, has_multi) {
            (true, true) => anyhow::bail!("stage '{}': set either 'agent' or 'agents', not both", self.id),
            (false, false) => anyhow::bail!("stage '{}': must set 'agent' or 'agents'", self.id),
            _ => Ok(()),
        }
    }
}

pub struct StageResult {
    pub stage_id: String,
    pub runs: Vec<crate::types::AgentRunResult>,
    pub synthesised: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_deserialises() {
        let f: OutputFormat = serde_yaml::from_str("json_object").unwrap();
        assert!(matches!(f, OutputFormat::JsonObject));
        let f: OutputFormat = serde_yaml::from_str("json_array").unwrap();
        assert!(matches!(f, OutputFormat::JsonArray));
        let f: OutputFormat = serde_yaml::from_str("text").unwrap();
        assert!(matches!(f, OutputFormat::Text));
    }

    #[test]
    fn skip_if_deserialises() {
        let s: SkipIf = serde_yaml::from_str("score_meets_target").unwrap();
        assert!(matches!(s, SkipIf::ScoreMeetsTarget));
        let s: SkipIf = serde_yaml::from_str("no_review_issues").unwrap();
        assert!(matches!(s, SkipIf::NoReviewIssues));
    }

    #[test]
    fn skip_if_unknown_fails_deserialisation() {
        let err = serde_yaml::from_str::<SkipIf>("fly_to_moon").unwrap_err();
        assert!(err.to_string().len() > 0);
    }

    #[test]
    fn stage_config_single_agent_resolves_names() {
        let yaml = r#"
id: review
agent: review
output_format: json_array
output_as: review_result
"#;
        let stage: StageConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(stage.agent_names(), vec!["review"]);
        assert_eq!(stage.output_as, "review_result");
        assert!(stage.skip_if.is_none());
    }

    #[test]
    fn stage_config_multi_agent_resolves_names() {
        let yaml = r#"
id: score
agents: [score_readability, score_security]
output_format: json_object
output_as: score_result
"#;
        let stage: StageConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(stage.agent_names(), vec!["score_readability", "score_security"]);
    }

    #[test]
    fn stage_config_errors_when_both_agent_and_agents_set() {
        let yaml = r#"
id: bad
agent: foo
agents: [bar]
output_format: text
output_as: out
"#;
        let stage: StageConfig = serde_yaml::from_str(yaml).unwrap();
        let err = stage.validate().unwrap_err();
        assert!(err.to_string().contains("agent"));
    }

    #[test]
    fn stage_config_errors_when_neither_set() {
        let yaml = r#"
id: bad
output_format: text
output_as: out
"#;
        let stage: StageConfig = serde_yaml::from_str(yaml).unwrap();
        let err = stage.validate().unwrap_err();
        assert!(err.to_string().contains("agent"));
    }
}
