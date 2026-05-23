# Cersei

Cersei is a Rust SDK for building coding agents. Falanx uses it as the LLM provider and agent execution layer.

Source: https://github.com/pacifio/cersei

---

## What Cersei Owns

- LLM provider abstraction (Anthropic, OpenAI-compatible, custom)
- Agent builder and agentic loop execution
- Context compaction at 90% usage (summarises old messages, preserves last 10)
- JSONL session persistence and replay
- Tool system with 30+ built-in tools + `#[derive(Tool)]` macro
- MCP client (JSON-RPC 2.0, stdio transport)
- Memory management (flat files, CLAUDE.md hierarchy, optional graph)
- Hook/middleware system (cost guards, audit logging, tool blocking)

## What Falanx Owns

- Orchestration sequencing and loop control
- Convergence logic, plateau detection, horizon resets
- Agent pipeline definition (CodeQualityAgent → CodeReviewAgent → CodeWritingAgent)
- Git diff extraction and input handling
- CLI and serve mode surfaces
- Run-level audit trail (separate from Cersei's session JSONL)

---

## Workspace Crates (v0.1.9)

| Crate | Role |
|---|---|
| `cersei` | Facade — `use cersei::prelude::*` |
| `cersei-types` | Provider-agnostic messages and stream events |
| `cersei-provider` | `Provider` trait + Anthropic / OpenAI-compatible impls |
| `cersei-agent` | Agent builder, agentic loop, compaction |
| `cersei-tools` | Built-in tools, permissions, bash classifier, skills |
| `cersei-tools-derive` | `#[derive(Tool)]` proc macro |
| `cersei-memory` | `Memory` trait, JSONL storage, sessions, optional graph |
| `cersei-hooks` | Middleware — cost guards, audit logging, blocking |
| `cersei-mcp` | MCP client |
| `cersei-lsp` | LSP integration |
| `cersei-embeddings` | Embedding support |
| `cersei-compression` | Context compression utilities |
| `cersei-skills` | Skills system |
| `cersei-vms` | VM/sandbox support |
| `abstract-cli` | Production CLI built on Cersei |

---

## Providers

- **Anthropic** — native, OAuth/PKCE
- **OpenAI-compatible** — covers Ollama, Azure, vLLM, LiteLLM
- **Custom** — implement the `Provider` trait

### `Provider` Trait (cersei-provider v0.1.9)

Uses `#[async_trait]` — NOT native async (Rust 2024). The `use async_trait::async_trait` attribute
is required on both the trait definition and all `impl Provider` blocks.

```rust
use async_trait::async_trait;
use cersei_provider::{Provider, CompletionRequest, CompletionResponse, CompletionStream};
use cersei_provider::ProviderCapabilities;
use cersei_types::Result;

#[async_trait]
pub trait Provider: Send + Sync {
    /// Human-readable provider name (e.g., "anthropic", "openai").
    fn name(&self) -> &str;

    /// Context window size for the given model.
    fn context_window(&self, model: &str) -> u64;

    /// Capabilities supported by the given model.
    fn capabilities(&self, model: &str) -> ProviderCapabilities;

    /// Required: send a streaming completion request.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionStream>;

    /// Provided with defaults (calls complete + collect):
    async fn complete_blocking(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Provided with defaults (estimates from char count):
    async fn count_tokens(&self, messages: &[Message], model: &str) -> Result<u64>;
}
```

There is also a blanket `impl Provider for Box<dyn Provider>`, so a `Box<dyn Provider>` can be
passed wherever `impl Provider` is expected.

### `CompletionRequest`

```rust
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub system: Option<String>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub stop_sequences: Vec<String>,
    pub options: ProviderOptions,
}

// Constructor (all other fields default):
CompletionRequest::new("claude-sonnet-4-6")
```

### `CompletionResponse`

```rust
pub struct CompletionResponse {
    pub message: Message,    // ← response text lives here
    pub usage: Usage,
    pub stop_reason: StopReason,
}
```

To extract text from the response:
- `response.message.get_text()` → `Option<&str>` (first text block)
- `response.message.get_all_text()` → `String` (all text blocks concatenated)

### `CompletionStream`

```rust
pub struct CompletionStream {
    rx: mpsc::Receiver<StreamEvent>,  // private field
}

// Constructors:
CompletionStream::new(rx: mpsc::Receiver<StreamEvent>) -> Self
stream.into_receiver() -> mpsc::Receiver<StreamEvent>
stream.collect() -> impl Future<Output = Result<CompletionResponse>>
```

**MockProvider pattern** — to build a `CompletionStream` from a static string:

```rust
use tokio::sync::mpsc;
use cersei_provider::CompletionStream;
use cersei_types::{StreamEvent, StopReason};

fn static_stream(text: &str) -> CompletionStream {
    let (tx, rx) = mpsc::channel(16);
    let text = text.to_string();
    tokio::spawn(async move {
        let _ = tx.send(StreamEvent::MessageStart {
            id: "mock-id".to_string(),
            model: "mock".to_string(),
        }).await;
        let _ = tx.send(StreamEvent::ContentBlockStart {
            index: 0,
            block_type: "text".to_string(),
            id: None,
            name: None,
        }).await;
        let _ = tx.send(StreamEvent::TextDelta { index: 0, text }).await;
        let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
        let _ = tx.send(StreamEvent::MessageDelta {
            stop_reason: Some(StopReason::EndTurn),
            usage: None,
        }).await;
        let _ = tx.send(StreamEvent::MessageStop).await;
    });
    CompletionStream::new(rx)
}
```

### `from_model_string()` — exact signature

```rust
// In cersei_provider::router (re-exported as cersei_provider::from_model_string)
pub fn from_model_string(model: &str) -> Result<(Box<dyn Provider>, String)>
```

**Note:** The actual signature takes only ONE argument (the model string) and returns
`(Box<dyn Provider>, String)` — the `String` is the resolved model name with the provider
prefix stripped. This is different from the placeholder in earlier docs which showed `(model, api_key, base_url)`.

The api key is sourced from environment variables automatically. The model string format is
`"provider/model"` (e.g. `"anthropic/claude-sonnet-4-6"`) or just `"claude-sonnet-4-6"` for
auto-detection.

---

## Agent API

### `AgentBuilder` — all builder methods

```rust
Agent::builder()
    // Required:
    .provider(p: impl Provider + 'static)        // accepts concrete type
    .provider_boxed(p: Box<dyn Provider>)        // alternative: pre-boxed

    // Tools:
    .tool(t: impl Tool + 'static)                // add single tool
    .tools(ts: Vec<Box<dyn Tool>>)               // add many tools

    // Prompts:
    .system_prompt(s: impl Into<String>)
    .append_system_prompt(s: impl Into<String>)  // appended to system prompt

    // Model / generation:
    .model(m: impl Into<String>)
    .max_turns(n: u32)                           // default: 10
    .max_tokens(n: u32)                          // default: 16384
    .temperature(t: f32)
    .thinking_budget(tokens: u32)

    // Environment:
    .working_dir(p: impl Into<PathBuf>)
    .permission_policy(p: impl PermissionPolicy + 'static)

    // Memory / sessions:
    .memory(m: impl Memory + 'static)
    .session_id(id: impl Into<String>)

    // Hooks:
    .hook(h: impl Hook + 'static)

    // MCP:
    .mcp_server(config: McpServerConfig)

    // Events / streaming:
    .on_event(f: impl Fn(&AgentEvent) + Send + Sync + 'static)
    .enable_broadcast(capacity: usize)           // enables multi-consumer channel
    .reporter(r: impl Reporter + 'static)
    .event_filter(f: impl Fn(&AgentEvent) -> bool + Send + Sync + 'static)

    // Cancellation:
    .cancel_token(token: tokio_util::sync::CancellationToken)

    // Context management:
    .auto_compact(enabled: bool)                 // default: true
    .compact_threshold(threshold: f64)           // default: 0.9 (90%)
    .tool_result_budget(chars: usize)            // default: 50_000
    .turns_elapsed_cadence(n: u32)               // default: 10 (0 = disabled)
    .compression_level(level: cersei_compression::CompressionLevel)

    // Advanced:
    .with_messages(msgs: Vec<Message>)           // pre-populate conversation history
    .benchmark_mode(enabled: bool)

    // Terminal:
    .build() -> Result<Agent>
    .run_with(prompt: &str) -> impl Future<Output = Result<AgentOutput>>  // build + run
```

### `Agent` — key methods

```rust
agent.run(prompt: &str) -> Result<AgentOutput>
agent.run_stream(self: &Arc<Self>, prompt: &str) -> AgentStream   // requires Arc<Agent>
agent.reply(message: &str) -> Result<AgentOutput>                 // multi-turn follow-up
agent.messages() -> Vec<Message>
agent.usage() -> Usage
agent.cancel()
agent.subscribe() -> Option<broadcast::Receiver<AgentEvent>>      // requires enable_broadcast
```

### `AgentOutput` — fields and text access

```rust
pub struct AgentOutput {
    pub message: Message,              // the final response message
    pub usage: Usage,                  // token usage for the full run
    pub stop_reason: StopReason,
    pub turns: u32,                    // number of agentic turns taken
    pub tool_calls: Vec<ToolCallRecord>,
}

impl AgentOutput {
    pub fn text(&self) -> &str { ... }  // convenience: message.get_text().unwrap_or("")
}
```

**To get response text:** `output.text()` (returns `&str`, empty string if no text block).

### `ToolCallRecord`

```rust
pub struct ToolCallRecord {
    pub name: String,
    pub id: String,
    pub input: serde_json::Value,
    pub result: String,
    pub is_error: bool,
    pub duration: Duration,
}
```

---

## Hooks API (cersei-hooks v0.1.9)

### `Hook` Trait — exact signature

Uses `#[async_trait]`:

```rust
#[async_trait]
pub trait Hook: Send + Sync {
    /// Which events this hook handles.
    fn events(&self) -> &[HookEvent];

    /// Called when a matching event fires. Returns an action to control flow.
    async fn on_event(&self, ctx: &HookContext) -> HookAction;

    /// Optional name for logging/debugging (default: "unnamed-hook").
    fn name(&self) -> &str {
        "unnamed-hook"
    }
}
```

**Key difference from earlier docs:** `on_event` takes only `ctx: &HookContext` (not a separate
`event` parameter). The event type is available as `ctx.event`.

### `HookEvent` — all variants

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PreModelTurn,
    PostModelTurn,
    Stop,
    Error,
    /// Fires every N turns (cadence configured on AgentBuilder, default 10).
    /// `HookContext::turn` carries the current turn counter.
    TurnsElapsed,
}
```

### `HookContext` — all fields

```rust
pub struct HookContext {
    pub event: HookEvent,
    pub tool_name: Option<String>,
    pub tool_input: Option<Value>,
    pub tool_result: Option<String>,
    pub tool_is_error: Option<bool>,
    pub turn: u32,
    pub cumulative_cost_usd: f64,
    pub message_count: usize,
}
```

### `HookAction` — all variants

```rust
pub enum HookAction {
    /// Continue normally.
    Continue,
    /// Block the operation (PreToolUse only). Includes a reason string.
    Block(String),
    /// Replace the tool input with modified data (PreToolUse only).
    ModifyInput(Value),
    /// Inject a message into the conversation.
    InjectMessage(Message),
}
```

### `run_hooks` — exact signature

```rust
pub async fn run_hooks(hooks: &[std::sync::Arc<dyn Hook>], ctx: &HookContext) -> HookAction
```

Hooks are stored as `Arc<dyn Hook>` (not `Box<dyn Hook>`). The builder's `.hook()` method
accepts `impl Hook + 'static` and wraps it in `Arc` internally.

