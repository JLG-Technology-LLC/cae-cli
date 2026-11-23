# Bug Assessment: Unauthenticated loopback auth callback allows CSRF wallet hijacking and completes on arbitrary paths

- **Slug**: pr-4-loopback-auth-csrf-hijack
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660959818)
- **GitHub Issue**: #6
- **Verdict**: valid
- **Status**: Verified
- **Severity**: high

## Report (verbatim or summarized)

From PR #4 review comment (`https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959818`, commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> `run_loopback_auth` / `handle_loopback_stream` listens on predictable `127.0.0.1:3000` with no CSRF `state` nonce, no HTTP method or callback route check, and no percent-decoding of query values. Any malicious webpage open in the user's browser can trigger a cross-origin request (`<img src="http://localhost:3000/callback?address=0xATTACKER...&email=a@b.com&session_ref=x">`) to silently hijack `evm_address` in `~/.cae/wallet.json`. Moreover, line 151 only ignores `/`, `/index.html`, and `/favicon.ico` when `query_str` is empty, so any ancillary browser request (`/robots.txt`, `/apple-touch-icon.png`, `/favicon.ico?v=1`) immediately completes auth with fallback email `"candidate@example.com"` and an unspendable burn address.

## Symptom

During `cae wallet init` or `cae wallet login` in browser loopback mode, the CLI accepts any HTTP request to almost any path on `127.0.0.1:3000` without verifying HTTP method, callback route, or a per-session CSRF `state` nonce, and silently fabricates fallback email, session reference, and address values when query parameters are absent or invalid. As a result, any open browser tab can silently overwrite `~/.cae/wallet.json` with an attacker-controlled EVM address via a simple cross-origin request, and routine ancillary browser requests (such as `/favicon.ico?v=1`, `/apple-touch-icon.png`, or `/robots.txt`) prematurely terminate authentication and write a bogus `"candidate@example.com"` profile with an unspendable address.

## Reproduction

1. Run `cae wallet init` (or `CAE_FORCE_BROWSER=1 CAE_SKIP_BROWSER_OPEN=1 CAE_AUTH_PORT=3000 cae wallet init --json`).
2. While the CLI is listening on `127.0.0.1:3000` (2-minute accept loop), trigger either:
   - **CSRF hijack**: `curl "http://127.0.0.1:3000/callback?address=0x1111111111111111111111111111111111111111&email=attacker@evil.test&session_ref=hijacked"` (equivalent to a cross-origin `<img>` tag on any open webpage).
   - **Ancillary path completion**: `curl "http://127.0.0.1:3000/robots.txt"` or `curl "http://127.0.0.1:3000/favicon.ico?v=1"`.
   - **Encoded email corruption**: `curl "http://127.0.0.1:3000/callback?address=0x53d2B9745A17a8c43B06C1e9C3848Ea13F89a27c&email=alice%2Bcae%40example.com&session_ref=cdp_sess_1"`.
3. Observe that `cae wallet init` exits `0` in all three cases: writing the attacker's address (case 1), writing `"candidate@example.com"` with a hash-derived burn address (case 2), or storing literal `"alice%2Bcae%40example.com"` undecoded (case 3).

## Suspected Code Paths

- `src/wallet/auth.rs:63-113` (`run_loopback_auth`) — binds to `127.0.0.1:{port}` (default predictable `3000`), generates no per-flow CSRF `state` nonce, opens `http://localhost:{port}/` with no `state` parameter, and invokes `handle_loopback_stream` without any expected state token.
- `src/wallet/auth.rs:137-200` (`handle_loopback_stream`) — ignores HTTP method (`first_line.split_whitespace().nth(1)` only reads the request target); line 151 only treats `/`, `/index.html`, and `/favicon.ico` with empty `query_str` as non-completing; lines 170-177 parse `query_str` with raw `split('&')`/`split_once('=')` without URL percent-decoding; lines 180-199 accept missing/invalid `email`, `session_ref`, and `address` by falling back to `"candidate@example.com"`, a locally generated `cdp_sess_*`, and `derive_address_for_email(&email)`.
- `src/wallet/auth.rs:290-294` (`crypto_random_bytes`) — calls `getrandom::getrandom(&mut b).unwrap_or_default()`, silently returning 16 zero bytes on RNG failure instead of propagating an error.
- `tests/wallet_tests.rs:598-707` and `tests/e2e_all_commands.rs:704-741` — existing loopback integration tests send `GET /callback?address=...&email=...&session_ref=...` without a `state` parameter and must be updated to read the per-flow `state` nonce.

## Root Cause Hypothesis

`handle_loopback_stream` was implemented as a permissive prototype HTTP handler rather than an authenticated OAuth/loopback callback endpoint: it omitted CSRF `state` generation and constant-time validation, used a narrow blocklist (`/` | `/index.html` | `/favicon.ico` with empty query) instead of an explicit `GET /callback` allowlist, skipped URL percent-decoding, and masked missing parameters with synthetic fallback values. **Confidence: high** (directly verified by inspecting `src/wallet/auth.rs:63-243`).

## Proposed Remediation

