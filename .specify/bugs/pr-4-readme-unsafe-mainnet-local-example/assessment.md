# Bug Assessment: pr-4-readme-unsafe-mainnet-local-example

- **Slug**: pr-4-readme-unsafe-mainnet-local-example
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660961228`)
- **Verdict**: valid
- **Status**: Verified
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660961228`)
- **GitHub Issue**: #22
- **Related Spec**: None

## Symptom & Reproduction

`README.md` recommends an unsafe command (`cae wallet init --local --network base --force`) as the production Base Mainnet wallet initialization example, and omits security warnings around private key material, overwrite hazards, and backup file permissions in the `~/.cae/wallet.json` documentation.

**Observed**:
- `README.md:289-290` presents `# Production mainnet wallet with force overwrite` followed by `cae wallet init --local --network base --force`.
- `README.md:473-487` documents `~/.cae/wallet.json` without noting that `local-eoa` wallets store key material (`encrypted_privkey`) and `cdp-embedded` wallets store `session_ref` on disk, nor warning about backup file (`wallet.json.bak`, `wallet.local.bak`) sensitivity and platform permission boundaries.
- `README.md:602-603` mentions `wallet.json.bak` without warning that repeated `--force` runs overwrite the single backup file or mentioning `wallet.local.bak` and preserving `0600` permissions on restore.

**Expected**: Production mainnet examples recommend CDP User Auth (`cae wallet init --network base`), never include `--force` in a mainnet setup snippet, clearly caveat `--local` security semantics, and document private key protection, overwrite behavior, and `0600` backup permission handling accurately.

**Reproduction**:
1. Read `README.md:285-294` and observe `cae wallet init --local --network base --force` labeled as the production mainnet example.
2. Cross-reference `src/wallet/mod.rs:224-256` and `src/wallet/profile.rs:66-83`, `137-198`: `--local` writes key material into `encrypted_privkey` in `~/.cae/wallet.json`, `--force` silently overwrites `wallet.json` and overwrites any prior `wallet.json.bak`, and non-Unix `open_private` (`src/wallet/profile.rs:190-198`) does not enforce POSIX `0600` permissions.
3. Read `README.md:473-487` and confirm no security warning or mention of sensitive fields (`encrypted_privkey`, `session_ref`) or backup permissions appears.

## Root Cause Analysis

- **Suspected Files**:
  - `README.md:271-294` (`cae wallet init` options and examples)
  - `README.md:473-487` (`2. Candidate Wallet Profile (~/.cae/wallet.json)`)
  - `README.md:602-603` (backup/restore FAQ)
  - Code references: `src/wallet/mod.rs:222-296`, `src/wallet/profile.rs:66-83`, `src/wallet/profile.rs:136-198`
- **Cause**: Documentation was drafted as a quick command showcase without applying security-first defaults for mainnet funds. Combining `--local` (local key file on disk), `--network base` (real mainnet funds), and `--force` (destructive overwrite of `wallet.json` and any existing `wallet.json.bak`) creates a copy-paste footgun where users can both expose mainnet keys on disk and permanently overwrite a previous funded wallet backup. Additionally, the `wallet.json` schema block omits sensitive fields and security guidance.
- **Adjacent Risks**:
  - Directly intersects with `pr-4-plaintext-privkey-storage-and-debug` (how `encrypted_privkey` is stored at rest) and `pr-4-login-skips-confirm-and-clobbers-backup` (backup overwrite semantics). Ensure README wording accurately describes current behavior without overclaiming encryption that isn't implemented (or align with whatever key-protection model lands in `pr-4-plaintext-privkey-storage-and-debug`).
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `README.md`

**Steps**:
1. Update the `cae wallet init` examples (`README.md:285-294`) to:
   - Recommend CDP User Auth (`cae wallet init --network base`) for Base Mainnet usage (keys managed in Coinbase's enclave).
   - Keep `cae wallet init --local` clearly scoped to local/testnet development or offline workflows with an explicit security caveat.
   - Remove `--force` from the mainnet example; document `--force` separately with a warning that it replaces `wallet.json` and overwrites any existing `wallet.json.bak` (advise exporting/backing up keys first).
2. Update the `Candidate Wallet Profile (~/.cae/wallet.json)` section (`README.md:473-487`) to show the complete schema (including `email`, `session_ref`, and `encrypted_privkey` as redacted/nullable fields matching `CandidateWalletProfile`) and add a prominent security callout covering:
   - Sensitive key/session storage in `wallet.json`, `wallet.json.bak`, and `wallet.local.bak` (never commit, share, or sync to cloud storage).
   - POSIX `0600` permissions enforced on Unix (and OS-level ACL / disk encryption precautions on non-Unix platforms).
3. Update the backup/restore FAQ (`README.md:602-603`) to note both `wallet.json.bak` (`init --force`) and `wallet.local.bak` (`login` over a local wallet), warn that `--force` overwrites the prior `.bak` file, and instruct users to preserve `0600` permissions when restoring.
4. Manually review `README.md` against `src/wallet/mod.rs` and `src/wallet/profile.rs` to verify every flag, path, and security claim is accurate.
