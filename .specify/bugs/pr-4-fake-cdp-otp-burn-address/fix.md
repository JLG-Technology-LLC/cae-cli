# Bug Fix: pr-4-fake-cdp-otp-burn-address
- **Status:** Fixed (Awaiting Verification)
- **Based On:** `.specify/bugs/pr-4-fake-cdp-otp-burn-address/assessment.md`
- **Commit SHA:** `pending`
- **GitHub Issue:** `#7`

## Summary of Changes
- `src/wallet/auth.rs`: Deleted `derive_address_for_email` and its unit test completely; rewrote `run_terminal_otp` to fail closed when `CAE_CDP_AUTH_URL` is unset (directing users to `cae wallet init --local`) and, when configured, verify OTP via `POST /otp/initiate` and `POST /otp/verify` using `reqwest::blocking::Client` (with `.no_proxy()` for loopback, `sanitize_url`/`without_url` error redaction, strict `evm_address` checksum normalization, and non-empty `session_ref` validation); generalized `random_bytes` / `random_bytes_with` for CSPRNG error-propagation tests.
- `tests/wallet_tests.rs`: Added `spawn_mock_cdp_auth`, updated `cdp_init_terminal_otp` to verify the server-returned checksummed address and session ref, and added `cdp_init_terminal_otp_without_endpoint_fails_closed` and `cdp_init_terminal_otp_rejects_bad_otp_and_malformed_server_address`.
- `tests/e2e_all_commands.rs`: Added `spawn_mock_cdp_auth` and wired section 14A to `CAE_CDP_AUTH_URL` asserting exact server-returned `evm_address`.

## Deviation from Assessment
None.
