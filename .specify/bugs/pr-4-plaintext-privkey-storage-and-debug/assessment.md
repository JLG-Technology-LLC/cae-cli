# Bug Assessment: Plaintext private key stored unencrypted under encrypted_privkey and exposed via Debug

- **Slug**: pr-4-plaintext-privkey-storage-and-debug
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: 4660959979)
- **GitHub Issue**: #8
- **Verdict**: valid
- **Status**: Verified
- **Severity**: high

## Report (verbatim or summarized)

From PR #4 review comment (`https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959979`, commit `f82e11d94db3dacd2ae0db526d2d973e0877af09`):

> `local-eoa` wallets store the raw, unencrypted 32-byte secp256k1 private key hex in `~/.cae/wallet.json` under the misleading field name `encrypted_privkey` (`src/wallet/mod.rs:251`), as well as `wallet.json.bak` and `wallet.local.bak`. Additionally, `#[derive(Debug)]` on `CandidateWalletProfile` (line 66) formats `encrypted_privkey` and `session_ref` in plaintext whenever debug-printed.

## Symptom

Two related secret-exposure defects affect `CandidateWalletProfile`:
1. `CandidateWalletProfile` derives `Debug` directly, so any `{:?}` formatting (e.g., error context, test assertion failure, or debug logging) emits the raw `encrypted_privkey` and `session_ref` strings in plaintext.
2. `cae wallet init --local` persists the raw unencrypted 64-character hex secp256k1 private key directly into `~/.cae/wallet.json` under the misleading key `"encrypted_privkey"`, and `backup_profile` / `backup_local_profile` propagate that plaintext private key into `wallet.json.bak` and `wallet.local.bak`.

## Reproduction

1. Run `cae wallet init --local`.
2. Inspect `~/.cae/wallet.json` and run `cae wallet export-key --force`. Observe that `wallet.json`'s `"encrypted_privkey"` value is byte-for-byte identical to the exported raw private key (minus the `0x` prefix).
3. Run `cae wallet init --local --force` and inspect `~/.cae/wallet.json.bak`. Observe the raw private key is also copied in plaintext to the backup file.
4. Format a `CandidateWalletProfile` instance using `format!("{:?}", profile)`. Observe both `encrypted_privkey` and `session_ref` appear verbatim in the debug output.

## Suspected Code Paths

- `src/wallet/profile.rs:66-83` (`CandidateWalletProfile`) — `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]` exposes `session_ref` and `encrypted_privkey` in plaintext via `Debug`; doc comment on line 77 confirms plaintext storage.
- `src/wallet/auth.rs:9-14` (`AuthResult`) — also derives `Debug` with `pub session_ref: String` (defense-in-depth candidate to redact `session_ref`).
- `src/wallet/mod.rs:242-256` (`handle_init --local`) — stores `encrypted_privkey: Some(key.private_key_hex)` directly without encryption.
- `src/wallet/mod.rs:603-621` (`handle_export_key`) — reads `profile.encrypted_privkey` directly as raw hex without decryption.
- `tests/wallet_tests.rs:81-87` and `tests/wallet_tests.rs:206-217` — `export_key_requires_confirmation_or_force` currently asserts `format!("0x{}", stored["encrypted_privkey"]) == key.trim()`, directly encoding plaintext storage in the test suite.

## Root Cause Hypothesis

During initial MVP implementation of `SPEC-005`, the schema field `encrypted_privkey` (defined in `contracts/wallet-profile.schema.json` as `"Private key hex or keystore cipher text for local-eoa wallets"`) was populated with raw hex and `#[derive(Debug)]` was left on the profile struct without custom redaction or AEAD key wrapping. **Confidence: high** (directly verified in `src/wallet/profile.rs:66-79` and `src/wallet/mod.rs:251,603-621`).

## Proposed Remediation

**Preferred** (uses ONLY crates already locked in `Cargo.lock` — zero network download required):
1. **Custom `fmt::Debug` redaction**:
   - Remove `Debug` from `#[derive(...)]` on `CandidateWalletProfile` in `src/wallet/profile.rs` and implement `std::fmt::Debug` manually, formatting `session_ref` as `self.session_ref.as_ref().map(|_| "[REDACTED]")` and `encrypted_privkey` as `self.encrypted_privkey.as_ref().map(|_| "[REDACTED]")`.
   - Also implement a custom `fmt::Debug` on `AuthResult` in `src/wallet/auth.rs` that redacts `session_ref` as `"[REDACTED]"`.
