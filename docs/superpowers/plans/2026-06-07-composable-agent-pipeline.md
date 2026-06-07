# Composable Agent Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hardcoded score→review→rewrite pipeline with a generic `WorkflowRunner` driven by `workflow.yaml` and agent directories, where each agent is a directory of markdown files and a config.

**Architecture:** A `WorkflowConfig` (loaded from embedded YAML or `.falanx/workflow.yaml`) describes stages, agents, output formats, and skip conditions. A `WorkflowRunner` drives a sequential stage loop, rendering minijinja prompt templates with prior stage outputs injected as template variables. All agent runs are stateless Cersei invocations; the `TemplateContext` is the only shared state between stages.

**Tech Stack:** Rust, Cersei (`cersei-agent`, `cersei-provider`), `minijinja` (new dep), `serde_yaml` (existing), `serde_json` (existing).

**Spec:** `docs/superpowers/specs/2026-06-04-composable-agent-pipeline-design.md`

**Note — predicate naming correction:** The spec lists `score_below_target` for the review skip condition, but the correct semantics are inverted. The plan uses `score_meets_target` (skip review when composite ≥ target — no need to review already-passing code) and `no_review_issues` (skip rewrite when review found nothing).

---

## Phase A — Foundation: types and agent definition

### Task A1: Add minijinja, delete scoring types, add AgentRunResult

**Files:**
- Modify: `rust/crates/falanx-engine/Cargo.toml`
- Modify: `rust/crates/falanx-engine/src/types.rs`

- [ ] **Add minijinja to Cargo.toml**

In `[dependencies]` section, add after `serde_yaml`:
```toml
minijinja = { version = "2", features = ["json"] }
```

- [ ] **Replace types.rs entirely**

```rust
// rust/crates/falanx-engine/src/types.rs
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
```

- [ ] **Verify it compiles (will have errors in dependents — expected)**

```bash
cd rust && cargo check 2>&1 | head -40
```

Expected: errors in `agents/`, `orchestrator/`, `session.rs` referencing deleted types. These are resolved in later tasks. Confirm `types.rs` itself compiles clean.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/Cargo.toml rust/crates/falanx-engine/src/types.rs
git commit -m "refactor(types): replace scoring types with generic AgentRunResult"
```

---

### Task A2: Create `agent/mod.rs` — AgentDef and AgentKind

**Files:**
- Create: `rust/crates/falanx-engine/src/agent/mod.rs`
- Create: `rust/crates/falanx-engine/src/agent/loader.rs` (stub)
- Create: `rust/crates/falanx-engine/src/agent/run.rs` (stub)

- [ ] **Write failing test first**

Create `rust/crates/falanx-engine/src/agent/mod.rs`:
```rust
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
```

Create stub `rust/crates/falanx-engine/src/agent/loader.rs`:
```rust
// populated in Task A3
```

Create stub `rust/crates/falanx-engine/src/agent/run.rs`:
```rust
// populated in Task A4
```

- [ ] **Register module in lib.rs**

Add to `rust/crates/falanx-engine/src/lib.rs`:
```rust
pub mod agent;
```

(Keep `pub mod agents;` for now — deleted in Phase F.)

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine agent::tests 2>&1
```

Expected: PASS.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/agent/
git add rust/crates/falanx-engine/src/lib.rs
git commit -m "feat(agent): add AgentDef and AgentKind structs"
```

---

### Task A3: Create `agent/loader.rs` — load AgentDef from directory

**Files:**
- Modify: `rust/crates/falanx-engine/src/agent/loader.rs`

Each agent directory contains `system.md`, `prompt.md`, `config.yaml`. The loader reads all three. Falls back to embedded defaults when no override directory is provided.

- [ ] **Write failing test**

```rust
// at the bottom of loader.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_agent_dir(dir: &std::path::Path, name: &str, max_turns: u32) -> std::path::PathBuf {
        let agent_dir = dir.join(name);
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(agent_dir.join("system.md"), "You are a reviewer.").unwrap();
        fs::write(agent_dir.join("prompt.md"), "Review: {{ diff }}").unwrap();
        fs::write(agent_dir.join("config.yaml"), format!("kind: cersei\nmax_turns: {}", max_turns)).unwrap();
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
}
```

- [ ] **Run test to verify it fails**

```bash
cd rust && cargo test -p falanx-engine agent::loader 2>&1 | tail -5
```

Expected: compile error — `load_from_dir` not defined.

- [ ] **Implement loader.rs**

```rust
use std::path::Path;
use crate::agent::{AgentDef, AgentKind};

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

    Ok(AgentDef {
        name: name.to_string(),
        kind: config.kind,
        system_prompt: system_prompt.trim().to_string(),
        prompt_template: prompt_template.trim().to_string(),
        max_turns: config.max_turns,
    })
}
```

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine agent::loader 2>&1
```

Expected: PASS.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/agent/loader.rs
git commit -m "feat(agent): loader reads AgentDef from agent directory"
```

---

### Task A4: Create `agent/run.rs` — run an agent, extract output

**Files:**
- Modify: `rust/crates/falanx-engine/src/agent/run.rs`

`run_agent` takes an `AgentDef`, a rendered prompt string, and `FalanxConfig`. Returns `AgentRunResult`. Output extraction uses `output_format` declared at the stage level — the caller passes it in.

- [ ] **Write failing test**

```rust
// agent/run.rs tests
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
```

- [ ] **Run test to verify it fails**

```bash
cd rust && cargo test -p falanx-engine agent::run 2>&1 | tail -5
```

Expected: compile error.

- [ ] **Implement run.rs**

```rust
use crate::{
    agent::AgentDef,
    config::FalanxConfig,
    provider::MockProvider,
    types::AgentRunResult,
    workflow::stage::OutputFormat,
};
use cersei_agent::Agent;

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
        turns_used: 1, // cersei does not expose turn count yet; placeholder
    })
}

pub fn extract_output(text: &str, format: &OutputFormat) -> anyhow::Result<serde_json::Value> {
    match format {
        OutputFormat::JsonObject => {
            let start = text.find('{').ok_or_else(|| anyhow::anyhow!("no JSON object in agent response"))?;
            let end = text.rfind('}').ok_or_else(|| anyhow::anyhow!("no JSON object end in agent response"))?;
            serde_json::from_str(&text[start..=end])
                .map_err(|e| anyhow::anyhow!("failed to parse JSON object: {}", e))
        }
        OutputFormat::JsonArray => {
            let start = text.find('[').ok_or_else(|| anyhow::anyhow!("no JSON array in agent response"))?;
            let end = text.rfind(']').ok_or_else(|| anyhow::anyhow!("no JSON array end in agent response"))?;
            serde_json::from_str(&text[start..=end])
                .map_err(|e| anyhow::anyhow!("failed to parse JSON array: {}", e))
        }
        OutputFormat::Text => Ok(serde_json::Value::String(text.to_string())),
    }
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
    // tests defined above
}
```

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine agent::run 2>&1
```

