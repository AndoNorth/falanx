/// One failed validation attempt for a single agent call. `agent/run.rs::run_agent` accumulates
/// these across retries; `workflow/runner.rs` turns each one into a `SessionEvent::AgentOutputInvalid`
/// JSONL line before the final `AgentCompleted` line, so a passing-after-retry run still leaves
/// a full trail of what the model got wrong along the way.
//
// `Clone` because `AgentRunResult` (which owns a `Vec<RetryDiagnostic>`) already derives `Clone`
// for use in tests and `synthesise_runs` - every field of a Clone type must itself be Clone.
#[derive(Debug, Clone)]
pub struct RetryDiagnostic {
    pub attempt: u32,
    pub errors: Vec<String>,
}

/// Appended once per schema-bearing agent call, replacing the old copy-pasted
/// "Respond with only JSON..." paragraph that used to live at the bottom of every prompt.md.
/// One function means one place to edit the wording, instead of seven.
pub fn wrap_prompt_with_schema(rendered_prompt: &str, schema: &serde_json::Value) -> String {
    // pretty-printed so the schema is readable in the JSONL audit trail's raw_output field,
    // not just parseable - this text is also what a human reviewing a session file sees.
    let schema_text = serde_json::to_string_pretty(schema).unwrap_or_else(|_| schema.to_string());
    format!(
        "{rendered_prompt}\n\nRespond with only JSON matching this schema — no preamble, no explanation, no other text:\n{schema_text}"
    )
}

/// Appended on a retry so the model sees exactly what it got wrong last time, instead of just
/// being asked to try again with no feedback.
pub fn wrap_prompt_with_validation_errors(prompt_with_schema: &str, errors: &[String]) -> String {
    let error_list = errors
        .iter()
        .map(|e| format!("- {e}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{prompt_with_schema}\n\nValidation errors from the previous attempt — fix these and respond again with only the corrected JSON:\n{error_list}"
    )
}

/// Strips a `<think>...</think>` reasoning block some local models emit before their real
/// answer. Safe to call unconditionally: a response with no such block passes through untouched,
/// which is what keeps a schema-less agent's behaviour byte-identical to before this existed.
//
// ponytail: handles exactly one fixed delimiter pair. If another reasoning-tagged model shows up
// using a different marker, extend this to check a small list of known pairs rather than
// generalising into a regex - most Ollama reasoning models converge on <think>...</think>.
pub fn strip_reasoning_preamble(text: &str) -> String {
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";
    match (text.find(OPEN), text.find(CLOSE)) {
        (Some(start), Some(end)) if end > start => {
            let mut out = String::with_capacity(text.len());
            out.push_str(&text[..start]);
            out.push_str(&text[end + CLOSE.len()..]);
            out
        }
        // No block, or a malformed one (close before open) - leave the text alone rather than
        // guessing at a partial strip.
        _ => text.to_string(),
    }
}

/// Validates an agent's already-extracted JSON value against its declared output schema.
/// Returns every violation found, not just the first, so a retry prompt can address them all
/// at once instead of the model fixing one field per attempt.
pub fn validate_output(
    value: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<(), Vec<String>> {
    // validator_for compiles the schema into a reusable Validator. We rebuild it on every call
    // rather than caching one per AgentDef - this only runs once per LLM round trip, so the
    // recompilation cost is negligible next to the network call that just happened.
    let validator = jsonschema::validator_for(schema).map_err(|e| {
        vec![format!(
            "agent's output.schema.json is not a valid schema: {e}"
        )]
    })?;

    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|e| format!("{e} (at {})", e.instance_path()))
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wrap_prompt_with_schema_includes_prompt_and_schema() {
        let schema = json!({"type": "object", "required": ["score"]});
        let wrapped = wrap_prompt_with_schema("Review this diff.", &schema);
        assert!(wrapped.contains("Review this diff."));
        assert!(wrapped.contains("\"score\""));
    }

    #[test]
    fn wrap_prompt_with_validation_errors_includes_prior_prompt_and_errors() {
        let wrapped = wrap_prompt_with_validation_errors(
            "previous prompt text",
            &["score must be an integer".to_string()],
        );
        assert!(wrapped.contains("previous prompt text"));
        assert!(wrapped.contains("score must be an integer"));
    }

    #[test]
    fn strip_reasoning_preamble_removes_think_block() {
        let text = "<think>let me consider the braces {}</think>{\"score\": 4}";
        assert_eq!(strip_reasoning_preamble(text), "{\"score\": 4}");
    }

    #[test]
    fn strip_reasoning_preamble_is_noop_without_think_block() {
        let text = "{\"score\": 4}";
        assert_eq!(strip_reasoning_preamble(text), text);
    }

    #[test]
    fn validate_output_passes_for_matching_value() {
        let schema = json!({
            "type": "object",
            "required": ["score"],
            "properties": {"score": {"type": "integer"}}
        });
        let value = json!({"score": 4});
        assert!(validate_output(&value, &schema).is_ok());
    }

    #[test]
    fn validate_output_returns_errors_for_mismatched_value() {
        let schema = json!({
            "type": "object",
            "required": ["score"],
            "properties": {"score": {"type": "integer"}}
        });
        let value = json!({"score": "not a number"});
        let errors = validate_output(&value, &schema).unwrap_err();
        assert!(!errors.is_empty());
    }
}
