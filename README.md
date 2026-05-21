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
falanx review --diff HEAD~1

# Review across a branch
falanx review --diff main..feature

# Review a single file
falanx review --file src/main.rs

# Score only — no rewrite
falanx score --diff HEAD~1

# Full pipeline: score + review + rewrite
falanx full --diff HEAD~1

# Loop until target score or max iterations
falanx full --diff HEAD~1 --target 4.5 --max-iter 3
```

### Sessions

Each run starts a new session. Sessions are scoped to your current branch.

```bash
# List sessions for this branch
falanx list-sessions

# Continue a previous session
falanx review --diff HEAD~1 --continue <session-id>
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

## Example Output

```
=== Code Quality Score ===
Readability:     4/5
Maintainability: 3/5
Performance:     4/5
Security:        5/5
Architecture:    3/5
Overall:         3.8/5

=== Review Feedback ===
[1] src/auth.rs:42 — token expiry check uses `<` not `<=`; off-by-one allows expired tokens through for one second window. Fix: change comparison operator.
[2] src/handlers/user.rs:118 — error variant returned without context; caller cannot distinguish network failure from validation failure. Fix: wrap in typed error enum.

=== Applied Changes ===
--- a/src/auth.rs
+++ b/src/auth.rs
@@ -42 +42 @@
-    if now < expiry {
+    if now <= expiry {
```

---

## Further Reading

- [Northstar](docs/NORTHSTAR.md) — vision, quality philosophy, core promises
- [Architecture](docs/ARCHITECTURE.md) — deployment modes, context management, session model, serve mode
- [Agents](docs/AGENTS.md) — agent pipeline, roles, and behavioural contracts
