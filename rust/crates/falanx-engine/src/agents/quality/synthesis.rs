use crate::types::{CategoryResult, ScoringResult};

pub async fn synthesize(
    agent: &cersei_agent::Agent,
    pipeline_name: &str,
    categories: Vec<CategoryResult>,
) -> anyhow::Result<ScoringResult> {
    let prompt = build_synthesis_prompt(&categories);
    let output = agent.run(&prompt).await
        .map_err(|e| anyhow::anyhow!("synthesis agent failed: {}", e))?;
    tracing::debug!(response = output.text(), "synthesis agent raw response");
    let (composite_score, synthesis) = parse_synthesis_response(output.text())?;
    Ok(ScoringResult {
        pipeline_name: pipeline_name.to_string(),
        categories,
        composite_score,
        synthesis,
    })
}

pub fn build_synthesis_prompt(categories: &[CategoryResult]) -> String {
    let scores_block = categories
        .iter()
        .map(|c| format!("- {}: {} — \"{}\"", c.name, c.score, c.reasoning))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are a senior code reviewer synthesising multiple category assessments.\n\n\
         CATEGORY SCORES:\n{}\n\n\
         Provide an overall code quality assessment considering the relative importance \
         of each finding. A critical security or architecture finding should weigh \
         heavily even if other scores are high.\n\n\
         Respond in JSON only:\n\
         {{\"overall_score\": N, \"summary\": \"max 120 words\"}}",
        scores_block
    )
}

fn parse_synthesis_response(text: &str) -> anyhow::Result<(f32, String)> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("no JSON in synthesis response"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("no JSON end in synthesis response"))?;
    let json = &text[start..=end];

    #[derive(serde::Deserialize)]
    struct SynthesisResponse {
        overall_score: u8,
        summary: String,
    }

    let r: SynthesisResponse = serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("failed to parse synthesis response: {}", e))?;
    Ok((r.overall_score as f32, r.summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_synthesis_prompt_includes_all_categories() {
        let cats = vec![
            CategoryResult { name: "readability".into(), score: 4, reasoning: "clear names".into() },
            CategoryResult { name: "security".into(), score: 2, reasoning: "injection risk".into() },
        ];
        let prompt = build_synthesis_prompt(&cats);
        assert!(prompt.contains("readability: 4"));
        assert!(prompt.contains("security: 2"));
        assert!(prompt.contains("injection risk"));
        assert!(prompt.contains("overall_score"));
    }

    #[test]
    fn parse_synthesis_response_extracts_score_and_summary() {
        let text = r#"{"overall_score": 3, "summary": "mixed results"}"#;
        let (score, summary) = parse_synthesis_response(text).unwrap();
        assert!((score - 3.0).abs() < f32::EPSILON);
        assert_eq!(summary, "mixed results");
    }

    #[test]
    fn parse_synthesis_response_handles_prose_wrapping() {
        let text = r#"Here is the result: {"overall_score": 4, "summary": "looks good"} end"#;
        let (score, summary) = parse_synthesis_response(text).unwrap();
        assert!((score - 4.0).abs() < f32::EPSILON);
        assert_eq!(summary, "looks good");
    }

    #[test]
    fn parse_synthesis_response_errors_on_missing_json() {
        let err = parse_synthesis_response("no json here").unwrap_err();
        assert!(err.to_string().contains("no JSON"));
    }

    #[tokio::test]
    async fn synthesize_returns_scoring_result_with_mock_provider() {
        use crate::provider::MockProvider;

        let provider = Box::new(MockProvider);
        let agent = cersei_agent::Agent::builder()
            .provider_boxed(provider)
            .model("mock")
            .build()
            .unwrap();

        let cats = vec![
            CategoryResult { name: "readability".into(), score: 3, reasoning: "ok".into() },
        ];
        let result = synthesize(&agent, "default", cats).await.unwrap();
        assert_eq!(result.pipeline_name, "default");
        assert_eq!(result.categories.len(), 1);
        assert!(result.composite_score > 0.0);
        assert!(!result.synthesis.is_empty());
    }
}
