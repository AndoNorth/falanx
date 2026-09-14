
# Falanx Architecture — Composable Agent Pipeline

> This document describes the composable agent pipeline architecture. Agents are defined as directories with prompts and config. Workflows are YAML-driven. The `WorkflowRunner` orchestrates execution. This replaces the hardcoded score → review → rewrite pipeline.

---

## User Story

> As a developer, after finishing a change, I run `falanx run` against my diff. The system executes a workflow of agents, each producing structured output. Workflow logic is defined in `workflow.yaml`, not Rust code. The system produces a full audit trail I can trust before I merge.

The tool runs locally or in Docker. It is versioned, shareable, and CI-ready. Workflows are composable and overrideable. Developer-triggered first; CI integration is additive later.

---

## Deployment Modes

Falanx runs in two modes from the same binary.

### CLI Mode

Direct invocation against a local git repo. The engine runs, loads the workflow, executes stages, writes output, and exits.

```
developer
    │
    │  falanx run --diff HEAD~1
    ▼
┌─────────────────────────────┐
│        Falanx CLI           │
│  (binary / Docker + volume) │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│      Rust Core Engine       │
│  (WorkflowRunner + agents)  │
└──────────────┬──────────────┘
               │
               ▼
        stdout + JSONL session file
        (stage outputs + full audit trail)
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
│           (WorkflowRunner + agents)               │
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
│   │                   WorkflowRunner                       │   │
│   │  - loads: workflow.yaml + agent definitions           │   │
│   │  - drives: sequential stage loop                      │   │
│   │  - manages: TemplateContext (accumulates outputs)     │   │
│   │  - owns: loop control, horizon reset, skip logic      │   │
│   │  - exits: target score | max iterations | plateau     │   │
│   │  - produces: combined report + JSONL audit trail      │   │
│   └───────────────────────┬───────────────────────────────┘   │
│                           │                                   │
│        ┌──────────────────┼──────────────────┐                │
│        ▼                  ▼                  ▼                │
│  ┌───────────┐    ┌──────────────┐    ┌─────────────┐         │
│  │  Score    │    │   Review     │    │  Rewrite    │         │
│  │  Agents   │→   │    Agent     │→   │   Agent     │         │
│  │  (5x)     │    │   (1x)       │    │   (1x)      │         │
│  └───────────┘    └──────────────┘    └─────────────┘         │
│                                                               │
│   ┌───────────────────────────────────────────────────────┐   │
│   │              Model Abstraction Layer                  │   │
│   │   LLM Client trait — swappable provider               │   │
│   │                                                       │   │
│   │   Anthropic API  │  OpenAI API  │  Ollama  │  Mock    │   │
│   └───────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

Agents do not call LLMs directly. All LLM access goes through the model abstraction layer. Each agent is invoked independently with the same inputs and extracted outputs via `AgentDef`. The orchestrator manages context accumulation and stage sequencing via `WorkflowRunner`.

---

## Context and Data Flow

### TemplateContext

The `TemplateContext` accumulates stage outputs across the workflow. It begins with the diff only:

```
{ diff: "..." }
```

After the score stage completes:

```
{
  diff: "...",
  score_result: [
    {"agent": "score_readability", "score": 4, "reasoning": "..."},
    {"agent": "score_maintainability", "score": 3, "reasoning": "..."},
    ...
  ]
}
```

After the review stage:

```
{
  diff: "...",
  score_result: [...],
  review_result: [
    {"location": "file:123", "problem": "...", "fix": "..."},
    ...
  ]
}
```

Each subsequent stage can access all prior outputs via minijinja template variables. On each outer loop iteration, context resets to `{ diff }` only — prior iteration outputs do not bleed.

### Cersei Agent Context

Cersei manages context internally for each agent. Each agent holds its own isolated message history in memory. Built-in mechanisms handle context pressure automatically:

| Mechanism | Behaviour |
|---|---|
| Auto-compact | At 90% context usage, older messages are summarised by the LLM; last 10 messages always preserved |
| Tool result budget | Oldest tool results truncated when cumulative size exceeds 50KB; last 6 always kept |
| Per-result cap | Individual tool results capped at 80 head + 80 tail lines or 20,000 chars |

These operate transparently. No manual context management is required in the Falanx layer.

### Horizon Reset

When the orchestrator detects reasoning degradation mid-loop (e.g. score plateau with no meaningful diff change, or circular critiques), it performs a horizon reset:

1. All live agent contexts are discarded
2. Fresh agents are spun up with clean context
3. Seed context is the original diff only — `TemplateContext` resets
4. Loop continues from the next iteration

This prevents wasted token spend on a poisoned context window. The JSONL session file retains the full history for audit purposes; only the live context resets.

---

## Loop Control and Exit Conditions

The `WorkflowRunner` iterates the stage pipeline toward a quality target. Loop configuration is loaded from `workflow.yaml`:

```yaml
loop:
  max_iterations: 5          # mandatory hard ceiling
  target_score: 4.5          # exit when score >= threshold
  plateau_threshold: 0.1     # horizon reset trigger
