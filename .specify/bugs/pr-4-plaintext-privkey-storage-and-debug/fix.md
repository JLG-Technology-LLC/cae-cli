# Bug Fix: pr-4-plaintext-privkey-storage-and-debug
- **Status:** Verified
- **Based On:** `.specify/bugs/pr-4-plaintext-privkey-storage-and-debug/assessment.md`
- **Commit SHA:** `669ddd1`
- **GitHub Issue:** `#8`

## Summary of Changes
- `Cargo.toml` & `Cargo.lock`: Promoted already-locked `ring = "0.17"` and `base64 = "0.22"` (plus `zeroize` lock entry from `#12`) for offline-safe AES-256-GCM + HKDF/PBKDF2 encryption.
- `src/wallet/crypto.rs`: Added `encrypt_private_key_hex`, `decrypt_private_key_hex_with`, `is_encrypted_privkey`, `WrappingKey`, and `KeyMaterial` producing versioned `enc:v1:aes256gcm:<kdf>:<salt>:<nonce>:<ciphertext_and_tag>` envelopes AAD-bound to lowercased `evm_address` (supporting `pbkdf2` with `CAE_WALLET_PASSPHRASE` and `hkdf` with `$CAE_HOME/.wallet.key`), plus unit tests for round-trip, wrong passphrase, wrong AAD, and tampered ciphertext.
- `src/wallet/profile.rs`: Replaced `#[derive(Debug)]` on `CandidateWalletProfile` with manual `fmt::Debug` redacting `session_ref` and `encrypted_privkey` as `"[REDACTED]"`; added `wallet_key_path`, `load_master_key`, and `load_or_create_master_key` storing `$CAE_HOME/.wallet.key` via `write_private_file` (`0600`), plus `debug_redacts_secret_fields` unit test.
- `src/wallet/auth.rs`: Replaced `#[derive(Debug)]` on `AuthResult` with manual `fmt::Debug` redacting `session_ref`.
- `src/wallet/mod.rs`: Encrypted `local-eoa` private keys on `handle_init --local` and decrypted `enc:v1:aes256gcm:...` envelopes in `handle_export_key` (with a deprecation warning for legacy plaintext files).
- `tests/wallet_tests.rs`: Updated `init_local_then_address_and_show` and `export_key_requires_confirmation_or_force` to verify `enc:v1:aes256gcm:` storage and absence of plaintext keys in `wallet.json` / `wallet.json.bak`, and added `passphrase_encrypted_local_wallet_roundtrip_and_wrong_passphrase_fails`.

## Deviation from Assessment
None.
