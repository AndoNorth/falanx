# Falanx

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
# Required
ANTHROPIC_API_KEY=sk-ant-xxxxxxxxxxxxxxxxxxxx

# Optional
OPENAI_API_KEY=
OLLAMA_HOST=http://localhost:11434
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

## Further Reading

- [Northstar](docs/NORTHSTAR.md) — vision, quality philosophy, core promises
- [Architecture](docs/ARCHITECTURE.md) — deployment modes, context management, session model, serve mode
- [Agents](docs/AGENTS.md) — agent pipeline, roles, and behavioural contracts
