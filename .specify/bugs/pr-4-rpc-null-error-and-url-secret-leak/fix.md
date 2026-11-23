# Bug Fix: pr-4-rpc-null-error-and-url-secret-leak
- **Status:** Fixed (Awaiting Verification)
- **Based On:** `.specify/bugs/pr-4-rpc-null-error-and-url-secret-leak/assessment.md`
- **Commit SHA:** `pending`
- **GitHub Issue:** `#16`

## Summary of Changes
- `src/wallet/mod.rs`: Added `pub(crate) fn sanitize_url(raw: &str) -> String` using `reqwest::Url::parse` to strip userinfo, path, query, and fragment (returning `"<invalid-url>"` on parse errors and `<scheme>://<redacted>` for hostless URLs), plus unit tests.
- `src/wallet/balance.rs`: Filtered `response.get("error").filter(|v| !v.is_null())` so valid `"error": null` JSON-RPC responses succeed; sanitized URLs via `super::sanitize_url(url)` in all error messages and stripped raw URLs from `reqwest::Error` via `.map_err(reqwest::Error::without_url)`.
- `src/wallet/faucet.rs`: Sanitized URLs via `super::sanitize_url(&url)` in all error contexts and stripped raw URLs from `reqwest::Error` on `.send()`.
- `tests/wallet_tests.rs`: Added `balance_succeeds_when_jsonrpc_error_is_null` and `rpc_and_faucet_errors_redact_url_secrets` integration tests.

## Deviation from Assessment
None.
