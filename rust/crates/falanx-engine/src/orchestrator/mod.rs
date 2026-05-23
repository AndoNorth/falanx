pub mod horizon;
pub mod pipeline;

use crate::{
    config::{FalanxConfig, LoopConfig},
    git::ReviewTarget,
    session::Session,
    types::{RewritePatch, ReviewScore, SessionId},
};

pub struct RunConfig {
    pub target: ReviewTarget,
    pub loop_cfg: LoopConfig,
    pub session: Session,
}

pub struct RunResult {
    pub final_score: ReviewScore,
    pub iterations: u32,
    pub patches: Vec<RewritePatch>,
    pub session_id: SessionId,
}

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    pipeline::run(config, falanx_cfg).await
}
