# Phase 1C+1D Validation Test Plan

**Date:** 2026-05-23  
**Branch:** feat/phase-1c-1d-spec  
**Status:** Partially validated — dry-run complete, live LLM path not exercised

---

## What Was Actually Tested

### Automated unit tests (40/40 pass)

These tests run against pure Rust logic — no network calls, no LLM, no filesystem outside of tempdir.

| Test Area | What It Covers |
|---|---|
| `config::tests` | LoopConfig defaults, env var overrides (FALANX_MAX_ITER etc.) |
| `session::tests` | Session file creation, JSONL append + flush, Session::list() walkdir scan |
| `types::tests` | ReviewScore composite, delta calculations |
| `agents::quality::tests` | `parse_score_response` JSON extraction — clean JSON, prose-wrapped JSON, error case |
| `agents::review::tests` | `parse_issues_response` — empty array, single issue, prose-wrapped, error case |
| `agents::writing::tests` | `parse_patches_response` — empty patches, single patch, error case; early-return on empty issues |
| `orchestrator::horizon::tests` | HorizonState new/reset/exhausted logic |
| `provider::tests` | MockProvider trait impl, name(), response routing (SCORE/CRITIQUE/default) |
| `audit_hook::tests` | FalanxAuditHook trait impl, name() |
| `orchestrator::pipeline::tests` | Dry-run pipeline: runs 1 iteration with max_iter=1, returns result |

### Dry-run smoke tests (manual, run during implementation)

```bash
# Review command end-to-end (MockProvider, no LLM)
echo "fn add(a: i32, b: i32) -> i32 { a + b }" > /tmp/test_review.rs
cargo run --bin falanx -- review --file /tmp/test_review.rs --dry-run

# Observed: exits 0, logs:
#   review complete final_score=3 iterations=3 patches=0 session_id=<uuid>

# List-sessions
cargo run --bin falanx -- list-sessions

# Observed: table with SESSION ID / STARTED / SCORE / ITERATIONS columns
```

---

## What Was NOT Tested

### Live LLM calls — BLOCKER

**No LLM calls were made at any point.** Every agent invocation went through `MockProvider` which returns hardcoded JSON:

- `SCORE` prompt → `{"readability":3,"maintainability":3,"performance":3,"security":3,"architecture":3}`
- `CRITIQUE` prompt → `[{"location":"mock:0","problem":"no issues found","fix":"none"}]`
- Default (REWRITE) → `[]`

This means:
- The prompts were **never sent to a model**
- The JSON parsing logic was tested against known-good strings, not real LLM output
- The agentic loop ran but produced no meaningful review output

### OpenCode integration — BLOCKED

`opencode` is not a registered provider prefix in `cersei-provider v0.1.9`. Calling `cersei_provider::from_model_string("opencode/big-pickle")` returns `Err("Unknown provider: 'opencode'")`.

The cersei-provider registry covers: anthropic, openai, google, mistral, groq, deepseek, xai, together, fireworks, perplexity, cerebras, ollama, openrouter, cohere, sambanova.

**To use OpenCode with Falanx, one of these must be true:**
1. OpenCode exposes an OpenAI-compatible API endpoint → use `FALANX_MODEL=openai/model-name` + `FALANX_BASE_URL=<opencode-endpoint>`
2. A custom `Provider` impl is written for OpenCode's native API
3. Cersei adds an opencode registry entry in a future version

---

## Gaps — What Needs to Be Validated Before Production

### P0 — Must fix before live use

- [ ] **Live LLM call through cersei agent** — run `falanx score` with a real provider (Anthropic or OpenAI-compatible) and verify `parse_score_response` handles actual model output (not just hardcoded JSON)
- [ ] **Prompt quality** — send the `SCORE`, `CRITIQUE`, and `REWRITE` prompts to a real model and verify the JSON shape matches `ReviewScore`, `ReviewIssue`, `RewritePatch`

### P1 — Important

- [ ] **OpenCode routing** — determine if OpenCode exposes an OpenAI-compatible API at a local endpoint; if yes, validate `FALANX_MODEL=openai/<model>` + `FALANX_BASE_URL=<opencode-url>`
- [ ] **Real diff input** — run `falanx review --diff HEAD~1 --dry-run` on an actual git range (not just a static file) and verify diff extraction + session trail
- [ ] **Plateau detection** — in dry-run, MockProvider returns the same score every iteration (3.0), so plateau is detected after 2 iterations. Verify this terminates correctly and HorizonReset fires as expected
- [ ] **Session JSONL correctness** — open a session file after a dry-run review and verify all event types appear in correct order: RunStarted → AgentInvoked → ScoreComputed → AgentInvoked (review) → IssuesFound → AgentInvoked (quality) → ScoreComputed → ... → RunCompleted

### P2 — Nice to have

- [ ] **LLM output robustness** — test `parse_score_response` against malformed LLM output (missing fields, wrong types, extra prose before/after JSON)
- [ ] **Large diff handling** — verify the pipeline doesn't hit context limits with a large diff (1000+ line diff)

---

## How to Run Live Validation (Once Provider Is Available)

### Option A: Anthropic

```bash
export ANTHROPIC_API_KEY=<your-key>
export OPENCODE_API_KEY=anything     # passes Falanx's validate() check
export FALANX_MODEL=anthropic/claude-haiku-4-5

echo "fn add(a: i32, b: i32) -> i32 { a + b }" > /tmp/live_test.rs
cd rust && cargo run --bin falanx -- score --file /tmp/live_test.rs
```

Expected: prints score with real values (not all 3s), exits 0.

### Option B: OpenCode via OpenAI-compatible endpoint

If OpenCode exposes an OpenAI-compatible API at e.g. `http://localhost:3000`:

```bash
export OPENAI_API_KEY=<opencode-key>
export OPENCODE_API_KEY=<same-key>
export FALANX_MODEL=openai/big-pickle
export FALANX_BASE_URL=http://localhost:3000/v1

cd rust && cargo run --bin falanx -- score --file /tmp/live_test.rs
```

The base URL is passed to the OpenAI provider via cersei's `ProviderOptions`. Check `docs/CERSEI.md` for the exact env var name if it differs from `FALANX_BASE_URL`.

### Validating the agents work end-to-end

After `falanx score` succeeds live, run:

```bash
cargo run --bin falanx -- review --file /tmp/live_test.rs --max-iter 1
```

Then inspect the session file:
```bash
cargo run --bin falanx -- list-sessions
# get the session path from the output, then:
cat ~/.falanx/sessions/<label>/<uuid>.jsonl | jq .
```

Verify:
- `type: "run_started"` is first
- `type: "agent_invoked"` appears for each agent
- `type: "score_computed"` has non-mock values
- `type: "run_completed"` is last with a real `final_score`
