use crate::{
    agent::AgentDef,
    agent::contract::{self, RetryDiagnostic},
    config::FalanxConfig,
    provider::MockProvider,
    types::AgentRunResult,
    workflow::stage::OutputFormat,
};
use cersei_agent::Agent;

/// Fixed, not configurable in v1 (see the spec's Open Questions) — 3 total attempts per
/// schema-bearing agent call: the first try plus this many retries.
const MAX_OUTPUT_RETRIES: u32 = 2;

/// Extract output from agent response in the specified format.
///
/// For JSON objects/arrays, finds the first occurrence and parses it.
/// For text, returns the full string as-is.
pub fn extract_output(text: &str, format: &OutputFormat) -> anyhow::Result<serde_json::Value> {
    // Strip a reasoning model's <think>...</think> block before brace-search - it's a no-op for
    // the common case (no such block), so this keeps a schema-less agent byte-identical while
    // fixing the case where a reasoning model's own example JSON inside <think> would otherwise
    // get grabbed by the first-{/last-} search instead of the real answer after it.
    let stripped = contract::strip_reasoning_preamble(text);
    let text = stripped.as_str();

    let preview = || {
        let s = text.trim();
        if s.len() > 200 {
            format!("{}...({}B total)", &s[..200], s.len())
        } else {
            s.to_string()
        }
    };
    match format {
        OutputFormat::JsonObject => {
            let start = text.find('{').ok_or_else(|| {
                anyhow::anyhow!("no JSON object in agent response — got: {}", preview())
            })?;
            let end = text.rfind('}').ok_or_else(|| {
                anyhow::anyhow!("no JSON object end in agent response — got: {}", preview())
            })?;
            serde_json::from_str(&text[start..=end]).map_err(|e| {
                anyhow::anyhow!("failed to parse JSON object: {} — got: {}", e, preview())
            })
        }
        OutputFormat::JsonArray => {
            let start = text.find('[').ok_or_else(|| {
                anyhow::anyhow!("no JSON array in agent response — got: {}", preview())
            })?;
            let end = text.rfind(']').ok_or_else(|| {
                anyhow::anyhow!("no JSON array end in agent response — got: {}", preview())
            })?;
            serde_json::from_str(&text[start..=end]).map_err(|e| {
                anyhow::anyhow!("failed to parse JSON array: {} — got: {}", e, preview())
            })
        }
        OutputFormat::Text => Ok(serde_json::Value::String(text.to_string())),
    }
}

/// Runs one agent, enforcing its output contract if it declares one.
///
/// This function owns the wrap → dispatch → validate → retry sequence, and it wraps
/// `dispatch_kind` rather than living inside it - that's what keeps the contract kind-agnostic:
/// a future non-Cersei `AgentKind` only has to implement `dispatch_kind`'s branch, and it
/// automatically gets schema validation and retries for free.
pub async fn run_agent(
    def: &AgentDef,
    rendered_prompt: &str,
    cfg: &FalanxConfig,
    output_format: &OutputFormat,
) -> anyhow::Result<AgentRunResult> {
    // `let ... else` here reads as "bind schema, or take this early-return branch" - it's the
    // cleanest way in current Rust to unwrap an Option only when the rest of the function needs
    // it, without an extra level of `if let Some(schema) = ... { ... } else { ... }` nesting.
    let Some(schema) = &def.output_schema else {
        return dispatch_kind(def, rendered_prompt, cfg, output_format).await;
    };

    let mut prompt = contract::wrap_prompt_with_schema(rendered_prompt, schema);
    let mut diagnostics: Vec<RetryDiagnostic> = Vec::new();

    for attempt in 0..=MAX_OUTPUT_RETRIES {
        // A response that fails to extract at all (unparseable, wrong shape) is just as much a
        // contract violation as one that parses but fails schema validation - both get folded
        // into the same errors/diagnostics/retry path, rather than the extraction failure
        // short-circuiting the loop via `?` before the model ever gets a chance to see feedback
        // and correct itself.
        let errors = match dispatch_kind(def, &prompt, cfg, output_format).await {
            Ok(mut result) => match contract::validate_output(&result.extracted, schema) {
                Ok(()) => {
                    result.retry_diagnostics = diagnostics;
                    return Ok(result);
                }
                Err(errors) => errors,
            },
            Err(e) => vec![e.to_string()],
        };

        diagnostics.push(RetryDiagnostic {
            attempt,
            errors: errors.clone(),
        });
        if attempt == MAX_OUTPUT_RETRIES {
            anyhow::bail!(
                "agent '{}' failed its output contract after {} attempt(s): {:?}",
                def.name,
                attempt + 1,
                diagnostics
            );
        }
        prompt = contract::wrap_prompt_with_validation_errors(&prompt, &errors);
    }

    // The for loop above always either returns Ok (validation passed) or bails (validation
    // failed on the final allowed attempt) from inside its body - it can never fall through to
    // here. The compiler can't prove that on its own, though, so this satisfies the return type.
    unreachable!("loop always returns Ok or bails on its final attempt")
}

