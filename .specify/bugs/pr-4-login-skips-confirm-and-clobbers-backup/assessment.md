# Bug Assessment: `wallet login` Bypasses Confirmation on `--json` and Clobbers Backups Before Auth Succeeds

- **Slug**: pr-4-login-skips-confirm-and-clobbers-backup
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660960788)
- **GitHub Issue**: #18
- **Verdict**: valid
- **Status**: Verified
- **Severity**: medium

## Report (verbatim or summarized)

From PR #4 review comment ([PR #4 discussion_r4660960788](https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960788), commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> Passing `--json` to `cae wallet login` skips the interactive confirmation prompt (`if !args.json && !confirm(&prompt)?`) and silently replaces an active `local-eoa` wallet without requiring `--force`. Moreover, `backup_local_profile(&path)?` (and `backup_profile(&path)?` in `handle_init` line 232) overwrites existing backup files *before* `auth::authenticate_user` succeeds, destroying prior backups if auth fails or times out.

## Symptom

1. Running `cae wallet login --json` when a `local-eoa` wallet is active silently skips confirmation and replaces the active local wallet without requiring an explicit `--force` flag, contradicting the fail-closed non-interactive convention used elsewhere (`cae wallet init --force`, `cae wallet export-key --force`).
2. Both `handle_login` (`wallet.local.bak`) and `handle_init --force` (`wallet.json.bak`) overwrite their respective backup files *before* authentication (`auth::authenticate_user`) or local key generation (`crypto::generate_local_key`) completes. If authentication fails, times out, or is aborted, any previously saved backup file has already been clobbered even though `wallet.json` was not replaced.

Expected behavior: `cae wallet login` should accept a `--force` flag, fail closed in `--json` (and non-interactive) mode when replacing an active `local-eoa` wallet unless `--force` is provided, and both `handle_login` and `handle_init` should defer writing backup files until after authentication / key generation succeeds and immediately before `save_profile`.

## Reproduction

1. **Silent overwrite in `--json` mode**:
   1. Run `CAE_HOME=/tmp/cae-repro cae wallet init --local` to create an active `local-eoa` wallet.
   2. Run `CAE_HOME=/tmp/cae-repro cae wallet login --json` (with stdin closed / non-TTY and loopback or OTP auth).
   3. Observe that `handle_login` evaluates `if !args.json && !confirm(&prompt)?` to `false`, skips confirmation, backs up the local wallet, and replaces it without requiring `--force`.
2. **Premature backup clobbering**:
   1. Create an active `local-eoa` wallet at `$CAE_HOME/wallet.json` and an existing backup at `$CAE_HOME/wallet.local.bak` (or `$CAE_HOME/wallet.json.bak` for `init --force`).
   2. Run `cae wallet login --no-browser --json` (or `cae wallet init --force --no-browser --json`) and supply an invalid email/OTP (or EOF on stdin) so `auth::authenticate_user` returns an error.
   3. Observe that `$CAE_HOME/wallet.local.bak` (or `$CAE_HOME/wallet.json.bak`) has already been overwritten with the current `wallet.json`, destroying the prior backup despite the command failing before `save_profile`.

## Suspected Code Paths

- `src/wallet/mod.rs:124-131` — `LoginArgs` struct lacks a `--force` CLI flag (unlike `InitArgs` at `src/wallet/mod.rs:117-118` and `WalletCommand::ExportKey` at `src/wallet/mod.rs:98-99`).
- `src/wallet/mod.rs:305-326` — `handle_login` guards confirmation with `if !args.json && !confirm(&prompt)?` (line 312), which bypasses confirmation whenever `--json` is passed, and invokes `backup_local_profile(&path)?` at line 315 *before* `auth::authenticate_user(...)` at line 326.
- `src/wallet/mod.rs:224-285` — `handle_init` invokes `backup_profile(&path)?` at line 232 *before* `crypto::generate_local_key()?` (line 243) and `auth::authenticate_user(...)` (line 271), overwriting `wallet.json.bak` prior to knowing whether the new wallet creation/auth will succeed.
- `src/wallet/profile.rs:137-150` — `backup_profile` and `backup_local_profile` atomically replace `wallet.json.bak` and `wallet.local.bak` via `write_private_file`, so any premature call permanently overwrites the prior backup.
- `tests/wallet_tests.rs:653-707` — `cdp_login_backs_up_active_local_wallet` initializes a `local-eoa` wallet (line 655) and then invokes `cae wallet login --json` (line 681) without `--force`; this test relies on the buggy confirmation bypass and must be updated to pass `--force`.
- `tests/e2e_all_commands.rs:727` — Invokes `cae wallet login --json` *after* step 14A (`cae wallet init --force --no-browser --json`, line 677) has already replaced the local wallet with a `cdp-embedded` profile, so `existing.wallet_type` is `CdpEmbedded` (not `LocalEoa`) and this caller does **not** require `--force`.

## Root Cause Hypothesis

