---
status: approved
date: 2026-09-14
updated: 2026-09-14
issues: []
scope: [agent, workflow, session]
---

# Agent Output Contract

## Problem

Every agent's expected JSON response shape is currently prose hand-typed onto the end of its
`prompt.md` — e.g. `Respond with only the following JSON — no preamble...`. This is duplicated
near-verbatim across all 7 default agents, is not machine-checked in any way, and is extracted
from the raw response by `agent/run.rs::extract_output` doing a first-`{`/last-`}` (or
first-`[`/last-`]`) string search. Nothing validates that the extracted value actually matches
what the rest of the pipeline expects before it lands in `TemplateContext` — a missing or
wrong-typed field (e.g. `score`) is silently treated as absent by `workflow/stage.rs`'s
`composite_from_context`, not surfaced as an error.

This is a capability gap, not a bug fix: there is no way today for an agent definition to
*declare* its output shape, and no way to enforce it. It matters more than it would with a
frontier hosted model because falanx's primary target is a local, likely quantized Ollama model
(see `docs/OLLAMA.md`) — smaller models follow ad-hoc prose formatting instructions far less
reliably, and reasoning-tuned models add inline chain-of-thought text that can actively break the
current brace-search extraction.

## Goals

- An agent directory can optionally declare `output.schema.json` describing its expected JSON
  response shape.
- The JSON-response instruction sent to the model is rendered from one shared function, not
  hand-copied prose per `prompt.md` — editing the wording happens in one place.
- An agent's extracted output is validated against its declared schema before it reaches
  `TemplateContext`; a schema-less agent (no `output.schema.json`) behaves exactly as it does
  today — this is additive, not a breaking change to existing custom `--agents` dirs.
- On validation failure, the agent is re-invoked with the validation errors appended to its
  prompt, bounded to a fixed number of attempts, rather than either silently accepting bad data
  or failing on the first bad response.
- Every failed validation attempt is visible in the JSONL audit trail via a new
  `SessionEvent::AgentOutputInvalid`, and a run that exhausts retries produces a
  `SessionEvent::RunFailed` record (which nothing appends today, despite the variant existing).
- The contract (wrap → dispatch → validate → retry) wraps the `AgentKind` dispatch rather than
  living inside the Cersei-specific code path, so a future non-Cersei `AgentKind` still has to
  satisfy the same contract.
- `extract_output` survives a reasoning model's inline chain-of-thought by stripping a known
  reasoning-block delimiter (e.g. `<think>...</think>`) before brace-search.

## Non-Goals

- **Provider-level structured-output enforcement** (Ollama's native `think` toggle, JSON-mode /
  constrained decoding). Confirmed unreachable from falanx today: `cersei-provider`'s
  OpenAI-compatible request builder (the path Ollama goes through) never forwards
  `CompletionRequest.options`, and Ollama's `think` parameter only works on its native `/api/chat`
  endpoint, which falanx doesn't call. See `docs/OLLAMA.md`. Enforcement here is
  prose-plus-validation-plus-retry only; provider-level enforcement would require forking or
  patching the pinned `cersei-provider = "0.1.9"` dependency, which is out of scope.
- **Per-agent `max_tokens` tuning.** A quantized model doing inline reasoning could exhaust the
  current fixed 16384-token budget before reaching its JSON answer. Flagged as an open risk
  below, not designed here — orthogonal to the output-contract mechanism itself.
- **Rust-struct/`schemars`-generated schemas.** Schemas are hand-written JSON Schema files so a
  fully custom `--agents` directory with no Rust involvement can still declare a contract.
- **Custom `skip_if` gate predicates, human-in-the-loop pipeline checkpoints, agent grouping,
  reference-doc injection for progressive disclosure (coding standards / scoring rubrics).**
  Real gaps identified during design review, but distinct problems from the output contract —
  see Out of Scope.
- **Adding an actual second `AgentKind` variant** (e.g. a coding-agent-with-skills backend). This
  spec only needs the contract mechanism to *not preclude* one later.

