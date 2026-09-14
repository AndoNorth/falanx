# Ollama

Falanx's primary local-LLM target is Ollama. This doc is shallow on purpose — capturing the
runtime constraints that shape design decisions elsewhere (agent output contracts, retry
bounds), not a full Ollama reference. Expand as we learn more.

---

## Target Environment

Reference dev setup: a quantized model running within **16GB VRAM**. That's a real ceiling, not
a lot of headroom — expect 7B-14B class models at Q4/Q5 quantization, not the flagship-sized
reasoning models. Quantized models at this size are measurably less reliable at strict output
formatting than a hosted frontier model, which is the whole reason the
[output-contract design](superpowers/specs/) (schema file + validation + bounded retry) exists —
it's not solving a hypothetical problem.

## Endpoint Split: Native vs OpenAI-Compatible

Ollama exposes two HTTP surfaces:

| Endpoint | Falanx uses it? | Notes |
|---|---|---|
| `/api/chat` (native) | No | Supports `think: true/false/"low"/"medium"/"high"/"max"`. Reasoning content comes back in a separate `message.thinking` field, cleanly split from `message.content`. |
| `/v1/chat/completions` (OpenAI-compatible) | **Yes** — via `cersei-provider`'s `openai.rs` | `think` is a no-op here. Reasoning models emit their chain-of-thought inline in `content` (often wrapped in `<think>...</think>` or similar), mixed with the final answer. |

Falanx goes through cersei's OpenAI-compatible provider, so **the native `think` toggle is not
reachable from falanx today**. Reaching the native endpoint would mean bypassing cersei's
`Provider` trait for Ollama specifically, which is out of scope for v1.

**Update (cersei 0.2.6):** `CompletionRequest.options` is no longer a dead field — the
OpenAI-compat request builder now forwards a `reasoning_effort` option
(`cersei-provider/src/openai.rs::reasoning_effort_for`) into the outgoing request body. This
does *not* close the gap above for Ollama, though: the forwarding is hardcoded to models whose
name starts with `gpt-5`, `o1`, or `o3` — OpenAI's own reasoning models — so an Ollama model name
never matches and the option is silently dropped regardless of what's in `options`. Still no
schema/`response_format` forwarding of any kind. Revisit if cersei ever widens
`reasoning_effort_for`'s model gate to include Ollama's reasoning models.

## What This Means for Agent Output

A reasoning model's `<think>` block can contain example/reference JSON with its own braces,
which breaks `extract_output`'s current find-first-`{`/find-last-`}` extraction — it's not just
a formatting nicety, it can silently grab the wrong JSON. The output-contract design strips
known reasoning-block delimiters before brace-search to guard against this.

Until that's built, the practical mitigation is model choice: pick a non-reasoning model
(plain `llama3.1`, `qwen2.5-coder`, etc.) over a reasoning-tuned one (`deepseek-r1`, `qwq`,
`gpt-oss`) if reliable structured output matters more than reasoning quality for a given agent.

---

## References

- [Thinking - Ollama docs](https://docs.ollama.com/capabilities/thinking)
- [Thinking · Ollama Blog](https://ollama.com/blog/thinking)
- [Document reasoning_effort support in OpenAI-compatible /v1/chat/completions API · Issue #14820](https://github.com/ollama/ollama/issues/14820)
- [Ollama v1 OpenAI Compat Drops Think Toggle - DEV Community](https://dev.to/apexgridtech/ollama-v1-openai-compat-drops-think-toggle-5a24)