```

| Exit condition | Config source | Notes |
|---|---|---|
| Target score hit | `loop.target_score` | Happy path — exits when composite score ≥ threshold |
| Max iterations | `loop.max_iterations` | Hard stop — mandatory, prevents runaway LLM spend |
| Plateau detected | `loop.plateau_threshold` | Safety net — exits when score delta falls below threshold |

All three are active. `max_iterations` is the only mandatory guard. Horizon reset fires before exit — it is a recovery attempt, not an exit condition. CLI flags (`--max-iter`, `--target`) override YAML values.

---

## Audit Trail & Sessions

Falanx writes a **JSONL audit trail** for every run. The session file captures workflow execution, agent invocations, outputs, loop state, and horizon resets. It exists so you can trust the output, not just accept it.

```
~/.falanx/sessions/<label>/<uuid>.jsonl
```

### Session Events

| Event | Fields | Notes |
|---|---|---|
| `RunStarted` | session_id, target, workflow | Marks start of run |
| `StageStarted` | stage_id, iteration | Stage begins in current iteration |
| `AgentStarted` | stage_id, agent_name, iteration | Individual agent invocation begins |
| `AgentCompleted` | stage_id, agent_name, turns_used, output, iteration | Agent completed; output is extracted (json_object/json_array/text) |
| `StageCompleted` | stage_id, output_as, result, iteration | Stage complete; result available in TemplateContext |
| `StageSkipped` | stage_id, condition, iteration | Stage skipped due to predicate (e.g. `score_meets_target`) |
| `HorizonReset` | iteration | Context reset triggered by plateau or degradation |
| `RunCompleted` | iterations, summary | Run complete; summary is final TemplateContext state |
| `RunFailed` | reason | Run failed; reason describes the error |

### Session Identity

Session ID is derived from repo root + branch name. Each run starts a new session by default. A branch accumulates multiple sessions — each is an independent review attempt.

### Session Commands

```bash
# Start a new session (default)
falanx run --diff HEAD~1

# List sessions for the current branch
falanx list-sessions

# Continue a specific session (resume context, continue loop)
falanx run --diff HEAD~1 --continue <session-id>
```

Continuation is intentional — you pick a session by ID after inspecting `list-sessions`. This covers the "run failed midway" and "switch LLM provider mid-review" cases.

---

## Observability API (Serve Mode)

JSONL session files are the backing store. HTTP API and MCP server read from them directly.

| Endpoint | Data | Priority |
|---|---|---|
| `GET /runs/:id/audit` | Full per-run trace from JSONL: stage execution, agent outputs, loop state, horizon resets | 1 |
| `GET /runs` | Run history: summary, timestamps, targets per branch | 2 |
| `GET /runs/:id/status` | Live run status: active stage, current iteration | 3 |

---

## Workflow Configuration

Workflows are defined in YAML. The default workflow is compiled into the binary but can be overridden:

```yaml
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
```

Agents are directories containing:
- `system.md` — system prompt for Cersei
- `prompt.md` — minijinja template, receives TemplateContext variables
- `config.yaml` — `kind` (cersei, default) and `max_turns`
- `output.schema.json` — optional JSON Schema for the agent's expected response shape; when
  present, the response is validated and retried on mismatch before reaching TemplateContext

User-defined workflows and agents override defaults. See below for agent definition details.

## Target Input

| Phase | Input | Notes |
|---|---|---|
| v1 | `--diff HEAD~1`, `--diff main..feature`, `--file src/foo.rs` | Git-native, works locally and in Docker via volume mount |
| Later | `--pr 123` | Platform API (GitHub/GitLab) — additive, no rearchitecting required |

Docker v1: `docker run -v $(pwd):/repo falanx run --diff HEAD~1`

---

## Agent and Workflow Definition

### Agent Definition (`AgentDef`)

Each agent is a Rust struct loaded from a directory:

```rust
pub struct AgentDef {
    pub name: String,
    pub kind: AgentKind,
    pub system_prompt: String,
    pub prompt_template: String,
    pub max_turns: u32,
}
```

Directory layout:

```
agents/
  score_readability/
    system.md
    prompt.md
    config.yaml
    output.schema.json
  review/
    system.md
    prompt.md
    config.yaml
    output.schema.json
