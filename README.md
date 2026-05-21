# Falanx

Automated code review for your merge request diffs. Scores, critiques, and optionally rewrites code — with a full audit trail you can trust before you merge.

Built in Rust. Developer-triggered. Fast, composable, auditable.

---

## Getting Started

### Prerequisites

- Rust 1.78+
- `cargo`
- An LLM provider API key (Anthropic, OpenAI, or Ollama)

### Install

```bash
cargo install falanx
```

Or build from source:

```bash
git clone <repository-url>
cd falanx
cargo build --release
```

### Configure

Create a `.env` file in your project root:

```env
ANTHROPIC_API_KEY="sk-ant-xxxxxxxxxxxxxxxxxxxx"
LLM_MODEL="claude-sonnet-4-6"
```

CLI args override `.env` values at invocation time.

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

- [Architecture](docs/ARCHITECTURE.md) — deployment modes, context management, session model, serve mode
- [Northstar](docs/NORTHSTAR.md) — vision, quality philosophy, core promises
- [Agents](docs/AGENTS.md) — agent pipeline, roles, and behavioural contracts
