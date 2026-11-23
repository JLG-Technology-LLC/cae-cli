# Bug Fix: pr-4-wallet-tests-open-real-browser

- **Slug**: pr-4-wallet-tests-open-real-browser
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **Status**: Fixed (Awaiting Verification)
- **Commit SHA**: pending
- **GitHub Issue**: #20

## Summary

Added `.env("CAE_SKIP_BROWSER_OPEN", "1")` to both loopback authentication integration tests (`cdp_init_loopback_captures_auth_and_persists_embedded_profile` and `cdp_login_backs_up_active_local_wallet`) and to `Sandbox::cae` in `tests/wallet_tests.rs` so running `wallet_tests` exercises the loopback HTTP auth flow hermetically without launching a real OS web browser or introducing browser-initiated request races.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `tests/wallet_tests.rs` | modified | Set `CAE_SKIP_BROWSER_OPEN=1` in `Sandbox::cae` (defense-in-depth), `cdp_init_loopback_captures_auth_and_persists_embedded_profile`, and `cdp_login_backs_up_active_local_wallet`. |

## Diff Highlights

- `tests/wallet_tests.rs`:
  - `Sandbox::cae` (lines 20–29): added `.env("CAE_SKIP_BROWSER_OPEN", "1")` so no wallet test invoked through the sandbox helper can launch an OS browser.
  - `cdp_init_loopback_captures_auth_and_persists_embedded_profile` (lines 844–852): added `.env("CAE_SKIP_BROWSER_OPEN", "1")` alongside `.env("CAE_FORCE_BROWSER", "1")`.
  - `cdp_login_backs_up_active_local_wallet` (lines 899–907): added `.env("CAE_SKIP_BROWSER_OPEN", "1")` alongside `.env("CAE_FORCE_BROWSER", "1")`.

## Tests Added or Updated

- `tests/wallet_tests.rs::cdp_init_loopback_captures_auth_and_persists_embedded_profile` — suppresses external browser launch via `CAE_SKIP_BROWSER_OPEN=1` while still testing loopback auth via `CAE_FORCE_BROWSER=1`.
- `tests/wallet_tests.rs::cdp_login_backs_up_active_local_wallet` — suppresses external browser launch via `CAE_SKIP_BROWSER_OPEN=1` while still testing loopback login and local wallet backup.

## Local Verification

- Commands run: `rustfmt --edition 2021 --check /Users/jlgtechnology/Developer/cae/cae-cli/tests/wallet_tests.rs` → exited 0 (formatting clean).
- Deferred to wave coordinator: `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` (deferred per Mode B parallel wave constraint while other Rust files are edited concurrently).

## Deviations from Assessment

- None. Followed the proposed remediation plan strictly, leaving `spawn_mock_faucet` (`#21`) and loopback port/thread logic (`#24`) untouched.

## Follow-ups

- Coordinator wave verification: run `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings`, then commit and proceed to Stage 3 (`/speckit-bug-test slug=pr-4-wallet-tests-open-real-browser`).
