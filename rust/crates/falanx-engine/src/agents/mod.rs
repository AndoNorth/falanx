// Task A - disabled during types refactor, re-enabled in Task F
pub mod quality;
pub mod review;
pub mod writing;

pub struct AgentContext<'a> {
    pub agent: &'a cersei_agent::Agent,
    pub diff: &'a crate::git::Diff,
}
