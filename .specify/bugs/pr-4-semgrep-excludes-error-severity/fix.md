# Bug Fix: Include `ERROR` Severity in Semgrep SAST Invocations

- **Slug**: pr-4-semgrep-excludes-error-severity
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **GitHub Issue**: #9
- **Status**: Fixed (Awaiting Verification)
- **Commit SHA**: pending

## Summary

Updated all Semgrep SAST invocations and corresponding documentation across CI, pre-commit, Makefile (both local and Docker fallback paths), and `README.md` to pass `--severity=WARNING --severity=ERROR --error`. Because Semgrep CLI treats `--severity` as an exact-match allowlist filter rather than a minimum threshold, explicitly including `--severity=ERROR` ensures high/critical `ERROR`-severity findings are no longer silently excluded.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `.github/workflows/test.yml` | modified | Updated `semgrep-sast` job step (line 64) to pass `--severity=WARNING --severity=ERROR --error`. |
| `.pre-commit-config.yaml` | modified | Updated `semgrep-sast` local hook entry (line 29) to pass `--severity=WARNING --severity=ERROR --error`. |
| `Makefile` | modified | Updated both local `semgrep` (line 45) and `docker run ... semgrep/semgrep:1.185.0` (line 47) branches in `semgrep-scan` to pass `--severity=WARNING --severity=ERROR --error`. |
| `README.md` | modified | Updated line 503 under `### 1. Semgrep SAST Scanning` to document `--severity=WARNING --severity=ERROR --error`. No other sections were modified. |

## Diff Highlights

```diff
--- a/.github/workflows/test.yml
+++ b/.github/workflows/test.yml
@@ -61,7 +61,7 @@ jobs:
       - name: Run Semgrep SAST Security Gate
-        run: semgrep scan --config auto --severity=WARNING --error
+        run: semgrep scan --config auto --severity=WARNING --severity=ERROR --error
```

```diff
--- a/.pre-commit-config.yaml
+++ b/.pre-commit-config.yaml
@@ -26,7 +26,7 @@ repos:
       - id: semgrep-sast
         name: Semgrep (Rust SAST Security Scan)
-        entry: semgrep scan --config auto --severity=WARNING --error
+        entry: semgrep scan --config auto --severity=WARNING --severity=ERROR --error
```

```diff
--- a/Makefile
+++ b/Makefile
@@ -42,9 +42,9 @@ docker-e2e: docker-build-test
 semgrep-scan:
 	@echo "Running Semgrep SAST security scan for Rust..."
 	@if command -v semgrep >/dev/null 2>&1; then \
-		semgrep scan --config auto --severity=WARNING --error; \
+		semgrep scan --config auto --severity=WARNING --severity=ERROR --error; \
 	else \
-		docker run --rm -v "$(CURDIR):/src" semgrep/semgrep:1.185.0 semgrep scan --config auto --severity=WARNING --error; \
+		docker run --rm -v "$(CURDIR):/src" semgrep/semgrep:1.185.0 semgrep scan --config auto --severity=WARNING --severity=ERROR --error; \
 	fi
```

```diff
--- a/README.md
+++ b/README.md
@@ -500,7 +500,7 @@ Scans Rust source files against automated static analysis rulesets:
-*(Runs `semgrep scan --config auto --severity=WARNING --error`, automatically falling back to Docker if `semgrep` is not locally installed).*
+*(Runs `semgrep scan --config auto --severity=WARNING --severity=ERROR --error`, automatically falling back to Docker if `semgrep` is not locally installed).*
```

## Tests Added or Updated

- Configuration/documentation bug; no unit test files were modified. Verified YAML syntax, Makefile target dry-run expansion, CLI flag semantics, and full repository search for remaining lone `--severity=WARNING` flags.

## Local Verification

- **YAML syntax validation**:
  - Command: `/usr/bin/ruby --disable-gems -ryaml -e 'ARGV.each { |p| d = YAML.load_file(p); puts "OK: #{p} (keys: #{d.keys.join(", ")})" }' .github/workflows/test.yml .pre-commit-config.yaml`
  - Result: Exit `0` — both `.github/workflows/test.yml` and `.pre-commit-config.yaml` parsed cleanly.
- **Makefile dry-run syntax check**:
  - Command: `make -C /Users/jlgtechnology/Developer/cae/cae-cli -n semgrep-scan`
  - Result: Exit `0` — printed both the local `semgrep` branch and the Docker fallback branch with `--severity=WARNING --severity=ERROR --error`.
- **Consistency search**:
  - Command: `rg --hidden --glob '!.git/**' --glob '!.specify/**' "severity=WARNING" /Users/jlgtechnology/Developer/cae/cae-cli`
  - Result: Exit `0` — all 5 occurrences across `.github/workflows/test.yml:64`, `.pre-commit-config.yaml:29`, `Makefile:45`, `Makefile:47`, and `README.md:503` include `--severity=WARNING --severity=ERROR --error`, with zero remaining lone `--severity=WARNING` occurrences.
- **Semgrep CLI help check**:
  - Command: `semgrep scan --help | grep -A 5 -- "--severity"`
  - Result: Exit `0` — confirmed `--severity=ENUM` accepts repeatable values (`INFO`, `WARNING`, `ERROR`) as an exact-match filter.

## Deviations from Assessment

None. Applied the exact preferred remediation across all four files specified in `assessment.md`.

## Follow-ups

- Stage 3 verification (`/speckit-bug-test slug=pr-4-semgrep-excludes-error-severity`) should run Semgrep against a temporary `ERROR`-severity fixture (and against the repository tree if network/ruleset access is available) to verify end-to-end enforcement and confirm no pre-existing `ERROR` findings block CI unexpectedly.