Expected: PASS. Note: `OutputFormat` won't exist yet — it's defined in Task B1. Add a temporary stub in `workflow/stage.rs` if needed, or implement Task B1 first before running this test.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/agent/run.rs
git commit -m "feat(agent): run_agent dispatches to cersei, extract_output handles json/text"
```

---

## Phase B — Workflow DSL

### Task B1: Create `workflow/stage.rs` — StageConfig, OutputFormat, SkipIf

**Files:**
- Create: `rust/crates/falanx-engine/src/workflow/mod.rs` (stub)
- Create: `rust/crates/falanx-engine/src/workflow/stage.rs`
- Create: `rust/crates/falanx-engine/src/workflow/context.rs` (stub)
- Create: `rust/crates/falanx-engine/src/workflow/runner.rs` (stub)

- [ ] **Create workflow module stubs**

`rust/crates/falanx-engine/src/workflow/mod.rs`:
```rust
pub mod context;
pub mod runner;
pub mod stage;
```

`rust/crates/falanx-engine/src/workflow/context.rs`:
```rust
// populated in Task B2
```

`rust/crates/falanx-engine/src/workflow/runner.rs`:
```rust
// populated in Task C1
```

Register in `lib.rs` — add:
```rust
pub mod workflow;
```

- [ ] **Write failing tests for stage.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_deserialises() {
        let f: OutputFormat = serde_yaml::from_str("json_object").unwrap();
        assert!(matches!(f, OutputFormat::JsonObject));
        let f: OutputFormat = serde_yaml::from_str("json_array").unwrap();
        assert!(matches!(f, OutputFormat::JsonArray));
        let f: OutputFormat = serde_yaml::from_str("text").unwrap();
        assert!(matches!(f, OutputFormat::Text));
    }

    #[test]
    fn skip_if_deserialises() {
        let s: SkipIf = serde_yaml::from_str("score_meets_target").unwrap();
        assert!(matches!(s, SkipIf::ScoreMeetsTarget));
        let s: SkipIf = serde_yaml::from_str("no_review_issues").unwrap();
        assert!(matches!(s, SkipIf::NoReviewIssues));
    }

    #[test]
    fn skip_if_unknown_fails_deserialisation() {
        let err = serde_yaml::from_str::<SkipIf>("fly_to_moon").unwrap_err();
        assert!(err.to_string().len() > 0);
    }

    #[test]
    fn stage_config_single_agent_resolves_names() {
        let yaml = r#"
id: review
agent: review
output_format: json_array
output_as: review_result
"#;
        let stage: StageConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(stage.agent_names(), vec!["review"]);
        assert_eq!(stage.output_as, "review_result");
        assert!(stage.skip_if.is_none());
    }

    #[test]
    fn stage_config_multi_agent_resolves_names() {
        let yaml = r#"
id: score
agents: [score_readability, score_security]
output_format: json_object
output_as: score_result
"#;
        let stage: StageConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(stage.agent_names(), vec!["score_readability", "score_security"]);
    }

    #[test]
    fn stage_config_errors_when_both_agent_and_agents_set() {
        let yaml = r#"
id: bad
agent: foo
agents: [bar]
output_format: text
output_as: out
"#;
        let stage: StageConfig = serde_yaml::from_str(yaml).unwrap();
        let err = stage.validate().unwrap_err();
        assert!(err.to_string().contains("agent"));
    }

    #[test]
    fn stage_config_errors_when_neither_set() {
        let yaml = r#"
id: bad
output_format: text
output_as: out
"#;
        let stage: StageConfig = serde_yaml::from_str(yaml).unwrap();
        let err = stage.validate().unwrap_err();
        assert!(err.to_string().contains("agent"));
    }
}
```

- [ ] **Run to verify failure**

```bash
cd rust && cargo test -p falanx-engine workflow::stage 2>&1 | tail -5
```

Expected: compile error.

- [ ] **Implement stage.rs**

```rust
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    JsonObject,
    JsonArray,
    Text,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipIf {
    ScoreMeetsTarget,
    NoReviewIssues,
}

impl SkipIf {
    /// Returns true if the stage should be skipped.
    pub fn evaluate(&self, ctx: &crate::workflow::context::TemplateContext, target_score: f32) -> bool {
        match self {
            SkipIf::ScoreMeetsTarget => {
                composite_from_context(ctx) >= target_score
            }
            SkipIf::NoReviewIssues => {
                ctx.stages
                    .get("review_result")
                    .and_then(|v| v.as_array())
                    .map(|a| a.is_empty())
                    .unwrap_or(true)
            }
        }
    }
}

fn composite_from_context(ctx: &crate::workflow::context::TemplateContext) -> f32 {
    let arr = match ctx.stages.get("score_result").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return 0.0,
    };
    if arr.is_empty() {
        return 0.0;
    }
    let sum: f32 = arr
        .iter()
        .filter_map(|v| v.get("score").and_then(|s| s.as_f64()))
        .map(|s| s as f32)
        .sum();
    sum / arr.len() as f32
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StageConfig {
    pub id: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    pub output_format: OutputFormat,
    pub output_as: String,
    pub skip_if: Option<SkipIf>,
}

impl StageConfig {
    pub fn agent_names(&self) -> Vec<String> {
        if let Some(ref name) = self.agent {
            vec![name.clone()]
        } else {
            self.agents.clone()
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let has_single = self.agent.is_some();
        let has_multi = !self.agents.is_empty();
        match (has_single, has_multi) {
            (true, true) => anyhow::bail!("stage '{}': set either 'agent' or 'agents', not both", self.id),
            (false, false) => anyhow::bail!("stage '{}': must set 'agent' or 'agents'", self.id),
            _ => Ok(()),
        }
    }
}

pub struct StageResult {
    pub stage_id: String,
    pub runs: Vec<crate::types::AgentRunResult>,
    pub synthesised: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    // tests defined above
}
```

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine workflow::stage 2>&1
```

Expected: PASS. (`TemplateContext` referenced in `SkipIf::evaluate` — add a temporary empty struct in `context.rs` if not yet implemented: `pub struct TemplateContext { pub stages: std::collections::HashMap<String, serde_json::Value>, pub diff: String, pub loop_iteration: u32 }`)

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/workflow/
git add rust/crates/falanx-engine/src/lib.rs
git commit -m "feat(workflow): StageConfig, OutputFormat, SkipIf with serde deserialization"
```

---

### Task B2: Create `workflow/context.rs` — TemplateContext and minijinja rendering

**Files:**
- Modify: `rust/crates/falanx-engine/src/workflow/context.rs`

- [ ] **Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
```

- [ ] **Run to verify failure**

```bash
cd rust && cargo test -p falanx-engine workflow::context 2>&1 | tail -5
```

Expected: compile error.

- [ ] **Implement context.rs**

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
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

        // Build flat context: all stage outputs at top level alongside diff and loop_iteration
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
    // tests defined above
}
```

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine workflow::context 2>&1
```

Expected: PASS.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/workflow/context.rs
git commit -m "feat(workflow): TemplateContext with minijinja rendering, stage outputs at top level"
```

