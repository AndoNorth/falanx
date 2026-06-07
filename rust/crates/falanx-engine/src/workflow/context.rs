use std::collections::HashMap;

#[derive(Clone)]
pub struct TemplateContext {
    pub diff: String,
    pub stages: HashMap<String, serde_json::Value>,
    pub loop_iteration: u32,
}

impl TemplateContext {
    pub fn new(diff: String) -> Self {
        Self { diff, stages: HashMap::new(), loop_iteration: 0 }
    }

    pub fn render(&self, template_str: &str) -> anyhow::Result<String> {
        let mut env = minijinja::Environment::new();
        env.add_template("t", template_str)
            .map_err(|e| anyhow::anyhow!("template parse error: {}", e))?;
        let tmpl = env.get_template("t")?;

        let mut map = serde_json::Map::new();
        map.insert("diff".to_string(), serde_json::Value::String(self.diff.clone()));
        map.insert("loop_iteration".to_string(), serde_json::Value::Number(self.loop_iteration.into()));
        for (key, val) in &self.stages {
            map.insert(key.clone(), val.clone());
        }

        let ctx_val = minijinja::Value::from_serialize(&serde_json::Value::Object(map));
        tmpl.render(ctx_val)
            .map_err(|e| anyhow::anyhow!("template render error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with_diff(diff: &str) -> TemplateContext {
        TemplateContext::new(diff.to_string())
    }

    #[test]
    fn render_diff_variable() {
        let ctx = ctx_with_diff("fn foo() {}");
        let rendered = ctx.render("DIFF: {{ diff }}").unwrap();
        assert_eq!(rendered, "DIFF: fn foo() {}");
    }

    #[test]
    fn render_loop_iteration() {
        let mut ctx = ctx_with_diff("");
        ctx.loop_iteration = 3;
        let rendered = ctx.render("iteration: {{ loop_iteration }}").unwrap();
        assert_eq!(rendered, "iteration: 3");
    }

    #[test]
    fn render_stage_output_at_top_level() {
        let mut ctx = ctx_with_diff("");
        ctx.stages.insert(
            "score_result".to_string(),
            serde_json::json!([{"agent": "score_readability", "score": 4}]),
        );
        let rendered = ctx.render("{{ score_result | length }}").unwrap();
        assert_eq!(rendered, "1");
    }

    #[test]
    fn render_nested_stage_value() {
        let mut ctx = ctx_with_diff("");
        ctx.stages.insert(
            "review_result".to_string(),
            serde_json::json!([{"location": "src/main.rs:10", "problem": "foo", "fix": "bar"}]),
        );
        let rendered = ctx.render("{{ review_result[0].location }}").unwrap();
        assert_eq!(rendered, "src/main.rs:10");
    }

    #[test]
    fn render_errors_on_invalid_template() {
        let ctx = ctx_with_diff("");
        let err = ctx.render("{{ unclosed").unwrap_err();
        assert!(err.to_string().len() > 0);
    }
}
