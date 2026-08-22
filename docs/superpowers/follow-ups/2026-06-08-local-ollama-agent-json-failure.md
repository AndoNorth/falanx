# Follow-up: local Ollama models fail the real agent path with "no JSON object in agent response"

**Context:** testing the composable-agent-loop refactor (`feat/refactor-to-composable-agent-loop`)
end-to-end against local Ollama models, since this machine has Ollama running with several
models pulled (`gemma4:e2b`, `hf.co/lmstudio-community/Qwen2.5-Coder-14B-GGUF:Q4_K_M`, etc).
This doc is for a machine **without** Ollama access — it can't reproduce the live failure, but
it can review/fix the code paths involved.

## What we were testing

`falanx run --diff HEAD~1 --agents <defaults dir>` with:

```
FALANX_MODEL='ollama/hf.co/lmstudio-community/Qwen2.5-Coder-14B-GGUF:Q4_K_M'
OLLAMA_HOST=http://localhost:11434
OPENCODE_API_KEY=dummy   # validate() requires a non-empty key even for keyless local providers
FALANX_MAX_DIFF_CHARS=6000
```

## Bug #1 ✅ (fixed + verified): `--agents` CLI flag was wired to nothing

`RunArgs.agents` (`rust/crates/falanx/src/main.rs:42`) was parsed by clap but never reached
`RunConfig` or `orchestrator::run`. `orchestrator::run` (`orchestrator/mod.rs:23-28`) always
called `resolve_agents_dir()`, which only checks `.falanx/agents` (cwd-relative) or
`<exe_dir>/defaults/agents` — neither exists for a `cargo build --release` binary, so the
embedded default agents (`src/defaults/agents/*`) were unreachable outside the source tree, and
`--agents <path>` silently did nothing.

**Fix applied** (already committed to the working tree on this branch):
- `RunConfig` gained `agents_dir: Option<PathBuf>` (`orchestrator/mod.rs`)
- `orchestrator::run` now does `config.agents_dir.clone().unwrap_or_else(resolve_agents_dir)`
  and passes that into `WorkflowConfig::load(Some(&agents_dir))` (so a custom workflow.yaml
  alongside a custom agents dir is picked up consistently with the existing `load()` semantics)
- `cmd_run` in `main.rs` now passes `agents_dir: args.agents` into `RunConfig`

This is a real, independent bug — worth keeping regardless of the JSON issue below. To
reproduce/verify without Ollama: `falanx run --dry-run --diff HEAD~1 --agents
crates/falanx-engine/src/defaults/agents` should now find and load the default agent set from
an arbitrary path (it previously errored with `agent directory 'score_readability' not found`).

## Side fixes applied alongside Bug #1 ✅

Three additional fixes were applied on the machine without Ollama access and verified on the Ollama machine:

**`extract_output` error now includes raw response preview** (`agent/run.rs`): errors now show
a 200-char prefix of the model's actual output. The original error discarded the text entirely.
Confirmed working — error messages now show the garbled shim output directly.

**`validate()` no longer rejects empty `ANTHROPIC_API_KEY` for Ollama models** (`config.rs`):
`ProviderConfig::is_keyless_local()` detects `ollama/...` model strings. No more dummy key
workaround needed.

**`OPENCODE_API_KEY` → `ANTHROPIC_API_KEY`** (`config.rs`): stale env var name from a previous
provider was renamed to match what cersei-provider actually reads. Default model updated from
`opencode/big-pickle` to `anthropic/claude-opus-4-7`.

**Prompt restructure attempt — did not resolve Bug #2** (`defaults/agents/*/prompt.md`): All 7
agent prompts were restructured to put DIFF first and the JSON instruction last (end-of-context
position where local models follow instructions more reliably). The root cause of the
cersei-agent `[system hint:]` injection was identified: `runner.rs:196` appends the hint
whenever the rendered prompt contains `"index"`, `"analyze"`, `"understand"`, etc., and every
git diff header contains `index xxxxxxx..yyyyyyy` so injection fires on every real run. Despite
the restructure, 3 test runs all returned different garbage with no JSON — confirmed the failure
is transport-layer, not prompt-layer. The prompt restructure is kept as a marginal improvement
for cloud models; it does not fix Ollama.

## Bug #2 ⚠️ (root cause confirmed, upstream fix needed)

### Symptom

Every run against a local Ollama model fails almost immediately on the very first agent:

```
Error: agent 'score_readability' in stage 'score' failed: no JSON object in agent response
```

This comes from `extract_output` in `rust/crates/falanx-engine/src/agent/run.rs:14-25` — it
searches the raw completion text for `{` ... `}` and fails when it can't find a brace pair (or
the braces don't bound valid JSON). The agent's prompt template
(`defaults/agents/score_readability/prompt.md`) **does** explicitly ask for
`Respond in JSON only: {"score": N, "reasoning": "..."}`, so the prompt itself is fine — the
model's raw response is the problem.

