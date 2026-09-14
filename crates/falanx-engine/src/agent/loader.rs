use crate::agent::{AgentDef, AgentKind};
use std::path::Path;

#[derive(serde::Deserialize)]
struct AgentConfig {
    #[serde(default)]
    kind: AgentKind,
    max_turns: u32,
}

pub fn load_from_dir(agents_base: &Path, name: &str) -> anyhow::Result<AgentDef> {
    let dir = agents_base.join(name);
    if !dir.exists() {
        anyhow::bail!("agent directory '{}' not found at {}", name, dir.display());
    }

    let system_prompt = std::fs::read_to_string(dir.join("system.md"))
        .map_err(|e| anyhow::anyhow!("agent '{}': missing system.md: {}", name, e))?;

    let prompt_template = std::fs::read_to_string(dir.join("prompt.md"))
        .map_err(|e| anyhow::anyhow!("agent '{}': missing prompt.md: {}", name, e))?;

    let config_str = std::fs::read_to_string(dir.join("config.yaml"))
        .map_err(|e| anyhow::anyhow!("agent '{}': missing config.yaml: {}", name, e))?;

    let config: AgentConfig = serde_yaml::from_str(&config_str)
        .map_err(|e| anyhow::anyhow!("agent '{}': invalid config.yaml: {}", name, e))?;

    // output.schema.json is optional - unlike system.md/prompt.md/config.yaml, its absence
    // is not an error. `Path::exists` plus an Option is simpler here than trying to distinguish
    // "file missing" from "other io error" through the Result from read_to_string.
    let output_schema = if dir.join("output.schema.json").exists() {
        let raw = std::fs::read_to_string(dir.join("output.schema.json")).map_err(|e| {
            anyhow::anyhow!("agent '{}': failed to read output.schema.json: {}", name, e)
        })?;
        let parsed: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("agent '{}': invalid output.schema.json: {}", name, e))?;
        Some(parsed)
    } else {
        None
    };

    Ok(AgentDef {
        name: name.to_string(),
        kind: config.kind,
        system_prompt: system_prompt.trim().to_string(),
        prompt_template: prompt_template.trim().to_string(),
        max_turns: config.max_turns,
        output_schema,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_agent_dir(dir: &std::path::Path, name: &str, max_turns: u32) -> std::path::PathBuf {
        let agent_dir = dir.join(name);
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(agent_dir.join("system.md"), "You are a reviewer.").unwrap();
        fs::write(agent_dir.join("prompt.md"), "Review: {{ diff }}").unwrap();
        fs::write(
            agent_dir.join("config.yaml"),
            format!("kind: cersei\nmax_turns: {}", max_turns),
        )
        .unwrap();
        agent_dir
    }

    #[test]
    fn loads_agent_from_directory() {
        let dir = tempfile::tempdir().unwrap();
        write_agent_dir(dir.path(), "review", 2);
        let def = load_from_dir(dir.path(), "review").unwrap();
        assert_eq!(def.name, "review");
        assert_eq!(def.max_turns, 2);
        assert_eq!(def.system_prompt, "You are a reviewer.");
        assert_eq!(def.prompt_template, "Review: {{ diff }}");
    }

    #[test]
    fn errors_on_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_from_dir(dir.path(), "nonexistent").unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn loads_agent_with_no_output_schema() {
        let dir = tempfile::tempdir().unwrap();
        write_agent_dir(dir.path(), "review", 2);
        let def = load_from_dir(dir.path(), "review").unwrap();
        // No output.schema.json was written for this fixture - the field must default to
        // None rather than erroring, since existing custom --agents dirs never have this file.
        assert!(def.output_schema.is_none());
    }

    #[test]
    fn loads_agent_with_output_schema() {
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = write_agent_dir(dir.path(), "score_readability", 1);
        fs::write(
            agent_dir.join("output.schema.json"),
            r#"{"type": "object", "required": ["score"], "properties": {"score": {"type": "integer"}}}"#,
        )
        .unwrap();

        let def = load_from_dir(dir.path(), "score_readability").unwrap();
        let schema = def.output_schema.expect("schema should have loaded");
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn errors_on_malformed_output_schema() {
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = write_agent_dir(dir.path(), "review", 1);
        fs::write(agent_dir.join("output.schema.json"), "{ not valid json").unwrap();

        let err = load_from_dir(dir.path(), "review").unwrap_err();
        assert!(err.to_string().contains("output.schema.json"));
    }
}
