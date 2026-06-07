use crate::{
    agent::AgentDef,
    config::FalanxConfig,
    provider::MockProvider,
    types::AgentRunResult,
    workflow::stage::OutputFormat,
};
use cersei_agent::Agent;

/// Extract output from agent response in the specified format.
///
/// For JSON objects/arrays, finds the first occurrence and parses it.
/// For text, returns the full string as-is.
pub fn extract_output(text: &str, format: &OutputFormat) -> anyhow::Result<serde_json::Value> {
    match format {
        OutputFormat::JsonObject => {
            let start = text
                .find('{')
                .ok_or_else(|| anyhow::anyhow!("no JSON object in agent response"))?;
            let end = text
                .rfind('}')
                .ok_or_else(|| anyhow::anyhow!("no JSON object end in agent response"))?;
            serde_json::from_str(&text[start..=end])
                .map_err(|e| anyhow::anyhow!("failed to parse JSON object: {}", e))
        }
        OutputFormat::JsonArray => {
            let start = text
                .find('[')
                .ok_or_else(|| anyhow::anyhow!("no JSON array in agent response"))?;
            let end = text
                .rfind(']')
                .ok_or_else(|| anyhow::anyhow!("no JSON array end in agent response"))?;
            serde_json::from_str(&text[start..=end])
                .map_err(|e| anyhow::anyhow!("failed to parse JSON array: {}", e))
        }
        OutputFormat::Text => Ok(serde_json::Value::String(text.to_string())),
    }
}

/// Execute a single agent run against the provided prompt.
/// Dispatches based on agent kind (currently only Cersei).
pub async fn run_agent(
    def: &AgentDef,
    rendered_prompt: &str,
    cfg: &FalanxConfig,
    output_format: &OutputFormat,
) -> anyhow::Result<AgentRunResult> {
    match def.kind {
        crate::agent::AgentKind::Cersei => {
            run_cersei(def, rendered_prompt, cfg, output_format).await
        }
    }
}

async fn run_cersei(
    def: &AgentDef,
    rendered_prompt: &str,
    cfg: &FalanxConfig,
    output_format: &OutputFormat,
) -> anyhow::Result<AgentRunResult> {
    let (provider, model_id) = build_provider(cfg)?;
    let agent = Agent::builder()
        .provider_boxed(provider)
        .model(&model_id)
        .system_prompt(&def.system_prompt)
        .max_turns(def.max_turns)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build agent '{}': {}", def.name, e))?;

    let output = agent
        .run(rendered_prompt)
        .await
        .map_err(|e| anyhow::anyhow!("agent '{}' failed: {}", def.name, e))?;

    let raw_output = output.text().to_string();
    let extracted = extract_output(&raw_output, output_format)?;

    Ok(AgentRunResult {
        agent_name: def.name.clone(),
        raw_output,
        extracted,
        turns_used: 1,
    })
}

fn build_provider(cfg: &FalanxConfig) -> anyhow::Result<(Box<dyn cersei_provider::Provider>, String)> {
    if cfg.provider.dry_run {
        Ok((Box::new(MockProvider), cfg.provider.model.clone()))
    } else {
        cersei_provider::from_model_string(&cfg.provider.model)
            .map_err(|e| anyhow::anyhow!("provider error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_object_finds_first_object() {
        let text = r#"Here is the result: {"score": 4, "reasoning": "good"} done."#;
        let val = extract_output(text, &OutputFormat::JsonObject).unwrap();
        assert_eq!(val["score"], 4);
        assert_eq!(val["reasoning"], "good");
    }

    #[test]
    fn extract_json_array_finds_first_array() {
        let text = r#"Issues: [{"location":"a","problem":"b","fix":"c"}]"#;
        let val = extract_output(text, &OutputFormat::JsonArray).unwrap();
        assert_eq!(val.as_array().unwrap().len(), 1);
    }

    #[test]
    fn extract_text_returns_full_string() {
        let val = extract_output("hello world", &OutputFormat::Text).unwrap();
        assert_eq!(val.as_str().unwrap(), "hello world");
    }

    #[test]
    fn extract_json_object_errors_on_missing() {
        let err = extract_output("no json here", &OutputFormat::JsonObject).unwrap_err();
        assert!(err.to_string().contains("no JSON object"));
    }

    #[test]
    fn extract_json_array_errors_on_missing() {
        let err = extract_output("no array", &OutputFormat::JsonArray).unwrap_err();
        assert!(err.to_string().contains("no JSON array"));
    }
}
