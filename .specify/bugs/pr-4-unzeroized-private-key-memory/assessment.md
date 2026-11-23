# Bug Assessment: Raw secp256k1 private key bytes and hex strings are never zeroized in memory

- **Slug**: pr-4-unzeroized-private-key-memory
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660960273)
- **GitHub Issue**: #12
- **Verdict**: valid
- **Severity**: medium

## Report (verbatim or summarized)

From PR #4 review comment (`https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960273`, commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> In `generate_local_key`, stack buffer `bytes: [u8; 32]` is never zeroized before returning, and `LocalKey.private_key_hex` is a plain `String` without `Drop` zeroization. In `address_from_private_key_hex`, the decoded `Vec<u8>` key buffer is likewise dropped unzeroized.

## Symptom

Raw 32-byte secp256k1 private key material and 64-character hex private key strings remain uncleared in stack and heap memory after key generation (`generate_local_key`), address derivation (`address_from_private_key_hex`), key export (`handle_export_key`), and profile teardown (`CandidateWalletProfile`). Because standard Rust drops do not overwrite freed memory, secret key material can persist in process memory, swap, or crash dumps.

## Reproduction

1. Inspect `src/wallet/crypto.rs:14-46`:
   - `generate_local_key` allocates `let mut bytes = [0u8; 32];` on the stack, fills it with OS CSPRNG entropy via `getrandom`, constructs `LocalKey { private_key_hex: hex::encode(bytes), ... }` (plain `String` with no `Drop` impl), and returns without zeroizing `bytes` (nor zeroizing `bytes` on rejected retry iterations).
   - `address_from_private_key_hex` decodes the hex private key into a heap `Vec<u8>` (`let bytes = hex::decode(raw)...`) and drops it unzeroized on both success and invalid-scalar error paths.
2. Inspect `src/wallet/mod.rs:243-256` and `src/wallet/mod.rs:603-633`: `key.private_key_hex` and `hex_key` are plain `String`s dropped without zeroization.

## Suspected Code Paths

- `src/wallet/crypto.rs:6-11` (`LocalKey`) — `private_key_hex` is a plain `String` with no `Drop` zeroization.
- `src/wallet/crypto.rs:14-29` (`generate_local_key`) — stack buffer `bytes: [u8; 32]` is never zeroized before loop iteration end or return.
- `src/wallet/crypto.rs:40-46` (`address_from_private_key_hex`) — decoded `Vec<u8>` private key buffer is dropped unzeroized.
- `src/wallet/profile.rs:68-83` (`CandidateWalletProfile`) — holds `encrypted_privkey: Option<String>` and `session_ref: Option<String>` with no `Drop` zeroization.
- `src/wallet/mod.rs:243-256, 327-340, 603-633` (`handle_init`, `handle_login`, `handle_export_key`) — handles `LocalKey`, `CandidateWalletProfile`, and exported `hex_key` strings.

## Root Cause Hypothesis

Key-handling code in `src/wallet/crypto.rs` and callers used plain `[u8; 32]`, `Vec<u8>`, and `String` types instead of `zeroize::Zeroizing<T>` / `Zeroize` drop guards, even though `zeroize` (`1.9.1`) is already present in `Cargo.lock` via `k256` → `elliptic-curve`. **Confidence: high** (directly verified in `src/wallet/crypto.rs:6-46` and `Cargo.lock`).

## Proposed Remediation

**Preferred** (offline-safe using already-locked `zeroize = "1.9"` — exact version `1.9.1` in `Cargo.lock`, WITHOUT `derive` feature because `zeroize_derive` is NOT in `Cargo.lock`):
1. **Add `zeroize = "1.9"` to `Cargo.toml`**:
   - Resolves directly to `zeroize 1.9.1` already present in `Cargo.lock` with default `alloc` support (`Zeroize`, `Zeroizing<[u8; 32]>`, `Zeroizing<Vec<u8>>`, `Zeroizing<String>`) and zero network fetches. Do **not** enable `features = ["derive"]` (which would require `zeroize_derive`).