---

## Memory API (cersei-memory v0.1.9)

### `Memory` Trait

```rust
#[async_trait]
pub trait Memory: Send + Sync {
    async fn store(&self, session_id: &str, messages: &[Message]) -> Result<()>;
    async fn load(&self, session_id: &str) -> Result<Vec<Message>>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>>;
    async fn sessions(&self) -> Result<Vec<SessionInfo>>;
    async fn delete(&self, session_id: &str) -> Result<()>;
}
```

### `InMemory` — constructor

```rust
/// In-memory store for tests and short-lived agents.
pub struct InMemory { /* private */ }

impl InMemory {
    pub fn new() -> Self { ... }
}

impl Default for InMemory {
    fn default() -> Self { Self::new() }
}
```

Usage:
```rust
use cersei_memory::InMemory;

let mem = InMemory::new();
// or
let mem = InMemory::default();
```

### `JsonlMemory` — file-backed backend

```rust
pub struct JsonlMemory { /* private */ }

impl JsonlMemory {
    pub fn new(dir: impl Into<PathBuf>) -> Self { ... }
}
```

---

## Context Controls

| Setting | Behaviour |
|---|---|
| `auto_compact(true)` | Summarise old messages at 90% context usage, preserve last 10 |
| `tool_result_budget(50_000)` | Truncate oldest tool results above 50KB threshold |
| `thinking_budget(8192)` | Extended thinking token allocation |

