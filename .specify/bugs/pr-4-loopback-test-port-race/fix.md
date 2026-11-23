# Bug Fix: pr-4-loopback-test-port-race
- **Status:** Verified
- **Based On:** `.specify/bugs/pr-4-loopback-test-port-race/assessment.md`
- **Commit SHA:** `9c43eb5`
- **GitHub Issue:** `#24`

## Summary of Changes
- `src/wallet/auth.rs`: Resolved `bound_port = listener.local_addr()?.port()` after `TcpListener::bind` so `CAE_AUTH_PORT=0` prints the OS-assigned port, flushed `stderr` immediately after printing the loopback URL, and added `loopback_listener_supports_ephemeral_port_zero` unit test.
- `tests/wallet_tests.rs`: Replaced bind-then-drop TOCTOU port allocation and 300ms-sleeping detached callback threads with `CAE_AUTH_PORT=0`, piped stderr URL discovery (`extract_loopback_url_from_stderr`), HTTP response reading, and `.join()` on callback threads (`cdp_init_loopback_captures_auth_and_persists_embedded_profile`, `cdp_login_backs_up_active_local_wallet`).
- `tests/e2e_all_commands.rs`: Updated section 14B (`cae wallet login --json`) to use `CAE_AUTH_PORT=0`, parse the bound URL from piped stderr, read the HTTP 200 callback response, and join the callback thread.

## Deviation from Assessment
None.
