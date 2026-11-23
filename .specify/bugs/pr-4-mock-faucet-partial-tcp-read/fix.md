# Bug Fix: pr-4-mock-faucet-partial-tcp-read
- **Status:** Fixed (Awaiting Verification)
- **Based On:** `.specify/bugs/pr-4-mock-faucet-partial-tcp-read/assessment.md`
- **Commit SHA:** `pending`
- **GitHub Issue:** `#21`

## Summary of Changes
- `tests/wallet_tests.rs`: Added `read_http_request` (with a 5-second read timeout and raw-byte `\r\n\r\n` + `Content-Length` framing loop), updated `spawn_mock_faucet` and `spawn_mock_faucet_response` to share it and flush responses, and added `mock_faucet_captures_split_tcp_post_body`.
- `tests/e2e_all_commands.rs`: Added `read_http_request` and updated `spawn_mock_faucet` to read the complete HTTP request before responding and flushing.

## Deviation from Assessment
None.
