pub mod config;
pub use config::ScoringConfig;

use super::AgentContext;
use crate::types::ReviewScore;

pub async fn score(ctx: &AgentContext<'_>) -> anyhow::Result<ReviewScore> {
    let prompt = build_score_prompt(&ctx.diff.0);
    let output = ctx.agent.run(&prompt).await?;
    tracing::debug!(response = output.text(), "quality agent raw response");
    parse_score_response(output.text())
}

fn build_score_prompt(diff: &str) -> String {
    format!(
        "SCORE this diff across five categories: readability, maintainability, \
         performance, security, architecture. Each score 1-5 (integer). \
         Respond with JSON only, no prose:\n\
         {{\"readability\":N,\"maintainability\":N,\"performance\":N,\
         \"security\":N,\"architecture\":N}}\n\nDIFF:\n{}",
        diff
    )
}

fn parse_score_response(text: &str) -> anyhow::Result<ReviewScore> {
    let start = text.find("{\"").ok_or_else(|| anyhow::anyhow!("no JSON in score response"))?;
    let end = text.rfind('}').ok_or_else(|| anyhow::anyhow!("no JSON end in score response"))?;
    let json = &text[start..=end];
    Ok(serde_json::from_str(json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_score_extracts_clean_json() {
        let json = r#"{"readability":3,"maintainability":4,"performance":3,"security":5,"architecture":2}"#;
        let score = parse_score_response(json).unwrap();
        assert_eq!(score.readability, 3);
        assert_eq!(score.security, 5);
        assert!((score.composite() - 3.4).abs() < 0.01);
    }

    #[test]
    fn parse_score_handles_prose_wrapping() {
        let text = r#"Here is the score: {"readability":3,"maintainability":3,"performance":3,"security":3,"architecture":3} Done."#;
        let score = parse_score_response(text).unwrap();
        assert_eq!(score.composite(), 3.0);
    }

    #[test]
    fn parse_score_errors_on_missing_json() {
        let err = parse_score_response("no json here").unwrap_err();
        assert!(err.to_string().contains("no JSON"));
    }
}
