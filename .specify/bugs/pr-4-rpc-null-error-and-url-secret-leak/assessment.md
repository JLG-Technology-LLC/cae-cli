# Bug Assessment: pr-4-rpc-null-error-and-url-secret-leak

- **Slug**: pr-4-rpc-null-error-and-url-secret-leak
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660960615`)
- **Verdict**: valid
- **Status**: Verified
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660960615`)
- **GitHub Issue**: #16
- **Related Spec**: None

## Symptom & Reproduction

`rpc_call` in `src/wallet/balance.rs` fails on valid JSON-RPC 2.0 responses containing `"error": null`, and both `src/wallet/balance.rs` and `src/wallet/faucet.rs` leak full RPC/faucet URLs—including embedded API keys in path segments, query parameters, or userinfo—in CLI error messages (both via direct interpolation and via the wrapped `reqwest::Error` chain printed by `src/main.rs:149`).

**Observed**:
1. A successful JSON-RPC payload `{"jsonrpc":"2.0","id":1,"result":"0x0","error":null}` trips `if let Some(err) = response.get("error")` and aborts with `RPC error from <url>: null`.
2. When an RPC or faucet request fails against a private endpoint such as `https://base-mainnet.g.alchemy.com/v2/SECRET_KEY` or `https://user:pass@rpc.example.com/?api_key=SECRET`, stderr prints the unredacted secret URL in both the `.with_context(...)` message and the underlying `reqwest::Error` cause (`eprintln!("Error: {:#}", err)` in `src/main.rs:149`).

**Expected**:
1. `rpc_call` ignores `"error": null` and only treats non-null `"error"` fields as RPC errors.
2. All error paths in `balance.rs` and `faucet.rs` display only a sanitized origin (`scheme://host[:port]`, or `<invalid-url>` on parse failure) and strip raw URLs from underlying `reqwest::Error` instances via `reqwest::Error::without_url`.

**Reproduction**:
1. Inspect `src/wallet/balance.rs:113-131` (`rpc_call`) and `src/wallet/faucet.rs:64-85` (`request_faucet`).
2. Return `{"jsonrpc":"2.0","id":1,"result":"0x16e360","error":null}` from a mock JSON-RPC server → `cae wallet balance` exits with code 1 and `RPC error from ...: null`.
3. Run `CAE_RPC_URL_BASE_SEPOLIA="http://user:secretpass@127.0.0.1:1/v2/SECRET_API_KEY?token=SECRET_Q" cae wallet balance` → stderr prints `secretpass`, `SECRET_API_KEY`, and `SECRET_Q` in plaintext.

## Root Cause Analysis

- **Suspected Files**:
  - `src/wallet/balance.rs:113-131` (`rpc_call` — `"error": null` check at line 123, raw `url` interpolation at lines 117, 119, 121, 124, 130, and unwrapped `reqwest::Error` at lines 116, 118, 120)
  - `src/wallet/faucet.rs:64-85` (`request_faucet` — raw `url` interpolation at lines 66-67, 80, and unwrapped `reqwest::Error` at lines 64, 79)
  - `src/wallet/mod.rs` (shared `pub(crate) fn sanitize_url(raw: &str) -> String`)
  - `src/main.rs:149` (`eprintln!("Error: {:#}", err)` prints the full `anyhow` cause chain, exposing `reqwest::Error`'s embedded URL unless stripped)
- **Cause**:
  - `serde_json::Value::get("error")` returns `Some(&Value::Null)` when `"error": null` is present; failing to filter `!v.is_null()` causes false-positive RPC errors against providers that serialize optional `error` as `null`.
  - Both `balance.rs` and `faucet.rs` interpolate the raw `url` string directly into `anyhow` error messages and wrap `reqwest::Error` directly via `.with_context(...)` without calling `.map_err(reqwest::Error::without_url)`. Because `reqwest::Error`'s `Display` output includes `for url (<full_url>)`, sanitizing only the `.with_context(...)` string is insufficient—the underlying `reqwest::Error` still leaks credentials/keys when `main.rs:149` formats `{:#}`.
- **Adjacent Risks**:
  - **Dependency check (`Cargo.toml:24`)**: `reqwest::Url` and `reqwest::Error::without_url` are both available unconditionally in `reqwest 0.12.28` with the current feature set (`blocking`, `json`, `rustls-tls`) — no `Cargo.toml` change is needed.
  - On invalid/unparseable URLs, `sanitize_url` MUST return a fixed placeholder (e.g., `"<invalid-url>"`) rather than echoing the raw input string, because malformed URLs may still contain API keys or credentials.
  - Overlap with `pr-4-ipv6-loopback-url-check-broken` and `pr-4-faucet-response-error-handling`: shares `src/wallet/mod.rs`, `src/wallet/balance.rs`, `src/wallet/faucet.rs`, and `tests/wallet_tests.rs`, so Stage 2 must schedule these bugs in separate waves.
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `src/wallet/mod.rs`
- `src/wallet/balance.rs`
- `src/wallet/faucet.rs`
- `tests/wallet_tests.rs`

**Steps**:
1. Add a shared `pub(crate) fn sanitize_url(raw: &str) -> String` helper in `src/wallet/mod.rs` using `reqwest::Url::parse(raw)`:
   - When parsed with `host_str()`, format `scheme://host` or `scheme://host:port` (stripping userinfo, path, query, and fragment; preserving IPv6 brackets).
   - When parsed without a host, return `format!("{}://<redacted>", parsed.scheme())`; on parse error, return `"<invalid-url>".to_string()`.
2. In `src/wallet/balance.rs:107-131` (`rpc_call`):
   - Compute `let safe_url = super::sanitize_url(url);` and use `safe_url` in all error messages (lines 117, 119, 121, 124, 130).
   - Chain `.map_err(reqwest::Error::without_url)` on `.send()`, `.error_for_status()`, and `.json()` before `.with_context(...)` so the `anyhow` cause chain never includes the full URL.
   - Change line 123 to `if let Some(err) = response.get("error").filter(|v| !v.is_null())`.
3. In `src/wallet/faucet.rs:64-85` (`request_faucet`):
   - Compute `let safe_url = super::sanitize_url(&url);`, use `safe_url` in all error messages, and chain `.map_err(reqwest::Error::without_url)` on `.send()` and `.json()` (or `.text()`).
4. Add unit tests for `sanitize_url` (userinfo, path API key, query token, IPv6 port, invalid URL) and integration tests in `tests/wallet_tests.rs` verifying that:
   - JSON-RPC responses containing `"error": null` succeed for `cae wallet balance`.
   - Failed RPC and faucet calls with credentials/API keys in userinfo, path, and query never leak those secrets to stderr.
   Run `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check`.
