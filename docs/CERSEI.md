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

---

## Agent API (from README examples)

```rust
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
- `.run_with(prompt)` — single-turn execution, returns final response
- `.run_stream(prompt)` — bidirectional stream with real-time control
- `.enable_broadcast(channel_size)` — multi-consumer event distribution

---

## Session / Memory API (from README examples)

```rust
mm.write_user_message("session-id", Message::user("Hello"))?;
let messages = mm.load_session_messages("session-id")?;
```

JSONL sessions are append-only with tombstone soft-delete. Supports replay and resumption.

---

## Context Controls

| Setting | Behaviour |
|---|---|
| `auto_compact(true)` | Summarise old messages at 90% context usage, preserve last 10 |
| `tool_result_budget(50_000)` | Truncate oldest tool results above 50KB threshold |
| `thinking_budget(8192)` | Extended thinking token allocation |

---

## Key Dependencies (shared workspace)

- `tokio 1.44` (full) — async runtime
- `async-trait 0.1` — async trait support
- `serde` / `serde_json` — serialization
- `schemars 0.8` — JSON schema generation for tools
- `reqwest 0.12` — HTTP with rustls-tls
- `uuid 1`, `chrono 0.4` — IDs and timestamps
- `anyhow` — error handling

---

## Notes for Falanx Integration

- Cersei agents should be used **per-pipeline-stage** — one agent instance per CodeQualityAgent / CodeReviewAgent / CodeWritingAgent invocation
- Falanx orchestrates **between** agents; Cersei handles execution **within** an agent
- Horizon reset = discard current Cersei agent instance, construct fresh one with seed context only
- Cersei's JSONL session files are an implementation detail; Falanx's audit trail is a separate concern at the run level
- `cersei-mcp` is the client; Falanx serve mode will expose its own MCP **server** — these are distinct

---

_To be expanded when the crate is pulled into the workspace._
