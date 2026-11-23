---
slug: pr-4-sync-labels-pep668-pip-fallback
pr_number: 4
comment_id: 4660960869
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960869"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCeZQ"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: .github/workflows/sync-labels.yml
line: 29
start_line: null
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: pip install pyyaml fallback fails under PEP 668 on ubuntu-26.04

- **Slug**: pr-4-sync-labels-pep668-pip-fallback
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960869
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960869
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `.github/workflows/sync-labels.yml:29`

## Finding

On `ubuntu-26.04`, system Python enforces PEP 668 (`EXTERNALLY-MANAGED`), so the fallback `pip install --quiet pyyaml` fails immediately with `externally-managed-environment` whenever `pyyaml` is absent.

- **Category**: `error handling`
- **Rule**: system / .github/workflows/**/*.{yaml,yml} — Reliability / Correctness

## Diff Context

```diff
+      - name: Apply Declarative Labels
+        env:
+          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
+        run: |
+          python3 -c "import yaml" 2>/dev/null || pip install --quiet pyyaml
```

## Proposed Direction (Not Applied)

Use an ephemeral virtualenv (`python3 -m venv "$RUNNER_TEMP/yaml-venv"`) instead of bare global `pip install`.

## Next Step

Run `/speckit-bug-assess slug=pr-4-sync-labels-pep668-pip-fallback` using this intake file.