```

`config.yaml` example:

```yaml
kind: cersei
max_turns: 1
```

The prompt template receives `TemplateContext` variables via minijinja:

```
{{ diff }}
{{ score_result }}
{{ score_result | selectattr("agent", "equalto", "score_security") | first }}
{{ review_result | tojson }}
```

### Agent Output Extraction

| `output_format` | Behaviour | Yields |
|---|---|---|
| `json_object` | Find first `{` … `}`, parse as JSON | serde_json::Value::Object |
| `json_array` | Find first `[` … `]`, parse as JSON | serde_json::Value::Array |
| `text` | Raw agent output | serde_json::Value::String |

### Multi-agent Stages

When a stage has multiple agents (e.g. five score agents), each runs independently with the same inputs. Outputs are collected and wrapped:

```json
[
  {"agent": "score_readability", "score": 4, "reasoning": "..."},
  {"agent": "score_maintainability", "score": 3, "reasoning": "..."},
  ...
]
```

Single-agent stages extract directly without wrapping.

---

---

## Runtime Configuration

| Source | Precedence | Purpose |
|---|---|---|
| CLI args | Highest | Override workflow YAML and env vars at invocation time |
| Workflow YAML | Middle | Loop config, stages, predicates |
| `.env` | Base | API keys, model selection |

Secrets stay out of source control. CLI args always win.

Environment variables:

| Variable | Scope |
|---|---|
| `FALANX_MODEL` | Model ID (e.g. claude-opus-4-1) |
| `ANTHROPIC_API_KEY` | API authentication |
| `FALANX_BASE_URL` | Provider URL override (Ollama, etc.) |
| `FALANX_DRY_RUN` | Mock provider, no LLM calls |
| `FALANX_SESSION_DIR` | Session storage path |
| `FALANX_MAX_DIFF_CHARS` | Diff size cap |
| `FALANX_MAX_ITER` | Default loop iteration cap (`loop.max_iterations` override) |
| `FALANX_TARGET_SCORE` | Default target score (`loop.target_score` override) |
| `FALANX_PLATEAU_THRESHOLD` | Default plateau threshold (`loop.plateau_threshold` override) |

---

## Module Layout

```
falanx-engine/src/
  workflow/
    mod.rs        — Workflow, Stage, StageResult
    runner.rs     — WorkflowRunner: orchestrates stage loop
    context.rs    — TemplateContext: accumulates outputs
  agent/
    mod.rs        — AgentDef, AgentKind
    loader.rs     — Load AgentDef from filesystem
    run.rs        — Run single agent, extract output
    contract.rs   — Output-contract enforcement: prompt wrapping, reasoning-strip, schema validation
  orchestrator/
    mod.rs        — Entry point, RunConfig, RunResult
    horizon.rs    — Horizon reset detection
  session.rs      — SessionEvent (generic), Session, JSONL I/O
  provider.rs     — Cersei provider abstraction
  config.rs       — FalanxConfig, LoopConfig
  types.rs        — RunResult, extracted types
```

Defaults compiled into binary:

```
falanx-engine/src/defaults/
  agents/
    score_readability/
    score_maintainability/
    score_architecture/
    score_performance/
    score_security/
    review/
    rewrite/
  workflow.yaml
```

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
| Post-rewrite validation | Run `cargo check` / linter after rewrite stage. v1: user validates manually or via their own coding agent. |
| Manual horizon reset | `--reset-horizon` flag to trigger context reset on demand mid-session. |
| Platform API input | `--pr 123` fetches diff from GitHub/GitLab. Requires auth, platform-specific surface. |
| Per-agent models | Model override per AgentDef. v1: all agents share FalanxConfig model. |
| Parallel stages | Concurrent agent execution within a stage. v1: sequential only. |
| CI integration | Falanx as a CI step. Developer-triggered first, CI additive. |
| HTTP API auth | API key middleware. v1 is localhost / trusted network only. |
| Remote git in Docker | Pull and push to remote repo from serve mode container. Volume mount sufficient for v1. |
| Institutional memory | RAG over historical JSONL sessions. Different problem from session continuity. |

---

## CLI Usage

```bash
# Run workflow against a diff
falanx run --diff HEAD~1

# Against a git range
falanx run --diff main..feature

# Against a single file
falanx run --file src/main.rs

# Override loop limits
falanx run --diff HEAD~1 --max-iter 10 --target 4.5

# Use custom workflow
falanx run --diff HEAD~1 --workflow custom

# Override agent directory
falanx run --diff HEAD~1 --agents /path/to/agents

# Mock run (no LLM calls)
falanx run --diff HEAD~1 --dry-run

# List sessions
falanx list-sessions

# Continue a session
falanx run --diff HEAD~1 --continue <session-id>

# Serve mode (MCP + HTTP)
falanx serve
```

---

## Invariants

- Same binary, same engine, two entry points
- CLI exits cleanly; serve mode runs until stopped
- MCP and HTTP are independent surfaces over the same engine
- No rewrite without prior critique — enforced by `skip_if: no_review_issues`
- Every run produces a JSONL audit trail regardless of mode
- Loop always has a hard iteration ceiling
- Horizon reset preserves audit history, resets only live context
- Agents are invoked independently; no shared Cersei sessions between stages
- Git-native input in v1; platform API is additive
- Workflows and agents are composable and overrideable via YAML