---

### Task B3: Create `workflow/mod.rs` — WorkflowConfig, load from YAML

**Files:**
- Modify: `rust/crates/falanx-engine/src/workflow/mod.rs`

The workflow loader checks `.falanx/workflow.yaml` in the current working directory first, then falls back to the embedded default (compiled in via `include_str!`).

- [ ] **Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_YAML: &str = r#"
loop:
  max_iterations: 3
  target_score: 4.0
  plateau_threshold: 0.1
stages:
  - id: score
    agents: [score_readability]
    output_format: json_object
    output_as: score_result
  - id: review
    agent: review
    output_format: json_array
    output_as: review_result
    skip_if: score_meets_target
"#;

    #[test]
    fn parses_minimal_workflow() {
        let wf = WorkflowConfig::from_str(MINIMAL_YAML).unwrap();
        assert_eq!(wf.loop_cfg.max_iterations, 3);
        assert!((wf.loop_cfg.target_score - 4.0).abs() < f32::EPSILON);
        assert_eq!(wf.stages.len(), 2);
        assert_eq!(wf.stages[0].id, "score");
        assert_eq!(wf.stages[0].agent_names(), vec!["score_readability"]);
        assert_eq!(wf.stages[1].id, "review");
        assert!(wf.stages[1].skip_if.is_some());
    }

    #[test]
    fn validates_stages_on_load() {
        let bad = r#"
loop:
  max_iterations: 1
  target_score: 4.0
  plateau_threshold: 0.1
stages:
  - id: broken
    output_format: text
    output_as: out
"#;
        let err = WorkflowConfig::from_str(bad).unwrap_err();
        assert!(err.to_string().contains("agent"));
    }

    #[test]
    fn loads_embedded_default() {
        let wf = WorkflowConfig::load_default().unwrap();
        assert!(!wf.stages.is_empty());
        assert!(wf.stages.iter().any(|s| s.id == "score"));
    }
}
```

- [ ] **Run to verify failure**

```bash
cd rust && cargo test -p falanx-engine workflow::tests 2>&1 | tail -5
```

Expected: compile error.

- [ ] **Implement mod.rs**

```rust
pub mod context;
pub mod runner;
pub mod stage;

use stage::StageConfig;

