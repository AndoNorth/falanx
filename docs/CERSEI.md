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

```rust
// async methods via async-trait
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn context_window(&self, model: &str) -> u64;
    fn capabilities(&self, model: &str) -> ProviderCapabilities;

    // Required: streaming completion
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionStream>;

    // Provided with defaults:
    async fn complete_blocking(&self, request: CompletionRequest) -> Result<CompletionResponse>;
    async fn count_tokens(&self, ...) -> Result<u64>;  // approximate if unsupported
}
```

Constructor: `cersei_provider::from_model_string(model_str, api_key, base_url)` — parses
`"provider/model"` strings (e.g. `"opencode/big-pickle"`) and returns `Box<dyn Provider>`.

---

## Agent API

```rust
// cersei-agent
Agent::builder()
    .provider(Anthropic::from_env()?)
    .tools(cersei::tools::coding())
    .model("claude-sonnet-4-6")
    .max_tokens(16384)
    .permission_policy(AllowAll)
    .build()?
    .run_with("Fix the failing tests")
    .await?
```

Execution modes:
- `.run_with(prompt)` — single-turn execution, returns `AgentOutput` (final response)
- `.run_stream(prompt)` — `AgentStream`: async event iterator with bidirectional control
- `.enable_broadcast(channel_size)` — multi-consumer event distribution

Key types:
- `Agent` — built via `AgentBuilder`
- `AgentOutput` — result of `.run_with()`; contains final response text
- `AgentStream` — async event iterator for streaming mode
- `AgentEvent` — event types during execution
- `Reporter` — trait for consuming agent events

---

## Hooks API (cersei-hooks v0.1.9)

The hook/middleware system intercepts agent lifecycle events.

```rust
// Key types (verify exact signatures against cersei-hooks source before implementing)
pub trait Hook: Send + Sync {
    async fn on_event(&self, ctx: &HookContext, event: &HookEvent) -> HookAction;
}

pub struct HookContext { /* carries run context */ }

pub enum HookEvent {
    // variants include pre/post tool use, model turns, etc.
    // ⚠ verify exact variants against cersei-hooks/src/lib.rs before implementing FalanxAuditHook
}

pub enum HookAction {
    Continue,   // pass through unchanged
    Block,      // prevent the action
    // ... other variants — verify before implementing
}

// Execute all matching hooks for an event, returns first non-Continue action
pub fn run_hooks(hooks: &[Box<dyn Hook>], ctx: &HookContext, event: &HookEvent) -> HookAction;
```

---

## Memory API (cersei-memory v0.1.9)

```rust
mm.write_user_message("session-id", Message::user("Hello"))?;
let messages = mm.load_session_messages("session-id")?;
```

- `InMemory` — in-process backend, suitable for tests (no filesystem)
- JSONL sessions: append-only with tombstone soft-delete, supports replay/resumption
- Optional graph memory via `features = ["graph"]` (Grafeo-backed indexed memory)

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
- `from_model_string()` is in `cersei_provider::router` module — parses `"provider/model"` strings
- `MockProvider` must implement all four required `Provider` trait methods; only `complete` and `complete_blocking` need real bodies for Falanx use

---

_Last updated: 2026-05-23. Hook trait method signatures and HookEvent variants are approximations — verify against cersei-hooks source when pulling crate._
