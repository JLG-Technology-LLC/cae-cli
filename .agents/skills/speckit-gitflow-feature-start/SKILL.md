---
name: speckit-gitflow-feature-start
description: Start a Git Flow feature branch from dev with Spec Kit sequential or timestamp numbering
compatibility: Requires spec-kit project structure with .specify/ directory and git flow config
metadata:
  author: project-local
  source: gitflow:commands/speckit.gitflow.feature-start.md
  version: 1.0.0
---

# Start Git Flow Feature Branch

Create and switch to a new Git Flow feature branch branched from `dev` (`gitflow.branch.develop`) for the given specification. This skill handles **branch creation only** — the spec directory and files are created by `/speckit-specify`.

## User Input

```text
$ARGUMENTS
```

## Prerequisites

1. Verify Git is available (`git rev-parse --is-inside-work-tree 2>/dev/null`). If not, warn and skip branch creation.
2. Read the configured develop branch and feature prefix:
   ```bash
   DEVELOP_BRANCH=$(git config --get gitflow.branch.develop || echo "dev")
   FEATURE_PREFIX=$(git config --get gitflow.prefix.feature || echo "feature/")
   ```

## Compute Feature Branch Name

1. If `GIT_BRANCH_NAME` is explicitly provided by the user, use it directly and extract `FEATURE_NUM` from its final path segment (if it starts with `[0-9]{3,}-` or `[0-9]{8}-[0-9]{6}-`).
2. Otherwise, generate a concise short name (2-4 words, action-noun kebab-case) from the feature description and compute the next number using Spec Kit's dry-run script so numbering stays consistent with `specs/` and existing branches:
   ```bash
   .specify/extensions/git/scripts/bash/create-new-feature-branch.sh --dry-run --json --short-name "<short-name>" "<feature description>"
   ```
   Extract `BRANCH_NAME` (e.g. `feature/012-user-auth`) and `FEATURE_NUM` (e.g. `012`) from the JSON output.
   Strip the leading `${FEATURE_PREFIX}` (e.g. `feature/`) to get `FLOW_FEATURE_NAME` (e.g. `012-user-auth`).

## Execution

1. Check whether `git flow` CLI is installed (`git flow version >/dev/null 2>&1`):
   - **If `git flow` is installed**:
     ```bash
     git flow feature start "$FLOW_FEATURE_NAME"
     ```
     *(This guarantees the branch is created from `dev` as `feature/<NNN>-<slug>`.)*
   - **Fallback (if `git flow` CLI is not installed)**:
     ```bash
     git checkout -b "${FEATURE_PREFIX}${FLOW_FEATURE_NAME}" "$DEVELOP_BRANCH"
     ```
2. If the branch already exists and the user intends to resume work on it, switch to it instead of failing.

## Output

Report the created branch in JSON format compatible with `/speckit-specify`:

```json
{"BRANCH_NAME": "feature/<NNN>-<slug>", "FEATURE_NUM": "<NNN>"}
```