const DEFAULT_WORKFLOW: &str = include_str!("../defaults/workflow.yaml");

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LoopConfig {
    pub max_iterations: u32,
    pub target_score: f32,
    pub plateau_threshold: f32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorkflowConfig {
    #[serde(rename = "loop")]
    pub loop_cfg: LoopConfig,
    pub stages: Vec<StageConfig>,
}

impl WorkflowConfig {
    pub fn from_str(yaml: &str) -> anyhow::Result<Self> {
        let wf: Self = serde_yaml::from_str(yaml)
            .map_err(|e| anyhow::anyhow!("invalid workflow YAML: {}", e))?;
        for stage in &wf.stages {
            stage.validate()?;
        }
        Ok(wf)
    }

    /// Load from `.falanx/workflow.yaml` if present, else embedded default.
    pub fn load(agents_dir_override: Option<&std::path::Path>) -> anyhow::Result<Self> {
        let override_path = agents_dir_override
            .map(|d| d.parent().unwrap_or(d).join("workflow.yaml"))
            .unwrap_or_else(|| std::path::PathBuf::from(".falanx/workflow.yaml"));

        if override_path.exists() {
            let yaml = std::fs::read_to_string(&override_path)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", override_path.display(), e))?;
            Self::from_str(&yaml)
        } else {
            Self::load_default()
        }
    }

    pub fn load_default() -> anyhow::Result<Self> {
        Self::from_str(DEFAULT_WORKFLOW)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // tests defined above
}
```

- [ ] **Create the embedded default files (required for `include_str!` to compile)**

The file `src/defaults/workflow.yaml` must exist. Create it now with a placeholder — the full content is added in Task B5.

```bash
mkdir -p rust/crates/falanx-engine/src/defaults
cat > rust/crates/falanx-engine/src/defaults/workflow.yaml << 'EOF'
loop:
  max_iterations: 5
  target_score: 4.5
  plateau_threshold: 0.1

stages:
  - id: score
    agents:
      - score_readability
      - score_maintainability
      - score_architecture
      - score_performance
      - score_security
    output_format: json_object
    output_as: score_result

  - id: review
    agent: review
    output_format: json_array
    output_as: review_result
    skip_if: score_meets_target

  - id: rewrite
    agent: rewrite
    output_format: json_array
    output_as: rewrite_result
    skip_if: no_review_issues
EOF
```

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine workflow::tests 2>&1
```

Expected: PASS.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/workflow/mod.rs
git add rust/crates/falanx-engine/src/defaults/
git commit -m "feat(workflow): WorkflowConfig loads from YAML with embedded default"
```

---

### Task B4: Create default agent directories — scoring agents

**Files:**
- Create: `rust/crates/falanx-engine/src/defaults/agents/score_readability/{system.md,prompt.md,config.yaml}`
- Create: `rust/crates/falanx-engine/src/defaults/agents/score_maintainability/{system.md,prompt.md,config.yaml}`
- Create: `rust/crates/falanx-engine/src/defaults/agents/score_architecture/{system.md,prompt.md,config.yaml}`
- Create: `rust/crates/falanx-engine/src/defaults/agents/score_performance/{system.md,prompt.md,config.yaml}`
- Create: `rust/crates/falanx-engine/src/defaults/agents/score_security/{system.md,prompt.md,config.yaml}`

All scoring agents share the same system prompt. Each has a unique prompt template and `max_turns: 1`.

- [ ] **Create scoring agent directories**

```bash
for agent in score_readability score_maintainability score_architecture score_performance score_security; do
  mkdir -p rust/crates/falanx-engine/src/defaults/agents/$agent
  echo "kind: cersei
max_turns: 1" > rust/crates/falanx-engine/src/defaults/agents/$agent/config.yaml
  echo "You are a code quality reviewer. You evaluate the quality of changes in a diff, not the codebase generally." > rust/crates/falanx-engine/src/defaults/agents/$agent/system.md
done
```

- [ ] **Write prompt.md for score_readability**

`rust/crates/falanx-engine/src/defaults/agents/score_readability/prompt.md`:
```markdown
Do these changes introduce readability problems?
Score 5 if the changes are clear or neutral. Score lower only if the changes actively make the code harder to read.

Questions to answer:
- Do the new names obscure intent?
- Does the new control flow become harder to follow?
- Do new abstractions add confusion rather than clarity?

Score 1-5. Respond in JSON only:
{"score": N, "reasoning": "max 80 words"}

DIFF:
{{ diff }}
```

- [ ] **Write prompt.md for score_maintainability**

`rust/crates/falanx-engine/src/defaults/agents/score_maintainability/prompt.md`:
```markdown
Do these changes introduce maintainability problems?
Score 5 if the changes are clean or neutral. Score lower only if the changes actively make the code harder to maintain.

Questions to answer:
- Do the changes blur responsibilities or mix concerns?
- Do the changes make future modifications riskier?
- Do the changes introduce hidden side effects or implicit dependencies?

Score 1-5. Respond in JSON only:
{"score": N, "reasoning": "max 80 words"}

DIFF:
{{ diff }}
```

- [ ] **Write prompt.md for score_architecture**

`rust/crates/falanx-engine/src/defaults/agents/score_architecture/prompt.md`:
```markdown
Do these changes introduce architectural problems?
Score 5 if the changes fit the existing structure or are neutral. Score lower only if the changes actively damage boundaries or structure.

Questions to answer:
- Do the changes violate existing module boundaries?
- Do the changes introduce inappropriate coupling?
- Do the changes add complexity without necessity?

Score 1-5. Respond in JSON only:
{"score": N, "reasoning": "max 80 words"}

DIFF:
{{ diff }}
```

- [ ] **Write prompt.md for score_performance**

`rust/crates/falanx-engine/src/defaults/agents/score_performance/prompt.md`:
```markdown
Do these changes introduce performance problems?
Score 5 if the changes are efficient or neutral. Score lower only if the changes actively introduce regressions.

Questions to answer:
- Do the changes add unnecessary allocations or copies?
- Do the changes introduce worse algorithmic complexity where better is feasible?
- Do the changes block where async would be appropriate?

Score 1-5. Respond in JSON only:
{"score": N, "reasoning": "max 80 words"}

DIFF:
{{ diff }}
```

- [ ] **Write prompt.md for score_security**

`rust/crates/falanx-engine/src/defaults/agents/score_security/prompt.md`:
```markdown
Do these changes introduce security problems?
Score 5 if the changes are safe or security is not relevant to this diff. Score lower only if the changes actively introduce risk.

Questions to answer:
- Do the changes create input validation gaps?
- Do the changes expose secrets or introduce injection risks?
- Do the changes cross trust boundaries unsafely?

Score 1-5. Respond in JSON only:
{"score": N, "reasoning": "max 80 words"}

DIFF:
{{ diff }}
```

- [ ] **Verify directory structure**

```bash
find rust/crates/falanx-engine/src/defaults/agents -type f | sort
```

Expected: 15 files (3 per agent × 5 agents).

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/defaults/agents/
git commit -m "feat(defaults): add scoring agent directories with prompts"
```

---

### Task B5: Create default agent directories — review and rewrite agents

**Files:**
- Create: `rust/crates/falanx-engine/src/defaults/agents/review/{system.md,prompt.md,config.yaml}`
- Create: `rust/crates/falanx-engine/src/defaults/agents/rewrite/{system.md,prompt.md,config.yaml}`

- [ ] **Create review agent**

`rust/crates/falanx-engine/src/defaults/agents/review/config.yaml`:
```yaml
kind: cersei
max_turns: 1
```

`rust/crates/falanx-engine/src/defaults/agents/review/system.md`:
```
You are a senior code reviewer. You identify concrete, actionable issues in code changes. You are scoped to the blast radius of the diff only — you do not critique the wider codebase.
```

`rust/crates/falanx-engine/src/defaults/agents/review/prompt.md`:
```markdown
Review this diff. The current quality scores are:

{% for item in score_result %}
- {{ item.agent | replace("score_", "") }}: {{ item.score }}/5 — {{ item.reasoning }}
{% endfor %}

{% set total = 0 %}
{% for item in score_result %}{% set total = total + item.score %}{% endfor %}
Composite: {{ (score_result | map(attribute="score") | list | join(" + ")) }} / {{ score_result | length }} categories

Focus your critique on the lowest-scoring categories. For each issue identify: the exact location, what is wrong and why it matters, and how to fix it.

Respond as JSON array only. Return [] if no issues found.
[{"location":"file:line","problem":"description of the issue","fix":"specific fix suggestion"}]

DIFF:
{{ diff }}
```

- [ ] **Create rewrite agent**

`rust/crates/falanx-engine/src/defaults/agents/rewrite/config.yaml`:
```yaml
kind: cersei
max_turns: 3
```

`rust/crates/falanx-engine/src/defaults/agents/rewrite/system.md`:
```
You are a precise code editor. You apply the minimum changes required to fix identified issues. Every change you make must be traceable to a specific critique item. You do not make unrequested improvements.
```

`rust/crates/falanx-engine/src/defaults/agents/rewrite/prompt.md`:
```markdown
Fix the following issues in this diff. Apply only the changes needed to address each issue — nothing more.

For each change, include the exact original text, the revised text, and which issue it addresses.

Respond as JSON array only. Return [] if no changes are needed.
[{"original":"exact original text from diff","revised":"replacement text","issue_ref":"location from issue"}]

ISSUES:
{{ review_result | tojson }}

DIFF:
{{ diff }}
```

- [ ] **Verify all 7 agents exist**

```bash
find rust/crates/falanx-engine/src/defaults/agents -name "config.yaml" | sort
```

Expected: 7 files.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/defaults/agents/review/
git add rust/crates/falanx-engine/src/defaults/agents/rewrite/
git commit -m "feat(defaults): add review and rewrite agent directories with prompts"
```

---

## Phase C — WorkflowRunner

### Task C1: Create `workflow/runner.rs` — stage execution loop

**Files:**
- Modify: `rust/crates/falanx-engine/src/workflow/runner.rs`

The runner drives stages sequentially. For multi-agent stages, runs agents one at a time (sequential). Synthesises results into `TemplateContext`. Owns the outer loop with exit conditions.

- [ ] **Write failing integration test**

```rust
// workflow/runner.rs tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{FalanxConfig, LoopConfig, ProviderConfig, SessionConfig},
        session::Session,
        workflow::WorkflowConfig,
    };

    fn dry_cfg() -> FalanxConfig {
        FalanxConfig {
            provider: ProviderConfig { model: "mock".into(), api_key: "".into(), base_url: None, dry_run: true },
            session: SessionConfig { dir: std::path::PathBuf::from("/tmp") },
            loop_cfg: LoopConfig::default(),
        }
    }

    #[tokio::test]
    async fn runner_executes_single_stage_dry_run() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dry_cfg();
        let session = Session::new(dir.path(), "test").unwrap();

        let yaml = r#"
loop:
  max_iterations: 1
  target_score: 5.0
  plateau_threshold: 0.1
stages:
  - id: score
    agents: [score_readability]
    output_format: json_object
    output_as: score_result
"#;
        let workflow = WorkflowConfig::from_str(yaml).unwrap();

        // Build a temporary agent dir with score_readability
        let agents_dir = dir.path().join("agents");
        std::fs::create_dir_all(agents_dir.join("score_readability")).unwrap();
        std::fs::write(agents_dir.join("score_readability/system.md"), "sys").unwrap();
        std::fs::write(agents_dir.join("score_readability/prompt.md"), "score: {{ diff }}").unwrap();
        std::fs::write(agents_dir.join("score_readability/config.yaml"), "kind: cersei\nmax_turns: 1").unwrap();

        let runner = WorkflowRunner::new(&workflow, &cfg, &session, &agents_dir);
        let result = runner.run("fn foo() {}").await.unwrap();

        assert_eq!(result.iterations, 1);
        assert!(result.summary.is_object() || result.summary.is_null());

        // Session should have stage events
        let content = std::fs::read_to_string(session.path()).unwrap();
        assert!(content.contains("stage_started"));
        assert!(content.contains("agent_completed"));
        assert!(content.contains("stage_completed"));
        assert!(content.contains("run_completed"));
    }
}
```

- [ ] **Run to verify failure**

```bash
cd rust && cargo test -p falanx-engine workflow::runner 2>&1 | tail -5
```

Expected: compile error.

- [ ] **Implement runner.rs**

```rust
use std::path::{Path, PathBuf};
use crate::{
    agent::{loader::load_from_dir, run::run_agent},
    config::FalanxConfig,
    orchestrator,
    session::{Session, SessionEvent},
    types::AgentRunResult,
    workflow::{
        context::TemplateContext,
        stage::{OutputFormat, SkipIf, StageConfig, StageResult},
        WorkflowConfig,
    },
};
use crate::orchestrator::horizon::HorizonState;