Confidence: **high**.

`handle_login` originally used `!args.json` to suppress interactive terminal prompts in machine-readable mode, inadvertently coupling prompt suppression with confirmation bypass instead of failing closed behind an explicit `--force` flag (as `handle_export_key` at `src/wallet/mod.rs:611-619` already does). Additionally, both `handle_init` and `handle_login` placed their backup writes inside the initial precondition validation block rather than deferring the side effect until after fallible key generation and authentication steps complete.

## Proposed Remediation

**Preferred**:
1. Add `#[arg(long)] force: bool` to `LoginArgs` in `src/wallet/mod.rs` with doc comment describing replacement of an active `local-eoa` wallet (matching `InitArgs` and `ExportKey`).
2. In `handle_login` (`src/wallet/mod.rs`):
   - Keep the confirmation / `--force` guard *before* calling `auth::authenticate_user` so unauthorized replacements fail fast without starting an auth flow.
   - Determine `let replacing_local = matches!(existing.as_ref(), Some(p) if p.wallet_type == WalletType::LocalEoa);`.
   - When `replacing_local && !args.force`:
     - If `args.json`, fail closed immediately via `bail!(...)` instructing the caller to re-run with `--force` (avoiding any interactive prompt on stderr/stdin in JSON mode).
     - Otherwise (`!args.json`), call `confirm(&prompt)?`; if `false`, `bail!` with a message noting the local wallet was kept and mentioning `--force`.
   - Remove the premature `backup_local_profile(&path)?` call before `authenticate_user`. Instead, after `auth::authenticate_user(...)` succeeds and immediately before `save_profile(&path, &profile)?`, if `replacing_local`, call `let local_bak = backup_local_profile(&path)?;` and emit the stderr backup notice when `!args.json`.
3. In `handle_init` (`src/wallet/mod.rs`):
   - Keep the `if path.exists() && !args.force { bail!(...); }` precondition check at the top of `handle_init`.
   - Defer `backup_profile(&path)?` (and its `!args.json` stderr notice) until *after* `crypto::generate_local_key()?` (for `--local`) or `auth::authenticate_user(...)` (for CDP) succeeds, immediately before `save_profile(&path, &profile)?` (e.g., via a small helper closure/function called right before `save_profile` when `had_existing`).
4. Update `tests/wallet_tests.rs:681` (`cdp_login_backs_up_active_local_wallet`) to pass `["wallet", "login", "--force", "--json"]` and add new regression tests covering fail-closed rejection without `--force` and backup preservation on failed auth.

**Alternatives**:
- Keep `--json` calling `confirm(&prompt)?` directly without a separate `if args.json` branch — works when stdin is non-TTY, *but* would still emit a `[y/N]` prompt and block on stdin if a user runs `cae wallet login --json` from an interactive terminal. Explicitly failing closed on `args.json && !args.force` keeps `--json` strictly non-interactive and predictable for scripts/agents (Constitution Principle V).

**Files likely to change**:
- `src/wallet/mod.rs`
- `tests/wallet_tests.rs`

**Tests to add or update**:
- Update `cdp_login_backs_up_active_local_wallet` in `tests/wallet_tests.rs` to pass `--force` (`["wallet", "login", "--force", "--json"]`) when replacing an active `local-eoa` wallet.
- Add test `cdp_login_json_refuses_local_wallet_without_force` in `tests/wallet_tests.rs`: initialize a `local-eoa` wallet, run `cae wallet login --json` without `--force`, assert non-zero exit, stderr mentions `--force`, `wallet.json` remains unchanged (`local-eoa`), and `wallet.local.bak` is not created.
- Add test `cdp_login_failed_auth_preserves_existing_local_backup` in `tests/wallet_tests.rs`: initialize a `local-eoa` wallet, seed a sentinel `wallet.local.bak`, run `cae wallet login --force --no-browser --json` with invalid OTP/EOF so auth fails, and assert `wallet.local.bak` still contains the original sentinel content and `wallet.json` is unchanged.
- Add test `cdp_init_force_failed_auth_preserves_existing_backup` in `tests/wallet_tests.rs`: initialize a wallet, seed a sentinel `wallet.json.bak`, run `cae wallet init --force --no-browser --json` with failing OTP/EOF, and assert `wallet.json.bak` retains its original sentinel content and `wallet.json` is unchanged.

## Risks & Considerations

- **CLI behavior change**: Automated callers invoking `cae wallet login --json` over an existing `local-eoa` wallet will now fail closed unless they pass `--force`. Callers re-authenticating an existing `cdp-embedded` wallet (such as `tests/e2e_all_commands.rs:727`) or logging in with no existing wallet are unaffected.
- **Constitution compliance**: Preserves hermetic tempdir tests (Principle IV), dual human/`--json` output with `anyhow::Result` errors on stderr (Principle V), and local-first atomic persistence (Principle I).
- **Zero-warning gate**: Ensure `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` all pass.

## Open Questions

None.
