
# Falanx Architecture — v1

> This document describes the v1 architecture. Decisions are intentional and bounded. Forward-looking items are explicitly marked — they are known directions, not scope creep.

---

## User Story

> As a developer, after finishing a change, I run `falanx` against my diff. It scores, reviews, and optionally rewrites my code — producing a full audit trail I can trust before I merge.

The tool runs locally or in Docker. It is versioned, shareable, and CI-ready. Developer-triggered first; CI integration is additive later.

---

## Deployment Modes

Falanx runs in two modes from the same binary.

### CLI Mode

Direct invocation against a local git repo. The engine runs, executes the agent pipeline, writes output, and exits.

```
developer
    │
    │  falanx review --diff HEAD~1
    ▼
┌─────────────────────────────┐
│        Falanx CLI           │
│  (binary / Docker + volume) │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│      Rust Core Engine       │
│  (orchestrator + agents)    │
└──────────────┬──────────────┘
               │
               ▼
        stdout + JSONL session file
        (score report + full audit trail)
```

### Serve Mode

Long-running process. Exposes two surfaces simultaneously:

- **MCP Server** — tool adapter for LLM hosts (Claude, etc.)
- **HTTP API** — observability endpoints for dashboards or scripting

```
                    falanx serve
                         │
         ┌───────────────┴────────────────┐
         │                                │
         ▼                                ▼
┌─────────────────┐             ┌──────────────────┐
│   MCP Server    │             │    HTTP API       │
│  (LLM adapter)  │             │  (observability)  │
└────────┬────────┘             └────────┬──────────┘
         │                               │
         │ tool calls                    │ REST
         ▼                               ▼
┌──────────────────────────────────────────────────┐
│                Rust Core Engine                   │
│           (orchestrator + agents)                 │
└──────────────────────────────────────────────────┘
         │                               │
         ▼                               ▼
  LLM host (Claude)             UI / dashboard / scripts
```

Auth: none in v1 (localhost / trusted Docker network). API key middleware slot reserved for later.

---

## Core Engine

Same engine, both modes. Mode only changes invocation and output surface.

```
┌──────────────────────────────────────────────────────────────┐
│                      Rust Core Engine                         │
│                                                               │
│   ┌───────────────────────────────────────────────────────┐   │
│   │            CodeReviewOrchestrationAgent               │   │
│   │  - accepts: file | git diff (git-native, v1)          │   │
│   │  - owns: sequencing, loop control, re-scoring         │   │
│   │  - exits: target score hit | max iterations | plateau │   │
│   │  - recovers: horizon reset on context degradation     │   │
│   │  - produces: combined report + JSONL audit trail      │   │
│   └───────────────────────┬───────────────────────────────┘   │
│                           │                                   │
│        ┌──────────────────┼──────────────────┐                │
│        ▼                  ▼                  ▼                │
│  ┌───────────┐    ┌──────────────┐    ┌─────────────┐         │
│  │ CodeQuality│    │ CodeReview   │    │ CodeWriting │         │
│  │  Agent    │→   │   Agent      │→   │   Agent     │         │
│  │  (score)  │    │  (critique)  │    │  (rewrite)  │         │
│  └───────────┘    └──────────────┘    └─────────────┘         │
│                                                               │
│   ┌───────────────────────────────────────────────────────┐   │
│   │              Model Abstraction Layer                  │   │
│   │   LLM Client trait — swappable provider               │   │
│   │                                                       │   │
│   │   Anthropic API  │  OpenAI API  │  Ollama  │  Proxy   │   │
│   └───────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

Agents do not call LLMs directly. All LLM access goes through the model abstraction layer. Sub-agents are fully isolated — the orchestrator receives only their final text output, never their internal context.

---

## Context Management

Cersei manages context internally. Each agent holds its own isolated message history in memory. Built-in mechanisms handle context pressure automatically:

| Mechanism | Behaviour |
|---|---|
| Auto-compact | At 90% context usage, older messages are summarised by the LLM; last 10 messages always preserved |
| Tool result budget | Oldest tool results truncated when cumulative size exceeds 50KB; last 6 always kept |
| Per-result cap | Individual tool results capped at 80 head + 80 tail lines or 20,000 chars |

These operate transparently. No manual context management is required in the Falanx layer.

### Horizon Reset

When the orchestrator detects reasoning degradation mid-loop (e.g. score plateau with no meaningful diff change, or circular critiques), it performs a horizon reset:

1. Current Cersei agent is discarded
2. A fresh agent is spun up with clean context
3. Seed context is the original diff + current scores only — no accumulated critique history
4. Loop continues from the next iteration

This prevents wasted token spend on a poisoned context window. The JSONL session file retains the full history for audit purposes; only the live context resets. Manual trigger (`--reset-horizon`) is a future addition on top of this automatic behaviour.

---

## Loop Mode

The orchestrator iterates the pipeline toward a quality target.

| Exit condition | Flag | Notes |
|---|---|---|
| Target score hit | `--target 4.5` | Happy path — exits when composite score ≥ threshold |
| Max iterations | `--max-iter 3` | Hard stop — mandatory, prevents runaway LLM spend |
| Plateau detected | `--plateau-threshold 0.1` | Safety net — exits when score delta between passes falls below threshold |

All three are active when loop mode is enabled. `--max-iter` is the only mandatory guard. Horizon reset fires before exit — it is a recovery attempt, not an exit condition.

---

## Audit Trail & Sessions

Cersei writes every prompt, response, and tool call to append-only **JSONL files** when session mode is enabled. Falanx uses these as both the audit trail and the context persistence mechanism.

```
~/.falanx/sessions/<repo-hash>/<branch-name>/<session-id>.jsonl
```

The JSONL file captures: agent steps, LLM prompts and responses, tool calls, scores per pass, horizon resets. It exists so you can trust the output, not just accept it.

### Session Identity

Session ID is derived from `repo root + branch name`. Each run starts a new session by default. A branch accumulates multiple sessions — each is an independent review attempt that can be continued or abandoned.

### Session Commands

```bash
# Start a new session (default)
falanx review --diff HEAD~1