---

## Notes for Falanx Integration

- Cersei agents should be used **per-pipeline-stage** — one agent instance per CodeQualityAgent / CodeReviewAgent / CodeWritingAgent invocation
- Falanx orchestrates **between** agents; Cersei handles execution **within** an agent
- Horizon reset = discard current Cersei agent instance, construct fresh one with seed context only
- Cersei's JSONL session files are an implementation detail; Falanx's audit trail is a separate concern at the run level
- `cersei-mcp` is the client; Falanx serve mode will expose its own MCP **server** — these are distinct
- `from_model_string()` is in `cersei_provider::router` module, re-exported as `cersei_provider::from_model_string`. Takes one arg (model string), returns `(Box<dyn Provider>, String)` — NOT `(model, api_key, base_url)`
- **`opencode` is NOT a registered provider in cersei-provider v0.1.9** — the registry contains: anthropic, openai, google, mistral, groq, deepseek, xai, together, fireworks, perplexity, cerebras, ollama, openrouter, cohere, sambanova. Using `"opencode/big-pickle"` as a model string will fail with "Unknown provider: 'opencode'". To use a live provider, set `FALANX_MODEL` to a supported `provider/model` string (e.g., `anthropic/claude-haiku-4-5`) and the corresponding API key env var
- The live provider path in `falanx score` (non-dry-run) calls `cersei_provider::from_model_string(&cfg.provider.model)` — the same pattern used by the orchestrator pipeline. Auth keys are sourced from env vars automatically (e.g., `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`). `OPENCODE_API_KEY` is validated by `FalanxConfig::validate()` as a guard, but cersei-provider itself does not use it — for live use, set both `OPENCODE_API_KEY` (to pass validation) and the provider-specific key
- `MockProvider` must implement all `Provider` trait methods with `#[async_trait]`; only `complete` needs a real body (the default `complete_blocking` calls `complete + collect`)
- Hooks are registered as `Arc<dyn Hook>` — the builder's `.hook()` wraps in `Arc` automatically
- `HookContext.event` carries the event type inside the context struct — `on_event` does NOT take a separate `event` parameter
- `provider_boxed(p: Box<dyn Provider>)` **consumes** the box — there is no `Arc<dyn Provider>` support on AgentBuilder. For pipelines that build multiple agents, reconstruct a fresh provider per agent call via a factory function
- `MockProvider::response_for` dispatches on `request.system` (the agent system prompt). When no system prompt is set on the builder, the agent runner puts the user prompt in `request.messages`, so routing logic must fall back to checking the last user message text if system is empty

---

_Last updated: 2026-05-23. All API shapes verified directly against cersei 0.1.9 source in ~/.cargo/registry._
