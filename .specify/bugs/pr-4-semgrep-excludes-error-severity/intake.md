---
slug: pr-4-semgrep-excludes-error-severity
pr_number: 4
comment_id: 4660960080
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960080"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCbUA"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: high
file: .github/workflows/test.yml
line: 64
start_line: null
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Semgrep SAST invocations filter to --severity=WARNING only, excluding ERROR (High/Critical) findings

- **Slug**: pr-4-semgrep-excludes-error-severity
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960080
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960080
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: High
- **Location**: `.github/workflows/test.yml:64`

## Finding

In Semgrep CLI, `--severity` is an exact-match allowlist filter rather than a minimum threshold. Passing only `--severity=WARNING` suppresses all `ERROR`-severity (highest-severity security) findings across `.github/workflows/test.yml:64`, `.pre-commit-config.yaml:29`, `Makefile:45-47`, and `README.md:503`, so CI, pre-commit, and `make security` silently exit 0 even when critical `ERROR`-level vulnerabilities are introduced.

- **Category**: `security`
- **Rule**: system / .github/workflows/**/*.{yaml,yml} — Correctness & Security

## Diff Context

```diff
+      - name: Run Semgrep SAST Security Gate
+        run: semgrep scan --config auto --severity=WARNING --error
```

## Proposed Direction (Not Applied)

Pass `--severity=WARNING --severity=ERROR` in `.github/workflows/test.yml`, `.pre-commit-config.yaml`, `Makefile` (both local and Docker branches), and `README.md`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-semgrep-excludes-error-severity` using this intake file.