# List sessions for the current branch (timestamps, scores, iteration count)
falanx list-sessions

# Continue a specific session (resume context, continue loop)
falanx review --diff HEAD~1 --continue <session-id>
```

Continuation is intentional — you pick a session by ID after inspecting `list-sessions`. This covers the "run failed midway" and "switch LLM provider mid-review" cases.

---

## Observability API (Serve Mode)

JSONL session files are the backing store. HTTP API and MCP server read from them directly.

| Endpoint | Data | Priority |
|---|---|---|
| `GET /runs/:id/audit` | Full per-run trace from JSONL: agent steps, prompts, tool calls, responses | 1 |
| `GET /runs` | Run history: scores, timestamps, targets per branch | 2 |
| `GET /runs/:id/status` | Live run status: active agent, current iteration | 3 |

---

## Target Input

| Phase | Input | Notes |
|---|---|---|
| v1 | `--diff HEAD~1`, `--diff main..feature`, `--file src/foo.rs` | Git-native, works locally and in Docker via volume mount |
| Later | `--pr 123` | Platform API (GitHub/GitLab) — additive, no rearchitecting required |

Docker v1: `docker run -v $(pwd):/repo falanx review --diff HEAD~1`

---

## Configuration

| Source | Precedence | Purpose |
|---|---|---|
| CLI args | Highest | Override anything at invocation time |
| `.env` | Base | API keys, model selection, loop defaults |

Secrets stay out of source control. CLI args always win.

---

## Distribution

| Method | Use case |
|---|---|
| `cargo install falanx` | Developer local install |
| `cargo build --release` | Build from source |
| Docker image | Team sharing, CI, serve mode deployment |

---

## Deferred (Post-v1)

| Item | Intent |
|---|---|
| Post-rewrite validation | Run `cargo check` / linter after `CodeWritingAgent`. v1: user validates manually or via their own coding agent. |
| Manual horizon reset | `--reset-horizon` flag to trigger context reset on demand mid-session. |
| Platform API input | `--pr 123` fetches diff from GitHub/GitLab. Requires auth, platform-specific surface. |
| CI integration | Falanx as a CI step. Developer-triggered first, CI additive. |
| HTTP API auth | API key middleware. v1 is localhost / trusted network only. |
| Remote git in Docker | Pull and push to remote repo from serve mode container. Volume mount sufficient for v1. |
| Cross-MR institutional memory | RAG over historical JSONL sessions. Different problem from session continuity. |

---

## Invariants

- Same binary, same engine, two entry points
- CLI exits cleanly; serve mode runs until stopped
- MCP and HTTP are independent surfaces over the same engine
- No rewrite without prior critique — orchestrator enforces sequence
- Every run produces a JSONL audit trail regardless of mode
- Loop mode always has a hard iteration ceiling
- Horizon reset preserves audit history, resets only live context
- Git-native input in v1; platform API is additive