pub struct WorkflowRunner<'a> {
    workflow: &'a WorkflowConfig,
    cfg: &'a FalanxConfig,
    session: &'a Session,
    agents_dir: PathBuf,
}

impl<'a> WorkflowRunner<'a> {
    pub fn new(
        workflow: &'a WorkflowConfig,
        cfg: &'a FalanxConfig,
        session: &'a Session,
        agents_dir: &Path,
    ) -> Self {
        Self { workflow, cfg, session, agents_dir: agents_dir.to_path_buf() }
    }

    pub async fn run(&self, diff: &str) -> anyhow::Result<orchestrator::RunResult> {
        let session_id = self.session.id().clone();
        let mut horizon = HorizonState::new();
        let mut iteration = 0u32;
        let mut prev_composite: Option<f32> = None;
        let mut plateau_streak = 0u32;
        let mut final_ctx = TemplateContext::new(diff.to_string());

        loop {
            let mut ctx = TemplateContext::new(diff.to_string());
            ctx.loop_iteration = iteration;

            for stage_cfg in &self.workflow.stages {
                if let Some(skip_if) = &stage_cfg.skip_if {
                    if skip_if.evaluate(&ctx, self.workflow.loop_cfg.target_score) {
                        self.session.append(SessionEvent::StageSkipped {
                            stage_id: stage_cfg.id.clone(),
                            condition: format!("{:?}", skip_if),
                            iteration,
                        })?;
                        continue;
                    }
                }
                let stage_result = self.run_stage(stage_cfg, &ctx).await?;
                ctx.stages.insert(stage_cfg.output_as.clone(), stage_result.synthesised.clone());
                self.session.append(SessionEvent::StageCompleted {
                    stage_id: stage_cfg.id.clone(),
                    output_as: stage_cfg.output_as.clone(),
                    result: stage_result.synthesised,
                    iteration,
                })?;
            }

            final_ctx = ctx.clone();
            let composite = composite_from_ctx(&final_ctx);

            if composite >= self.workflow.loop_cfg.target_score {
                break;
            }
            if iteration >= self.workflow.loop_cfg.max_iterations {
                break;
            }

            if let Some(prev) = prev_composite {
                if (composite - prev).abs() < self.workflow.loop_cfg.plateau_threshold {
                    plateau_streak += 1;
                } else {
                    plateau_streak = 0;
                }
            }
            prev_composite = Some(composite);

            if plateau_streak >= 2 {
                if horizon.should_reset(true) {
                    horizon.record_reset();
                    plateau_streak = 0;
                    prev_composite = None;
                    self.session.append(SessionEvent::HorizonReset { iteration })?;
                } else {
                    break;
                }
            }

            iteration += 1;
        }

        let summary = serde_json::to_value(&final_ctx.stages).unwrap_or_default();
        self.session.append(SessionEvent::RunCompleted {
            iterations: iteration,
            summary: summary.clone(),
        })?;

        Ok(orchestrator::RunResult { iterations: iteration, session_id, summary })
    }

    async fn run_stage(&self, stage_cfg: &StageConfig, ctx: &TemplateContext) -> anyhow::Result<StageResult> {
        self.session.append(SessionEvent::StageStarted {
            stage_id: stage_cfg.id.clone(),
            iteration: ctx.loop_iteration,
        })?;

        let mut runs: Vec<AgentRunResult> = Vec::new();

        for agent_name in stage_cfg.agent_names() {
            let def = load_from_dir(&self.agents_dir, &agent_name)?;
            let rendered = ctx.render(&def.prompt_template)?;

            self.session.append(SessionEvent::AgentStarted {
                stage_id: stage_cfg.id.clone(),
                agent_name: agent_name.clone(),
                iteration: ctx.loop_iteration,
            })?;

            let result = run_agent(&def, &rendered, self.cfg, &stage_cfg.output_format).await?;

            self.session.append(SessionEvent::AgentCompleted {
                stage_id: stage_cfg.id.clone(),
                agent_name: agent_name.clone(),
                turns_used: result.turns_used,
                output: result.extracted.clone(),
                iteration: ctx.loop_iteration,
            })?;

            runs.push(result);
        }

        let synthesised = if runs.len() == 1 {
            runs[0].extracted.clone()
        } else {
            let arr = runs.iter().map(|r| {
                let mut obj = r.extracted.as_object().cloned().unwrap_or_default();
                obj.insert("agent".to_string(), serde_json::Value::String(r.agent_name.clone()));
                serde_json::Value::Object(obj)
            }).collect();
            serde_json::Value::Array(arr)
        };

        Ok(StageResult { stage_id: stage_cfg.id.clone(), runs, synthesised })
    }
}

fn composite_from_ctx(ctx: &TemplateContext) -> f32 {
    let arr = match ctx.stages.get("score_result").and_then(|v| v.as_array()) {
        Some(a) if !a.is_empty() => a,
        _ => return 0.0,
    };
    let sum: f32 = arr.iter()
        .filter_map(|v| v.get("score").and_then(|s| s.as_f64()))
        .map(|s| s as f32)
        .sum();
    sum / arr.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    // tests defined above
}
```

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine workflow::runner 2>&1
```