## Architecture / Design

### Components

| Component | Change |
|---|---|
| `output.schema.json` | New, optional file per agent dir, sibling to `system.md` / `prompt.md` / `config.yaml`. Plain JSON Schema. |
| `agent/loader.rs` | `load_from_dir` reads it if present, parses to `serde_json::Value`, stores on new `AgentDef.output_schema: Option<serde_json::Value>`. |
| `agent/contract.rs` (new) | `wrap_prompt_with_schema` — appends one shared instruction block + schema JSON to a rendered prompt. `strip_reasoning_preamble` — removes a known reasoning-block delimiter before extraction. `validate_output` — checks a `serde_json::Value` against a schema via the `jsonschema` crate (new dependency), returning `Result<(), Vec<String>>`. |
| `agent/run.rs` | `run_agent` becomes the retry-owning outer function: if `output_schema` is `Some`, wrap the prompt, dispatch via `match def.kind`, strip-then-extract, validate, and on failure re-dispatch with errors appended — bounded to `MAX_OUTPUT_RETRIES = 2` (3 attempts total, fixed constant, not user-configurable in v1). `AgentRunResult` gains `retry_diagnostics: Vec<RetryDiagnostic>` (one entry per failed attempt; empty on first-try success). `agent/run.rs` stays free of any `Session` dependency — it returns diagnostics, it doesn't log them. |
| `session.rs` | New `SessionEvent::AgentOutputInvalid { stage_id, agent_name, iteration, attempt, errors }`. `orchestrator::run`'s top-level error path is wired to append the existing-but-unused `SessionEvent::RunFailed { reason }` before returning — today nothing ever appends it. |
| `workflow/runner.rs::run_stage` | After `run_agent` returns, appends one `AgentOutputInvalid` per `retry_diagnostics` entry, before the existing `AgentCompleted` append. No other change to the stage loop. |
| Default agents | All 7 `prompt.md` files lose their trailing "Respond with only JSON..." paragraph; each gains an `output.schema.json`. |

### Data Flow

Two paths, selected by whether `AgentDef.output_schema` is `Some`:

- **No schema (any existing custom agent, unchanged):** `prompt.md` renders → dispatch →
  `extract_output` → done. Byte-identical to current behavior.
- **Schema present:** `prompt.md` renders → `wrap_prompt_with_schema` → dispatch → response text
  → `strip_reasoning_preamble` → `extract_output` → `validate_output`. Valid → `AgentRunResult`
  returned. Invalid → append validation errors to the prompt, redispatch, up to
  `MAX_OUTPUT_RETRIES`; each failed attempt is recorded in `retry_diagnostics`.

`workflow/runner.rs::run_stage` is unaware of which path ran — it only reads
`result.retry_diagnostics` and `result.extracted`, same as today's `result.extracted` handling.
`synthesise_runs`, `TemplateContext` accumulation, and the orchestrator's `composite_from_ctx` /
`skip_if` evaluation are all unchanged.

See the diagrams in `docs/assets/2026-09-14-output-contract-diagrams.html`
(Diagram 1: whole-pipeline topology as it exists today; Diagram 2: this contract mechanism
zoomed into one `run_agent` call, showing the kind-agnostic boundary).

### Kind-Agnostic Boundary

The wrap → dispatch → strip/extract → validate → retry sequence lives in `run_agent`, wrapping
`match def.kind { AgentKind::Cersei => run_cersei(...) }`. A future `AgentKind` variant (e.g. one
that drives a coding-agent-with-skills backend instead of a one-shot completion) only needs to
return a final response string from its own dispatch function — the contract logic around it
does not change. This is why the contract does not live inside `run_cersei` itself.

### Error Handling

- **Retries exhausted:** `run_agent` returns `Err`. `run_stage`'s existing `.map_err(...)`
  wrapping is unchanged. The error propagates to `orchestrator::run`, which now appends
  `SessionEvent::RunFailed { reason }` before returning — closing the existing gap where a failed
  run left no terminal record in the JSONL.
