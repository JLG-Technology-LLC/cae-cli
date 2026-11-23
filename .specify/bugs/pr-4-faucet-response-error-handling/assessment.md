# Bug Assessment: pr-4-faucet-response-error-handling

- **Slug**: pr-4-faucet-response-error-handling
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660960706`)
- **Verdict**: valid
- **Status**: Assessed (Awaiting Fix)
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660960706`)
- **GitHub Issue**: #17
- **Related Spec**: None

## Symptom & Reproduction

`request_faucet` in `src/wallet/faucet.rs` fails to reject non-429 HTTP error status codes (400/401/403/500/502/503), falsely rejects 200 OK responses containing `"error": null` or an informational `"message"` field, and discards structured JSON error objects by falling back to `"Unknown faucet error"`.

**Observed**:
1. On HTTP 401/403/500 with a plain-text or HTML body, `request_faucet` skips status validation and fails misleadingly with `CDP Faucet endpoint ... returned non-JSON response` instead of reporting the HTTP status code.
2. On HTTP 400/500 with a JSON body lacking `"error"`/`"message"` (e.g., `{"code":"bad_request","detail":"invalid address"}`), `request_faucet` does not reject the non-2xx status and instead fails with `missing transaction hash` (or could even accept a non-2xx response containing a hash field).
3. On HTTP 200 OK with `{"transaction_hash":"0x...","error":null}` or `{"transaction_hash":"0x...","message":"Dispatched"}`, line 82 treats `"error": null` or `"message"` as a refusal and bails.
4. On structured JSON errors such as `{"error":{"code":"limit","message":"Daily cap reached"}}`, `err.as_str().unwrap_or("Unknown faucet error")` discards the object and prints `"Unknown faucet error"`.

**Expected**:
- HTTP 429 continues to return the dedicated rate-limit guidance.
- Any other non-2xx status (`!status.is_success()`) fails immediately with the HTTP status code and extracted error detail (from structured `error.message` / `error` / `errorMessage` / `message` / serialized JSON / bounded raw body text).
- On 2xx responses, only a present **non-null** `"error"` field fails the request (extracting structured error details cleanly), while `"error": null` and informational `"message"` fields do not cause false failures.

**Reproduction**:
1. Inspect `src/wallet/faucet.rs:71-86`.
2. Serve HTTP `200 OK` with `{"transaction_hash":"0xabc","error":null,"message":"Funds queued"}` from a mock faucet → `request_faucet` bails with `CDP Faucet refused request: Unknown faucet error` (or `Funds queued` if `"error"` is absent).
3. Serve HTTP `500 Internal Server Error` with `Internal Server Error` → `request_faucet` fails with `returned non-JSON response` with no HTTP status code.
4. Serve HTTP `400 Bad Request` with `{"error":{"code":"invalid_address","message":"Address not allowed"}}` → `request_faucet` prints `Unknown faucet error` instead of `Address not allowed`.

## Root Cause Analysis

- **Suspected Files**:
  - `src/wallet/faucet.rs:71-100` (`request_faucet` response status & body handling)
  - `tests/wallet_tests.rs:507-591` (mock faucet integration tests)
- **Cause**:
  - Only `StatusCode::TOO_MANY_REQUESTS` is checked before unconditionally calling `response.json()`.
  - Error detection on parsed JSON combines `res_json.get("error").or_else(|| res_json.get("message"))` without checking `status.is_success()` or `!err.is_null()`, conflating informational 2xx fields and JSON `null` with errors.
  - Extracting the error string with `.as_str().unwrap_or("Unknown faucet error")` only handles JSON string values and drops nested JSON objects/numbers/arrays.
- **Adjacent Risks**:
  - Reading the response body via `response.text().map_err(reqwest::Error::without_url)` once before branching on `!status.is_success()` allows both JSON parsing and safe fallback to a truncated raw body snippet on non-JSON 4xx/5xx error pages, while preventing `reqwest::Error` from leaking secret faucet URLs.
  - Bounds-truncating raw non-JSON error bodies (e.g., to ~256–512 chars on a valid UTF-8 boundary) prevents huge HTML gateway error pages from flooding stderr.
  - Overlap with `pr-4-ipv6-loopback-url-check-broken` and `pr-4-rpc-null-error-and-url-secret-leak`: shares `src/wallet/faucet.rs` and `tests/wallet_tests.rs`, so Stage 2 must schedule them in separate waves.
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `src/wallet/faucet.rs`
- `tests/wallet_tests.rs`

**Steps**:
1. Add a helper `extract_faucet_error(val: &serde_json::Value) -> Option<String>` in `src/wallet/faucet.rs` that extracts a meaningful message from a structured error value:
   - For `"error"` (only when `!is_null()`): return the string directly, or if an object look for string `message` / `errorMessage` / `detail`, otherwise fall back to compact `val.to_string()`.
   - On non-2xx responses, also check top-level `errorMessage`, `message`, and `detail` (when non-null), falling back to the compact JSON or a trimmed/bounded raw text snippet.
2. In `request_faucet` (`src/wallet/faucet.rs:71-86`):
   - Keep the `StatusCode::TOO_MANY_REQUESTS` fast path.
   - Read the response body as text (`response.text().map_err(reqwest::Error::without_url)...`).
   - If `!status.is_success()`, attempt `serde_json::from_str::<serde_json::Value>(&body)` to extract structured error details (or fall back to trimmed/truncated body text) and `bail!("CDP Faucet returned HTTP {}: {}", status, detail)`.
   - On 2xx responses, parse JSON with context (using sanitized URL), fail only when `res_json.get("error").filter(|v| !v.is_null())` is `Some(err)` using the structured extractor, and ignore informational `"message"` fields.
3. Add unit tests in `src/wallet/faucet.rs` for structured error extraction and integration tests in `tests/wallet_tests.rs` covering:
   - HTTP 200 OK with `"error": null` and informational `"message"` (succeeds).
   - HTTP 400/500 with structured `{"error": {"message": "..."}}` (fails with HTTP status and nested message, not `"Unknown faucet error"`).
   - HTTP 502 with plain-text/HTML body (fails with HTTP status code rather than only `"non-JSON response"`).
   - HTTP 429 (preserves rate-limit message).
   Run `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all -- --check`.
