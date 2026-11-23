# Bug Assessment: Predictable temp path and non-exclusive open bypass 0600 permissions and allow symlink/race attacks

- **Slug**: pr-4-wallet-temp-file-permission-race
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660960209)
- **GitHub Issue**: #11
- **Verdict**: valid
- **Severity**: medium

## Report (verbatim or summarized)

From PR #4 review comment (`https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960209`, commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> `write_private_file` uses a predictable `<path>.tmp<pid>` filename and opens it with `.create(true).truncate(true).mode(0o600)`. On Unix, `.mode(0o600)` only applies when creating a new file; if `<path>.tmp<pid>` already exists (pre-created with `0o644` or planted as a symlink in a shared dir), `open()` follows symlinks and keeps broader permissions, and concurrent in-process writes collide on the same temp file.

## Symptom

When persisting or backing up wallet profiles (`save_profile`, `backup_profile`, `backup_local_profile`), `write_private_file` creates a predictable temporary file `<path>.tmp<pid>` without `O_EXCL` (`create_new(true)`), does not enforce `0o600` on the opened descriptor, and creates parent directories with default umask (`0o755`). If `<path>.tmp<pid>` already exists with permissive mode (e.g., `0o644`) or is a pre-planted symlink, sensitive wallet material is written with overly broad permissions or redirected to an arbitrary target; concurrent threads within the same PID also race on and corrupt the same temp file.

## Reproduction

1. In a test directory `dir`, pre-create `dir/wallet.json.tmp<pid>` with mode `0o644` (or create a symlink at `dir/wallet.json.tmp<pid>` pointing to `dir/victim.txt`).
2. Call `save_profile(&dir.join("wallet.json"), &profile)` from the same process.
3. Observe that:
   - In the pre-created file case, `wallet.json` inherits mode `0o644` instead of `0o600` (violating `SPEC-005` FR-012).
   - In the symlink case, `write_private_file` follows the symlink and overwrites `dir/victim.txt`, then renames the symlink itself over `wallet.json`.
   - When spawning two threads calling `save_profile` concurrently on the same path, both open the identical `<path>.tmp<pid>` and can truncate or unlink each other's temp file before `fs::rename`.

## Suspected Code Paths

- `src/wallet/profile.rs:152-176` (`write_private_file`) — calls `fs::create_dir_all(parent)` without setting `0o700` on Unix; derives a predictable temp filename via `format!(".tmp{}", std::process::id())`; cleans up `tmp_path` on error even if `open_private` failed before creation.
- `src/wallet/profile.rs:178-198` (`open_private`) — opens with `.write(true).create(true).truncate(true).mode(0o600)` instead of `.create_new(true)`, allowing symlink following and retaining pre-existing wider permissions on Unix.

## Root Cause Hypothesis

`write_private_file` relied on PID-only naming and `OpenOptions::create(true).mode(0o600)` under the mistaken assumption that `.mode()` always enforces `0o600` and that PID uniqueness prevents collisions. On POSIX, `open(..., O_CREAT | O_TRUNC, 0600)` ignores the mode argument for existing files and follows symlinks unless `O_EXCL` (`create_new(true)`) is set. **Confidence: high** (directly verified in `src/wallet/profile.rs:152-198`).

## Proposed Remediation

**Preferred** (uses only `std` + already-direct dependencies `getrandom = "0.2"` and `hex = "0.4"` — zero new crates):
1. **Restrictive parent directory creation**:
   - Add a small helper `create_private_dir(parent: &Path) -> Result<()>` that checks `!parent.exists()` and on Unix uses `std::fs::DirBuilder::new().recursive(true).mode(0o700).create(parent)` (via `std::os::unix::fs::DirBuilderExt`) so newly created wallet directories (such as `~/.cae/`) are owner-only `0o700` without mutating permissions of pre-existing shared directories like `/tmp`.
2. **Unpredictable random temp filename + exclusive creation (`O_CREAT | O_EXCL`)**:
   - In `write_private_file`, loop up to 16 attempts: generate 16 cryptographically random bytes via `getrandom::getrandom` and construct `<path>.tmp.<32-hex-chars>`.
   - Update `open_private(path)` to use `.write(true).create_new(true)` (plus `.mode(0o600)` on Unix). On `ErrorKind::AlreadyExists`, retry with a fresh random suffix; propagate any other I/O error immediately without removing a file we did not create.
3. **Explicit fd-level permission enforcement (`fchmod`)**:
   - Immediately after `open_private` succeeds on Unix, call `file.set_permissions(fs::Permissions::from_mode(0o600))` so `0o600` is enforced on the open file descriptor regardless of process umask and immune to path-based TOCTOU races.
   - Write `contents`, call `file.sync_all()`, drop `file`, and atomically `fs::rename(&tmp_path, path)`. Only remove `tmp_path` if an error occurs *after* `open_private` successfully created it.
4. **Make `write_private_file` `pub(crate)`** (optional convenience) if sibling wallet modules (e.g., `.wallet.key` creation in `pr-4-plaintext-privkey-storage-and-debug`) need identical atomic `0o600` persistence.

**Alternatives**:
- Use the `tempfile` crate (`NamedTempFile::new_in`). *Trade-off*: `tempfile` is not currently in `Cargo.toml` or `Cargo.lock`, so adding it would require network access to crates.io. Using `getrandom` + `OpenOptions::create_new(true)` + `file.set_permissions(0o600)` achieves identical POSIX security guarantees offline with ~25 lines of auditable stdlib code.

**Files likely to change**:
- `src/wallet/profile.rs`

**Tests to add or update**:
- Unit tests in `src/wallet/profile.rs`:
  - `ignores_preexisting_predictable_tmp_and_keeps_0600` (Unix): pre-create `<path>.tmp<pid>` with mode `0o644`, call `save_profile`, assert `wallet.json` has mode `0o600` and pre-existing `.tmp<pid>` was not clobbered/adopted.
  - `creates_parent_directory_owner_only` (Unix): save to a newly nested directory and assert `fs::metadata(parent)?.permissions().mode() & 0o777 == 0o700`.
  - `concurrent_saves_do_not_collide`: spawn multiple threads (e.g., 8 threads x 10 iterations) calling `save_profile` concurrently on the same target path and verify every call succeeds and the final file parses cleanly as a valid `CandidateWalletProfile` with mode `0o600`.

## Risks & Considerations

- **Non-Unix targets (Windows)**: Keep `#[cfg(unix)]` / `#[cfg(not(unix))]` split cleanly so Windows still uses `create_new(true)` with random temp suffixes and compiles with zero warnings under `cargo clippy --all-targets -- -D warnings`.
- **Coordination with `pr-4-plaintext-privkey-storage-and-debug` and `pr-4-unzeroized-private-key-memory`**: Both also edit `src/wallet/profile.rs` (struct impls at lines 66-83 vs file I/O at lines 152-198); changes are in disjoint sections of the file.

## Open Questions

- None — all required primitives (`std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt}`, `getrandom`, `hex`) are already available offline.
