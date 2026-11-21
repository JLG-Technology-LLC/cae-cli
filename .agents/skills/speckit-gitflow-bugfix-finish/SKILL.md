---
name: speckit-gitflow-bugfix-finish
description: Finish a verified Git Flow bugfix or hotfix branch with Conventional Commits
compatibility: Requires spec-kit project structure with .specify/ directory and git flow config
metadata:
  author: project-local
  source: gitflow:commands/speckit.gitflow.bugfix-finish.md
  version: 1.0.0
---

# Finish Git Flow Bugfix / Hotfix Branch

Complete the active Git Flow `bugfix/<BUG_SLUG>` or `hotfix/<BUG_SLUG>` branch after `/speckit-bug-test` records `Result: verified`.

## User Input

```text
$ARGUMENTS
```

## Prerequisites

1. Verify `.specify/bugs/<BUG_SLUG>/test.md` has `Result: verified`. If not verified, warn the user and do not finish the branch unless they explicitly override.
2. Verify the working tree is clean (`git status --porcelain`).
3. Detect branch type from `CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)`:
   - `bugfix/<slug>` → target `dev` (`gitflow.branch.develop`)
   - `hotfix/<slug>` → targets both `main` (`gitflow.branch.master`) and `dev` (`gitflow.branch.develop`)

## Execution

Ask the user (unless specified in `$ARGUMENTS`) whether to:
1. **Open a Pull Request (Recommended)**:
   - Push the branch: `git push -u origin "$CURRENT_BRANCH"`
   - Open a PR targeting `dev` (for `bugfix/*`) or `main` (for `hotfix/*`) with a Conventional Commit title `fix(<scope>): <summary>`.
2. **Finish locally with `git flow`**:
   - **If `git flow` CLI is installed**:
     - Bugfix: `git flow bugfix finish "$BUG_SLUG"`
     - Hotfix: `git flow hotfix finish "$BUG_SLUG"`
   - **Fallback (if `git flow` CLI is not installed)**:
     - Bugfix:
       ```bash
       DEVELOP_BRANCH=$(git config --get gitflow.branch.develop || echo "dev")
       git checkout "$DEVELOP_BRANCH"
       git merge --no-ff "$CURRENT_BRANCH" -m "fix(${BUG_SLUG}): resolve bug"
       git branch -d "$CURRENT_BRANCH"
       ```
     - Hotfix: merge into `main` and `dev` with Conventional Commit message `fix(${BUG_SLUG}): <summary>`, then delete `$CURRENT_BRANCH`.
