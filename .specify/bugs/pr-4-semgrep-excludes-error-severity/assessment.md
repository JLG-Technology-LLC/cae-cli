# Bug Assessment: Semgrep SAST Invocations Exclude `ERROR` (High/Critical) Severity Findings

- **Slug**: pr-4-semgrep-excludes-error-severity
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660960080)
- **GitHub Issue**: #9
- **Verdict**: valid
- **Status**: Verified
- **Severity**: high

## Report (verbatim or summarized)

From PR #4 review comment ([PR #4 discussion_r4660960080](https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960080), commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> In Semgrep CLI, `--severity` is an exact-match allowlist filter rather than a minimum threshold. Passing only `--severity=WARNING` suppresses all `ERROR`-severity (highest-severity security) findings across `.github/workflows/test.yml:64`, `.pre-commit-config.yaml:29`, `Makefile:45-47`, and `README.md:503`, so CI, pre-commit, and `make security` silently exit 0 even when critical `ERROR`-level vulnerabilities are introduced.

## Symptom

Every Semgrep SAST invocation in the repository passes `--severity=WARNING --error` without `--severity=ERROR`. Because Semgrep treats `--severity` as an exact-match allowlist rather than a minimum-severity threshold, CI (`semgrep-sast` job), local `pre-commit` hooks, and `make semgrep-scan` / `make security` silently filter out all `ERROR`-severity findings (the highest-severity security rules) and exit `0` even when critical SAST vulnerabilities are present.

Expected behavior: All Semgrep SAST gates should enforce at least both `WARNING` and `ERROR` severity findings (`--severity=WARNING --severity=ERROR`) and exit non-zero via `--error` when either is detected.

## Reproduction

1. Inspect the Semgrep command flags across `.github/workflows/test.yml:64`, `.pre-commit-config.yaml:29`, `Makefile:45`, `Makefile:47`, and `README.md:503`.
2. Notice each command runs `semgrep scan --config auto --severity=WARNING --error`.
3. Run `semgrep scan --config <rule-with-severity-ERROR> --severity=WARNING --error` against a snippet triggering an `ERROR`-severity rule; Semgrep filters out the rule because its severity (`ERROR`) is not in the `--severity` allowlist (`[WARNING]`) and exits `0`.
4. Re-run with `--severity=WARNING --severity=ERROR --error`; Semgrep reports the finding and exits non-zero.

## Suspected Code Paths

- `.github/workflows/test.yml:64` — `semgrep-sast` CI job runs `semgrep scan --config auto --severity=WARNING --error` inside `semgrep/semgrep:1.185.0`, excluding all `ERROR`-severity findings from the PR/push security gate.
- `.pre-commit-config.yaml:29` — `semgrep-sast` local pre-commit hook runs `semgrep scan --config auto --severity=WARNING --error`.
- `Makefile:45` — `semgrep-scan` local binary branch runs `semgrep scan --config auto --severity=WARNING --error`.
- `Makefile:47` — `semgrep-scan` Docker fallback branch runs `docker run --rm -v "$(CURDIR):/src" semgrep/semgrep:1.185.0 semgrep scan --config auto --severity=WARNING --error`.
- `README.md:503` — Security documentation records the outdated command `semgrep scan --config auto --severity=WARNING --error`.

## Root Cause Hypothesis

Confidence: **high**.

The CI workflow, pre-commit hook, Makefile, and README were authored under the assumption that Semgrep's `--severity=WARNING` acts as a minimum-severity floor (similar to log levels or tools that accept a threshold). In Semgrep CLI, however, `--severity` is a repeatable exact-match filter: specifying only `--severity=WARNING` restricts rule execution/reporting strictly to `WARNING`-level rules and drops `ERROR`-level rules entirely.

## Proposed Remediation

**Preferred**:
Update all Semgrep scan invocations and documentation to pass both `--severity=WARNING --severity=ERROR`:
1. `.github/workflows/test.yml:64`: change to `semgrep scan --config auto --severity=WARNING --severity=ERROR --error`.
2. `.pre-commit-config.yaml:29`: change to `semgrep scan --config auto --severity=WARNING --severity=ERROR --error`.
3. `Makefile:45` and `Makefile:47`: update both the local `semgrep` branch and the `docker run ... semgrep/semgrep:1.185.0` branch to pass `--severity=WARNING --severity=ERROR --error`.
4. `README.md:503`: update the documented command to ``semgrep scan --config auto --severity=WARNING --severity=ERROR --error``.

**Alternatives**:
- Omit `--severity` altogether so Semgrep runs all severities (including `INFO`/`LOW`) — broader coverage, but may introduce noisy informational findings that break CI on style/advisory rules.
- Additionally pass modern severity aliases (`--severity=MEDIUM --severity=HIGH --severity=CRITICAL`) alongside `WARNING`/`ERROR` — verify against `semgrep/semgrep:1.185.0` CLI help whether `ERROR` already maps to `HIGH`/`CRITICAL` or if modern rule severities require explicit inclusion.

**Files likely to change**:
- `.github/workflows/test.yml`
- `.pre-commit-config.yaml`
- `Makefile`
- `README.md`

**Tests to add or update**:
- Verify all four files consistently specify `--severity=WARNING --severity=ERROR` with no remaining lone `--severity=WARNING` occurrences (`rg --hidden "severity=WARNING" .`).
- Validate YAML syntax for `.github/workflows/test.yml` and `.pre-commit-config.yaml`.
- If `semgrep` (or Docker with `semgrep/semgrep:1.185.0`) is available in the verification environment, run `make semgrep-scan` (or `semgrep scan --help`) to confirm `--severity=WARNING --severity=ERROR --error` is accepted and exits 0 on the current tree (and fails on a temporary `ERROR`-severity fixture).

## Risks & Considerations

- **Newly surfaced findings**: Enabling `ERROR`-severity rules may immediately flag existing `ERROR`-level issues in the repository on the next CI run; Stage 3 verification should confirm whether any existing code triggers `ERROR` findings under `--config auto`.
- **File overlap with README PR-4 bugs**: `README.md` is also touched by other PR #4 bug slugs; confine the edit in `README.md` strictly to line 503 to avoid merge conflicts.

## Open Questions

None.
