# Nix Dev Environment Bootstrap — Design Spec

**Date:** 2026-05-21  
**Status:** Approved

## Goal

Bootstrap a reproducible Nix + direnv development environment for the Falanx project. Any developer with Nix installed runs `direnv allow` once and gets an identical, pinned shell — no manual toolchain installation.

## File Structure

```
automated-code-review/
├── flake.nix              # root orchestrator (flake-parts + import-tree)
├── flake.lock             # pinned input hashes — committed to git
├── .envrc                 # direnv: use flake + watch config.nix + source .env.local
└── rust/
    ├── config.nix         # devShell: Rust toolchain, packages, validate commands
    ├── rust-toolchain.toml
    └── crates/
        └── falanx-engine/
            ├── Cargo.toml
            └── src/
```

Future languages (e.g. a UI) add a `ui/config.nix` — `import-tree` picks it up automatically, no wiring needed.

## Nix Inputs

| Input | Source | Purpose |
|---|---|---|
| `nixpkgs` | nixpkgs-unstable | base package set |
| `flake-parts` | hercules-ci/flake-parts | modular flake composition |
| `rust-overlay` | oxalica/rust-overlay | pins Rust toolchain from `rust-toolchain.toml` |
| `crane` | ipetkov/crane | Rust builds in nix (for `nix build` later) |
| `import-tree` | vic/import-tree | auto-discovers all `*/config.nix` files in repo |

`flake.nix` loads all discovered `config.nix` files via `import-tree` and wires them into `flake-parts`. DevShells merge automatically into `default`.

## Rust Toolchain (`rust/rust-toolchain.toml`)

```toml
[toolchain]
channel = "1.89.0"
components = ["rustfmt", "clippy", "rust-src"]
```

Matches rust-crates pinned version. Update explicitly when ready to upgrade.

## DevShell Packages (`rust/config.nix`)

| Package | Purpose |
|---|---|
| Rust `1.89.0` via rust-overlay | compiler + cargo |
| `cargo-nextest` | parallel test runner |
| `cargo-audit` | CVE checks via RustSec advisory database |
| `ast-grep` | structural code search/transform (useful for testing the review engine) |
| `git` | version control |
| `jq` | API response inspection |

## Shell Commands

Exposed via `shellHook` in `rust/config.nix`:

```bash
validate() {
  cargo fmt --check &&
  cargo clippy -- -D warnings &&
  cargo nextest run
}

validate-full() {
  validate &&
  cargo audit
}
```

- `validate` — fast dev loop check before pushing
- `validate-full` — includes CVE audit; intended as a pre-commit hook in future

## Direnv (`.envrc`)

```bash
use flake .
watch_file flake.nix flake.lock
watch_file rust/config.nix

source_env_if_exists .env.local
```

- Shell reloads automatically when any watched file changes
- `.env.local` loads API keys into the environment silently — never committed

## README Updates

The existing README dependency/setup section must be replaced to reflect nix-first setup:

1. **Setup section** — replace manual `cargo install` / toolchain instructions with:
   - Install Nix (with flakes enabled) + direnv
   - Clone repo, run `direnv allow`
   - Shell configures automatically — no further setup

2. **`.env.local` section** — point developers to `example.env.local` at repo root; copy and fill in values:
   - `ANTHROPIC_API_KEY` (required)
   - `OPENAI_API_KEY` (optional)
   - `OLLAMA_HOST` (optional, default `http://localhost:11434`)
   - Note: `.env.local` is gitignored, never commit it

3. **Remove** any manual dependency definition sections — Nix owns versions now.
