# Bug Fix: Provide Repo Context, Job-Scoped `issues: write`, Fork Guard, and Stderr Handling in `pr-hygiene` Auto-Labeling

- **Slug**: pr-4-pr-hygiene-gh-missing-repo-context
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **GitHub Issue**: #10
- **Status**: Verified
- **Commit SHA**: `61877f2`

## Summary

Fixed `.github/workflows/pr-hygiene.yml` so PR auto-labeling succeeds on same-repository pull requests without requiring `actions/checkout`, skips cleanly on forked PRs, and surfaces real `gh pr edit` failures instead of swallowing stderr. Top-level `permissions` is now restricted to `contents: read`, write permissions (`pull-requests: write`, `issues: write`) are scoped to the `pr-hygiene` job, `GH_REPO: ${{ github.repository }}` and `--repo "$GH_REPO"` supply repository context to `gh pr edit`, and stderr is captured so only missing-label errors emit `::warning::` while real errors emit `::error::` and fail the step.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `.github/workflows/pr-hygiene.yml` | modified | Restricted top-level `permissions` to `contents: read`; added job-scoped `permissions` (`contents: read`, `pull-requests: write`, `issues: write`) on `pr-hygiene` (preserving top-level `concurrency` with `cancel-in-progress: true`); added fork guard `if: github.event.pull_request.head.repo.full_name == github.repository` and `GH_REPO: ${{ github.repository }}` to `Auto-Label PR from Branch & Title`; passed `--repo "$GH_REPO"` to `gh pr edit` and replaced `2>/dev/null \|\| echo "::warning::..."` with stderr capture that warns only on missing-label errors and exits non-zero on real errors. |

## Diff Highlights

```diff
--- a/.github/workflows/pr-hygiene.yml
+++ b/.github/workflows/pr-hygiene.yml
@@ -6,17 +6,20 @@ on:
 
 permissions:
   contents: read
-  pull-requests: write
 
 concurrency:
   group: ${{ github.workflow }}-${{ github.head_ref || github.ref_name }}
   cancel-in-progress: true
 
 jobs:
   pr-hygiene:
     name: Validate PR Title, Git Flow Branch & Labels
     runs-on: ubuntu-26.04
     timeout-minutes: 10
+    permissions:
+      contents: read
+      pull-requests: write
+      issues: write
     steps:
```

```diff
       - name: Auto-Label PR from Branch & Title
+        if: github.event.pull_request.head.repo.full_name == github.repository
         env:
           GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
+          GH_REPO: ${{ github.repository }}
           PR_NUMBER: ${{ github.event.pull_request.number }}
           HEAD_REF: ${{ github.head_ref }}
           PR_TITLE: ${{ github.event.pull_request.title }}
...
           for label in "${LABELS[@]}"; do
-            gh pr edit "$PR_NUMBER" --add-label "$label" 2>/dev/null || echo "::warning::Could not apply label '$label'"
+            if ! err_output=$(gh pr edit "$PR_NUMBER" --repo "$GH_REPO" --add-label "$label" 2>&1); then
+              err_single_line="${err_output//$'\n'/ }"
+              if [[ "$err_output" == *"'$label' not found"* ]] \
+                || [[ "$err_output" == *"\"$label\" not found"* ]] \
+                || [[ "$err_output" == *"label $label not found"* ]] \
+                || [[ "$err_output" == *"'$label' does not exist"* ]] \
+                || [[ "$err_output" == *"\"$label\" does not exist"* ]]; then
+                echo "::warning::Could not apply missing label '$label' to PR #$PR_NUMBER: $err_single_line"
+              else
+                echo "::error::Failed to apply label '$label' to PR #$PR_NUMBER: $err_single_line" >&2
+                exit 1
+              fi
+            fi
           done
```

## Tests Added or Updated

- Workflow configuration bug; no repository unit test files were modified (per strict allowlist). Verified YAML parsing, static workflow assertions, `bash -n` syntax validation on all `run` blocks, and stubbed behavioral execution of the auto-label shell block.

## Local Verification

- **YAML syntax & static assertions**:
  - Command: `ruby --disable-gems -ryaml -e '...'` parsing `/Users/jlgtechnology/Developer/cae/cae-cli/.github/workflows/pr-hygiene.yml`
  - Result: Exit `0` — `YAML + static assertions OK` (verified top-level `permissions == {"contents" => "read"}`, `concurrency.cancel-in-progress == true`, job `pr-hygiene` `permissions == {"contents" => "read", "pull-requests" => "write", "issues" => "write"}`, fork guard `if`, `env.GH_REPO == "${{ github.repository }}"`, `--repo "$GH_REPO"` present, and `2>/dev/null` absent).
- **Bash syntax validation (`bash -n`)**:
  - Command: `bash -n` on both `run:` scripts extracted from `.github/workflows/pr-hygiene.yml`
  - Result: Exit `0` — `bash -n OK: Enforce Git Flow Branch Targeting`, `bash -n OK: Auto-Label PR from Branch & Title`.
- **Auto-label script behavioral verification (with stubbed `gh` shell function under `bash -e`)**:
  - Success path (`feature/foo`, `feat(ci): bar`): Exit `0`; verified exact arguments `pr edit 4 --repo JLG-Technology-LLC/cae-cli --add-label enhancement`.
  - Empty `LABELS` path (`chore/foo`, `chore: housekeeping`): Exit `0`; `gh` not invoked.
  - Missing-label error (`could not add label: 'enhancement' not found` and multi-line missing-label stderr): Exit `0`; emitted single-line `::warning::Could not apply missing label ...`.
  - Permission error (`GraphQL: Resource not accessible by integration`): Exit `1`; emitted `::error::Failed to apply label ...`.
  - HTTP 404 repo/PR error (`HTTP 404: Not Found`): Exit `1`; emitted `::error::Failed to apply label ...` (not misclassified as missing label).
- **Optional linters**: `actionlint` and `shellcheck` are not installed in this environment.

## Deviations from Assessment

- **Fork guard + strict failure on real errors**: `assessment.md` suggested warning on all `gh pr edit` failures to avoid breaking fork PRs. Instead of masking real errors on same-repo PRs, added the explicit fork guard `if: github.event.pull_request.head.repo.full_name == github.repository` on the `Auto-Label PR from Branch & Title` step and narrowed the non-fatal `::warning::` fallback to missing-label errors only, failing the step (`exit 1` with `::error::`) on real errors (auth, permissions, network, invalid repo/PR).

## Follow-ups

- Stage 3 verification (`/speckit-bug-test slug=pr-4-pr-hygiene-gh-missing-repo-context`) should confirm workflow validity and verify auto-labeling behavior on a live GitHub Actions run once pushed to a PR branch.
