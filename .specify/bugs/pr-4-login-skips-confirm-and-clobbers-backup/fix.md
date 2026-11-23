# Bug Fix: pr-4-login-skips-confirm-and-clobbers-backup
- **Status:** Verified
- **Based On:** `.specify/bugs/pr-4-login-skips-confirm-and-clobbers-backup/assessment.md`
- **Commit SHA:** `63d323a`
- **GitHub Issue:** `#18`

## Summary of Changes
- `src/wallet/mod.rs`: Added `#[arg(long)] pub force: bool` to `LoginArgs`; required `--force` (failing closed in `--json` and non-interactive sessions) before replacing an active `local-eoa` wallet in `handle_login`; deferred `backup_local_profile` in `handle_login` and `backup_profile` in `handle_init` (via `maybe_backup_existing_profile`) until after authentication/keygen succeeds and immediately before `save_profile`.
- `tests/wallet_tests.rs`: Updated `cdp_login_backs_up_active_local_wallet` to pass `--force` alongside `--json`, and added `cdp_login_json_refuses_local_wallet_without_force`, `cdp_login_failed_auth_preserves_existing_local_backup`, and `cdp_init_force_failed_auth_preserves_existing_backup`.

## Deviation from Assessment
None.
