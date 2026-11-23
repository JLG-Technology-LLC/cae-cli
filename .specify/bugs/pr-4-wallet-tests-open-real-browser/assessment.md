# Bug Assessment: pr-4-wallet-tests-open-real-browser

- **Slug**: pr-4-wallet-tests-open-real-browser
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660960941`)
- **Verdict**: valid
- **Status**: Assessed (Awaiting Fix)
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660960941`)
- **GitHub Issue**: #20
- **Related Spec**: None

## Symptom & Reproduction

Running `cargo test --test wallet_tests` launches real OS web browser tabs pointing to `http://localhost:<port>/` on developer workstations during the two loopback authentication integration tests, violating hermetic test isolation (Constitution Principle IV) and introducing a race between the real browser's requests and the test's synthetic callback thread.

**Observed**: Both `cdp_init_loopback_captures_auth_and_persists_embedded_profile` and `cdp_login_backs_up_active_local_wallet` set `CAE_FORCE_BROWSER=1` without setting `CAE_SKIP_BROWSER_OPEN=1`, causing `cae` to invoke `open` (macOS), `xdg-open` (Linux), or `cmd /c start` (Windows).
**Expected**: Integration tests exercise the loopback HTTP listener hermetically without launching an external browser process.

**Reproduction**:
1. Run `cargo test --test wallet_tests cdp_init_loopback_captures_auth_and_persists_embedded_profile` on a workstation with a default browser configured.
2. Observe a real browser window/tab open to `http://localhost:<ephemeral_port>/`.
3. Repeat for `cdp_login_backs_up_active_local_wallet`.

## Root Cause Analysis

- **Suspected Files**:
  - `tests/wallet_tests.rs:626-633` (`cdp_init_loopback_captures_auth_and_persists_embedded_profile`)
  - `tests/wallet_tests.rs:680-687` (`cdp_login_backs_up_active_local_wallet`)
  - `src/wallet/auth.rs:30-36`, `src/wallet/auth.rs:84`, `src/wallet/auth.rs:115-135` (`authenticate_user`, `run_loopback_auth`, `open_browser`)
- **Cause**: `authenticate_user` (`src/wallet/auth.rs:30-36`) bypasses non-TTY headless detection when `CAE_FORCE_BROWSER=1` and enters `run_loopback_auth`. Inside `run_loopback_auth` (`src/wallet/auth.rs:84`), `open_browser(&url)` (`src/wallet/auth.rs:115-135`) spawns the platform browser launcher unless `CAE_SKIP_BROWSER_OPEN` is `"1"` or `"true"`. Unlike `tests/e2e_all_commands.rs:110` (which sets `CAE_SKIP_BROWSER_OPEN=1` in `TestHarness::cmd`), the two loopback tests in `tests/wallet_tests.rs` construct `Command::new(env!("CARGO_BIN_EXE_cae"))` directly and omit `CAE_SKIP_BROWSER_OPEN=1`.
- **Adjacent Risks**:
  - When a real browser opens `http://localhost:<port>/`, it issues background requests (e.g., `/favicon.ico`, `/apple-touch-icon.png`, `/robots.txt`) that race with the test's callback thread; under the current `handle_loopback_stream` routing (`src/wallet/auth.rs:151`), unrecognized paths can prematurely complete authentication with fallback values and fail test assertions.
  - On headless Linux CI machines without `xdg-open`, `open_browser` fails silently (the `Result` is discarded via `let _ = open_browser(&url);`), masking the missing env var in CI while polluting developer machines locally.
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `tests/wallet_tests.rs`

**Steps**:
1. Add `.env("CAE_SKIP_BROWSER_OPEN", "1")` to the `Command` invocations in both `cdp_init_loopback_captures_auth_and_persists_embedded_profile` (`tests/wallet_tests.rs:626-633`) and `cdp_login_backs_up_active_local_wallet` (`tests/wallet_tests.rs:680-687`), and optionally set `CAE_SKIP_BROWSER_OPEN=1` in `Sandbox::cae` as defense-in-depth so future wallet tests cannot accidentally launch a browser.
2. Re-run `cargo test --test wallet_tests` and verify both loopback tests pass without spawning an OS browser process.
