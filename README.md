# Falanx 🛡️

Automated code review for your merge request diffs. Scores, critiques, and optionally rewrites code — with a full audit trail you can trust before you merge.

Built in Rust. Developer-triggered. Fast, composable, auditable.

---

## Getting Started

### Prerequisites

- [Nix](https://nixos.org/download/) with flakes enabled
- [direnv](https://direnv.net/) with shell hook configured

### Setup

```bash
git clone <repository-url>
cd falanx 
direnv allow
```

That's it. Nix provisions the exact Rust toolchain, tools, and dependencies. No manual `cargo install` or toolchain management needed.

### Configure API keys

Copy the example env file and fill in your values:

```bash
cp example.env.local .env.local
```

Edit `.env.local`:

```env
# Required for Anthropic (Claude) models
ANTHROPIC_API_KEY=sk-ant-xxxxxxxxxxxxxxxxxxxx

# Model to use (default: anthropic/claude-opus-4-7)
FALANX_MODEL=anthropic/claude-opus-4-7
```

`.env.local` is gitignored and loaded automatically by direnv. Never commit it.

### Dev commands

Available in the nix shell:

| Command | What it does |
|---|---|
| `validate` | `cargo fmt --check` + `cargo clippy` + `cargo nextest run` |
| `validate-full` | `validate` + `cargo audit` (CVE check) |

---

## Usage

```bash
# Review a git diff
falanx run --diff HEAD~1

# Review across a branch range
falanx run --diff main..feature

# Review a single file
falanx run --file src/main.rs

# Override loop settings
falanx run --diff HEAD~1 --target 4.5 --max-iter 3

# Load additional agents from a local directory
falanx run --diff HEAD~1 --agents ./.falanx/agents
```

### Sessions

Each run writes a JSONL audit trail scoped to your current target.

```bash
# List past sessions
falanx list-sessions

# Inspect a session directly
cat ~/.falanx/sessions/<label>/<session-id>.jsonl | jq .
```

### Serve Mode

Run Falanx as a long-running process with an MCP server and HTTP observability API:

```bash
falanx serve
```

Via Docker:

```bash
docker run -v $(pwd):/repo -p 3000:3000 falanx serve
```

---

## Testing

### Without an API key (dry-run)

The mock provider runs the full pipeline with no LLM calls. Good for verifying config, agent loading, and session output:

```bash
FALANX_DRY_RUN=true falanx run --diff HEAD~1 --agents rust/crates/falanx-engine/src/defaults/agents
```

Or with the flag directly:

```bash
cargo run --bin falanx -- run --dry-run --diff HEAD~1 --agents rust/crates/falanx-engine/src/defaults/agents
```

Check the session output:

```bash
cat $(ls -t ~/.falanx/sessions/**/*.jsonl | head -1) | jq .
```

### With a local Ollama model

> **Known limitation:** cersei-provider 0.1.9 routes Ollama through the OpenAI-compatible `/v1/chat/completions` shim. Some models produce garbled, off-topic responses through this shim even when they work correctly via Ollama's native `/api/chat` endpoint. If agents fail with "no JSON object in agent response", this is the likely cause — verify by curling Ollama's native endpoint directly with the same prompt.

```bash
FALANX_MODEL='ollama/<model-name>' \
falanx run --diff HEAD~1 --agents rust/crates/falanx-engine/src/defaults/agents
```

No `ANTHROPIC_API_KEY` needed for Ollama models. `FALANX_MAX_DIFF_CHARS=6000` helps keep diffs within smaller context windows.

### Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `FALANX_MODEL` | `anthropic/claude-opus-4-7` | Model ID (`anthropic/...`, `ollama/...`) |
| `ANTHROPIC_API_KEY` | — | Required for Anthropic models |
| `FALANX_BASE_URL` | — | Provider URL override (e.g. Ollama's host) |
| `FALANX_DRY_RUN` | `false` | Use mock provider, no LLM calls |
| `FALANX_MAX_DIFF_CHARS` | `20000` | Truncate large diffs before sending |
| `FALANX_SESSION_DIR` | `~/.falanx/sessions` | Where JSONL audit trails are written |
| `FALANX_MAX_ITER` | `3` | Default loop iteration cap |
| `FALANX_TARGET_SCORE` | `4.5` | Default target score for exit |
| `FALANX_PLATEAU_THRESHOLD` | `0.1` | Default plateau threshold for horizon reset |

---

## Further Reading

- [Northstar](docs/NORTHSTAR.md) — vision, quality philosophy, core promises
- [Architecture](docs/ARCHITECTURE.md) — deployment modes, context management, session model, serve mode
- [Agents](docs/FALANX_AGENTS.md) — agent pipeline, roles, and behavioural contracts
