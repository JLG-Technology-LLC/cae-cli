---
slug: pr-4-plaintext-privkey-storage-and-debug
pr_number: 4
comment_id: 4660959979
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959979"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCa6w"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: high
file: src/wallet/profile.rs
line: 79
start_line: 66
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Plaintext private key stored unencrypted under encrypted_privkey and exposed via Debug

- **Slug**: pr-4-plaintext-privkey-storage-and-debug
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660959979
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959979
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: High
- **Location**: `src/wallet/profile.rs:66-79`

## Finding

`local-eoa` wallets store the raw, unencrypted 32-byte secp256k1 private key hex in `~/.cae/wallet.json` under the misleading field name `encrypted_privkey` (`src/wallet/mod.rs:251`), as well as `wallet.json.bak` and `wallet.local.bak`. Additionally, `#[derive(Debug)]` on `CandidateWalletProfile` (line 66) formats `encrypted_privkey` and `session_ref` in plaintext whenever debug-printed.

- **Category**: `security`
- **Rule**: Security-Sensitive Code & User Rule 3 (Security) — protect private keys and prevent secret leakage

## Diff Context

```diff
+#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
+#[serde(deny_unknown_fields)]
+pub struct CandidateWalletProfile {
...
+    #[serde(default)]
+    pub session_ref: Option<String>,
+    /// Hex-encoded 32-byte private key for `local-eoa` wallets (never printed by `show`).
+    #[serde(default)]
+    pub encrypted_privkey: Option<String>,
```

## Proposed Direction (Not Applied)

Implement a custom `fmt::Debug` for `CandidateWalletProfile` that redacts `encrypted_privkey` and `session_ref` as `"[REDACTED]"`, and either encrypt private keys at rest using audited AEAD + KDF (or OS keychain / encrypted envelope) with backward-compatible detection or restrict/document clearly while enforcing strict file permissions.

## Next Step

Run `/speckit-bug-assess slug=pr-4-plaintext-privkey-storage-and-debug` using this intake file.
