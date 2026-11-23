# Bug Assessment: `sync-labels.yml` `pip install pyyaml` Fallback Fails Under PEP 668 on `ubuntu-26.04`

- **Slug**: pr-4-sync-labels-pep668-pip-fallback
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660960869)
- **GitHub Issue**: #19
- **Verdict**: valid
- **Severity**: medium

## Report (verbatim or summarized)

From PR #4 review comment ([PR #4 discussion_r4660960869](https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960869), commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> On `ubuntu-26.04`, system Python enforces PEP 668 (`EXTERNALLY-MANAGED`), so the fallback `pip install --quiet pyyaml` fails immediately with `externally-managed-environment` whenever `pyyaml` is absent.

## Symptom

In `.github/workflows/sync-labels.yml:29`, the `Apply Declarative Labels` step attempts `python3 -c "import yaml" 2>/dev/null || pip install --quiet pyyaml` on `ubuntu-26.04`. Because Ubuntu 24.04+ / 26.04 marks the system Python installation as `EXTERNALLY-MANAGED` per PEP 668, invoking bare global `pip install` fails immediately with `error: externally-managed-environment` whenever `pyyaml` is not pre-installed in the runner's system Python, causing label synchronization to fail.

Expected behavior: When `pyyaml` is not already importable by system `python3`, the workflow should create an isolated ephemeral virtual environment (e.g., under `$RUNNER_TEMP/yaml-venv`), install `pyyaml` into it, and execute the Python sync script using that virtual environment's interpreter.

## Reproduction

1. On an `ubuntu-26.04` (or any PEP 668 Debian/Ubuntu) runner where `python3-yaml` / `pyyaml` is not installed in system Python, run line 29 of `.github/workflows/sync-labels.yml`:
   `python3 -c "import yaml" 2>/dev/null || pip install --quiet pyyaml`
2. `python3 -c "import yaml"` exits with code 1 (`ModuleNotFoundError: No module named 'yaml'`).
3. `pip install --quiet pyyaml` executes against the externally managed system Python and aborts with `error: externally-managed-environment` (PEP 668).

## Suspected Code Paths

- `.github/workflows/sync-labels.yml:19` — Targets `runs-on: ubuntu-26.04`, whose system Python enforces PEP 668 (`EXTERNALLY-MANAGED`).
- `.github/workflows/sync-labels.yml:29-30` — Fallback runs bare `pip install --quiet pyyaml` into the system environment and then unconditionally invokes system `python3 - <<'PYEOF'`.

## Root Cause Hypothesis

Confidence: **high**.

Modern Debian/Ubuntu distributions (including `ubuntu-26.04`) ship `/usr/lib/python3.*/EXTERNALLY-MANAGED` in accordance with PEP 668 to prevent `pip` from mutating apt-managed system packages. Calling `pip install --quiet pyyaml` outside a virtual environment is rejected by `pip` unless an isolated venv is created and used.

## Proposed Remediation

**Preferred**:
Replace line 29–30 in `.github/workflows/sync-labels.yml` with an ephemeral virtual environment fallback in `${RUNNER_TEMP:-/tmp}/yaml-venv` and invoke the sync script with the resolved interpreter:
```bash
PYTHON="python3"
if ! "$PYTHON" -c "import yaml" 2>/dev/null; then
  VENV_DIR="${RUNNER_TEMP:-/tmp}/yaml-venv"
  python3 -m venv "$VENV_DIR"
  "$VENV_DIR/bin/pip" install --quiet pyyaml
  PYTHON="$VENV_DIR/bin/python3"
fi
"$PYTHON" - <<'PYEOF'
...
PYEOF
```
This preserves the zero-overhead fast path when `pyyaml` is already available in system `python3`, cleanly complies with PEP 668 when `pyyaml` is absent, avoids mutating global system packages, and guarantees the heredoc runs under the exact interpreter where `pyyaml` was installed.

**Alternatives**:
- Pass `--break-system-packages` to `pip install` — bypasses PEP 668 safety guardrails and risks conflicting with OS-managed packages; discouraged by both PEP 668 and workspace Python dependency practices.
- Replace Python + `pyyaml` with `yq` + shell loop — avoids Python entirely if `yq` is guaranteed on `ubuntu-26.04`, but introduces a dependency on `yq` syntax/version differences (`mikefarah/yq` vs `kislyuk/yq`).

**Files likely to change**:
- `.github/workflows/sync-labels.yml`

**Tests to add or update**:
- Validate `.github/workflows/sync-labels.yml` YAML syntax via parser (`yaml.safe_load` / `actionlint` if available).
- Run `bash -n` on the extracted `run:` shell block and test the venv fallback logic locally (in a temp dir simulating missing `yaml` or invoking `python3 -m venv`) to confirm it resolves `$PYTHON`, parses `.github/labels.yml` cleanly, and contains no bare global `pip install`.

## Risks & Considerations

- **Availability of `python3-venv`**: GitHub-hosted Ubuntu runners include `python3-venv` by default; if ever run on a stripped container without `ensurepip`, `python3 -m venv` would fail explicitly with a clear diagnostic rather than attempting unsafe global writes.
- **No repo context issue**: Unlike `pr-hygiene.yml`, `sync-labels.yml` already runs `actions/checkout` (line 23) and declares `issues: write` (line 13), so `gh label create` already has valid repository context and permissions.

## Open Questions

None.
