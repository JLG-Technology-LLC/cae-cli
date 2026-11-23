---
slug: pr-4-fake-cdp-otp-burn-address
pr_number: 4
comment_id: 4660959904
comment_url: "https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959904"
comment_node_id: "PRRC_kwDOUxZ2Vs8AAAABFdCaoA"
commit_id: f82e11d94db3dacd2ae0db526d2d973e0877af09
severity: high
file: src/wallet/auth.rs
line: 300
start_line: 274
side: RIGHT
subject_type: line
source: open-code-review-delegate
created: 2026-11-22
status: open
---

# Bug Intake: Terminal OTP and fallback auth fabricate CDP verification and produce unspendable burn addresses

- **Slug**: pr-4-fake-cdp-otp-burn-address
- **Created**: 2026-11-22
- **Source**: `open-code-review-delegate`
- **PR Number**: 4
- **Comment ID**: 4660959904
- **PR Thread URL**: https://github.com/JLG-Technology-LLC/cae-cli/pull/4#discussion_r4660959904
- **Commit**: `f82e11d94db3dacd2ae0db526d2d973e0877af09`
- **Severity**: High
- **Location**: `src/wallet/auth.rs:274-300`

## Finding

`run_terminal_otp` claims to dispatch and verify a 6-digit CDP OTP but makes zero network/CDP API calls: it accepts any 6 ASCII digits, generates a random local `cdp_sess_<hex>` string, and computes `evm_address` via `derive_address_for_email(&email)` = `Keccak256(email)[12..]` (also used as fallback in `handle_loopback_stream:198`). A raw Keccak-256 hash of an email is not derived from any secp256k1 public key or Coinbase enclave wallet—no private key exists for it, so any deposited ETH or USDC is permanently burned.

- **Category**: `security`
- **Rule**: Security-Sensitive Code — no ad hoc cryptographic/authentication implementations

## Diff Context

```diff
+    let session_ref = format!("cdp_sess_{}", hex::encode(crypto_random_bytes()));
+    let evm_address = derive_address_for_email(&email);
...
+fn derive_address_for_email(email: &str) -> String {
+    use sha3::{Digest, Keccak256};
+    let hash = Keccak256::digest(email.as_bytes());
+    crate::wallet::crypto::to_checksum_address(&hash[12..])
+}
```

## Proposed Direction (Not Applied)

Remove `derive_address_for_email` from production auth paths. Require a verified CDP endpoint/response (or explicit test-only mock configuration) or fail closed with an actionable error directing users to `cae wallet init --local` until real CDP end-user OTP endpoints are wired up.

## Next Step

Run `/speckit-bug-assess slug=pr-4-fake-cdp-otp-burn-address` using this intake file.