Expected: PASS.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/workflow/runner.rs
git commit -m "feat(workflow): WorkflowRunner drives sequential stage loop with horizon reset"
```

---

### Task C2: Rewrite `orchestrator/mod.rs` to use WorkflowRunner

**Files:**
- Modify: `rust/crates/falanx-engine/src/orchestrator/mod.rs`

The orchestrator's public interface (`RunConfig`, `RunResult`, `run()`) is preserved. Internals are replaced — the hardcoded pipeline call is removed and `WorkflowRunner` is wired in.

- [ ] **Write test verifying orchestrator delegates to WorkflowRunner**

```rust
// orchestrator/mod.rs tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{FalanxConfig, LoopConfig, ProviderConfig, SessionConfig},
        git::ReviewTarget,
        session::Session,
    };

    fn dry_cfg() -> FalanxConfig {
        FalanxConfig {
            provider: ProviderConfig { model: "mock".into(), api_key: "".into(), base_url: None, dry_run: true },
            session: SessionConfig { dir: std::path::PathBuf::from("/tmp") },
            loop_cfg: LoopConfig::default(),
        }
    }

    #[tokio::test]
    async fn run_returns_result_with_session_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = dry_cfg();
        cfg.session.dir = dir.path().to_path_buf();
        cfg.loop_cfg.max_iter = 1;

        let f = dir.path().join("test.rs");
        std::fs::write(&f, "fn main() {}").unwrap();

        let session = Session::new(&cfg.session.dir, "test").unwrap();
        let run_cfg = RunConfig {
            target: ReviewTarget::File(f),
            session,
        };

        let result = run(run_cfg, &cfg).await.unwrap();
        assert_eq!(result.iterations, 1);
        assert!(!result.session_id.0.is_empty());
    }
}
```

- [ ] **Run to verify failure**

```bash
cd rust && cargo test -p falanx-engine orchestrator::tests 2>&1 | tail -5
```

Expected: compile error or test failure on old pipeline call.

- [ ] **Rewrite orchestrator/mod.rs**

```rust
pub mod horizon;

use crate::{
    config::FalanxConfig,
    git::ReviewTarget,
    session::{Session, SessionEvent},
    types::SessionId,
    workflow::{runner::WorkflowRunner, WorkflowConfig},
};

pub struct RunConfig {
    pub target: ReviewTarget,
    pub session: Session,
}

pub struct RunResult {
    pub iterations: u32,
    pub session_id: SessionId,
    pub summary: serde_json::Value,
}

pub async fn run(config: RunConfig, falanx_cfg: &FalanxConfig) -> anyhow::Result<RunResult> {
    let RunConfig { target, session } = config;
    let session_id = session.id().clone();

    let diff = target.extract_diff()?;
    let max_diff_chars: usize = std::env::var("FALANX_MAX_DIFF_CHARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    let diff_text = if diff.0.len() > max_diff_chars {
        tracing::warn!(original = diff.0.len(), truncated_to = max_diff_chars, "diff truncated");
        diff.0.chars().take(max_diff_chars).collect()
    } else {
        diff.0.clone()
    };

    session.append(SessionEvent::RunStarted {
        session_id: session_id.0.clone(),
        target: "run".to_string(),
        workflow: "default".to_string(),
    })?;

    // Resolve agents directory: check .falanx/agents first, else embedded defaults
    let agents_dir = resolve_agents_dir(None);
    let workflow = WorkflowConfig::load(None)
        .map_err(|e| anyhow::anyhow!("failed to load workflow: {}", e))?;

    // Apply CLI loop overrides on top of workflow defaults
    let mut workflow = workflow;
    workflow.loop_cfg.max_iterations = falanx_cfg.loop_cfg.max_iter;
    workflow.loop_cfg.target_score = falanx_cfg.loop_cfg.target_score;
    workflow.loop_cfg.plateau_threshold = falanx_cfg.loop_cfg.plateau_threshold;

    let runner = WorkflowRunner::new(&workflow, falanx_cfg, &session, &agents_dir);
    let result = runner.run(&diff_text).await?;

    tracing::info!(
        iterations = result.iterations,
        session_id = %result.session_id.0,
        "run complete"
    );

    Ok(result)
}

fn resolve_agents_dir(override_path: Option<&std::path::Path>) -> std::path::PathBuf {
    if let Some(p) = override_path {
        return p.to_path_buf();
    }
    let local = std::path::PathBuf::from(".falanx/agents");
    if local.exists() {
        local
    } else {
        // Caller falls back to embedded defaults — loader handles missing dir gracefully
        // by returning an error; embedded defaults are loaded differently (see Task E1)
        local
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // tests defined above
}
```

**Note:** The embedded-defaults agent loading requires wiring `include_str!` constants for each agent. This is done in Task E1 when the CLI wires the agents directory. For the test to pass with dry_run, the `MockProvider` must handle the case where agents are not found — adjust the test to point to the defaults dir if needed.

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine orchestrator 2>&1
```

Expected: PASS.

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/orchestrator/mod.rs
git commit -m "refactor(orchestrator): delegate to WorkflowRunner, remove hardcoded pipeline"
```

---

## Phase D — Session refactor

### Task D1: Refactor `session.rs` — generic SessionEvent, updated SessionMeta

**Files:**
- Modify: `rust/crates/falanx-engine/src/session.rs`

Remove `CategoryScored`, `ScoringComplete`, `IssuesFound`, `RewriteApplied`. Add `StageStarted`, `AgentStarted`, `AgentCompleted`, `StageCompleted`, `StageSkipped`. Update `RunStarted` to include `workflow` field. Update `RunCompleted` to carry `summary`. Update `SessionMeta` to use `summary`.

- [ ] **Write failing tests for new events**

```rust
// in session.rs tests
#[test]
fn stage_started_serialises() {
    let dir = tempfile::tempdir().unwrap();
    let session = Session::new(dir.path(), "t").unwrap();
    session.append(SessionEvent::StageStarted { stage_id: "score".into(), iteration: 0 }).unwrap();
    let content = std::fs::read_to_string(session.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(v["type"], "stage_started");
    assert_eq!(v["stage_id"], "score");
}

#[test]
fn agent_completed_serialises_with_output() {
    let dir = tempfile::tempdir().unwrap();
    let session = Session::new(dir.path(), "t").unwrap();
    session.append(SessionEvent::AgentCompleted {
        stage_id: "score".into(),
        agent_name: "score_readability".into(),
        turns_used: 1,
        output: serde_json::json!({"score": 4, "reasoning": "ok"}),
        iteration: 0,
    }).unwrap();
    let content = std::fs::read_to_string(session.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(v["type"], "agent_completed");
    assert_eq!(v["agent_name"], "score_readability");
    assert_eq!(v["output"]["score"], 4);
}

#[test]
fn run_completed_carries_summary() {
    let dir = tempfile::tempdir().unwrap();
    let session = Session::new(dir.path(), "t").unwrap();
    session.append(SessionEvent::RunStarted {
        session_id: session.id().0.clone(),
        target: "HEAD~1".into(),
        workflow: "default".into(),
    }).unwrap();
    session.append(SessionEvent::RunCompleted {
        iterations: 2,
        summary: serde_json::json!({"score_result": []}),
    }).unwrap();
    let sessions = Session::list(dir.path()).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].iterations, Some(2));
    assert!(sessions[0].summary.is_some());
}
```

- [ ] **Run to verify failure**

```bash
cd rust && cargo test -p falanx-engine session 2>&1 | grep "FAILED\|error" | head -10
```

Expected: test failures on missing event variants.

- [ ] **Replace SessionEvent in session.rs**

Replace the `SessionEvent` enum with:
```rust
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    RunStarted   { session_id: String, target: String, workflow: String },
    RunFailed    { reason: String },
    HorizonReset { iteration: u32 },
    StageStarted   { stage_id: String, iteration: u32 },
    AgentStarted   { stage_id: String, agent_name: String, iteration: u32 },
    AgentCompleted {
        stage_id: String,
        agent_name: String,
        turns_used: u32,
        output: serde_json::Value,
        iteration: u32,
    },
    StageCompleted { stage_id: String, output_as: String, result: serde_json::Value, iteration: u32 },
    StageSkipped   { stage_id: String, condition: String, iteration: u32 },
    RunCompleted   { iterations: u32, summary: serde_json::Value },
}
```

Update `SessionMeta`:
```rust
pub struct SessionMeta {
    pub id: SessionId,
    pub path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub summary: Option<serde_json::Value>,
    pub iterations: Option<u32>,
}
```

Update `Session::list()` — the `RunStarted` line now has a `workflow` field. Update the first-line parse to extract `session_id` from the nested field (it's now inside the flattened event fields — with `#[serde(flatten)]` on `SessionEntry`, `session_id` appears at the top level when `type == run_started`). Update `RunCompleted` match to extract `summary` and `iterations`.

