pub mod quality;
pub mod review;
pub mod writing;

pub struct AgentContext<'a> {
    pub config: &'a crate::config::FalanxConfig,
    pub diff: &'a crate::git::Diff,
}
