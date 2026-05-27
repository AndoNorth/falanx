use super::AgentContext;
use crate::types::ReviewIssue;

pub async fn critique(
    ctx: &AgentContext<'_>,
    composite: &f32,
) -> anyhow::Result<Vec<ReviewIssue>> {
    let prompt = build_critique_prompt(&ctx.diff.0, *composite);
    let output = ctx.agent.run(&prompt).await?;
    parse_issues_response(output.text())
}

fn build_critique_prompt(diff: &str, composite: f32) -> String {
    format!(
        "CRITIQUE this diff. Composite score is {:.1}. Identify concrete issues.\n\
         Respond with JSON array only:\n\
         [{{\"location\":\"file:line\",\"problem\":\"description\",\"fix\":\"suggestion\"}}]\n\
         Return empty array [] if no issues found.\n\nDIFF:\n{}",
        composite,
        diff
    )
}

fn parse_issues_response(text: &str) -> anyhow::Result<Vec<ReviewIssue>> {
    let start = text.find('[').ok_or_else(|| anyhow::anyhow!("no JSON array in critique response"))?;
    let end = text.rfind(']').ok_or_else(|| anyhow::anyhow!("no JSON array end"))?;
    let json = &text[start..=end];
    Ok(serde_json::from_str(json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_issues_empty_array() {
        let issues = parse_issues_response("[]").unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn parse_issues_single_issue() {
        let json = r#"[{"location":"src/main.rs:10","problem":"unused variable","fix":"prefix with _"}]"#;
        let issues = parse_issues_response(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location, "src/main.rs:10");
    }

    #[test]
    fn parse_issues_handles_prose_wrapping() {
        let text = r#"Here are the issues: [{"location":"src/lib.rs:5","problem":"foo","fix":"bar"}] Done."#;
        let issues = parse_issues_response(text).unwrap();
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn parse_issues_errors_on_missing_array() {
        let err = parse_issues_response("no array here").unwrap_err();
        assert!(err.to_string().contains("no JSON array"));
    }
}
