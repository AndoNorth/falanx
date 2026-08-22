# Git Conventions

## Worktree rule

Worktrees live inside `.worktrees/<type>/<slug>`

e.g. `.worktrees/feat/serve-mode-mcp`
e.g. `.worktrees/fix/horizon-reset-plateau-detect`

## Branch naming rule

Branches follow `<type>/<slug>`

e.g. `feat/serve-mode-mcp`
e.g. `fix/horizon-reset-plateau-detect`

## Commit message format

```
<type>(<context>): [#N] <message>       # when a GH issue exists
<type>(<context>): [no-issue] <message> # when no GH issue
```

`<context>` is typically a crate or module name (`workflow`, `agent`,
`orchestrator`, `cli`, `session`, `docs`).

e.g. `feat(workflow): [#4] add plateau-based horizon reset`
e.g. `chore(docs): [no-issue] flesh out serve mode architecture`

## PR + issue association

- Add `#N` in the PR body to link to an issue
- Use `Closes #N` or `Fixes #N` in the PR body to auto-close on merge
- Do NOT use `gh-N` anywhere - use GitHub-native `#N` syntax
