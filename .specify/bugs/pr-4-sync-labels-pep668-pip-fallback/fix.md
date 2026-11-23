# Bug Fix: `sync-labels.yml` Ephemeral Virtualenv Fallback for PEP 668

- **Slug**: pr-4-sync-labels-pep668-pip-fallback
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **Status**: Fixed (Awaiting Verification)
- **Commit SHA**: pending
- **GitHub Issue**: #19

## Summary

Replaced the bare global `pip install --quiet pyyaml` fallback in `.github/workflows/sync-labels.yml` with an ephemeral virtualenv fallback (`"${RUNNER_TEMP:-/tmp}/yaml-venv"`) so `pyyaml` installation succeeds under PEP 668 (`EXTERNALLY-MANAGED`) on `ubuntu-26.04` runners, and updated the inline Python heredoc to execute with the resolved `"$PYTHON"` interpreter.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `.github/workflows/sync-labels.yml` | modified | Replaced `python3 -c "import yaml" 2>/dev/null \|\| pip install --quiet pyyaml` and bare `python3 - <<'PYEOF'` with ephemeral venv fallback (`python3 -m venv "$VENV"`, `"$VENV/bin/python" -m pip install --quiet pyyaml`) and `"$PYTHON" - <<'PYEOF'`. |

## Diff Highlights

```yaml
        run: |
          PYTHON="python3"
          if ! "$PYTHON" -c "import yaml" 2>/dev/null; then
            VENV="${RUNNER_TEMP:-/tmp}/yaml-venv"
            python3 -m venv "$VENV"
            "$VENV/bin/python" -m pip install --quiet pyyaml
            PYTHON="$VENV/bin/python"
          fi
          "$PYTHON" - <<'PYEOF'
```

## Tests Added or Updated

- No dedicated unit test file exists for GitHub Actions workflows in this repository; validated via YAML parsing, `bash -n` syntax validation on the extracted `run:` script, Python AST parsing of the `PYEOF` heredoc, assertion that no bare `pip install` remains, and local `python3 -m venv` + `"$VENV/bin/python" -m pip --version` smoke verification.

## Local Verification

- **YAML syntax** (`/usr/bin/ruby --disable-gems -ryaml`): `YAML.load_file` parsed `.github/workflows/sync-labels.yml` (`name = 'Sync GitHub Labels'`) and `.github/labels.yml` (10 labels) with exit code `0`.
- **Bash syntax** (`bash -n` on extracted `steps[1].run` block): passed with exit code `0`.
- **Bare pip check**: confirmed the only `pip` invocation in `run:` is `"$VENV/bin/python" -m pip install --quiet pyyaml`.
- **Embedded Python syntax** (`ast.parse` on `PYEOF` heredoc): passed with exit code `0`.
- **Venv creation & pip smoke check**: `python3 -m venv <tmpdir>/yaml-venv` succeeded (`Python 3.15.0`; `pip 26.2.2` available via `"$VENV/bin/python" -m pip`).
- **`actionlint` / `yq`**: not installed in local environment (skipped).

## Deviations from Assessment

- Minor refinement over the assessment snippet: used variable `VENV` with `"$VENV/bin/python" -m pip install --quiet pyyaml` and `PYTHON="$VENV/bin/python"` instead of `VENV_DIR` / `"$VENV_DIR/bin/pip"` / `"$VENV_DIR/bin/python3"`. Using `python -m pip` guarantees `pip` installs directly into the exact virtual environment interpreter and avoids wrapper-script shebang issues.

## Follow-ups

- Stage 3 verification (`/speckit-bug-test slug=pr-4-sync-labels-pep668-pip-fallback`) and commit isolation by the coordinator.