**Preferred**:
1. **Per-flow CSPRNG `state` nonce**: Replace `crypto_random_bytes()` with a fallible `generate_random_hex(len) -> Result<String>` backed by `getrandom::getrandom` (never `.unwrap_or_default()`). In `run_loopback_auth`, generate a 32-byte (256-bit) random `state` hex token and include it in the bound portal URL (`http://localhost:{actual_port}/?state={state}`, using `listener.local_addr()?.port()` so ephemeral port `0` also works cleanly). Keep printing this URL to `stderr` so both desktop users (if browser launch fails) and hermetic integration tests can read the per-flow `state` without introducing any production bypass env var.
2. **Strict route, method, and `state` validation in `handle_loopback_stream`**:
   - Guard against empty/truncated reads (`n == 0` → return `Ok(None)`).
   - Parse `(method, target, _version)` from the HTTP request line. Reject any non-`GET` method with `HTTP/1.1 405 Method Not Allowed` and return `Ok(None)`.
   - Split `target` into `(route, query_str)`. If `route == "/" || route == "/index.html"`, serve the landing page (`200 OK`) and return `Ok(None)`.
   - For any route other than `"/callback"` (including `/favicon.ico`, `/favicon.ico?v=1`, `/robots.txt`, `/apple-touch-icon.png`), return `HTTP/1.1 404 Not Found` and `Ok(None)`.
   - Parse query parameters using URL percent-decoding (`url::form_urlencoded::parse` via the already-locked `url = "2.5"` / `form_urlencoded` crates in `Cargo.lock`, or a self-contained RFC 3986 percent-decoder helper if keeping `Cargo.toml` unchanged).
   - Require the `state` query parameter and compare it to `expected_state` in constant time (using `subtle::ConstantTimeEq` already locked in `Cargo.lock` at `2.6.1`, or a constant-time byte-XOR accumulator). On missing or mismatched `state`, return `HTTP/1.1 403 Forbidden` (with an explanatory error page) and return `Ok(None)` so an attacker cannot abort or hijack the active listener.
3. **Fail closed on missing/invalid callback parameters**:
   - Require `address` (or `evm_address`) to be present and pass `crate::wallet::crypto::is_valid_address`.
   - Require `email` to be present (non-empty and containing `@`, or provided via `email_hint`) — never fall back to `"candidate@example.com"`.
   - Require `session_ref` (or `token`) to be present and non-empty — never synthesize a local `cdp_sess_*`.
   - On any missing/invalid parameter, return `HTTP/1.1 400 Bad Request` and return `Ok(None)`.
4. **Update integration tests cleanly without backdoors**:
   - In `tests/wallet_tests.rs` (`cdp_init_loopback_captures_auth_and_persists_embedded_profile`, `cdp_login_backs_up_active_local_wallet`) and `tests/e2e_all_commands.rs` (section 14B), spawn `cae` with `Stdio::piped()` for `stderr`, read the printed URL line from `stderr` to parse `state=<nonce>` (and port), send `GET /callback?address=...&email=...&session_ref=...&state=<nonce>`, and verify exit `0`.

**Alternatives**:
- Accept a test-only `CAE_AUTH_STATE` env var override so existing tests don't have to parse `stderr`. *Trade-off*: Introduces an environment-variable CSRF bypass into the release binary; parsing `state=` from `stderr` is strictly safer and verifies the real random nonce path end-to-end.
- Also bind on an ephemeral port (`127.0.0.1:0`) by default instead of `3000`. *Trade-off*: `SPEC-005` (`specs/005-cdp-wallet-cli/spec.md:14,82,161`) explicitly specifies `http://localhost:3000` for allowlisted CDP redirect origin compatibility; keeping port `3000` as default while enforcing a 256-bit CSPRNG `state` nonce preserves spec compatibility and prevents CSRF hijacking.

**Files likely to change**:
- `src/wallet/auth.rs`
- `Cargo.toml` (optional: promote already-locked `url = "2.5"` and `subtle = "2.6"` from `Cargo.lock` if preferred over in-file percent-decode / constant-time helper; zero network access required)
- `tests/wallet_tests.rs`
- `tests/e2e_all_commands.rs`

**Tests to add or update**:
- Unit tests in `src/wallet/auth.rs` exercising `handle_loopback_stream` over a local `TcpListener` pair for:
  - Missing `state` and wrong `state` → returns `403` and `Ok(None)`.
  - Ancillary paths (`/robots.txt`, `/favicon.ico?v=1`, `/apple-touch-icon.png`) → returns `404` and `Ok(None)`.
  - Non-`GET` method (`POST /callback?...&state=<valid>`) → returns `405` and `Ok(None)`.
  - Missing `address`, invalid `address`, missing `session_ref`, or missing `email` → returns `400` and `Ok(None)` (never synthesizes fallback values).
  - Valid `GET /callback` with percent-encoded `email` (e.g., `alice%2Bcae%40example.com`) and valid `state` → returns `200` and decoded `AuthResult`.
- Update loopback integration tests in `tests/wallet_tests.rs` and `tests/e2e_all_commands.rs` to extract `state=<nonce>` from piped `stderr`, plus an integration assertion that a forged callback without `state` is rejected while the subsequent valid callback with `state` succeeds.

## Risks & Considerations

- **Coordination with `pr-4-fake-cdp-otp-burn-address`**: Both bugs modify `src/wallet/auth.rs` (including removing the `derive_address_for_email` fallback in `handle_loopback_stream`) and `tests/wallet_tests.rs` / `tests/e2e_all_commands.rs`. Fixes should be merged sequentially or cleanly rebased to avoid conflicts.
- **No `#[allow(dead_code)]` / clippy warnings**: Ensure all helpers respect `.opencodereview/rule.json` (`cargo clippy --all-targets -- -D warnings`, no `.unwrap()` in production paths, doc comments on `pub` functions).
- **Buffered `stderr` in tests**: Call `std::io::stderr().flush()` after printing the loopback URL in `run_loopback_auth` so piped test readers receive the `state=` line immediately without deadlock.

## Open Questions

- None — requirements align directly with `SPEC-005` and `Cargo.lock` offline constraints.