- [ ] **Run tests**

```bash
cd rust && cargo test -p falanx-engine session 2>&1
```

Expected: PASS on new tests. Fix any compile errors in files that referenced deleted events (`pipeline.rs` — already targeted for deletion; ignore those for now).

- [ ] **Commit**

```bash
git add rust/crates/falanx-engine/src/session.rs
git commit -m "refactor(session): generic SessionEvent, remove scoring-specific events"
```

---

## Phase E — CLI refactor

### Task E1: Rewrite `main.rs` — remove `score`, add `run` with new flags

**Files:**
- Modify: `rust/crates/falanx/src/main.rs`

`Commands::Score` and `ScoreArgs` are removed. `Commands::Review` becomes `Commands::Run` with `RunArgs`. New flags: `--workflow`, `--agents`.

- [ ] **Write the new main.rs**

```rust
use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use falanx_engine::{
    config::FalanxConfig,
    git::ReviewTarget,
    orchestrator::{self, RunConfig},
    session::Session,
};

#[derive(Parser)]
#[command(name = "falanx", version, about = "Automated code review runtime")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the agent workflow against a diff or file
    Run(RunArgs),
    /// List past sessions
    ListSessions,
    /// Start falanx in serve mode (MCP + HTTP API)
    Serve(ServeArgs),
}

#[derive(Args)]
struct RunArgs {
    /// Git range to diff (e.g. HEAD~1, main..feature)
    #[arg(long)]
    diff: Option<String>,
    /// Path to a source file to review
    #[arg(long)]
    file: Option<PathBuf>,
    /// Workflow to run (default: embedded default workflow)
    #[arg(long)]
    workflow: Option<String>,
    /// Directory of additional agent definitions (merged with embedded defaults)
    #[arg(long)]
    agents: Option<PathBuf>,
    /// Override workflow max_iterations
    #[arg(long)]
    max_iter: Option<u32>,
    /// Override workflow target_score
    #[arg(long)]
    target: Option<f32>,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct ServeArgs {
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct CommonArgs {
    #[arg(long, env = "FALANX_CONFIG")]
    config: Option<PathBuf>,
    #[arg(long, env = "FALANX_DRY_RUN")]
    dry_run: bool,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;
    rt.block_on(run(args))
}

async fn run(args: Cli) -> anyhow::Result<()> {
    match args.command {
        Commands::Run(args) => cmd_run(args).await,
        Commands::ListSessions => cmd_list_sessions().await,
        Commands::Serve(_) => {
            tracing::warn!("falanx serve: not yet implemented");
            std::process::exit(0);
        }
    }
}

async fn cmd_run(args: RunArgs) -> anyhow::Result<()> {
    let mut cfg = FalanxConfig::from_env()?;
    if args.common.dry_run { cfg.provider.dry_run = true; }
    if let Some(n) = args.max_iter { cfg.loop_cfg.max_iter = n; }
    if let Some(t) = args.target { cfg.loop_cfg.target_score = t; }
    cfg.validate()?;

    let target = resolve_target(args.file, args.diff)?;
    let label = target_label(&target);
    let session = Session::new(&cfg.session.dir, &label)?;

    let run_cfg = RunConfig { target, session };
    let result = orchestrator::run(run_cfg, &cfg).await?;

    tracing::info!(
        iterations = result.iterations,
        session_id = %result.session_id.0,
        "run complete"
    );
    Ok(())
}

async fn cmd_list_sessions() -> anyhow::Result<()> {
    let cfg = FalanxConfig::from_env()?;
    let sessions = Session::list(&cfg.session.dir)?;

    if sessions.is_empty() {
        println!("No sessions found in {}", cfg.session.dir.display());
        return Ok(());
    }

    println!("{:<38} {:<24} {}", "SESSION ID", "STARTED", "ITERATIONS");
    for meta in &sessions {
        let iters = meta.iterations.map(|i| i.to_string()).unwrap_or_else(|| "-".into());
        println!("{:<38} {:<24} {}", meta.id.0, meta.started_at.format("%Y-%m-%d %H:%M:%S UTC"), iters);
    }
    Ok(())
}

fn resolve_target(file: Option<PathBuf>, diff: Option<String>) -> anyhow::Result<ReviewTarget> {
    match (file, diff) {
        (Some(path), None) => Ok(ReviewTarget::File(path)),
        (None, Some(range)) => Ok(ReviewTarget::GitRange(range)),
        (Some(_), Some(_)) => anyhow::bail!("specify --file or --diff, not both"),
        (None, None) => anyhow::bail!("specify --file <path> or --diff <range>"),
    }
}

fn target_label(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::File(path) => path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        ReviewTarget::GitRange(range) => range.clone(),
    }
}
```

- [ ] **Build to check for compile errors**

```bash
cd rust && cargo build 2>&1 | head -40
```

Fix any remaining compile errors from old references before proceeding.

- [ ] **Run all tests**

```bash
cd rust && cargo test 2>&1 | tail -20
```

Expected: all passing tests pass, deleted-module tests gone.

- [ ] **Commit**

```bash
git add rust/crates/falanx/src/main.rs
git commit -m "refactor(cli): replace score/review commands with generic run command"
```

---

## Phase F — Cleanup

### Task F1: Delete old agent modules and pipeline.rs

