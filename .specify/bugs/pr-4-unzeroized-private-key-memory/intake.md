---
slug: pr-4-unzeroized-private-key-memory
pr_number: 4
comment_id: 4660960273
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960273"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCcEQ"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: medium
file: src/wallet/crypto.rs
line: 46
start_line: 18
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Raw secp256k1 private key bytes and hex strings are never zeroized in memory

- **Slug**: pr-4-unzeroized-private-key-memory
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660960273
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660960273
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: Medium
- **Location**: `src/wallet/crypto.rs:18-46`

## Finding

In `generate_local_key`, stack buffer `bytes: [u8; 32]` is never zeroized before returning, and `LocalKey.private_key_hex` is a plain `String` without `Drop` zeroization. In `address_from_private_key_hex`, the decoded `Vec<u8>` key buffer is likewise dropped unzeroized.

- **Category**: `security`
- **Rule**: Security-Sensitive Code — cryptographic key handling

## Diff Context

```diff
+        let mut bytes = [0u8; 32];
+        getrandom::getrandom(&mut bytes)...
+        if let Ok(secret) = SecretKey::from_slice(&bytes) {
+            return Ok(LocalKey {
+                private_key_hex: hex::encode(bytes),
+                address: address_from_secret(&secret),
+            });
+        }
```

## Proposed Direction (Not Applied)

Use `zeroize` (`Zeroizing<[u8; 32]>`, `Zeroizing<Vec<u8>>`, and `Drop`/`ZeroizeOnDrop` on `LocalKey`) so secret key buffers are deterministically wiped on drop.

## Next Step

Run `/speckit-bug-assess slug=pr-4-unzeroized-private-key-memory` using this intake file.
