# Bug Verification: pr-4-readme-schema-and-faq-mismatches
- **Slug**: pr-4-readme-schema-and-faq-mismatches
- **Tested**: 2026-11-22
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Commit SHA**: `24946f2`
- **GitHub Issue**: #23
- **Result**: verified
- **Verdict**: verified

## Summary
Verified commit `24946f2` (`align balance and ledger JSON schemas and headless FAQ`) cleanly resolves `README balance JSON, project ledger JSON, and CAE_SKIP_BROWSER_OPEN FAQ mismatch actual CLI behavior` with scope restricted to `README.md` and `.specify/bugs/pr-4-readme-schema-and-faq-mismatches/`. All Rust unit, E2E, integration, wallet tests, `cargo clippy --offline --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check` passed after the final `cae-cli` edit.

## Checks Performed
| Check | Command / Scope | Outcome |
| --- | --- | --- |
| Commit diff & scope audit | `git show --stat 24946f2` | Pass — isolated 1-commit-per-bug patch (`README.md`) |
| JSON schema & FAQ alignment | `python3 JSON block parse on README.md + struct field comparison` | Pass — `wallet balance --json`, `ProjectLedger` JSON schemas, and `CAE_SKIP_BROWSER_OPEN` vs `--no-browser` FAQ match Rust code |
| Formatting gate | `cargo fmt --all -- --check` | Pass — clean |
| Static analysis gate | `cargo clippy --offline --all-targets --all-features -- -D warnings` | Pass — zero warnings |
| Full Rust regression suite | `cargo test --offline --all-targets --all-features` | Pass — 56 unit + 1 E2E + 1 integration + 37 wallet tests (95 passed, 0 failed) |

## Commands Executed
```bash
git -C /Users/jlgtechnology/Developer/cae/cae-cli show --stat 24946f2
python3 JSON block parse on README.md + struct field comparison
cargo fmt --all -- --check
cargo clippy --offline --all-targets --all-features -- -D warnings
cargo test --offline --all-targets --all-features
```

## Commit Diff Stat
```text
24946f2 fix(docs): align balance and ledger JSON schemas and headless FAQ

 .../assessment.md                                  | 63 +++++++++++++++
 .../pr-4-readme-schema-and-faq-mismatches/fix.md   | 93 ++++++++++++++++++++++
 .../intake.md                                      | 57 +++++++++++++
 README.md                                          | 78 ++++++++++++++----
 4 files changed, 277 insertions(+), 14 deletions(-)
```

## Results
- **Reproduction Check:** Pass
- **Regression Suite:** Pass (`95 passed, 0 failed` across unit, integration, E2E, and wallet suites; `clippy` and `rustfmt` clean)
- **Notes:** Verified offline on local `dev` branch after all 19 `cae-cli` bug fixes landed.
