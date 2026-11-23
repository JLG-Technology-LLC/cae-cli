# Bug Verification: pr-4-semgrep-excludes-error-severity
- **Slug**: pr-4-semgrep-excludes-error-severity
- **Tested**: 2026-11-22
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Commit SHA**: `0e47394`
- **GitHub Issue**: #9
- **Result**: verified
- **Verdict**: verified

## Summary
Verified commit `0e47394` (`include ERROR severity in Semgrep SAST scans`) cleanly resolves `Semgrep SAST invocations filter to --severity=WARNING only, excluding ERROR (High/Critical) findings` with scope restricted to `.github/workflows/test.yml`, `.pre-commit-config.yaml`, `Makefile`, `README.md` and `.specify/bugs/pr-4-semgrep-excludes-error-severity/`. All Rust unit, E2E, integration, wallet tests, `cargo clippy --offline --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check` passed after the final `cae-cli` edit.

## Checks Performed
| Check | Command / Scope | Outcome |
| --- | --- | --- |
| Commit diff & scope audit | `git show --stat 0e47394` | Pass — isolated 1-commit-per-bug patch (`.github/workflows/test.yml`, `.pre-commit-config.yaml`, `Makefile`, `README.md`) |
| YAML, Makefile & README consistency | `make -n semgrep-scan && ruby --disable-gems -ryaml ...` | Pass — `test.yml`, `.pre-commit-config.yaml`, `Makefile`, and `README.md` all include `--severity=WARNING --severity=ERROR --error` |
| Formatting gate | `cargo fmt --all -- --check` | Pass — clean |
| Static analysis gate | `cargo clippy --offline --all-targets --all-features -- -D warnings` | Pass — zero warnings |
| Full Rust regression suite | `cargo test --offline --all-targets --all-features` | Pass — 56 unit + 1 E2E + 1 integration + 37 wallet tests (95 passed, 0 failed) |

## Commands Executed
```bash
git -C /Users/jlgtechnology/Developer/cae/cae-cli show --stat 0e47394
make -n semgrep-scan && ruby --disable-gems -ryaml ...
cargo fmt --all -- --check
cargo clippy --offline --all-targets --all-features -- -D warnings
cargo test --offline --all-targets --all-features
```

## Commit Diff Stat
```text
0e47394 fix(security): include ERROR severity in Semgrep SAST scans

 .github/workflows/test.yml                         |  2 +-
 .pre-commit-config.yaml                            |  2 +-
 .../assessment.md                                  | 74 +++++++++++++++++
 .../pr-4-semgrep-excludes-error-severity/fix.md    | 92 ++++++++++++++++++++++
 .../pr-4-semgrep-excludes-error-severity/intake.md | 51 ++++++++++++
 Makefile                                           |  4 +-
 README.md                                          |  2 +-
 7 files changed, 222 insertions(+), 5 deletions(-)
```

## Results
- **Reproduction Check:** Pass
- **Regression Suite:** Pass (`95 passed, 0 failed` across unit, integration, E2E, and wallet suites; `clippy` and `rustfmt` clean)
- **Notes:** Verified offline on local `dev` branch after all 19 `cae-cli` bug fixes landed.
