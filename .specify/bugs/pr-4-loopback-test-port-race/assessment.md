# Bug Assessment: pr-4-loopback-test-port-race

- **Slug**: pr-4-loopback-test-port-race
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660961122`)
- **Verdict**: valid
- **Status**: Assessed (Awaiting Fix)
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660961122`)
- **GitHub Issue**: #24
- **Related Spec**: None

## Symptom & Reproduction

Loopback authentication integration tests in `tests/wallet_tests.rs` and `tests/e2e_all_commands.rs` allocate a free port by binding `127.0.0.1:0` and immediately dropping the `TcpListener`, then spawn a detached thread (dropping its `JoinHandle`) that sleeps 300ms and sends `GET /callback` without reading the HTTP response. Under parallel `cargo test` runs, another test or process can claim the released port before `cae` binds it (TOCTOU race), causing `run_loopback_auth` to fail and fall back to `run_terminal_otp` (which fails on `Stdio::null()`). Detached thread panics also go unobserved, and `CAE_AUTH_PORT=0` cannot currently be used because `run_loopback_auth` prints the requested `0` instead of `listener.local_addr()?.port()`.

**Observed**:
- Tests pre-bind and drop `127.0.0.1:0` (`tests/wallet_tests.rs:602-604`, `658-660`; `tests/e2e_all_commands.rs:705-707`), creating a TOCTOU port race.
- Callback threads are detached (`std::thread::spawn` return value ignored), sleep an arbitrary 300ms, and drop `TcpStream` immediately after `write_all` without reading the server's HTTP response.
- In `src/wallet/auth.rs:64-81`, passing `CAE_AUTH_PORT=0` binds an OS-assigned ephemeral port, but line 76 formats `url` using the parameter `port` (`0`) instead of `listener.local_addr()?.port()`, printing `http://localhost:0/`.

**Expected**: `cae` supports `CAE_AUTH_PORT=0` by reporting the OS-assigned `listener.local_addr()?.port()` in the loopback URL on `stderr`, and tests discover that bound port (and any CSRF `state` token) from `cae`'s `stderr`, read the HTTP callback response before closing the socket, and `.join()` the callback thread `JoinHandle`.

**Reproduction**:
1. Inspect `tests/wallet_tests.rs:601-624`, `tests/wallet_tests.rs:658-678`, and `tests/e2e_all_commands.rs:704-724`.
2. Inspect `src/wallet/auth.rs:16-21` and `src/wallet/auth.rs:63-85` — pass `CAE_AUTH_PORT=0 CAE_FORCE_BROWSER=1 CAE_SKIP_BROWSER_OPEN=1 cae wallet init --json` and observe `stderr` prints `http://localhost:0/` instead of the kernel-assigned port.
3. Under concurrent `cargo test` execution (or if another process grabs the dropped port before `cae` starts), `run_loopback_auth` fails to bind, `authenticate_user` (`src/wallet/auth.rs:50-58`) falls back to `run_terminal_otp`, and the test fails with `"A valid email address is required for CDP User Auth"` while the detached thread eventually panics unobserved.

## Root Cause Analysis

- **Suspected Files**:
  - `src/wallet/auth.rs:63-85` (`run_loopback_auth`)
  - `tests/wallet_tests.rs:598-650` (`cdp_init_loopback_captures_auth_and_persists_embedded_profile`)
  - `tests/wallet_tests.rs:653-707` (`cdp_login_backs_up_active_local_wallet`)
  - `tests/e2e_all_commands.rs:704-741` (`test_ci_cd_all_commands_and_subcommands_end_to_end`, section 14B)
- **Cause**:
  1. `run_loopback_auth(port, ...)` binds `127.0.0.1:{port}` but never queries `listener.local_addr()?.port()`, so `port == 0` prints `http://localhost:0/` instead of the bound port, forcing tests into a bind-then-drop workaround.
  2. Tests drop the `TcpListener` before spawning `cae` (classic TOCTOU race window while the child process starts) and synchronize via a hardcoded 300ms sleep + retry loop on a detached thread whose `JoinHandle` is dropped.
  3. The callback client writes the HTTP request and immediately drops `TcpStream` without reading the HTTP response, risking a premature `RST` / `BrokenPipe` while `handle_loopback_stream` writes the success HTML page.
- **Adjacent Risks**:
  - Coordinates directly with `pr-4-loopback-auth-csrf-hijack`: once `run_loopback_auth` emits a per-flow CSRF `state` token in the printed URL on `stderr`, discovering both the OS-assigned port and the `state` token from `cae`'s piped `stderr` makes loopback tests completely deterministic and CSRF-compatible.
  - Ensure `eprintln!` in `run_loopback_auth` flushes stderr immediately (line-buffered/flushed) so tests reading piped `stderr` do not block waiting for process exit.
  - Ensure `std::io::stderr()` output in `--json` mode stays on `stderr` (never `stdout`) to uphold Constitution Principle V's dual output contract.
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `src/wallet/auth.rs`
- `tests/wallet_tests.rs`
- `tests/e2e_all_commands.rs`

**Steps**:
1. In `src/wallet/auth.rs` (`run_loopback_auth`), resolve `let actual_port = listener.local_addr().context("Reading bound loopback port")?.port();` after `TcpListener::bind`, use `actual_port` when constructing `url` (and include any CSRF `state` query param if introduced), and flush `stderr` right after printing the URL.
2. Add a unit test in `src/wallet/auth.rs` verifying `run_loopback_auth` works cleanly with port `0` (OS-assigned ephemeral port).
3. Update all three loopback integration tests (`tests/wallet_tests.rs:598-650`, `tests/wallet_tests.rs:653-707`, `tests/e2e_all_commands.rs:704-741`) to:
   - Pass `.env("CAE_AUTH_PORT", "0")` (and `.env("CAE_SKIP_BROWSER_OPEN", "1")`) and spawn `cae` with `Stdio::piped()` for `stdout` and `stderr`.
   - Read `stderr` lines to extract the actual bound port (and `state` token if present) without pre-binding/dropping a socket or sleeping 300ms.
   - Spawn the callback thread, capture its `JoinHandle`, read the HTTP response from `TcpStream` until EOF before closing, and assert `handle.join().expect("callback thread panicked")` after `child.wait_with_output()`.
4. Run `cargo test --all-targets` repeatedly (including parallel runs) to confirm zero flakiness.
