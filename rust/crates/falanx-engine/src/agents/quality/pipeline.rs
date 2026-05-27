use std::collections::HashMap;

use crate::{
    config::FalanxConfig,
    git::Diff,
    session::{Session, SessionEvent},
    types::CategoryResult,
};
use super::config::{CategoryConfig, PipelineConfig};

pub async fn run_categories(
    diff: &Diff,
    falanx_cfg: &FalanxConfig,
    pipeline: &PipelineConfig,
    categories: &HashMap<String, CategoryConfig>,
    session: &Session,
    iteration: u32,
) -> anyhow::Result<Vec<CategoryResult>> {
    let mut results = Vec::with_capacity(pipeline.categories.len());

    for cat_name in &pipeline.categories {
        let cat_cfg = &categories[cat_name];
        let prompt = build_category_prompt(&cat_cfg.prompt, &diff.0);

        let (provider, model_id) = build_provider(falanx_cfg)?;
        let agent = cersei_agent::Agent::builder()
            .provider_boxed(provider)
            .model(&model_id)
            .system_prompt(&cat_cfg.system_prompt)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build category agent: {}", e))?;

        let output = agent
            .run(&prompt)
            .await
            .map_err(|e| anyhow::anyhow!("category agent '{}' failed: {}", cat_name, e))?;

        tracing::debug!(category = %cat_name, response = output.text(), "category agent raw response");

        let (score, reasoning) = parse_category_response(output.text())?;
        let reasoning = truncate_reasoning(reasoning, cat_cfg.max_reasoning_chars);

        session.append(SessionEvent::CategoryScored {
            category: cat_name.clone(),
            score,
            reasoning: reasoning.clone(),
            iteration,
        })?;

        tracing::info!(
            category = %cat_name,
            score = score,
            iteration = iteration,
            "category scored"
        );

        results.push(CategoryResult {
            name: cat_name.clone(),
            score,
            reasoning,
        });
    }

    Ok(results)
}

pub fn build_category_prompt(prompt_template: &str, diff: &str) -> String {
    if prompt_template.contains("{diff}") {
        prompt_template.replace("{diff}", diff)
    } else {
        format!("{}\n\nDIFF:\n{}", prompt_template, diff)
    }
}

fn parse_category_response(text: &str) -> anyhow::Result<(u8, String)> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("no JSON in category response"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("no JSON end in category response"))?;
    let json = &text[start..=end];

    #[derive(serde::Deserialize)]
    struct CategoryResponse {
        score: u8,
        reasoning: String,
    }

    let r: CategoryResponse = serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("failed to parse category response: {}", e))?;
    Ok((r.score, r.reasoning))
}

fn truncate_reasoning(reasoning: String, max_chars: usize) -> String {
    if reasoning.len() <= max_chars {
        reasoning
    } else {
        reasoning[..max_chars].to_string()
    }
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

    #[test]
    fn build_category_prompt_appends_diff_when_no_placeholder() {
        let template = "Review this diff.";
        let result = build_category_prompt(template, "fn foo() {}");
        assert!(result.contains("fn foo() {}"));
        assert!(result.contains("Review this diff."));
    }

    #[test]
    fn build_category_prompt_injects_diff_with_placeholder() {
        let template = "Review this.\nDIFF:\n{diff}";
        let result = build_category_prompt(template, "fn foo() {}");
        assert!(result.contains("fn foo() {}"));
        assert!(result.contains("Review this."));
        assert!(!result.contains("{diff}"));
    }

    #[test]
    fn parse_category_response_extracts_score_and_reasoning() {
        let text = r#"{"score": 4, "reasoning": "clear names"}"#;
        let (score, reasoning) = parse_category_response(text).unwrap();
        assert_eq!(score, 4);
        assert_eq!(reasoning, "clear names");
    }

    #[test]
    fn parse_category_response_handles_prose_wrapping() {
        let text = r#"Here is the score: {"score": 3, "reasoning": "ok"} done."#;
        let (score, reasoning) = parse_category_response(text).unwrap();
        assert_eq!(score, 3);
        assert_eq!(reasoning, "ok");
    }

    #[test]
    fn parse_category_response_errors_on_missing_json() {
        let err = parse_category_response("no json").unwrap_err();
        assert!(err.to_string().contains("no JSON"));
    }

    #[test]
    fn truncate_reasoning_leaves_short_strings_unchanged() {
        let s = "short".to_string();
        assert_eq!(truncate_reasoning(s.clone(), 100), s);
    }

    #[test]
    fn truncate_reasoning_truncates_long_strings() {
        let s = "a".repeat(500);
        let result = truncate_reasoning(s, 100);
        assert_eq!(result.len(), 100);
    }

    #[tokio::test]
    async fn run_categories_emits_category_scored_events() {
        use crate::config::{FalanxConfig, LoopConfig, ProviderConfig, SessionConfig};

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

        let mut cats = HashMap::new();
        cats.insert("readability".to_string(), CategoryConfig {
            system_prompt: "You are a code quality reviewer. You evaluate the quality of changes in a diff, not the codebase generally.".into(),
            prompt: "Do these changes introduce readability problems?\nScore 1-5. Respond in JSON only:\n{\"score\": N, \"reasoning\": \"max 80 words\"}".into(),
            max_reasoning_chars: 400,
        });

        let pipeline = PipelineConfig {
            name: "test".into(),
            parallel: false,
            categories: vec!["readability".into()],
        };

        let diff = Diff("fn foo() {}".into());
        let results = run_categories(&diff, &falanx_cfg, &pipeline, &cats, &session, 0).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "readability");
        assert!(results[0].score > 0);

        let content = std::fs::read_to_string(session.path()).unwrap();
        assert!(content.contains("category_scored"));
        assert!(content.contains("readability"));
    }
}
