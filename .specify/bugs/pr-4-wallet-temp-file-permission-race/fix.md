# Bug Fix: Predictable temp path and non-exclusive open bypass 0600 permissions and allow symlink/race attacks

- **Slug**: pr-4-wallet-temp-file-permission-race
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **Status**: Verified
- **Commit SHA**: `3ff6e51`
- **GitHub Issue**: #11

## Summary

Replaced the predictable `<path>.tmp<pid>` temporary filename and non-exclusive `create(true).truncate(true)` open in `src/wallet/profile.rs` with cryptographically random 16-byte hex temp paths opened exclusively (`create_new(true)` / `O_CREAT | O_EXCL`), explicit descriptor-level `0o600` permission enforcement (`file.set_permissions`), and owner-only (`0o700`) creation for newly created parent directories on Unix. Added unit tests covering `0o600` file permissions, `0o700` parent directory creation, symlink and pre-existing temp file refusal, and concurrent writes without collision.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/wallet/profile.rs` | modified | Replaced `write_private_file` / `open_private` with `create_private_dir` (`0o700` on Unix via `DirBuilderExt`), `create_private_temp` (16-byte `getrandom` hex suffix with up to 8 `AlreadyExists` retries), exclusive `open_private` (`create_new(true)` + `.mode(0o600)` + fd-level `file.set_permissions(from_mode(0o600))`), and `write_and_replace` (write, `sync_all`, close handle, atomic `fs::rename`, cleanup only on post-create errors). Made `write_private_file` `pub(crate)`. |
| `src/wallet/profile.rs` | added tests | Added 4 unit tests in `#[cfg(test)] pub(crate) mod tests` covering `0o700` parent directories, pre-existing predictable `.tmp<pid>` `0o644` files, pre-existing live/dangling symlinks and regular files at temp paths, and 8-thread concurrent saves. |

## Diff Highlights

- **`create_private_dir`**: On Unix, uses `fs::DirBuilder::new().recursive(true).mode(0o700).create(parent)` so newly created parent directories (such as `~/.cae`) are owner-only without altering existing shared ancestors.
- **`create_private_temp`**: Generates 16 cryptographically random bytes via `getrandom::getrandom` and appends `.tmp.<32-hex>` to the target path, retrying up to `MAX_TEMP_ATTEMPTS` (8) times on `ErrorKind::AlreadyExists` and never deleting a path it did not create.
- **`open_private`**: Uses `OpenOptions::new().write(true).create_new(true)` (`O_CREAT | O_EXCL`, refusing to follow symlinks or reuse existing files). On Unix, sets `.mode(0o600)` and immediately calls `file.set_permissions(fs::Permissions::from_mode(0o600))` on the open file handle to defeat umask surprises and avoid path-based TOCTOU races.
- **`write_and_replace`**: Writes contents, calls `file.sync_all()`, drops the handle before `fs::rename`, and `write_private_file` removes `&tmp_path` only if write/sync/rename fails.

## Tests Added or Updated

- `wallet::profile::tests::creates_parent_directory_owner_only` (Unix) — verifies newly created nested parent directories receive mode `0o700` and the saved profile file receives `0o600`.
- `wallet::profile::tests::ignores_preexisting_predictable_tmp_and_keeps_0600` (Unix) — verifies a pre-existing `0o644` `<path>.tmp<pid>` file is ignored, left untouched, and `wallet.json` is written with mode `0o600`.
- `wallet::profile::tests::refuses_preexisting_symlink_at_temp_path` (Unix) — verifies `open_private` rejects pre-existing live symlinks, dangling symlinks, and regular `0o644` files with `AlreadyExists` without modifying targets, and `save_profile` ignores a symlink planted at `<path>.tmp<pid>`.
- `wallet::profile::tests::concurrent_saves_do_not_collide` — verifies 8 threads performing 10 concurrent `save_profile` calls each succeed without collision, leave no stale temp files behind, parse cleanly, and preserve `0o600` permissions on Unix.

## Local Verification

- Commands run: deferred to coordinator (Mode B parallel wave — concurrent edits in `src/wallet/` prohibit running `cargo` inside this subagent).
- Manual checks: audited all Rust types, `#[cfg(unix)]` / `#[cfg(not(unix))]` branches, imports (`std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink}`, `getrandom`, `hex`, `anyhow`), ownership/drop ordering before `rename`/`remove_file`, and `clippy` lint cleanliness across `src/wallet/profile.rs`.

## Deviations from Assessment

- Retry limit set to `MAX_TEMP_ATTEMPTS = 8` (per coordinator guidance) instead of 16; with 128-bit cryptographic random suffixes (`2^128` space), collision probability across 8 retries is negligible.

## Follow-ups

- Coordinator to run `cargo test` and `cargo clippy --all-targets -- -D warnings` after this wave completes, commit isolated to `#11`, and run `/speckit-bug-test slug=pr-4-wallet-temp-file-permission-race`.