### What we found by going around falanx and hitting Ollama directly

1. **Ollama's native `/api/chat` endpoint** (`http://localhost:11434/api/chat`), given the
   exact same system prompt + rendered user prompt (with the real diff substituted for
   `{{ diff }}`), returns a clean, on-topic, valid JSON object:
   ```json
   {"score": 4, "reasoning": "The changes introduce minor readability improvements by using
   more descriptive names and organizing the code. ..."}
   ```
   — i.e. the model is perfectly capable of following these instructions.

2. **Ollama's OpenAI-compatible endpoint** (`http://localhost:11434/v1/chat/completions`),
   given the *identical* messages payload, returns garbled, off-topic, hallucinated text that
   has nothing to do with the supplied diff or even matches the prompt template's exact wording
   (e.g. it paraphrases/invents scoring rubrics like "Score 7-8 if it's hard not to enjoy
   reading" — wording that appears nowhere in our prompt). Repeating the same request produces
   *different* garbage each time. Never valid JSON, never on-topic.

### Why this matters for the real path

Look at `cersei-provider`'s router (`cersei-provider-0.1.9/src/router.rs`):

- The `ollama` registry entry (`registry.rs:381`) has `api_base: "http://localhost:11434/v1"`
  and `ApiFormat::OpenAiCompatible`.
- `build_provider` for `ApiFormat::OpenAiCompatible` (`router.rs:103-124`) always constructs
  an `OpenAi::builder().base_url(entry.api_base)...` client — i.e. **it always talks to Ollama
  through the OpenAI-compatible `/v1/chat/completions` shim**, never the native `/api/chat`
  endpoint.

So the "real" code path (`falanx` → `cersei_agent::Agent` → `cersei-provider`'s `OpenAi` client
→ Ollama's `/v1/...` shim) is *exactly* the broken path we reproduced manually with curl. The
`extract_output` failure is a downstream symptom: the model is being asked correctly, but the
transport/shim between cersei and Ollama is mangling the conversation (likely a chat-template /
prompt-formatting mismatch specific to Ollama's OpenAI-compatibility layer combined with this
community-imported GGUF's chat template — possibly a context/cache collision too, since results
were non-deterministic across identical requests).

This is **not a regression in falanx's prompt templates, extraction logic, or the new
composable-agent code** — `extract_output` is doing exactly what it should with what it's
given, and the prompt asks for JSON correctly. The break is one layer down, in how the
configured provider talks to a local Ollama model.

### What's worth investigating in the real path (no Ollama needed to start)

- **cersei-provider has no native Ollama path — confirmed.** Grepped
  `cersei-provider-0.1.9/src/`. The `ollama` registry entry sets `api_base:
  "http://localhost:11434/v1"` with `ApiFormat::OpenAiCompatible`. `build_provider` routes all
  OpenAI-compat providers through `OpenAi::builder().base_url(entry.api_base)...`. No native
  `/api/chat` or `/api/generate` path exists anywhere in 0.1.9.
- **Check whether a newer cersei-provider version handles Ollama differently.** 0.1.9 is
  pinned in `falanx-engine/Cargo.toml`. If upstream added a native Ollama path in a later
  version, bumping the dependency is the cleanest fix.
- **Alternative: implement a thin native Ollama provider in falanx itself**, bypassing
  cersei-provider for `ollama/...` models. `build_provider` in `agent/run.rs` is the right
  seam — it already dispatches by `cfg.provider.dry_run`; a second branch on
  `is_keyless_local()` could construct a native `/api/chat` client directly. This avoids
  waiting on cersei-provider upstream.
- If/when testing locally again: curl both `/api/chat` and `/v1/chat/completions` with the
  same messages payload before assuming a falanx-side bug — the divergence between the two is
  the smoking gun.

### Side notes from this session (context, not action items)

- `gemma4:e2b` (5.1B, Q4_K_M, 4096 ctx) was tried first and was simply too slow — it never
  completed even one agent turn within ~4 minutes against the ~10K-char diff for `HEAD~1`
  (`fe4d124`, a ~7700-line refactor commit), so it was killed before it could even reach the
  JSON-parsing stage.
- Switching to `hf.co/lmstudio-community/Qwen2.5-Coder-14B-GGUF:Q4_K_M` (100% GPU) and reducing
  `FALANX_MAX_DIFF_CHARS` to `6000` (roughly aligned with the model's 4096-token context window)
  got us through prompt construction fast enough to hit the real failure quickly — that's how
  bug #2 surfaced.
- `FalanxConfig::validate()` (`config.rs:83-90`) requires `OPENCODE_API_KEY` to be non-empty
  even when the configured provider is a keyless local one (`ollama/...`) — a `dummy` value is
  needed to get past validation. Not in scope to fix here, but worth knowing if retesting.