- **Malformed `output.schema.json`** (invalid JSON, or JSON that isn't a valid schema): fails at
  `load_from_dir` time, same posture as a missing `system.md` today. Caught before any LLM call.
- **`jsonschema` compile error** (e.g. a bad `$ref` in a hand-written schema): same load-time
  failure bucket, with an error naming the offending agent.
- **Reasoning-block stripping finds nothing to strip:** a no-op, not an error — most models don't
  emit a `<think>` block, and `strip_reasoning_preamble` must be safe to call unconditionally.

## Validation

Follows the existing test style in this codebase — `#[test]` / `#[tokio::test]` plus
`tempfile`, no new test framework:

- `agent/contract.rs`: `validate_output` — a valid value passes, an invalid one returns a
  populated error list; `wrap_prompt_with_schema` — output contains both the original prompt
  text and the schema; `strip_reasoning_preamble` — a `<think>...</think>`-wrapped response has
  the block removed and the remaining text unchanged, and a response with no such block passes
  through unmodified.
- `agent/loader.rs`: `output_schema` is `None` when the file is absent, `Some` when present and
  valid, and a load error when the file is malformed.
- `workflow/runner.rs`: extend the existing dry-run integration tests (`MockProvider`-backed) —
  one case where the mock returns invalid JSON once then valid (asserts `retry_diagnostics.len()
  == 1` and the stage still completes), one where it never validates (asserts the stage fails and
  `RunFailed` lands in the session JSONL).

## Open Questions

- Provider-level structured-output enforcement (Ollama `think` / JSON-mode) stays unreachable
  without forking `cersei-provider`. Revisit if a future cersei release forwards
  `CompletionRequest.options` or adds a dedicated structured-output field.
- `max_tokens` is a fixed 16384 today; a quantized model's inline reasoning could exhaust that
  before reaching JSON. Not designed here — flagged as a risk to watch once this ships against
  real Ollama models.
- `MAX_OUTPUT_RETRIES = 2` is a fixed constant chosen for v1. May need to become configurable if
  real usage against small quantized models shows 2 retries is consistently too few (or wasteful
  if too many).

## Out of Scope

- Custom `skip_if` gate predicates (today a closed Rust enum: `ScoreMeetsTarget`,
  `NoReviewIssues`).
- Human-in-the-loop pipeline checkpoints (no pause/approve stage type exists).
- Agent grouping (stages hold a flat `agents: [...]` list today).
- Reference-doc injection for progressive disclosure (coding standards / scoring rubrics as
  first-class agent-dir files, beyond `system.md` / `prompt.md`).
- `falanx serve` (MCP + HTTP) — unrelated, pre-existing gap, still a stub.

## References

- `crates/falanx-engine/src/agent/run.rs` — `extract_output`, `run_cersei`, `build_provider`
- `crates/falanx-engine/src/agent/loader.rs` — `load_from_dir`
- `crates/falanx-engine/src/agent/mod.rs` — `AgentDef`, `AgentKind`
- `crates/falanx-engine/src/workflow/runner.rs` — `run_stage`, `synthesise_runs`
- `crates/falanx-engine/src/workflow/stage.rs` — `SkipIf`, `composite_from_context`
- `crates/falanx-engine/src/orchestrator/mod.rs` — `run`, `run_with_workflow_and_agents`
- `crates/falanx-engine/src/session.rs` — `SessionEvent`
- `docs/ARCHITECTURE.md` — pipeline topology, module layout, invariants
- `docs/OLLAMA.md` — native vs OpenAI-compatible endpoint split, `think` parameter limitation
- `docs/assets/2026-09-14-output-contract-diagrams.html` — pipeline + contract
  diagrams referenced above
- cersei source (pinned `0.1.9`, via cargo registry): `cersei-provider/src/openai.rs` (request
  body construction — confirms `options` is never read), `cersei-provider/src/lib.rs`
  (`CompletionRequest`, `ProviderOptions`), `cersei-agent/src/runner.rs` (existing retry loop is
  transient-provider-error only, not output-validation)
