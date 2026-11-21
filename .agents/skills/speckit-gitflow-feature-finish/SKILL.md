---
name: speckit-gitflow-feature-finish
description: Finish a Git Flow feature branch into dev (or open a PR targeting dev) with Conventional Commits
compatibility: Requires spec-kit project structure with .specify/ directory and git flow config
metadata:
  author: project-local
  source: gitflow:commands/speckit.gitflow.feature-finish.md
  version: 1.0.0
---

# Finish Git Flow Feature Branch

Complete the active Git Flow feature branch after `/speckit-implement` finishes, merging into `dev` (`gitflow.branch.develop`) or opening a Pull Request into `dev`.

## User Input

```text
$ARGUMENTS
```

## Prerequisites

1. Verify the working tree is clean (`git status --porcelain`). If there are uncommitted changes, run `/speckit-git-commit` or ask the user to commit first.
2. Verify the current branch starts with `gitflow.prefix.feature` (default `feature/`):
   ```bash
   CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
   DEVELOP_BRANCH=$(git config --get gitflow.branch.develop || echo "dev")
   FEATURE_PREFIX=$(git config --get gitflow.prefix.feature || echo "feature/")
   ```
   Extract `FLOW_FEATURE_NAME="${CURRENT_BRANCH#"$FEATURE_PREFIX"}"`.

## Execution

Ask the user (unless specified in `$ARGUMENTS`) whether to:
1. **Open a Pull Request into `dev` (Recommended for protected branches)**:
   - Push the feature branch: `git push -u origin "$CURRENT_BRANCH"`
   - Create a PR targeting `$DEVELOP_BRANCH` with a Conventional Commit title (e.g., `feat(<scope>): <summary>`):
     ```bash
     gh pr create --base "$DEVELOP_BRANCH" --title "feat(<scope>): <summary>" --body "Implements spec \`specs/${FLOW_FEATURE_NAME}/spec.md\`."
     ```
2. **Finish locally with `git flow`**:
   - Ensure the merge commit message follows Conventional Commits so `.git/hooks/commit-msg` (`commitlint`) passes:
   - **If `git flow` CLI is installed**:
     ```bash
     git flow feature finish "$FLOW_FEATURE_NAME"
     ```
     *(If prompted for a merge commit message, use `feat(<scope>): <summary>`.)*
   - **Fallback (if `git flow` CLI is not installed)**:
     ```bash
     git checkout "$DEVELOP_BRANCH"
     git merge --no-ff "$CURRENT_BRANCH" -m "feat(<scope>): <summary>"
     git branch -d "$CURRENT_BRANCH"
     ```
