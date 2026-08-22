---
name: pr-format
description: Use when opening or editing a pull request in this repo (gh pr create / gh pr edit) - defines the required PR title and body format. Trigger on "open a PR", "create the PR", "PR isn't in the right format", or before any gh pr create call.
---

# PR Format

This repo's PRs follow a fixed title and body shape. Before calling
`gh pr create`, or if a PR needs reformatting after the fact with
`gh pr edit`, match this structure. When in doubt, check the most recent
well-formed PR for the current convention:

```bash
gh pr list --state all --limit 5
gh pr view <N> --json title,body
```

## Title

```
<type>(<context>): [#<PR-number>,#<issue-number>] <message>
```

- `<type>(<context>): ...` follows `.claude/rules/git-conventions.md`
  (same types/scopes as commit messages).
- The bracket carries **both** numbers, PR first: `[#18,#10]` for PR #18
  closing issue #10. This differs from the commit-message convention,
  which carries only the issue number.
- If the PR has no linked issue, fall back to the commit convention's
  `[no-issue]` marker instead of a bracket pair.
- The PR number isn't known until after `gh pr create` runs once - create
  first with a placeholder-free best-guess title (issue number only is
  fine), then `gh pr edit` to add the PR number once assigned. Or check
  the next PR number in advance with `gh pr list --state all --limit 1`
  (highest number + 1) if you want it right the first time.
- Self-assign the PR (`gh pr create --assignee @me` or
  `gh pr edit <N> --add-assignee @me` after creation).

## Body

Three sections, in this order. Omit a section only if it's genuinely
empty (e.g. no notable decisions worth recording) - don't pad with filler
to keep the section present.

### `## Summary`

- First line: `Closes #<issue-number>.` followed by one sentence on what
  this PR delivers and why (the "so what", not a changelog entry).
- Then a bullet list of what changed, one bullet per meaningful piece of
  work - not one bullet per file or per commit.

### `## Notable decisions`

Judgment calls a reviewer would otherwise have to reconstruct from the
diff: rejected alternatives, deferred follow-ups (tag which future
issue/ticket owns them), anything ruled on without asking mid-implementation.
Skip this section entirely if there's nothing non-obvious to record.

### `## Test plan`

A checklist of what was actually verified, checked off (`- [x]`) because
it already happened - not a TODO list for the reviewer to run. Each line
names the concrete command or verification, not a vague claim:

```
- [x] Unit tests pass (`make test`, service X)
- [x] Manual verification: <what was manually confirmed and how>
```

## Creating vs. editing

```bash
# Create (issue number known, PR number not yet)
gh pr create --title "..." --body "$(cat <<'EOF'
...
EOF
)"

# Reformat after creation (PR number now known)
gh pr edit <N> --title "..." --body "$(cat <<'EOF'
...
EOF
)"
```

Always pass `--body` via a heredoc so multi-line Markdown formatting
survives shell quoting.