/// Execute a single agent run against the provided prompt.
/// Dispatches based on agent kind (currently only Cersei).
async fn dispatch_kind(
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
        // dispatch_kind never retries by itself - run_agent is the only place that accumulates
        // diagnostics, and it overwrites this before returning on the schema-bearing path.
        retry_diagnostics: Vec::new(),
    })
}

fn build_provider(
    cfg: &FalanxConfig,
) -> anyhow::Result<(Box<dyn cersei_provider::Provider>, String)> {
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

    fn dry_run_cfg() -> FalanxConfig {
        FalanxConfig {
            provider: crate::config::ProviderConfig {
                model: "mock".into(),
                api_key: "".into(),
                base_url: None,
                dry_run: true,
            },
            session: crate::config::SessionConfig {
                dir: std::path::PathBuf::from("/tmp"),
            },
            loop_cfg: crate::config::LoopConfig::default(),
        }
    }

    #[tokio::test]
    async fn run_agent_skips_contract_when_no_schema_declared() {
        let def = AgentDef::from_parts(
            "score_readability",
            "You are a code quality reviewer.",
            "score this",
            1,
        );
        let cfg = dry_run_cfg();
        let result = run_agent(&def, "score this", &cfg, &OutputFormat::JsonObject)
            .await
            .unwrap();
        // No schema means no retry loop ever ran - diagnostics must be empty, and the plain
        // MockProvider response (routed by "code quality reviewer" in the system prompt) comes
        // straight through unchanged.
        assert!(result.retry_diagnostics.is_empty());
        assert_eq!(result.extracted["score"], 3);
    }

    #[tokio::test]
    async fn run_agent_retries_once_then_succeeds_against_schema() {
        let mut def = AgentDef::from_parts("flaky", "You are a flaky test agent.", "score this", 1);
        def.output_schema = Some(serde_json::json!({
            "type": "object",
            "required": ["score", "reasoning"],
            "properties": {
                "score": {"type": "integer"},
                "reasoning": {"type": "string"}
            }
        }));
        let cfg = dry_run_cfg();

        let result = run_agent(&def, "score this", &cfg, &OutputFormat::JsonObject)
            .await
            .unwrap();

        assert_eq!(result.retry_diagnostics.len(), 1);
        assert_eq!(result.retry_diagnostics[0].attempt, 0);
        assert_eq!(result.extracted["score"], 4);
    }

    #[tokio::test]
    async fn run_agent_fails_after_exhausting_retries() {
        let mut def = AgentDef::from_parts(
            "always_wrong",
            "You are a precise code editor.",
            "rewrite this",
            1,
        );
        // The "precise code editor" MockProvider branch always returns "[]" - valid JSON, but
        // this schema demands a non-empty array, so validation fails on every attempt no matter
        // how many times we retry.
        def.output_schema = Some(serde_json::json!({
            "type": "array",
            "minItems": 1
        }));
        let cfg = dry_run_cfg();

        let err = run_agent(&def, "rewrite this", &cfg, &OutputFormat::JsonArray)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("always_wrong"));
    }

    #[test]
    fn extract_json_object_ignores_braces_inside_think_block() {
        // A reasoning model's own example JSON inside <think> must not be what gets grabbed -
        // the real answer after the block should win. This exercises extract_output's new
        // strip_reasoning_preamble call, added in Step 5 below.
        let text =
            r#"<think>maybe respond like {"score": 1}?</think>{"score": 4, "reasoning": "ok"}"#;
        let val = extract_output(text, &OutputFormat::JsonObject).unwrap();
        assert_eq!(val["score"], 4);
    }
}
