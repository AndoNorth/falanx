use super::AgentContext;
use crate::types::{ReviewIssue, ReviewScore};

pub async fn critique(
    _ctx: &AgentContext<'_>,
    _score: &ReviewScore,
) -> anyhow::Result<Vec<ReviewIssue>> {
    anyhow::bail!("CodeReviewAgent not yet implemented — planned for Phase 1D")
}
