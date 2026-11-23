# Bug Verification: pr-4-readme-unsafe-mainnet-local-example
- **Slug**: pr-4-readme-unsafe-mainnet-local-example
- **Tested**: 2026-11-22
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Commit SHA**: `1c03c38`
- **GitHub Issue**: #22
- **Result**: verified
- **Verdict**: verified

## Summary
Verified commit `1c03c38` (`recommend CDP auth for mainnet and document key security`) cleanly resolves `README recommends unencrypted --local wallet with --force for mainnet` with scope restricted to `README.md` and `.specify/bugs/pr-4-readme-unsafe-mainnet-local-example/`. All Rust unit, E2E, integration, wallet tests, `cargo clippy --offline --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check` passed after the final `cae-cli` edit.

## Checks Performed
| Check | Command / Scope | Outcome |
| --- | --- | --- |
| Commit diff & scope audit | `git show --stat 1c03c38` | Pass — isolated 1-commit-per-bug patch (`README.md`) |
| JSON schema & security docs audit | `python3 JSON block parse on README.md + diff inspection` | Pass — all 3 JSON blocks in `README.md` parse cleanly; mainnet CDP auth, `--force` caveat, and `0600` `.bak` warnings verified |
| Formatting gate | `cargo fmt --all -- --check` | Pass — clean |
| Static analysis gate | `cargo clippy --offline --all-targets --all-features -- -D warnings` | Pass — zero warnings |
| Full Rust regression suite | `cargo test --offline --all-targets --all-features` | Pass — 56 unit + 1 E2E + 1 integration + 37 wallet tests (95 passed, 0 failed) |

## Commands Executed
```bash
git -C /Users/jlgtechnology/Developer/cae/cae-cli show --stat 1c03c38
python3 JSON block parse on README.md + diff inspection
cargo fmt --all -- --check
cargo clippy --offline --all-targets --all-features -- -D warnings
cargo test --offline --all-targets --all-features
```

## Commit Diff Stat
```text
1c03c38 fix(docs): recommend CDP auth for mainnet and document key security

 .../assessment.md                                  | 55 +++++++++++++
 .../fix.md                                         | 95 ++++++++++++++++++++++
 .../intake.md                                      | 51 ++++++++++++
 README.md                                          | 45 +++++++---
 4 files changed, 236 insertions(+), 10 deletions(-)
```

## Results
- **Reproduction Check:** Pass
- **Regression Suite:** Pass (`95 passed, 0 failed` across unit, integration, E2E, and wallet suites; `clippy` and `rustfmt` clean)
- **Notes:** Verified offline on local `dev` branch after all 19 `cae-cli` bug fixes landed.
