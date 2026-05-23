pub mod horizon;
pub mod pipeline;

use crate::{config::FalanxConfig, git::ReviewTarget, session::Session,
            types::{RewritePatch, ReviewScore, SessionId}};

pub struct RunConfig {
    pub target: ReviewTarget,
    pub session: Session,
}

pub struct RunResult {
    pub final_score: ReviewScore,
    pub iterations: u32,
    pub patches: Vec<RewritePatch>,
    pub session_id: SessionId,
}

pub async fn run(_config: RunConfig, _falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    anyhow::bail!("Orchestrator not yet implemented — planned for Phase 1C")
}
