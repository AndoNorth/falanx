use std::collections::HashMap;

pub struct TemplateContext {
    pub diff: String,
    pub stages: HashMap<String, serde_json::Value>,
    pub loop_iteration: u32,
}
