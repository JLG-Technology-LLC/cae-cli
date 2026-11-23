# Bug Fix: Raw secp256k1 private key bytes and hex strings are never zeroized in memory

- **Slug**: pr-4-unzeroized-private-key-memory
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **Status**: Fixed (Awaiting Verification)
- **Commit SHA**: pending
- **GitHub Issue**: #12

## Summary

Promoted the already-locked `zeroize = "1.9"` crate (resolving offline to `zeroize 1.9.1` without `derive`) into `Cargo.toml` and wrapped raw private key buffers, hex key strings, and wallet profile secrets in `zeroize::Zeroizing` / `Drop`-based zeroization across `src/wallet/crypto.rs`, `src/wallet/profile.rs`, and `src/wallet/mod.rs`. Added unit tests in `crypto.rs` and `profile.rs` verifying `Zeroizing` wrapper types and secret zeroization behavior.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `Cargo.toml` | modified | Added direct dependency `zeroize = "1.9"` (default `alloc` feature only; no `derive` feature so `Cargo.lock` resolves offline to locked `1.9.1`). |
| `src/wallet/crypto.rs` | modified | Changed `LocalKey.private_key_hex` to `Zeroizing<String>`; wrapped the 32-byte CSPRNG stack buffer in `generate_local_key` (`Zeroizing<[u8; 32]>`) and decoded key bytes in `address_from_private_key_hex` (`Zeroizing<Vec<u8>>`). Updated `generated_keys_are_valid_and_unique` and added `local_key_private_hex_is_zeroizing_and_clears_on_zeroize`. |
| `src/wallet/profile.rs` | modified | Implemented `Drop` (via `CandidateWalletProfile::zeroize_secrets`) to zeroize `encrypted_privkey` and `session_ref`; wrapped temporary serialized/raw wallet file buffers in `load_profile`, `save_profile`, `backup_profile`, and `backup_local_profile` with `Zeroizing`; added `zeroizes_secret_fields_before_drop` unit test; formatted line 216 to satisfy `rustfmt --edition 2021 --check`. |
| `src/wallet/mod.rs` | modified | Updated `handle_init --local` to copy from `key.private_key_hex` and drop the `Zeroizing<String>` guard immediately; updated `handle_login` to borrow `existing.as_ref()` (avoiding partial moves out of `Drop`-implementing `CandidateWalletProfile`) and drop `existing` early; explicitly zeroized the taken `session_ref` in `handle_logout`; wrapped exported `hex_key` and JSON output payload in `Zeroizing<String>` using a borrowed `ExportKeyPayload` struct in `handle_export_key`. |

## Diff Highlights

- **`Cargo.toml`**: Added `zeroize = "1.9"` without `features = ["derive"]`, matching the existing `zeroize 1.9.1` entry in `Cargo.lock` with no transitive crate additions.
- **`src/wallet/crypto.rs`**:
  - `LocalKey.private_key_hex` is now `Zeroizing<String>`, guaranteeing volatile zeroization when `LocalKey` is dropped.
  - `generate_local_key` uses `let mut bytes = Zeroizing::new([0u8; 32]);`, so both rejected retry iterations and the returning iteration wipe the stack key buffer on scope exit.
  - `address_from_private_key_hex` wraps the `hex::decode` output in `Zeroizing<Vec<u8>>`, wiping decoded private key bytes on both valid and error return paths.
- **`src/wallet/profile.rs`**:
  - `impl Drop for CandidateWalletProfile` calls `self.zeroize_secrets()`, which invokes `Zeroize::zeroize` on `self.encrypted_privkey` and `self.session_ref`.
  - Temporary file contents in `load_profile`, `save_profile`, `backup_profile`, and `backup_local_profile` are wrapped in `Zeroizing<String>` / `Zeroizing<Vec<u8>>` so serialized secret fields do not linger unzeroized in memory.
- **`src/wallet/mod.rs`**:
  - `handle_login` switches `match existing` to `match existing.as_ref()` (cloning `p.last_spend_date`) so `CandidateWalletProfile`'s `Drop` implementation compiles cleanly without partial moves, then drops `existing` immediately.
  - `handle_logout` explicitly calls `session.zeroize()` on the `Option<String>` removed via `profile.session_ref.take()`.
  - `handle_export_key` wraps `hex_key` and the pretty-printed JSON payload (serialized directly from a borrowed `ExportKeyPayload<'a>` struct without intermediate `serde_json::Value` string copies) in `Zeroizing<String>`.

## Tests Added or Updated

- `wallet::crypto::tests::generated_keys_are_valid_and_unique` (updated) — calls `address_from_private_key_hex(a.private_key_hex.as_str())` with the `Zeroizing<String>` field.
- `wallet::crypto::tests::local_key_private_hex_is_zeroizing_and_clears_on_zeroize` (added) — asserts compile-time `Zeroizing<String>` wrapping on `LocalKey.private_key_hex` and verifies `.zeroize()` clears `LocalKey.private_key_hex`, `Zeroizing<[u8; 32]>`, and `Zeroizing<Vec<u8>>`.
- `wallet::profile::tests::zeroizes_secret_fields_before_drop` (added) — verifies `std::mem::needs_drop::<CandidateWalletProfile>()`, confirms `zeroize_secrets()` clears both `encrypted_privkey` and `session_ref` while leaving non-secret fields intact, and verifies dropping profiles with `None` secrets succeeds cleanly.

## Local Verification

- Commands run:
  - `rustfmt --edition 2021 --check src/wallet/crypto.rs src/wallet/profile.rs src/wallet/mod.rs` — passed (exit code 0).
  - `cargo test` / `cargo clippy` — deferred to coordinator (Mode B parallel wave — concurrent edits in `src/wallet/faucet.rs` prohibit running `cargo` inside this subagent).
- Manual checks: audited all `LocalKey` and `CandidateWalletProfile` call sites across `src/` and `tests/` for partial-move compatibility with `impl Drop`, verified `zeroize 1.9.1` offline resolution in `Cargo.lock`, verified `Zeroizing` trait usage (`as_slice`, `as_mut_slice`, `as_str`, `as_bytes`) without requiring `zeroize/serde` or `zeroize/derive`, and confirmed no changes overlap `#8` (encryption / `Debug` redaction) or `#18` (`handle_login --force`).

## Deviations from Assessment

- In addition to the assessment's core targets (`LocalKey`, `generate_local_key`, `address_from_private_key_hex`, `CandidateWalletProfile::drop`, `handle_init`, `handle_login`, `handle_export_key`), also wrapped temporary serialized wallet buffers in `profile.rs` (`load_profile`, `save_profile`, `backup_profile`, `backup_local_profile`), zeroized the removed `session_ref` in `handle_logout`, and serialized `handle_export_key --json` via a borrowed struct into `Zeroizing<String>` to eliminate unzeroized `serde_json::Value` string copies.
- Intentionally did not add a custom redacting `Debug` implementation on `LocalKey` or `CandidateWalletProfile` to keep `#12` strictly scoped to memory zeroization and avoid overlapping `#8`.

## Follow-ups

- Coordinator to run `cargo test` and `cargo clippy --all-targets -- -D warnings` after this wave completes, commit isolated to `#12`, and run `/speckit-bug-test slug=pr-4-unzeroized-private-key-memory`.
