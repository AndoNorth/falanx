pub mod horizon;

use crate::{
    config::{FalanxConfig, LoopConfig},
    git::ReviewTarget,
    session::Session,
    types::SessionId,
};

pub struct RunConfig {
    pub target: ReviewTarget,
    pub loop_cfg: LoopConfig,
    pub session: Session,
}

pub struct RunResult {
    pub iterations: u32,
    pub session_id: SessionId,
    pub summary: serde_json::Value,
}

pub async fn run(_config: RunConfig, _falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    todo!("replaced by WorkflowRunner in Task C2")
}
