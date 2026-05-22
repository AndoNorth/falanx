use super::AgentContext;
use crate::types::{ReviewIssue, RewritePatch};

pub async fn rewrite(
    _ctx: &AgentContext<'_>,
    _issues: &[ReviewIssue],
) -> anyhow::Result<Vec<RewritePatch>> {
    anyhow::bail!("CodeWritingAgent not yet implemented — planned for Phase 1D")
}
