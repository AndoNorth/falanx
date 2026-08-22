pub mod loader;
pub mod run;

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    #[default]
    Cersei,
}

#[derive(Debug, Clone)]
pub struct AgentDef {
    pub name: String,
    pub kind: AgentKind,
    pub system_prompt: String,
    pub prompt_template: String,
    pub max_turns: u32,
}

impl AgentDef {
    pub fn from_parts(
        name: impl Into<String>,
        system_prompt: impl Into<String>,
        prompt_template: impl Into<String>,
        max_turns: u32,
    ) -> Self {
        Self {
            name: name.into(),
            kind: AgentKind::default(),
            system_prompt: system_prompt.into(),
            prompt_template: prompt_template.into(),
            max_turns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_def_from_parts() {
        let def = AgentDef::from_parts("review", "sys", "tmpl", 1);
        assert_eq!(def.name, "review");
        assert_eq!(def.max_turns, 1);
        assert!(matches!(def.kind, AgentKind::Cersei));
    }

    #[test]
    fn agent_kind_deserialises_cersei() {
        let kind: AgentKind = serde_yaml::from_str("cersei").unwrap();
        assert!(matches!(kind, AgentKind::Cersei));
    }
}
