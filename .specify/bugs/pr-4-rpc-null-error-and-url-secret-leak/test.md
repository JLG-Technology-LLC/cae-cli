# Bug Verification: pr-4-rpc-null-error-and-url-secret-leak
- **Slug**: pr-4-rpc-null-error-and-url-secret-leak
- **Tested**: 2026-11-22
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Commit SHA**: `fe8773c`
- **GitHub Issue**: #16
- **Result**: verified
- **Verdict**: verified

## Summary
Verified commit `fe8773c` (`ignore null JSON-RPC errors and redact URLs in error logs`) cleanly resolves `rpc_call fails on JSON-RPC "error": null and leaks private RPC URL credentials in error logs` with scope restricted to `src/wallet/mod.rs`, `src/wallet/balance.rs`, `src/wallet/faucet.rs`, `tests/wallet_tests.rs` and `.specify/bugs/pr-4-rpc-null-error-and-url-secret-leak/`. All Rust unit, E2E, integration, wallet tests, `cargo clippy --offline --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check` passed after the final `cae-cli` edit.

## Checks Performed
| Check | Command / Scope | Outcome |
| --- | --- | --- |
| Commit diff & scope audit | `git show --stat fe8773c` | Pass — isolated 1-commit-per-bug patch (`src/wallet/mod.rs`, `src/wallet/balance.rs`, `src/wallet/faucet.rs`, `tests/wallet_tests.rs`) |
| Formatting gate | `cargo fmt --all -- --check` | Pass — clean |
| Static analysis gate | `cargo clippy --offline --all-targets --all-features -- -D warnings` | Pass — zero warnings |
| Full Rust regression suite | `cargo test --offline --all-targets --all-features` | Pass — 56 unit + 1 E2E + 1 integration + 37 wallet tests (95 passed, 0 failed) |

## Commands Executed
```bash
git -C /Users/jlgtechnology/Developer/cae/cae-cli show --stat fe8773c
cargo fmt --all -- --check
cargo clippy --offline --all-targets --all-features -- -D warnings
cargo test --offline --all-targets --all-features
```

## Commit Diff Stat
```text
fe8773c fix(wallet): ignore null JSON-RPC errors and redact URLs in error logs

 .../assessment.md                                  | 67 +++++++++++++++
 .../pr-4-rpc-null-error-and-url-secret-leak/fix.md | 14 ++++
 .../intake.md                                      | 52 ++++++++++++
 src/wallet/balance.rs                              | 16 ++--
 src/wallet/faucet.rs                               | 31 +++++--
 src/wallet/mod.rs                                  | 43 +++++++++-
 tests/wallet_tests.rs                              | 97 ++++++++++++++++++++++
 7 files changed, 304 insertions(+), 16 deletions(-)
```

## Results
- **Reproduction Check:** Pass
- **Regression Suite:** Pass (`95 passed, 0 failed` across unit, integration, E2E, and wallet suites; `clippy` and `rustfmt` clean)
- **Notes:** Verified offline on local `dev` branch after all 19 `cae-cli` bug fixes landed.
