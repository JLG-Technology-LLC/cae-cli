# Bug Assessment: `pr-hygiene` Auto-Labeling Fails Without Repo Context or `issues: write` and Swallows Stderr

- **Slug**: pr-4-pr-hygiene-gh-missing-repo-context
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660960143)
- **GitHub Issue**: #10
- **Verdict**: valid
- **Status**: Verified
- **Severity**: high

## Report (verbatim or summarized)

From PR #4 review comment ([PR #4 discussion_r4660960143](https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960143), commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> The `pr-hygiene` job never runs `actions/checkout`, does not set `GH_REPO: ${{ github.repository }}` or pass `--repo`, and only declares `pull-requests: write` without `issues: write` (required by `gh pr edit --add-label` under GitHub's token model). Consequently `gh pr edit` fails unconditionally on every PR, and `2>/dev/null || echo "::warning::..."` masks the error.

## Symptom

The `Auto-Label PR from Branch & Title` step in `.github/workflows/pr-hygiene.yml` fails unconditionally on every pull request because `gh pr edit` has no repository context (neither a checked-out `.git` repository nor `GH_REPO` / `--repo`) and the workflow token lacks `issues: write` permission required to apply labels. Furthermore, `2>/dev/null || echo "::warning::Could not apply label '$label'"` discards all `gh` stderr output, hiding the real failure cause in CI logs.

Expected behavior: `pr-hygiene.yml` should provide `GH_REPO: ${{ github.repository }}` (and/or `--repo`) to `gh pr edit`, grant job-scoped `issues: write` alongside `pull-requests: write` (while keeping top-level `permissions` restricted to `contents: read`), and preserve stderr diagnostics instead of discarding them to `/dev/null`.

## Reproduction

1. Open or update any pull request in `JLG-Technology-LLC/cae-cli` matching a labeling rule (e.g., `feature/*`, `bugfix/*`, `feat: ...`, `fix: ...`, `docs: ...`, or `release/*`).
2. Observe the `PR Hygiene (Commitlint, Git Flow & Auto-Label)` workflow run on `.github/workflows/pr-hygiene.yml`.
3. Because no `actions/checkout` step runs and `GH_REPO` / `--repo` is unset, `gh pr edit "$PR_NUMBER" --add-label "$label"` immediately exits non-zero (`not a git repository` / unable to determine repository), and even with repo context set, `GITHUB_TOKEN` lacks `issues: write` required to mutate PR labels.
4. Observe that `2>/dev/null` suppresses the actual `gh` error message and only prints `::warning::Could not apply label '<label>'`, leaving the PR unlabeled.

## Suspected Code Paths

- `.github/workflows/pr-hygiene.yml:7-9` — Top-level `permissions` sets `contents: read` and `pull-requests: write`, omitting `issues: write` (required by GitHub's token permission model for adding labels to PRs/issues) and applying write scope at workflow level instead of job level.
- `.github/workflows/pr-hygiene.yml:16-57` — The `pr-hygiene` job contains no `actions/checkout` step, so the runner workspace is empty with no `.git` remote context for `gh`.
- `.github/workflows/pr-hygiene.yml:58-63` — `Auto-Label PR from Branch & Title` sets `GH_TOKEN`, `PR_NUMBER`, `HEAD_REF`, and `PR_TITLE` in `env:`, omitting `GH_REPO: ${{ github.repository }}`.
- `.github/workflows/pr-hygiene.yml:82-84` — Runs `gh pr edit "$PR_NUMBER" --add-label "$label" 2>/dev/null || echo "::warning::Could not apply label '$label'"`, lacking `--repo` and discarding stderr with `2>/dev/null`.

## Root Cause Hypothesis

Confidence: **high**.

Three compounding workflow configuration defects cause auto-labeling to fail silently on every PR: (1) `gh pr edit` relies on either a local `.git` remote or `GH_REPO` / `--repo` to identify the target repository, none of which are provided in `pr-hygiene`; (2) GitHub's `GITHUB_TOKEN` requires `issues: write` (in addition to `pull-requests: write`) to add labels to pull requests via `gh pr edit --add-label`; and (3) redirecting stderr to `/dev/null` prevented the failure from being visible in CI logs.

## Proposed Remediation

**Preferred**:
Update `.github/workflows/pr-hygiene.yml` as follows:
1. Keep top-level `permissions:` minimal (`contents: read`) and declare job-scoped `permissions:` on `pr-hygiene`:
   ```yaml
   permissions:
     contents: read
     pull-requests: write
     issues: write
   ```
2. In the `Auto-Label PR from Branch & Title` step `env:` block, add:
   ```yaml
   GH_REPO: ${{ github.repository }}
   ```
3. In the label application loop (lines 82–84), remove `2>/dev/null` (and optionally pass `--repo "$GH_REPO"` explicitly for clarity) so any failure from `gh pr edit` is visible in workflow logs rather than silently swallowed, e.g.:
   ```bash
   for label in "${LABELS[@]}"; do
     if ! gh pr edit "$PR_NUMBER" --repo "$GH_REPO" --add-label "$label"; then
       echo "::warning::Could not apply label '$label' to PR #$PR_NUMBER"
     fi
   done
   ```

**Alternatives**:
- Add an `actions/checkout` step to provide `.git` context — unnecessary overhead (clones the repo solely for `gh` remote detection) when `GH_REPO: ${{ github.repository }}` / `--repo` solves it cleanly with zero checkout latency.
- Use `actions/github-script` (`github.rest.issues.addLabels`) instead of `gh pr edit` — avoids CLI invocation differences, but introduces a third-party action dependency to pin and maintain whereas `gh` is preinstalled on `ubuntu-26.04` runners.

**Files likely to change**:
- `.github/workflows/pr-hygiene.yml`

**Tests to add or update**:
- Validate `.github/workflows/pr-hygiene.yml` YAML syntax and structure (e.g., Python `yaml.safe_load` / `actionlint` if installed).
- Verify static assertions on `.github/workflows/pr-hygiene.yml`: top-level `permissions` has only `contents: read`, `pr-hygiene` job `permissions` includes `issues: write` and `pull-requests: write`, step `env` defines `GH_REPO: ${{ github.repository }}`, and `gh pr edit` no longer redirects stderr to `/dev/null`.
- Shell-check the bash snippet in the auto-label step (e.g., `bash -n`) to confirm valid syntax.

## Risks & Considerations

- **Forked PRs**: On `pull_request` events originating from forks, GitHub automatically downgrades `GITHUB_TOKEN` to read-only regardless of workflow `permissions`. Keeping the non-fatal `if ! gh pr edit ...; then echo "::warning::..."; fi` fallback (without `2>/dev/null`) prevents fork PRs from failing the entire hygiene check while surfacing clear permission/label diagnostics in logs.
- **Missing labels**: If a label name in `LABELS` (`enhancement`, `bug`, `documentation`, `release`) has not yet been synced to the repo via `sync-labels.yml` (all four exist in `.github/labels.yml:1-24`), iterating one label at a time ensures one missing label does not block applying the remaining valid labels.

## Open Questions

None.
