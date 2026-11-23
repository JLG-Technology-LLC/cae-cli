# Bug Fix: pr-4-faucet-response-error-handling

- **Slug**: pr-4-faucet-response-error-handling
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **Status**: Fixed (Awaiting Verification)
- **Commit SHA**: pending
- **GitHub Issue**: #17

## Summary

Updated `request_faucet` in `src/wallet/faucet.rs` to read the HTTP response body once, explicitly reject all non-2xx statuses with the HTTP status code and extracted structured/bounded error detail (while preserving the dedicated HTTP 429 rate-limit message), and on 2xx responses only treat a present, non-null `"error"` field as a failure while ignoring informational `"message"` fields. Added structured JSON/text error extraction helpers along with unit and integration tests covering 200 with `"error": null` + `"message": "queued"`, 400/500 structured JSON errors, 502 non-JSON HTML responses, and 429 rate limits.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/wallet/faucet.rs` | modified | Read response text once (stripping URL from `reqwest::Error`), reject `!status.is_success()` with HTTP status + structured/bounded detail, fail 2xx only on non-null `"error"`, add `truncate_snippet`, `format_json_error_value`, `extract_faucet_error`, `extract_http_error_detail`, and unit tests. |
| `tests/wallet_tests.rs` | modified | Added `spawn_mock_faucet_response` helper and integration tests for 200 null-error + informational message, 400/500 structured JSON errors, 502 non-JSON HTML error body, and 429 rate-limit preservation. |

## Diff Highlights

- `src/wallet/faucet.rs`:
  - Preserves the `StatusCode::TOO_MANY_REQUESTS` fast path before reading the body.
  - Reads the response body once via `response.text().map_err(reqwest::Error::without_url)` and branches on `!status.is_success()` to bail with `CDP Faucet returned HTTP {status}: {detail}`.
  - Extracts nested `error.message` / `error.errorMessage` / `error.detail`, direct string `error`, compact JSON fallback, top-level `errorMessage` / `message` / `detail` on non-2xx responses, or a UTF-8-safe bounded raw body snippet (`MAX_ERROR_SNIPPET_CHARS = 256`).
  - On 2xx responses, checks `res_json.get("error").filter(|v| !v.is_null())` via `extract_faucet_error` so `"error": null` and informational `"message"` fields do not trigger false refusals.

## Tests Added or Updated

- `src/wallet/faucet.rs::tests::extract_faucet_error_ignores_null_error_and_informational_message` — verifies `"error": null` and informational `"message"` fields do not fail 2xx responses.
- `src/wallet/faucet.rs::tests::extract_faucet_error_extracts_structured_and_string_errors` — verifies nested `error.message`, plain string `error`, compact JSON object/scalar fallback, and blank string handling.
- `src/wallet/faucet.rs::tests::extract_http_error_detail_handles_structured_json_and_non_json` — verifies non-2xx extraction across structured JSON, `"error": null` + top-level `"message"`, `"detail"`, compact JSON fallback, HTML 502, blank body canonical reason, and multi-byte UTF-8 truncation.
- `tests/wallet_tests.rs::faucet_200_with_null_error_and_informational_message_succeeds` — end-to-end test confirming 200 OK with `"error": null`, `"message": "queued"`, and `transactionHash` succeeds.
- `tests/wallet_tests.rs::faucet_400_and_500_structured_error_reports_http_status_and_message` — end-to-end test confirming HTTP 400 and 500 report status codes and nested `error.message` instead of `"Unknown faucet error"`.
- `tests/wallet_tests.rs::faucet_502_non_json_body_reports_http_status_and_snippet` — end-to-end test confirming non-JSON 502 HTML reports HTTP 502 and body snippet rather than `"non-JSON response"`.
- `tests/wallet_tests.rs::faucet_429_preserves_rate_limit_message` — end-to-end test confirming HTTP 429 preserves the Base Sepolia rate-limit guidance.

## Local Verification

- Commands run: `rustfmt --edition 2021 --check /Users/jlgtechnology/Developer/cae/cae-cli/src/wallet/faucet.rs /Users/jlgtechnology/Developer/cae/cae-cli/tests/wallet_tests.rs` → exited 0 (formatting clean).
- Deferred to wave coordinator: `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` (deferred per Mode B parallel wave constraint while other Rust files are edited concurrently).

## Deviations from Assessment

- Kept URL sanitization (`sanitize_url`), `is_loopback_url`, and the existing `spawn_mock_faucet` TCP loop untouched so changes remain strictly scoped to `#17` and avoid overlapping later-wave fixes (`pr-4-ipv6-loopback-url-check-broken`, `pr-4-rpc-null-error-and-url-secret-leak`, `pr-4-mock-faucet-partial-tcp-read`).

## Follow-ups

- Coordinator wave verification: run `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings`, then commit and proceed to Stage 3 (`/speckit-bug-test slug=pr-4-faucet-response-error-handling`).