2. **Authenticated Encryption at Rest (AES-256-GCM) via already-locked `ring = "0.17"` (`0.17.14` in `Cargo.lock`)**:
   - Promote `ring = "0.17"` to `[dependencies]` in `Cargo.toml` (already compiled in the build graph by `reqwest`'s `rustls-tls` feature and locked at `0.17.14` in `Cargo.lock`, requiring zero network access).
   - Add encryption/decryption helpers in `src/wallet/crypto.rs` (e.g., `encrypt_private_key_hex` / `decrypt_private_key_hex`) producing a versioned self-describing string that remains 100% compliant with `contracts/wallet-profile.schema.json` (`type: ["string", "null"]`, `additionalProperties: false`):
     - Format: `enc:v1:aes256gcm:<kdf>:<salt_hex>:<nonce_hex>:<ciphertext_and_tag_hex>` where `<kdf>` is `"pbkdf2"` (when `CAE_WALLET_PASSPHRASE` env var is set) or `"hkdf"` (default single-command mode using a per-home random 32-byte keyfile `$CAE_HOME/.wallet.key` created with strict `0600` exclusive permissions).
     - Generate a fresh 16-byte random salt and 12-byte random AES-GCM nonce via `getrandom::getrandom` on every encryption.
     - Derive a 32-byte AES-256-GCM key via `ring::pbkdf2::derive(PBKDF2_HMAC_SHA256, NonZeroU32::new(100_000), &salt, passphrase)` when `CAE_WALLET_PASSPHRASE` is non-empty, or via `ring::hkdf::Salt::new(HKDF_SHA256, &salt).extract(&master_key).expand(&[b"cae-cli:wallet-privkey:v1"], ...)` from the sibling `$CAE_HOME/.wallet.key` file (which is never duplicated into `wallet.json.bak` or `wallet.local.bak`).
     - Bind the lowercased `evm_address` bytes as AES-256-GCM AAD (`ring::aead::Aad::from(...)`) so an encrypted blob cannot be silently transplanted to another profile or tampered with.
   - In `src/wallet/mod.rs`:
     - `handle_init --local`: encrypt `key.private_key_hex` before storing it in `profile.encrypted_privkey` (so neither `wallet.json`, `wallet.json.bak`, nor `wallet.local.bak` ever contains a plaintext private key).
     - `handle_export_key`: if `encrypted_privkey` starts with `"enc:v1:"`, decrypt it (using `CAE_WALLET_PASSPHRASE` for `pbkdf2` or `$CAE_HOME/.wallet.key` for `hkdf`) and verify `address_from_private_key_hex(&decrypted)` matches `profile.evm_address`. If a legacy unencrypted 64-char hex value is present in a pre-existing `wallet.json`, emit a warning on `stderr` advising re-initialization/re-encryption while still allowing recovery export, and never write plaintext keys on any write path.
   - Update `CandidateWalletProfile`'s doc comment on `encrypted_privkey` to accurately describe the `enc:v1:aes256gcm:...` envelope format.
   - Update `tests/wallet_tests.rs:206-217` (`export_key_requires_confirmation_or_force`) to verify that `stored["encrypted_privkey"]` starts with `"enc:v1:aes256gcm:"`, does NOT contain the exported plaintext key, and that the exported key cryptographically derives the wallet's stored `evm_address`.

**Alternatives**:
- **Mandatory interactive passphrase only (no keyfile fallback)**: Require a user-typed passphrase or `CAE_WALLET_PASSPHRASE` on every `cae wallet init --local` and `cae wallet export-key`. *Trade-off*: Protects against full `$CAE_HOME` directory exfiltration, but breaks `SPEC-005` SC-002 (single-command non-interactive `cae wallet init --local`) and breaks all ~15 existing integration/E2E tests that invoke `cae wallet init --local` with `Stdio::null()`. Supporting `CAE_WALLET_PASSPHRASE` opt-in with a separate `0600` keyfile default preserves full CLI/test compatibility while removing plaintext keys from `wallet.json` and `.bak` files.
- **OS Keychain (`keyring` crate)**: Store the wrapping key or private key in macOS Keychain / Linux Secret Service / Windows Credential Manager. *Trade-off*: Requires adding a new native crate not present in `Cargo.lock` (blocked in offline builds and breaks hermetic `CAE_HOME` tempdir testing required by Constitution Principle IV).

**Files likely to change**:
- `Cargo.toml`
- `src/wallet/profile.rs`
- `src/wallet/crypto.rs`
- `src/wallet/mod.rs`
- `src/wallet/auth.rs`
- `tests/wallet_tests.rs`

**Tests to add or update**:
- Unit test in `src/wallet/profile.rs` asserting `format!("{:?}", profile)` contains `"[REDACTED]"` and does NOT contain the `encrypted_privkey` or `session_ref` secret substrings.
- Unit tests in `src/wallet/crypto.rs` testing AES-256-GCM round-trip encryption/decryption (both `hkdf` keyfile and `pbkdf2` passphrase modes), rejecting tampered ciphertext, rejecting mismatched `evm_address` AAD, and rejecting wrong passphrase.
- Update `export_key_requires_confirmation_or_force` in `tests/wallet_tests.rs` to assert `wallet.json` stores `enc:v1:aes256gcm:...` ciphertext (not plaintext hex) and that the exported key derives `evm_address`.
- Add integration test in `tests/wallet_tests.rs` verifying `CAE_WALLET_PASSPHRASE` encrypts with `pbkdf2` and refuses `export-key` with a wrong passphrase, plus verifying `wallet.json.bak` never contains the plaintext private key.

## Risks & Considerations

- **Keyfile lifecycle**: Ensure `$CAE_HOME/.wallet.key` is created via the hardened `write_private_file` / exclusive `0600` creation helper from `pr-4-wallet-temp-file-permission-race`, and warn clearly if `.wallet.key` is missing when attempting to decrypt an `hkdf`-wrapped wallet.
- **Coordination with `pr-4-wallet-temp-file-permission-race` and `pr-4-unzeroized-private-key-memory`**: All three touch `src/wallet/profile.rs` (and two touch `src/wallet/crypto.rs` / `src/wallet/mod.rs` / `Cargo.toml`). Decrypted key buffers returned by `decrypt_private_key_hex` should use `Zeroizing<String>` to align seamlessly with `pr-4-unzeroized-private-key-memory`.
- **Schema compatibility**: Storing the versioned string inside `encrypted_privkey` preserves exact compatibility with `contracts/wallet-profile.schema.json` (`additionalProperties: false`).

## Open Questions

- None — `ring = "0.17"` (`0.17.14`) is verified present in `Cargo.lock`, enabling audited AES-256-GCM + HKDF/PBKDF2 offline without breaking `SPEC-005` contracts or hermetic tempdir tests.