2. **Harden `src/wallet/crypto.rs`**:
   - Change `LocalKey.private_key_hex` to `Zeroizing<String>` (or keep `private_key_hex: Zeroizing<String>` with `impl Drop` / deref-to-`&str`, which automatically wipes the heap string bytes using volatile writes on drop and panic; also implement a redacting `fmt::Debug` for `LocalKey` if helpful).
   - In `generate_local_key`: allocate `let mut bytes = Zeroizing::new([0u8; 32]);`, call `getrandom::getrandom(&mut *bytes)`, derive `SecretKey::from_slice(&*bytes)`, and construct `private_key_hex: Zeroizing::new(hex::encode(&*bytes))`. Every iteration's stack buffer `bytes` is deterministically zeroized when it goes out of scope (both on retry and on return).
   - In `address_from_private_key_hex`: wrap decoded bytes in `let bytes = Zeroizing::new(hex::decode(raw).map_err(...)?);` and pass `&*bytes` to `SecretKey::from_slice` so the heap buffer is deterministically zeroized on both `Ok` and `Err` paths.
3. **Harden callers in `src/wallet/profile.rs` and `src/wallet/mod.rs`**:
   - Implement `Drop` for `CandidateWalletProfile` in `src/wallet/profile.rs` to call `s.zeroize()` on `self.encrypted_privkey` and `self.session_ref` when `Some`.
   - **Important compiler note**: In `src/wallet/mod.rs:327-333` (`handle_login`), `match existing { Some(p) => (..., p.last_spend_date) }` currently performs a partial move out of `CandidateWalletProfile`; once `CandidateWalletProfile` implements `Drop`, update this to `match existing.as_ref()` (cloning `p.last_spend_date.clone()`).
   - In `src/wallet/mod.rs:243-256` (`handle_init --local`) and `src/wallet/mod.rs:603-633` (`handle_export_key`): borrow `&*key.private_key_hex` and wrap the formatted `hex_key` string in `Zeroizing::new(...)` so exported private keys are wiped immediately after stdout output.

**Alternatives**:
- Import `k256::elliptic_curve::zeroize::{Zeroize, Zeroizing}` directly without adding `zeroize = "1.9"` to `Cargo.toml`. *Trade-off*: Zero `Cargo.toml` changes, but relies on a transitive re-export from `k256` rather than declaring a direct dependency in `Cargo.toml` (whereas `.opencodereview/rule.json` favors explicit version-pinned dependencies).

**Files likely to change**:
- `Cargo.toml`
- `src/wallet/crypto.rs`
- `src/wallet/profile.rs`
- `src/wallet/mod.rs`

**Tests to add or update**:
- Unit tests in `src/wallet/crypto.rs`:
  - Existing `generated_keys_are_valid_and_unique` continues to pass (updating assertions to work with `&*a.private_key_hex` / `Zeroizing<String>`).
  - Add a unit test verifying `Zeroizing` / `Drop` zeroization behavior on `LocalKey` (and `CandidateWalletProfile` secret fields via `zeroize()`) — e.g., verifying `LocalKey::private_key_hex` is wrapped in `Zeroizing<String>` and that manual `zeroize()` clears secret buffers completely without panic.
  - Ensure `address_from_private_key_hex` still passes all valid and invalid key tests (`derives_known_address_for_private_key_one`, `rejects_bad_keys`).

## Risks & Considerations

- **Offline build safety**: Do NOT add `zeroize = { version = "1.9", features = ["derive"] }` because `zeroize_derive` is absent from `Cargo.lock` and would fail in offline/sandboxed builds. Plain `zeroize = "1.9"` resolves cleanly to locked `zeroize 1.9.1`.
- **Partial-move ergonomics with `impl Drop`**: Implementing `Drop` on `CandidateWalletProfile` forbids partial field moves (such as `p.last_spend_date` in `src/wallet/mod.rs:332`); switching to `existing.as_ref()` avoids any compile error.
- **Coordination with `pr-4-plaintext-privkey-storage-and-debug`**: If `pr-4-plaintext-privkey-storage-and-debug` adds encryption/decryption helpers in `src/wallet/crypto.rs`, those helpers should likewise wrap plaintext key bytes and derived AES key material in `Zeroizing<...>` so no unzeroized copies are re-introduced.

## Open Questions

- None — `zeroize 1.9.1` is verified locked in `Cargo.lock` and compatible with offline builds.
