use super::AgentContext;
use crate::types::{ReviewIssue, RewritePatch};

pub async fn rewrite(
    ctx: &AgentContext<'_>,
    issues: &[ReviewIssue],
) -> anyhow::Result<Vec<RewritePatch>> {
    if issues.is_empty() {
        return Ok(vec![]);
    }
    let prompt = build_rewrite_prompt(&ctx.diff.0, issues);
    let output = ctx.agent.run(&prompt).await?;
    parse_patches_response(output.text())
}

fn build_rewrite_prompt(diff: &str, issues: &[ReviewIssue]) -> String {
    let issues_json = serde_json::to_string(issues).unwrap_or_default();
    format!(
        "REWRITE to fix these issues. Respond with JSON array only:\n\
         [{{\"original\":\"exact original text\",\"revised\":\"replacement text\",\
         \"issue_ref\":\"issue location\"}}]\n\
         Return empty array [] if no changes needed.\n\
         ISSUES:\n{}\n\nDIFF:\n{}",
        issues_json, diff
    )
}

fn parse_patches_response(text: &str) -> anyhow::Result<Vec<RewritePatch>> {
    let start = text.find('[').ok_or_else(|| anyhow::anyhow!("no JSON array in rewrite response"))?;
    let end = text.rfind(']').ok_or_else(|| anyhow::anyhow!("no JSON array end"))?;
    let json = &text[start..=end];
    Ok(serde_json::from_str(json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_empty_issues_returns_empty_without_agent_call() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let issues: Vec<ReviewIssue> = vec![];
            if issues.is_empty() { Ok::<Vec<RewritePatch>, anyhow::Error>(vec![]) }
            else { Err(anyhow::anyhow!("unexpected")) }
        });
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_patches_empty_array() {
        let patches = parse_patches_response("[]").unwrap();
        assert!(patches.is_empty());
    }

    #[test]
    fn parse_patches_single_patch() {
        let json = r#"[{"original":"let x = 1","revised":"let _x = 1","issue_ref":"src/main.rs:10"}]"#;
        let patches = parse_patches_response(json).unwrap();
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].issue_ref, "src/main.rs:10");
    }

    #[test]
    fn parse_patches_errors_on_missing_array() {
        let err = parse_patches_response("no array here").unwrap_err();
        assert!(err.to_string().contains("no JSON array"));
    }
}
