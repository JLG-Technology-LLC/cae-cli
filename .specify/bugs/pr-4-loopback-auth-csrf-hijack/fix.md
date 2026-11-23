# Bug Fix: pr-4-loopback-auth-csrf-hijack
- **Status:** Verified
- **Based On:** `.specify/bugs/pr-4-loopback-auth-csrf-hijack/assessment.md`
- **Commit SHA:** `dc8ed84`
- **GitHub Issue:** `#6`

## Summary of Changes
- `Cargo.toml` & `Cargo.lock`: Promoted already-locked `subtle = "2.6"` and `url = "2.5"` into `[dependencies]` (offline-safe).
- `src/wallet/auth.rs`: Generated a per-flow 256-bit CSPRNG `state` token (`generate_state_token`) included in the loopback URL; hardened `handle_loopback_stream` to enforce `GET`, serve landing page only on `/` or `/index.html`, return `404` for non-`/callback` routes, percent-decode query strings via `url::form_urlencoded::parse`, verify `state` in constant time (`subtle::ConstantTimeEq`, returning `403` on mismatch/missing), fail closed (`400`) on missing/invalid `email`, `session_ref`, or `address` without fabricating fallback values, and added `loopback_rejects_csrf_ancillary_paths_non_get_and_missing_params` unit tests.
- `tests/wallet_tests.rs` & `tests/e2e_all_commands.rs`: Updated loopback integration tests to parse `state=` from piped stderr, verify CSRF `403` and `/robots.txt` `404` rejections, and verify percent-encoded email decoding.

## Deviation from Assessment
None.
