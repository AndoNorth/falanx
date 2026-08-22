# Sandcastle — Architecture Reference & Falanx Mapping

This document covers two things:

1. **Sandcastle architecture** — what it is, how its composable execution model works, available agents, prompt templates, and sandbox providers.
2. **Falanx mapping** — how the same composable patterns could be achieved in Falanx using Cersei, with a focus on code quality, review, and rewrite workflows.

Reference repo: [`mattpocock/sandcastle`](https://github.com/mattpocock/sandcastle)

---

## Part 1 — Sandcastle Architecture

### What It Is

Sandcastle is a TypeScript execution harness that orchestrates AI coding agents (Claude Code, Codex, Pi, etc.) inside isolated sandbox environments. It manages the full lifecycle of a run: sandbox setup, prompt resolution, agent invocation, session tracking, branch management, and iteration control.

The core primitive is `run()`. You compose runs in a `main.mts` script to build multi-agent workflows.

---

### Execution Model

A **run** is a single invocation of one agent inside one sandbox, against one prompt, for up to `maxIterations` iterations. Each iteration is one agent invocation — the agent picks a task, works on it, emits a completion signal, and exits.

```
run({
  sandbox,       ← where the agent runs
  agent,         ← which agent runs
  promptFile,    ← what the agent is told
  branchStrategy ← how changes land in git
  maxIterations, ← upper bound on agent invocations
  output,        ← structured output extraction (optional)
})
→ RunResult { commits, output, sessionId, resume(), fork() }
```

Multiple `run()` calls are composed in plain TypeScript — sequential, parallel (`Promise.allSettled`), or looped. There is no framework magic. The orchestration logic lives in your `main.mts`.

---

### Sandbox Providers

A sandbox is the isolation boundary around the agent. Three providers ship with Sandcastle:

| Provider | Description | Use case |
|---|---|---|
| **No-sandbox** | Agent runs directly on the host machine | Local dev, read-only tasks |
| **Bind-mount** | Host filesystem mounted into a container | Fast iteration, no file sync cost |
| **Isolated** | Container with its own filesystem; files synced in/out | Full isolation, CI, parallel agents on same repo |

Providers are injected at `run()` time:

```ts
import { docker } from "@ai-hero/sandcastle/sandboxes/docker";
import { noSandbox } from "@ai-hero/sandcastle/sandboxes/no-sandbox";

sandbox: docker()      // isolated Docker container
sandbox: noSandbox()   // agent runs on host
```

Custom providers implement the `SandboxProvider` interface — Sandcastle ships with Docker, Podman, Vercel, and Daytona out of the box.

---

### Agent Providers

Sandcastle does not call LLM APIs directly. It invokes **coding agent CLIs** as subprocesses and streams their stdout. Each built-in provider knows how to:
- Build the CLI command (flags, prompt delivery method)
- Parse the agent's output stream (JSON lines, tool call events, session IDs)

Built-in agent providers:

| Provider | CLI invoked | Notes |
|---|---|---|
| **Claude Code** | `claude` | Default. JSON stream output. Session storage via `.jsonl` files. |
| **Codex** | `codex` | OpenAI Codex CLI. Session resume/fork supported. |
| **Pi** | `pi` | Session-capable. |
| **Cursor** | `cursor` | Print mode — prompt as CLI arg (120KB limit). |
| **OpenCode** | `opencode` | SQLite session store. |
| **Copilot** | `gh copilot` | GitHub Copilot CLI. |

All providers are injected into `run()`:

```ts
import { claudeCode, codex } from "@ai-hero/sandcastle";

agent: claudeCode("claude-opus-4-8")   // model string passed to Claude Code
agent: codex()
```

To use a different agent, swap the provider. The prompt templates and orchestration logic are unchanged.

---

### Prompt Templates

Prompts are plain markdown files. Two mechanisms make them dynamic:

**Shell expressions** — `` !`command` `` is evaluated inside the sandbox before each iteration. The stdout replaces the expression. This ensures the agent always sees fresh data:

```markdown
## Open issues

!`gh issue list --state open --json number,title,body --limit 100`

## Recent commits

!`git log --oneline -10`
```

**Placeholder substitution** — `{{KEY}}` is replaced with values from `promptArgs` passed to `run()`:

```markdown
Working on issue {{TASK_ID}}: {{ISSUE_TITLE}}

Target branch: {{BRANCH}}
```

These are resolved in order: shell expressions evaluated in sandbox, then `{{KEY}}` substituted from `promptArgs`. Inline prompts (string passed directly as `prompt:`) skip both — they reach the agent verbatim.

**Built-in args** — Sandcastle injects some `{{KEY}}` values automatically (e.g. `{{LIST_TASKS_COMMAND}}`, `{{CLOSE_TASK_COMMAND}}`), derived from the issue tracker configured during `init`.

---

### Structured Output

An agent can emit typed JSON inside a caller-specified XML tag. Sandcastle extracts and validates it using any [Standard Schema](https://standardschema.dev) validator (Zod, Valibot, ArkType):

```ts
import { z } from "zod";
import { Output } from "@ai-hero/sandcastle";

const planSchema = z.object({
  issues: z.array(z.object({ id: z.string(), branch: z.string() })),
});

const result = await run({
  // ...
  output: Output.object({ tag: "plan", schema: planSchema }),
});

result.output.issues // typed — z.infer<typeof planSchema>["issues"]
```

The prompt **must** contain the literal opening tag (e.g. `<plan>`). Sandcastle validates this at `run()` time and fails early if the tag is absent.

`Output.string({ tag })` is also available when you want raw text extraction without schema validation.

---

### Branch Strategies

Branch strategy controls how the agent's commits land in git:

| Strategy | Behaviour |
|---|---|
| **Head** | Agent works directly in the host working directory. No worktree. |
| **Merge-to-head** | Sandcastle creates a temp branch, agent commits to it, merged back to HEAD on completion. |
| **Branch** | Agent commits to an explicitly named branch. Caller owns merging. |

```ts
branchStrategy: { type: "head" }
branchStrategy: { type: "merge-to-head" }
branchStrategy: { type: "branch", branch: "feature/my-branch" }
```

For isolated sandbox providers, a **worktree** is created in `.sandcastle/worktrees/` as the sync point — commits are pulled from the sandbox into the worktree, then merged or left on the named branch.

---

### Session Management

Sandcastle tracks agent sessions across runs. A `RunResult` exposes:

```ts
result.sessionId          // ID of the session from this run
result.resume(prompt)     // continue the same session — same session ID, prior context intact
result.fork(prompt)       // branch into a new session — new ID, original session unchanged
```

Session storage is **agent-owned** — Claude Code writes `.jsonl` files under `~/.claude/projects/`; Codex uses `~/.codex/sessions/`. Sandcastle transfers session files into the sandbox at the start of a run and back out afterward.

---

### Init and Config Directory

`npx sandcastle init` scaffolds a `.sandcastle/` directory in your repo. You pick a template:

| Template | What it gives you |
|---|---|
| `blank` | `main.mts` + `prompt.md` — write everything yourself |
| `simple-loop` | Single agent, picks issues one by one, commits and closes each |
| `parallel-planner` | Opus plans → N Sonnet agents in parallel → Sonnet merges |
| `parallel-planner-with-review` | Same + a reviewer agent per branch |
| `sequential-reviewer` | Implement → review in sequence |

The result is:

```
.sandcastle/
├── main.mts              ← your orchestration script
├── prompt.md             ← (or multiple phase-specific prompt files)
└── CODING_STANDARDS.md  ← optional, injected into prompts via shell expression
```

Run with: `npx tsx .sandcastle/main.mts`

---

### Composability in Practice — `parallel-planner`

The most instructive template shows how multi-agent composition works in plain TypeScript:

```ts
for (let i = 0; i < MAX_ITERATIONS; i++) {
  // Phase 1 — Opus plans which issues are unblocked
  const plan = await run({
    agent: claudeCode("claude-opus-4-8"),
    promptFile: ".sandcastle/plan-prompt.md",
    output: Output.object({ tag: "plan", schema: planSchema }),
    maxIterations: 1,
  });

  if (plan.output.issues.length === 0) break;

  // Phase 2 — N Sonnet agents implement in parallel, each on its own branch
  await Promise.allSettled(
    plan.output.issues.map(issue =>
      run({
        agent: claudeCode("claude-sonnet-4-6"),
        promptFile: ".sandcastle/implement-prompt.md",
        branchStrategy: { type: "branch", branch: issue.branch },
        promptArgs: { TASK_ID: issue.id, ISSUE_TITLE: issue.title },
        maxIterations: 100,
      })
    )
  );

  // Phase 3 — Sonnet merges all branches
  await run({
    agent: claudeCode("claude-sonnet-4-6"),
    promptFile: ".sandcastle/merge-prompt.md",
    promptArgs: { BRANCHES: branches.join("\n") },
    maxIterations: 1,
  });
}
```

Key points:
- Different models per phase — Opus where reasoning depth matters, Sonnet for throughput
- Structured output carries typed data between phases (plan → implement)
- `promptArgs` pass runtime values into prompt templates
- `Promise.allSettled` gives parallelism; one failing agent doesn't cancel the others
- The outer `for` loop is plain code — no framework, no DSL

---

## Part 2 — Falanx Mapping (superseded)

> **This section predates the composable-agent-loop refactor and no longer matches the shipped design.**
> It proposed hardcoded `ScoreAgent`/`ReviewAgent`/`RewriteAgent` Rust types with XML-tag output
> extraction (`<score>...</score>`). What actually shipped is declarative: agents are directories
> (`AgentDef`) composed by `workflow.yaml`, and output is extracted by `output_format`
> (`json_object` / `json_array` / `text` — first-brace/bracket parsing, no XML tags). See
> `ARCHITECTURE.md` and `FALANX_AGENTS.md` for the current model. Kept below for historical context on
> where the phase/loop/horizon-reset concepts originated.

Falanx's current architecture implements its own orchestration loop in Rust. Cersei is the underlying agent SDK. The Sandcastle patterns above map directly onto Cersei primitives — with one key difference: **Cersei calls LLM APIs directly, Sandcastle invokes coding agent CLIs**. The orchestration concepts — phases, structured output, prompt composition, session management, iteration control — are equivalent.

---

### Concept Mapping

| Sandcastle concept | Cersei / Falanx equivalent |
|---|---|
| `run()` | `Agent::builder()...run(prompt)` — one Cersei agent execution |
| `AgentProvider` | Cersei `Provider` trait — `AnthropicProvider`, `OpenAICompatibleProvider` |
| `SandboxProvider` | Not needed for code review — Falanx is git-native on host |
| Prompt template file (`.md`) | System prompt string / file loaded at agent construction |
| Shell expression `` !`cmd` `` | Rust `std::process::Command` output injected into prompt string |
| `{{KEY}}` substitution | String formatting / template substitution before `Agent::run()` |
| `Output.object({ tag, schema })` | Parse XML tag from `AgentOutput::text()`, deserialise with `serde_json` |
| `branchStrategy: merge-to-head` | Falanx creates a git branch before rewrite, merges after |
| `result.resume()` | `cersei_memory::JsonlMemory` session resume via `session_id` |
| `result.fork()` | New `Agent` instance seeded with a copy of prior messages |
| Horizon reset (discard context) | Construct a fresh `Agent` instance — no session, no prior messages |
| `maxIterations` | `.max_turns(n)` on `AgentBuilder` |
| `Promise.allSettled` parallel | `tokio::join!` or `futures::future::join_all` |
| `main.mts` orchestration script | Falanx orchestrator in Rust (the `CodeReviewOrchestrationAgent` layer) |

---

### Composable Agent Loop in Falanx

Rather than a monolithic orchestration agent, the Sandcastle model suggests building Falanx as a set of focused agents composed by a thin orchestrator — one Cersei agent per phase, called in sequence or parallel by the orchestrator:

```
Orchestrator (Rust, owns loop control)
│
├── ScoreAgent         — one Agent::builder().run(score_prompt)
│   └── returns structured ScoreReport (parsed from <score> XML tag)
│
├── ReviewAgent        — one Agent::builder().run(review_prompt)
│   └── returns structured ReviewReport (parsed from <review> XML tag)
│
└── RewriteAgent       — one Agent::builder().run(rewrite_prompt)
    └── makes file edits, orchestrator handles git commit
```

Each agent is **stateless between phases** — constructed fresh, no shared context. The orchestrator owns all state: score history, review output, loop counter, session IDs. This mirrors Sandcastle exactly: `run()` is stateless; `main.mts` owns state.

---

### Prompt Composition

Sandcastle resolves prompts from template files with shell expressions and key substitution. In Falanx (Rust), the equivalent is assembling prompt strings before calling `Agent::run()`:

```rust
// Shell expression equivalent — run git diff, inject output into prompt
let diff = std::process::Command::new("git")
    .args(["diff", &diff_ref])
    .output()?;
let diff_text = String::from_utf8_lossy(&diff.stdout);

// Key substitution equivalent — build prompt string with phase inputs
let review_prompt = format!(
    include_str!("prompts/review.md"),
    diff = diff_text,
    prior_score = serde_json::to_string(&score_report)?,
);

let output = agent.run(&review_prompt).await?;
```

Prompt files live under `falanx/src/prompts/` or loaded from `.falanx/` in the repo — analogous to `.sandcastle/` prompt files.

---

### Structured Output

Sandcastle uses `Output.object({ tag, schema })` to extract typed JSON from agent output. In Falanx, the same pattern applies — the agent emits JSON inside an XML tag, the orchestrator parses it:

```rust
// Agent prompt instructs: emit score as JSON inside <score> tags
let output = score_agent.run(&prompt).await?;
let text = output.text();

// Extract content between <score>...</score>
let score_json = extract_xml_tag(text, "score")?;
let score: ScoreReport = serde_json::from_str(&score_json)?;
```

The prompt must include the literal tag and a JSON shape example — same constraint as Sandcastle.

---

### Session Resume and Horizon Reset

Cersei's `JsonlMemory` maps to Sandcastle's session resume:

```rust
// Resume — same session_id, prior context loaded
let memory = JsonlMemory::new(&sessions_dir);
let agent = Agent::builder()
    .provider(provider)
    .memory(memory)
    .session_id(&prior_session_id)   // loads prior messages
    .build()?;

// Horizon reset — discard context, start fresh
let fresh_agent = Agent::builder()
    .provider(provider)
    // No .session_id() — clean context
    .system_prompt(&seed_context)    // original diff + current scores only
    .build()?;
```

---

### Loop Control

The orchestration loop in Falanx maps to the `for` loop in a Sandcastle `main.mts`:

```rust
let mut prev_composite: Option<f32> = None;
let mut session_id: Option<String> = None;

for iteration in 0..max_iter {
    // Phase 1: Score
    let score = run_score_agent(&diff_ref, session_id.as_deref()).await?;

    if score.composite >= target_score { break; }

    // Plateau detection → horizon reset
    if let Some(prev) = prev_composite {
        if (score.composite - prev).abs() < plateau_threshold {
            session_id = None;  // drop session = horizon reset
            prev_composite = None;
            continue;
        }
    }
    prev_composite = Some(score.composite);

    // Phase 2: Review
    let review = run_review_agent(&diff_ref, &score).await?;

    if !score_only {
        // Phase 3: Rewrite
        let rewrite_result = run_rewrite_agent(&diff_ref, &review).await?;
        session_id = Some(rewrite_result.session_id);
    }
}
```

---

### Code Review Workflow — Phase Summary

Applied specifically to Falanx's use case:

| Phase | Agent | Model | Input | Output |
|---|---|---|---|---|
| **Score** | `ScoreAgent` | Sonnet (fast, structured) | git diff | `<score>` JSON — per-category scores + composite |
| **Review** | `ReviewAgent` | Opus (deeper critique) | git diff + score | `<review>` JSON — issues with location, problem, fix |
| **Rewrite** | `RewriteAgent` | Sonnet (targeted edits) | diff + review issues | file edits committed to branch |

Exit conditions (orchestrator loop):
- `composite >= target_score` — happy path
- `iteration >= max_iter` — hard stop (mandatory)
- `score_delta < plateau_threshold` → horizon reset, then continue (not an exit)

---

_Reference: [Sandcastle CONTEXT.md](../../../sandcastle/CONTEXT.md) · [Falanx Agents](FALANX_AGENTS.md) · [Cersei API](CERSEI.md) · [Falanx ARCHITECTURE.md](ARCHITECTURE.md)_
