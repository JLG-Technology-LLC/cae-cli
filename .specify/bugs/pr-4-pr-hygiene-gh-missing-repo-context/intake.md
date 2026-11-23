---
slug: pr-4-pr-hygiene-gh-missing-repo-context
pr_number: 4
comment_id: 4660960143
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960143"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCbjw"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: high
file: .github/workflows/pr-hygiene.yml
line: 83
start_line: 59
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: PR auto-labeling fails without repo checkout or GH_REPO, lacks issues:write, and swallows stderr

- **Slug**: pr-4-pr-hygiene-gh-missing-repo-context
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960143
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960143
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: High
- **Location**: `.github/workflows/pr-hygiene.yml:59-83`

## Finding

The `pr-hygiene` job never runs `actions/checkout`, does not set `GH_REPO: ${{ github.repository }}` or pass `--repo`, and only declares `pull-requests: write` without `issues: write` (required by `gh pr edit --add-label` under GitHub's token model). Consequently `gh pr edit` fails unconditionally on every PR, and `2>/dev/null || echo "::warning::..."` masks the error.

- **Category**: `correctness`
- **Rule**: system / .github/workflows/**/*.{yaml,yml} — Best Practices (error swallowing) & Correctness

## Diff Context

```diff
+      - name: Auto-Label PR from Branch & Title
+        env:
+          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
+          PR_NUMBER: ${{ github.event.pull_request.number }}
...
+          for label in "${LABELS[@]}"; do
+            gh pr edit "$PR_NUMBER" --add-label "$label" 2>/dev/null || echo "::warning::Could not apply label '$label'"
+          done
```

## Proposed Direction (Not Applied)

Add `GH_REPO: ${{ github.repository }}` to `env:`, add `issues: write` to job-scoped `permissions` (keeping top-level `contents: read`), and stop discarding stderr with `2>/dev/null`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-pr-hygiene-gh-missing-repo-context` using this intake file.
