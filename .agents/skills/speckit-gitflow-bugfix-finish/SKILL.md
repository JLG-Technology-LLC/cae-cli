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
3. Detect branch type and extract `BUG_SLUG` from `CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)`:
   - `bugfix/<BUG_SLUG>` → `BUG_SLUG="${CURRENT_BRANCH#bugfix/}"`, target `dev` (`gitflow.branch.develop`)
   - `hotfix/<BUG_SLUG>` → `BUG_SLUG="${CURRENT_BRANCH#hotfix/}"`, targets both `main` (`gitflow.branch.master`) and `dev` (`gitflow.branch.develop`)

## Execution

Ask the user (unless specified in `$ARGUMENTS`) whether to:
1. **Open a Pull Request (Recommended)**:
   - Push the branch: `git push -u origin "$CURRENT_BRANCH"`
   - Look up the corresponding open GitHub bug issue and extract the verification summary from `.specify/bugs/${BUG_SLUG}/test.md`:
     ```bash
     ISSUE_JSON=$(gh issue list --state open --search "speckit-bug:${BUG_SLUG}" --json number,url --jq '.[0]')
     ISSUE_NUMBER=$(echo "$ISSUE_JSON" | jq -r '.number // empty')
     TEST_SUMMARY=$(cat ".specify/bugs/${BUG_SLUG}/test.md" 2>/dev/null || echo "")
     ```
   - Build the PR body: include `Fixes #${ISSUE_NUMBER}` (if `ISSUE_NUMBER` is non-empty) and attach the `.specify/bugs/${BUG_SLUG}/test.md` verification summary.
   - Open a PR targeting `dev` (for `bugfix/*`) or `main` (for `hotfix/*`) with a Conventional Commit title `fix(<scope>): <summary>`:
     ```bash
     PR_BODY=""
     if [ -n "$ISSUE_NUMBER" ]; then
       PR_BODY="Fixes #${ISSUE_NUMBER}\n\n"
     fi
     if [ -n "$TEST_SUMMARY" ]; then
       PR_BODY="${PR_BODY}## Verification Summary (\`.specify/bugs/${BUG_SLUG}/test.md\`)\n\n${TEST_SUMMARY}"
     fi
     gh pr create --base "$TARGET_BRANCH" --title "fix(${BUG_SLUG}): <summary>" --body "$(printf '%b' "$PR_BODY")"
     ```
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
