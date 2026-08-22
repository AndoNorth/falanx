# Falanx Agents

Describes the composable `AgentDef` model, how to define agents, and the default agent pipeline. Each agent is a stateless, declarative unit: system prompt + prompt template + configuration. All agents run through the same Cersei execution engine.

---

## AgentDef Model

An agent is defined by:

```yaml
name: String              # Agent identifier
kind: AgentKind          # Execution backend (currently: Cersei)
system_prompt: String    # System instructions
prompt_template: String  # minijinja template with context variables
max_turns: u32          # Max conversation turns per invocation
```

**Key principle:** Agents are fully isolated — each run is stateless. Context flows one direction: `TemplateContext` carries read-only state between pipeline stages. No agent modifies shared state or influences another agent's session.

---

## Agent Directory Structure

Agents live in `.falanx/agents/<agent_name>/`:

```
.falanx/agents/
├── score_readability/
│   ├── system.md       # System prompt text
│   ├── prompt.md       # minijinja template
│   └── config.yaml     # kind + max_turns
├── score_maintainability/
│   ├── system.md
│   ├── prompt.md
│   └── config.yaml
├── review/
│   ├── system.md
│   ├── prompt.md
│   └── config.yaml
└── rewrite/
    ├── system.md
    ├── prompt.md
    └── config.yaml
```

**Template context variables** passed to prompt.md:

- `{{ diff }}` — The target code diff
- `{{ score_result }}` — Array of scoring agent outputs (available in review stage)
- `{{ review_result }}` — Array of review issues (available in rewrite stage)

---

## Default Agents

| Agent | Purpose | max_turns | Template Variables | Output Format |
|---|---|---|---|---|
| `score_readability` | Readability score 1–5 | 1 | `{{ diff }}` | `{"score": N, "reasoning": "..."}` |
| `score_maintainability` | Maintainability score 1–5 | 1 | `{{ diff }}` | `{"score": N, "reasoning": "..."}` |
| `score_architecture` | Architecture score 1–5 | 1 | `{{ diff }}` | `{"score": N, "reasoning": "..."}` |
| `score_performance` | Performance score 1–5 | 1 | `{{ diff }}` | `{"score": N, "reasoning": "..."}` |
| `score_security` | Security score 1–5 | 1 | `{{ diff }}` | `{"score": N, "reasoning": "..."}` |
| `review` | Critique issues from diff + scores | 1 | `{{ diff }}`<br>`{{ score_result }}` | `[{"location": "...", "issue": "...", "fix": "..."}]` |
| `rewrite` | Apply fixes from critique | 3 | `{{ diff }}`<br>`{{ review_result }}` | `[{"location": "...", "change": "..."}]` |

**Scoring agents:**
- Receive the diff only
- Respond with a score (1–5) and reasoning
- Run independently and in parallel

**Review agent:**
- Receives the diff plus all scoring outputs
- Identifies specific issues: code smells, anti-patterns, logic errors, edge cases, architectural concerns
- Each issue must include location, problem statement, and fix recommendation
- Scoped to the blast radius of the diff only

**Rewrite agent:**
- Receives the diff plus all review issues
- Applies minimum changes required to address each issue
- Every change must map to a specific issue — nothing implicit
- Max 3 turns to iterate if fix applications fail

---

## Behavioural Contracts

These hold across all agents:

- **Isolation:** Each agent run is fully stateless. No agent shares a Cersei session with another.
- **Context flow:** `TemplateContext` is the only carrier of state between stages. Agents read from context but never modify it.
- **Blast radius:** All agents are scoped to the diff only — not whole-codebase analysis.
- **Traceability:** Every agent output is captured in the JSONL audit trail.
- **No rewrite without critique:** Review output is a prerequisite for rewrite invocation.

---

## Extending Agents

To add a new agent:

1. Create `.falanx/agents/<new_name>/` with:
   - `system.md` — System prompt text
   - `prompt.md` — minijinja template (variables depend on pipeline stage)
   - `config.yaml` — `kind: Cersei` and `max_turns: N`

2. Reference the agent in the workflow pipeline (orchestrator configuration)

3. For future backends: `AgentKind` can be extended to route to Claude Code CLI, Codex, Pi, or other execution targets
