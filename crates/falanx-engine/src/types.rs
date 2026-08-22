#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionId(pub String);

/// Output from a single agent run. Used by workflow/runner.rs.
#[derive(Debug, Clone)]
pub struct AgentRunResult {
    pub agent_name: String,
    pub raw_output: String,
    pub extracted: serde_json::Value,
    pub turns_used: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_equality() {
        let a = SessionId("abc".into());
        let b = SessionId("abc".into());
        assert_eq!(a, b);
    }
}
