# Bug Fix: Replace Unsafe Mainnet `--local --force` Example and Document Wallet Storage Security

- **Slug**: pr-4-readme-unsafe-mainnet-local-example
- **Fixed**: 2026-11-22
- **Assessment**: ./assessment.md
- **GitHub Issue**: #22
- **Status**: Fixed (Awaiting Verification)
- **Commit SHA**: pending

## Summary

Updated `README.md` to recommend CDP User Auth (`cae wallet init --network base`) for production Base Mainnet wallets, remove the unsafe `cae wallet init --local --network base --force` example, clearly scope `--local` to local/testnet or offline workflows with explicit `--force` overwrite warnings, document the complete `CandidateWalletProfile` schema (`email`, `session_ref`, `encrypted_privkey`) with sensitive-file and `0600`/non-Unix permission guidance, and update the wallet backup/restore FAQ to cover both `wallet.json.bak` and `wallet.local.bak` as well as preserving `0600` permissions.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `README.md` | modified | Updated `cae wallet init` options/examples (lines 279–297) to recommend `cae wallet init --network base` for mainnet, remove `--force` from the mainnet snippet, caveat `--local` on-disk key storage, and warn that `--force` overwrites `wallet.json` and any existing `wallet.json.bak`. |
| `README.md` | modified | Updated `2. Candidate Wallet Profile (~/.cae/wallet.json)` (lines 475–501) to include all 10 `CandidateWalletProfile` fields (`email`, `session_ref`, `encrypted_privkey`), clarify `0700` dir / `0600` Unix permissions vs. non-Unix ACL/encryption requirements, and add a security warning covering `wallet.json`, `wallet.json.bak`, and `wallet.local.bak`. |
| `README.md` | modified | Updated the wallet backup/restore FAQ (lines 615–629) to document both `~/.cae/wallet.json.bak` (`init --force`) and `~/.cae/wallet.local.bak` (`login` over a `local-eoa` wallet), warn about single-slot backup overwrites, and show `install -m 600` / `chmod 600` restore instructions. |

## Diff Highlights

```diff
--- a/README.md
+++ b/README.md
@@ -283,16 +283,18 @@ cae wallet init [OPTIONS]
 ```bash
-# Recommended: Local development wallet on Base Sepolia
-cae wallet init --local
-
-# Production mainnet wallet with force overwrite
-cae wallet init --local --network base --force
+# Recommended for production Base Mainnet (CDP User Auth — keys managed in Coinbase's secure enclave)
+cae wallet init --network base
 
-# CDP User Auth via terminal OTP
+# CDP User Auth via terminal OTP (headless / SSH environments)
 cae wallet init --no-browser
+
+# Local/testnet development or offline workflow (Base Sepolia default; stores key material on disk)
+cae wallet init --local
 ```
 
+> **Security Note:** Avoid using `--local` for production Base Mainnet funds because private key material is stored on local disk (`~/.cae/wallet.json`); use CDP User Auth (`cae wallet init --network base`) instead. Only pass `--force` when you intentionally want to replace your current wallet — it overwrites `~/.cae/wallet.json` and replaces any previous `~/.cae/wallet.json.bak` backup, so back up or export any funded key first (`cae wallet export-key`).
```

```diff
--- a/README.md
+++ b/README.md
@@ -472,20 +474,31 @@ cae wallet export-key --force
 
 ### 2. Candidate Wallet Profile (`~/.cae/wallet.json`)
 
-Stored strictly with POSIX permissions `0600` (`-rw-------`):
+On Unix systems, `~/.cae` (or `$CAE_HOME`) is created with `0700` permissions and `~/.cae/wallet.json` (along with its `.bak` backup files) is atomically written with POSIX `0600` (`-rw-------`) owner-only permissions:
 
 ```json
 {
   "wallet_type": "local-eoa",
   "evm_address": "0x4B20993Bc481177ec7E8f571ceCaE8A9e22C02db",
+  "email": null,
   "network": "base-sepolia",
   "created_at": "2026-11-12T21:00:00.000000+00:00",
+  "session_ref": null,
+  "encrypted_privkey": "<redacted-local-eoa-key-material>",
   "daily_limit_usdc": 0.15,
   "spent_today_usdc": 0.03,
   "last_spend_date": "2026-11-12"
 }
 ```
```

## Tests Added or Updated

- Documentation-only bug; no Rust test files were modified.

## Local Verification

- **Cross-reference against source code**: Verified all flags, filenames, struct fields, and permission semantics directly against `src/wallet/mod.rs` (`InitArgs`, `handle_init`, `handle_login`, `handle_export_key`) and `src/wallet/profile.rs` (`CandidateWalletProfile`, `backup_path`, `backup_local_path`, `create_private_dir`, `open_private`).
- **JSON & safety validation**:
  - Command: `python3 -c '...'` extracting the `Candidate Wallet Profile` JSON block from `README.md`, parsing with `json.loads`, asserting exact key order match against all 10 `CandidateWalletProfile` fields, and asserting `--local --network base --force` is absent.
  - Result: Exit `0` — valid JSON with exact 10 fields and no unsafe mainnet `--local --force` example.
- **Diff scope check**:
  - Command: `git -C /Users/jlgtechnology/Developer/cae/cae-cli diff -- README.md`
  - Result: Exit `0` — confirmed only the 3 target `#22` hunks in `README.md` were touched; Semgrep (`#9`) and balance/ledger JSON / `CAE_SKIP_BROWSER_OPEN` FAQ (`#23`) were left untouched.

## Deviations from Assessment

None. All steps in `assessment.md` were applied as specified, intentionally describing `encrypted_privkey` as "private key material on disk" without overclaiming at-rest encryption prior to `#8` landing.

## Follow-ups

- Stage 3 verification (`/speckit-bug-test slug=pr-4-readme-unsafe-mainnet-local-example`) should confirm all three `README.md` sections match `src/wallet/mod.rs` and `src/wallet/profile.rs`.
- Once `#8` (`pr-4-plaintext-privkey-storage-and-debug`) lands, optionally note the `enc:v1:aes256gcm:...` envelope and `CAE_WALLET_PASSPHRASE` / `.wallet.key` details if desired.
