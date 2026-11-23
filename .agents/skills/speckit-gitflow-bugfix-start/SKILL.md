---
name: speckit-gitflow-bugfix-start
description: Start a Git Flow bugfix (from dev) or hotfix (from main) branch for a Spec Kit bug assessment
compatibility: Requires spec-kit project structure with .specify/ directory and git flow config
metadata:
  author: project-local
  source: gitflow:commands/speckit.gitflow.bugfix-start.md
  version: 1.0.0
---

# Start Git Flow Bugfix / Hotfix Branch

Create and switch to a Git Flow `bugfix/<BUG_SLUG>` branch from `dev` (or `hotfix/<BUG_SLUG>` from `main` for production emergencies) before `/speckit-bug-fix` applies remediation.

## User Input

```text
$ARGUMENTS
```

## Slug & Branch Type Resolution

1. Reuse `BUG_SLUG` from the active `/speckit-bug-fix` session or `$ARGUMENTS`.
2. Determine branch type:
   - Default: **bugfix** (branches from `gitflow.branch.develop`, default `dev`, with prefix `bugfix/`).
   - If `$ARGUMENTS` explicitly specifies `--hotfix` or `hotfix`: **hotfix** (branches from `gitflow.branch.master`, default `main`, with prefix `hotfix/`).

## Execution

1. Check if already on `${PREFIX}${BUG_SLUG}` — if so, report it and exit cleanly without recreating.
2. Check whether `git flow` CLI is installed (`git flow version >/dev/null 2>&1`):
   - **If `git flow` is installed**:
     - Bugfix: `git flow bugfix start "$BUG_SLUG"`
     - Hotfix: `git flow hotfix start "$BUG_SLUG"`
   - **Fallback (if `git flow` CLI is not installed)**:
     - Bugfix: `git checkout -b "bugfix/${BUG_SLUG}" "$(git config --get gitflow.branch.develop || echo dev)"`
     - Hotfix: `git checkout -b "hotfix/${BUG_SLUG}" "$(git config --get gitflow.branch.master || echo main)"`

## Output

Report the active branch name (`bugfix/<BUG_SLUG>` or `hotfix/<BUG_SLUG>`) so `/speckit-bug-fix` proceeds on the isolated branch.