**Files:**
- Delete: `rust/crates/falanx-engine/src/agents/` (entire directory)
- Delete: `rust/crates/falanx-engine/src/orchestrator/pipeline.rs`
- Modify: `rust/crates/falanx-engine/src/lib.rs`

- [ ] **Delete old modules**

```bash
rm -rf rust/crates/falanx-engine/src/agents/
rm rust/crates/falanx-engine/src/orchestrator/pipeline.rs
```

- [ ] **Update lib.rs — remove `pub mod agents`**

`rust/crates/falanx-engine/src/lib.rs`:
```rust
pub mod agent;
pub mod audit_hook;
pub mod config;
pub mod git;
pub mod orchestrator;
pub mod provider;
pub mod session;
pub mod types;
pub mod workflow;
```

- [ ] **Build clean**

```bash
cd rust && cargo build 2>&1
```

Expected: no errors.

- [ ] **Run full test suite**

```bash
cd rust && cargo test 2>&1
```

Expected: all passing, no references to deleted modules.

- [ ] **Run validate**

```bash
validate
```

Expected: fmt + clippy + nextest all pass.

- [ ] **Commit**

```bash
git add -A
git commit -m "refactor: delete old agents module and hardcoded pipeline"
```

---

## Phase G — Documentation

### Task G1: Rewrite `docs/ARCHITECTURE.md`

**Files:**
- Modify: `docs/ARCHITECTURE.md`

Rewrite to reflect the WorkflowRunner model. Remove references to `CodeQualityAgent`, `CodeReviewAgent`, `CodeWritingAgent`. Update the core engine diagram. Update CLI section to `falanx run`. Update session events table.

- [ ] **Rewrite ARCHITECTURE.md**

Replace the entire file. Key sections that change:

**Core Engine diagram** — replace the three-agent box with:
```
┌──────────────────────────────────────────────────────────────┐
│                      Rust Core Engine                         │
│                                                               │
│   ┌───────────────────────────────────────────────────────┐   │
│   │                  WorkflowRunner                       │   │
│   │  - loads workflow.yaml (embedded default or override) │   │
│   │  - drives sequential stage loop                       │   │
│   │  - owns TemplateContext — accumulates stage outputs   │   │
│   │  - exits: target score | max iterations | plateau     │   │
│   │  - recovers: horizon reset on context degradation     │   │
│   │  - produces: JSONL audit trail per run                │   │
│   └───────────────────────────────────────────────────────┘   │
│                                                               │
│   Stage loop (sequential):                                    │
│   score stage (N agents) → review stage → rewrite stage       │
│   Each stage: render template → run Cersei agent → extract    │
│                                                               │
│   ┌───────────────────────────────────────────────────────┐   │
│   │              Model Abstraction Layer                  │   │
│   │   Provider trait — Anthropic │ OpenAI-compat │ Ollama │   │
│   └───────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

**Session events** — update table to new generic events (StageStarted, AgentCompleted, StageCompleted, etc.).

**CLI section** — update usage to `falanx run --diff HEAD~1`.

- [ ] **Commit**

```bash
git add docs/ARCHITECTURE.md
git commit -m "docs: rewrite ARCHITECTURE.md for composable agent pipeline"
```

---

### Task G2: Rewrite `docs/AGENTS.md`

**Files:**
- Modify: `docs/AGENTS.md`

Remove `CodeQualityAgent`, `CodeReviewAgent`, `CodeWritingAgent` sections. Replace with `AgentDef` model, agent directory structure, and workflow stage descriptions.

- [ ] **Rewrite AGENTS.md**

Key sections:

**Agent definition** — directory structure (`system.md`, `prompt.md`, `config.yaml`), `AgentDef` fields, `AgentKind`.

**Default agents** — table of the 7 embedded agents with their purpose, max_turns, output format.

| Agent | Purpose | max_turns | output_format |
|---|---|---|---|
| `score_readability` | Readability score 1–5 | 1 | json_object |
| `score_maintainability` | Maintainability score 1–5 | 1 | json_object |
| `score_architecture` | Architecture score 1–5 | 1 | json_object |
| `score_performance` | Performance score 1–5 | 1 | json_object |
| `score_security` | Security score 1–5 | 1 | json_object |
| `review` | Critique issues from diff + scores | 1 | json_array |
| `rewrite` | Apply fixes from critique | 3 | json_array |

**Behavioural contracts** — update to reflect stateless runs, TemplateContext as data carrier, no shared Cersei sessions.

- [ ] **Commit**

```bash
git add docs/AGENTS.md
git commit -m "docs: rewrite AGENTS.md for AgentDef model and default agent directory"
```

---

### Task G3: Update `README.md`

**Files:**
- Modify: `README.md`

Update usage section: remove `falanx score`, update `falanx review` → `falanx run`, add new flags, update example output to generic stage output format.

- [ ] **Update README usage section**

Replace the Usage section:
```markdown
## Usage

```bash
# Review a git diff
falanx run --diff HEAD~1

# Review across a branch range
falanx run --diff main..feature

# Review a single file
falanx run --file src/main.rs

# Override loop settings
falanx run --diff HEAD~1 --target 4.5 --max-iter 3

# Use a custom workflow
falanx run --diff HEAD~1 --workflow security

# Load additional agents from a local directory
falanx run --diff HEAD~1 --agents ./.falanx/agents
```

### Sessions

Each run starts a new session scoped to your current branch.

```bash
# List sessions
falanx list-sessions

# Inspect a session JSONL directly
cat ~/.falanx/sessions/<label>/<session-id>.jsonl | jq .
```
```

Remove the `falanx score` and `falanx full` examples. Update the Example Output section to show JSONL stage events rather than formatted score output.

- [ ] **Commit**

```bash
git add README.md
git commit -m "docs: update README for falanx run command and generic workflow"
```

---

## Self-review checklist (run before marking plan complete)

- [ ] All spec sections covered by a task:
  - Core architecture + CLI changes → Tasks C2, E1, F1
  - Workflow DSL → Tasks B1–B5
  - AgentDef + AgentKind → Tasks A2–A4
  - RunResult + template context → Tasks A1, B2, C1
  - Session storage + audit trail → Task D1
  - Loop control + horizon reset → Task C1 (runner.rs)
  - Docs rewrite → Tasks G1–G3
- [ ] No TBDs or placeholder steps
- [ ] Type names consistent: `AgentRunResult` (types.rs), `orchestrator::RunResult` (orchestrator/mod.rs), `WorkflowConfig` (workflow/mod.rs), `StageConfig` (workflow/stage.rs), `TemplateContext` (workflow/context.rs), `WorkflowRunner` (workflow/runner.rs)
- [ ] `SkipIf` predicate naming: `ScoreMeetsTarget` / `score_meets_target` (corrected from spec)
- [ ] `minijinja` dep added before any code that imports it (Task A1)
- [ ] Embedded default YAML + agent dirs created before `include_str!` references compile (Task B3 creates the YAML stub)
